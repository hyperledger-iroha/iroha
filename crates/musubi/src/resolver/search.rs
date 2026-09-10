//! Deterministic depth-first search with heap-owned branch continuations.
//!
//! Native call depth must not grow with the number of pending edges: even a shallow
//! dependency graph can fill the 512-edge corridor. Frames retain the same candidate
//! order, conflict selection and branch-local state while one loop drives backtracking.

use super::{
    ConflictReasonV1, MusubiDependencyKindV1, MusubiExactDependencyEdgeV1, MusubiReleaseIdV1,
    ParentKey, PendingEdge, ResolutionConflictV1, ResolverError, SearchState, Solver,
    parallel_version_excess, select_better_conflict,
};
use std::{collections::btree_map::Entry, ops::ControlFlow, sync::Arc, vec::IntoIter};

struct SearchFrame {
    state: SearchState,
    pending: Vec<Arc<PendingEdge>>,
    task: Arc<PendingEdge>,
    candidates: IntoIter<Arc<MusubiReleaseIdV1>>,
    active_candidate: Option<Arc<MusubiReleaseIdV1>>,
    preserved_candidate: Option<Arc<MusubiReleaseIdV1>>,
    minimum_parallel_excess: usize,
    best_conflict: Option<ResolutionConflictV1>,
    best_solution: Option<SearchState>,
}

enum SearchStep {
    Descend(SearchState, Vec<Arc<PendingEdge>>),
    Complete(Result<SearchState, ResolverError>),
}

impl SearchFrame {
    fn accept(
        &mut self,
        result: Result<SearchState, ResolverError>,
    ) -> Option<Result<SearchState, ResolverError>> {
        let candidate = self.active_candidate.take().expect("one returning branch");
        match result {
            Ok(solution) => {
                // A successful still-valid locked branch wins over duplicate-version
                // minimization. Failed locked branches still allow ordinary backtracking.
                if self.preserved_candidate.as_ref() == Some(&candidate)
                    || (self.preserved_candidate.is_none()
                        && parallel_version_excess(&solution) == self.minimum_parallel_excess)
                {
                    return Some(Ok(solution));
                }
                if self.best_solution.as_ref().is_none_or(|current| {
                    parallel_version_excess(&solution) < parallel_version_excess(current)
                }) {
                    self.best_solution = Some(solution);
                }
            }
            Err(ResolverError::Conflict(conflict)) => {
                select_better_conflict(&mut self.best_conflict, *conflict);
            }
            Err(error) => return Some(Err(error)),
        }
        None
    }

    fn finish(&mut self) -> Result<SearchState, ResolverError> {
        if let Some(solution) = self.best_solution.take() {
            return Ok(solution);
        }
        Err(ResolverError::Conflict(Box::new(
            self.best_conflict
                .take()
                .unwrap_or_else(|| ResolutionConflictV1 {
                    chain: self.task.chain.to_vec(),
                    reason: ConflictReasonV1::NoCandidate,
                }),
        )))
    }
}

impl Solver {
    pub(super) fn search(
        &self,
        state: SearchState,
        pending: Vec<Arc<PendingEdge>>,
        attempts: &mut usize,
    ) -> Result<SearchState, ResolverError> {
        let mut frames = Vec::new();
        let mut step = SearchStep::Descend(state, pending);
        loop {
            step = match step {
                SearchStep::Descend(state, pending) => match self.search_frame(state, pending) {
                    ControlFlow::Break(result) => SearchStep::Complete(result),
                    ControlFlow::Continue(mut frame) => {
                        let next = self.advance_search(&mut frame, attempts);
                        if matches!(next, SearchStep::Descend(..)) {
                            frames.push(frame);
                        }
                        next
                    }
                },
                SearchStep::Complete(result) => {
                    let Some(mut frame) = frames.pop() else {
                        return result;
                    };
                    frame.accept(result).map_or_else(
                        || {
                            let next = self.advance_search(&mut frame, attempts);
                            if matches!(next, SearchStep::Descend(..)) {
                                frames.push(frame);
                            }
                            next
                        },
                        SearchStep::Complete,
                    )
                }
            };
        }
    }

    fn search_frame(
        &self,
        state: SearchState,
        mut pending: Vec<Arc<PendingEdge>>,
    ) -> ControlFlow<Result<SearchState, ResolverError>, SearchFrame> {
        if pending.is_empty() {
            return ControlFlow::Break(self.precise_conflict(&state).map_or_else(
                || Ok(state),
                |conflict| Err(ResolverError::Conflict(Box::new(conflict))),
            ));
        }
        let task = pending.remove(0);
        let conflict = |reason| {
            ControlFlow::Break(Err(ResolverError::Conflict(Box::new(
                ResolutionConflictV1 {
                    chain: task.chain.to_vec(),
                    reason,
                },
            ))))
        };
        if task.depth > self.limits.depth {
            return conflict(ConflictReasonV1::DepthLimit);
        }
        let preserved_candidate = self.preservable_locked_candidate(&task);
        let candidates = self.candidates(&state, &task);
        if candidates.is_empty() {
            return conflict(ConflictReasonV1::NoCandidate);
        }
        let minimum_parallel_excess = parallel_version_excess(&state);
        ControlFlow::Continue(SearchFrame {
            state,
            pending,
            task,
            candidates: candidates.into_iter(),
            active_candidate: None,
            preserved_candidate,
            minimum_parallel_excess,
            best_conflict: None,
            best_solution: None,
        })
    }

    fn advance_search(&self, frame: &mut SearchFrame, attempts: &mut usize) -> SearchStep {
        for candidate in frame.candidates.by_ref() {
            if *attempts >= self.limits.attempts {
                return SearchStep::Complete(Err(ResolverError::SearchLimitExceeded {
                    limit: self.limits.attempts,
                }));
            }
            *attempts += 1;
            match self.candidate_branch(&frame.state, &frame.task, &candidate) {
                Ok((state, mut pending)) => {
                    pending.extend(frame.pending.iter().cloned());
                    frame.active_candidate = Some(candidate);
                    return SearchStep::Descend(state, pending);
                }
                Err(conflict) => select_better_conflict(&mut frame.best_conflict, *conflict),
            }
        }
        SearchStep::Complete(frame.finish())
    }

    fn candidate_branch(
        &self,
        state: &SearchState,
        task: &PendingEdge,
        candidate: &Arc<MusubiReleaseIdV1>,
    ) -> Result<(SearchState, Vec<Arc<PendingEdge>>), Box<ResolutionConflictV1>> {
        let conflict = |reason| {
            Box::new(ResolutionConflictV1 {
                chain: task.chain.to_vec(),
                reason,
            })
        };
        if Self::would_cycle(state, &task.parent, candidate.as_ref()) {
            return Err(conflict(ConflictReasonV1::Cycle(
                candidate.as_ref().clone(),
            )));
        }
        if state.selected.contains(candidate.as_ref())
            && let Some(chain) =
                self.selected_subtree_depth_conflict(state, task, candidate.as_ref())
        {
            return Err(Box::new(ResolutionConflictV1 {
                chain,
                reason: ConflictReasonV1::DepthLimit,
            }));
        }
        let mut next = state.clone();
        let is_new = next.selected.insert(Arc::clone(candidate));
        if is_new && next.selected.len() > self.limits.nodes {
            return Err(conflict(ConflictReasonV1::NodeLimit));
        }
        let edge = MusubiExactDependencyEdgeV1 {
            alias: task.alias.clone(),
            kind: task.kind,
            package: task.package.clone(),
            requirement: task.requirement.as_ref().clone(),
            selected: candidate.as_ref().clone(),
        };
        match next
            .edges
            .entry(task.parent.clone())
            .or_default()
            .entry(task.alias.clone())
        {
            Entry::Occupied(mut occupied) => {
                occupied.insert(Arc::new(edge));
            }
            Entry::Vacant(vacant) => {
                let edge_count = next
                    .edge_count
                    .checked_add(1)
                    .filter(|count| *count <= self.limits.edges)
                    .ok_or_else(|| conflict(ConflictReasonV1::EdgeLimit))?;
                next.edge_count = edge_count;
                vacant.insert(Arc::new(edge));
            }
        }
        let mut pending = Vec::new();
        if is_new {
            let row = self
                .rows
                .get(candidate.as_ref())
                .expect("candidate row exists");
            let parent = ParentKey::shared_release(Arc::clone(candidate));
            let origin_parent = task
                .origin
                .as_ref()
                .map(|origin| ParentKey::Release(Arc::clone(&origin.selected)));
            for dependency in &row.dependencies {
                let chain = task.chain.push(PendingEdge::step(
                    &parent,
                    &dependency.alias,
                    &dependency.package,
                    &dependency.requirement,
                ));
                pending.push(Arc::new(PendingEdge {
                    parent: parent.clone(),
                    alias: dependency.alias.clone(),
                    kind: MusubiDependencyKindV1::Normal,
                    package: dependency.package.clone(),
                    requirement: Arc::new(dependency.requirement.clone()),
                    depth: task.depth.saturating_add(1),
                    chain,
                    origin: origin_parent.as_ref().and_then(|origin_parent| {
                        self.previous_edge(origin_parent, &dependency.alias, &dependency.package)
                    }),
                }));
            }
        }
        Ok((next, pending))
    }
}
