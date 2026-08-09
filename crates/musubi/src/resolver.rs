//! Deterministic dependency resolution for the Musubi V1 universal sparse index.
//!
//! Resolution consumes only finalized [`MusubiResolverReleaseRowV1`] values and
//! produces a consumer-owned exact [`LockfileV1`]. The search order is part of
//! the first-release behavior: reuse a compatible selection already present in
//! the graph, then a still-valid parent-local lock edge, then fresh releases in
//! descending `SemVer` order.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use iroha_data_model::{
    ChainId,
    musubi::{
        MUSUBI_MAX_DEPENDENCIES_V1, MUSUBI_MAX_RESOLUTION_DEPTH_V1, MUSUBI_MAX_RESOLUTION_NODES_V1,
        MusubiArtifactGovernanceStateV1, MusubiDependencyKindV1, MusubiDependencyReqV1,
        MusubiExactDependencyEdgeV1, MusubiPackageIdV1, MusubiPackageSelectorV1,
        MusubiRegistrySnapshotV1, MusubiReleaseIdV1, MusubiResolverReleaseRowV1,
        MusubiStorageAvailabilityV1, MusubiVerificationNodeV1, MusubiVersionReqV1, MusubiVersionV1,
    },
    name::Name,
};

use crate::lockfile::{LockedRootV1, LockfileV1};

/// One registry requirement declared by a selected workspace root.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceDependencyReqV1 {
    /// Parent-local import alias.
    pub alias: Name,
    /// Normal or selected-root-only development dependency.
    pub kind: MusubiDependencyKindV1,
    /// Stable registry package identity.
    pub package: MusubiPackageIdV1,
    /// Canonical published version range.
    pub requirement: MusubiVersionReqV1,
}

impl WorkspaceDependencyReqV1 {
    fn validate(&self) -> Result<(), ResolverError> {
        MusubiDependencyReqV1 {
            alias: self.alias.clone(),
            package: self.package.clone(),
            requirement: self.requirement.clone(),
        }
        .validate()
        .map_err(|error| ResolverError::invalid(error.reason()))
    }
}

/// Requirements of one selected workspace package.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceRootReqV1 {
    /// Canonical namespaced local package selector used as the lock root identity.
    pub package: MusubiPackageSelectorV1,
    /// Normal and root-local development requirements.
    pub dependencies: Vec<WorkspaceDependencyReqV1>,
}

/// A targeted Cargo-style update request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetedUpdateV1 {
    /// Stable package selected with `-p`.
    pub package: MusubiPackageIdV1,
    /// Optional locked version from `PACKAGE@VERSION`.
    pub locked_version: Option<MusubiVersionV1>,
    /// Optional exact replacement requested by `--precise`.
    pub precise: Option<MusubiVersionV1>,
}

/// Lock mutation policy for one resolution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResolveModeV1 {
    /// Return a changed lock graph when resolution requires one.
    #[default]
    UpdateLock,
    /// Fail if the exact previous graph cannot be retained.
    Locked,
}

/// Complete deterministic resolver input.
#[derive(Clone, Debug)]
pub struct ResolveRequestV1 {
    /// Exact chain identity written into a new lock.
    pub chain_id: ChainId,
    /// Exact non-zero genesis block identity.
    pub genesis_hash: [u8; 32],
    /// Finalized universal-index snapshot represented by `rows`.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Selected workspace roots and their effective requirements.
    pub roots: Vec<WorkspaceRootReqV1>,
    /// Resolver-grade sparse-index rows. Input order has no meaning.
    pub rows: Vec<MusubiResolverReleaseRowV1>,
    /// Existing consumer lock, when present.
    pub previous: Option<LockfileV1>,
    /// Optional targeted update. Every other valid parent edge remains locked.
    pub update: Option<TargetedUpdateV1>,
    /// Whether graph changes are allowed.
    pub mode: ResolveModeV1,
}

/// Successful exact resolution and whether its graph changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveOutcomeV1 {
    /// Canonical consumer lock. If `changed` is false this is the previous lock,
    /// including its older snapshot anchor.
    pub lockfile: LockfileV1,
    /// Whether the caller must durably replace its lockfile.
    pub changed: bool,
}

/// Parent identity printed in a dependency conflict chain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConflictParentV1 {
    /// Selected local workspace package.
    Workspace(MusubiPackageSelectorV1),
    /// Exact registry release whose manifest declared the edge.
    Release(MusubiReleaseIdV1),
}

impl fmt::Display for ConflictParentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(name) => write!(formatter, "workspace `{name}`"),
            Self::Release(release) => write!(formatter, "`{release}`"),
        }
    }
}

/// One parent-local edge in a minimal resolution conflict.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConflictStepV1 {
    /// Parent declaring the requirement.
    pub parent: ConflictParentV1,
    /// Parent-local import alias.
    pub alias: Name,
    /// Stable required package.
    pub package: MusubiPackageIdV1,
    /// Canonical unsatisfied range.
    pub requirement: MusubiVersionReqV1,
}

impl fmt::Display for ConflictStepV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} -> {} ({}, {})",
            self.parent, self.alias, self.package, self.requirement
        )
    }
}

/// Stable reason attached to a deterministic conflict chain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConflictReasonV1 {
    /// No allowed release satisfies the terminal edge.
    NoCandidate,
    /// Selecting the release would introduce an exact dependency cycle.
    Cycle(MusubiReleaseIdV1),
    /// The selected graph would exceed the depth-64 consensus bound.
    DepthLimit,
    /// The selected graph would exceed the 1,024-node consensus bound.
    NodeLimit,
}

/// Minimal deterministic dependency conflict returned after backtracking.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolutionConflictV1 {
    /// Root-to-terminal parent-local dependency chain.
    pub chain: Vec<ConflictStepV1>,
    /// Terminal failure reason.
    pub reason: ConflictReasonV1,
}

impl fmt::Display for ResolutionConflictV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            ConflictReasonV1::NoCandidate => formatter.write_str("no selectable release"),
            ConflictReasonV1::Cycle(release) => {
                write!(formatter, "dependency cycle through `{release}`")
            }
            ConflictReasonV1::DepthLimit => formatter.write_str("dependency depth exceeds 64"),
            ConflictReasonV1::NodeLimit => {
                formatter.write_str("dependency graph exceeds 1,024 nodes")
            }
        }?;
        for step in &self.chain {
            write!(formatter, "\n  {step}")?;
        }
        Ok(())
    }
}

/// Deterministic resolver failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolverError {
    /// Input rows, roots, identities, or an existing lock are invalid.
    InvalidInput(String),
    /// Exhaustive deterministic backtracking found no valid graph.
    Conflict(Box<ResolutionConflictV1>),
    /// A valid result exists but `--locked` forbids writing it.
    LockChangeRequired,
}

impl ResolverError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
}

impl fmt::Display for ResolverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid resolver input: {message}"),
            Self::Conflict(conflict) => {
                write!(formatter, "dependency resolution failed: {conflict}")
            }
            Self::LockChangeRequired => {
                formatter.write_str("Musubi.lock must change, but --locked forbids rewriting it")
            }
        }
    }
}

impl Error for ResolverError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ParentKey {
    Root(MusubiPackageSelectorV1),
    Release(MusubiReleaseIdV1),
}

impl ParentKey {
    fn conflict_parent(&self) -> ConflictParentV1 {
        match self {
            Self::Root(name) => ConflictParentV1::Workspace(name.clone()),
            Self::Release(release) => ConflictParentV1::Release(release.clone()),
        }
    }
}

#[derive(Clone)]
struct PendingEdge {
    parent: ParentKey,
    alias: Name,
    kind: MusubiDependencyKindV1,
    package: MusubiPackageIdV1,
    requirement: MusubiVersionReqV1,
    depth: u16,
    chain: Vec<ConflictStepV1>,
    // The exact prior edge this task replaces, retained across parent-version backtracking.
    origin: Option<PreviousEdge>,
}

impl PendingEdge {
    fn step(
        parent: &ParentKey,
        alias: &Name,
        package: &MusubiPackageIdV1,
        requirement: &MusubiVersionReqV1,
    ) -> ConflictStepV1 {
        ConflictStepV1 {
            parent: parent.conflict_parent(),
            alias: alias.clone(),
            package: package.clone(),
            requirement: requirement.clone(),
        }
    }
}

#[derive(Clone)]
struct PreviousEdge {
    selected: MusubiReleaseIdV1,
}

// Exact parent-local identity of one edge that selected the targeted locked node.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PreviousEdgeKey {
    parent: ParentKey,
    alias: Name,
}

#[derive(Clone, Default)]
struct SearchState {
    selected: BTreeSet<MusubiReleaseIdV1>,
    edges: BTreeMap<ParentKey, BTreeMap<Name, MusubiExactDependencyEdgeV1>>,
}

#[derive(Default)]
struct PreciseReplay {
    paths: BTreeMap<(ParentKey, ParentKey), Vec<ConflictStepV1>>,
    missing: BTreeMap<PreviousEdgeKey, Vec<ConflictStepV1>>,
}

#[derive(Clone, Copy)]
struct Limits {
    nodes: usize,
    depth: u16,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            nodes: MUSUBI_MAX_RESOLUTION_NODES_V1,
            depth: MUSUBI_MAX_RESOLUTION_DEPTH_V1,
        }
    }
}

#[derive(Clone)]
struct UpdatePlan {
    target: MusubiReleaseIdV1,
    precise: Option<MusubiVersionV1>,
}

struct Solver {
    chain_id: ChainId,
    genesis_hash: [u8; 32],
    snapshot: MusubiRegistrySnapshotV1,
    roots: Vec<WorkspaceRootReqV1>,
    rows: BTreeMap<MusubiReleaseIdV1, MusubiResolverReleaseRowV1>,
    by_package: BTreeMap<MusubiPackageIdV1, Vec<MusubiReleaseIdV1>>,
    previous: Option<LockfileV1>,
    preserved: BTreeMap<ParentKey, BTreeMap<Name, MusubiExactDependencyEdgeV1>>,
    locked_nodes: BTreeMap<MusubiReleaseIdV1, MusubiVerificationNodeV1>,
    update: Option<UpdatePlan>,
    precise_occurrences: BTreeSet<PreviousEdgeKey>,
    // Bounded by the square of the V1 node limit and shared by every search leaf.
    precise_target_descendants: BTreeMap<MusubiReleaseIdV1, BTreeSet<MusubiReleaseIdV1>>,
    mode: ResolveModeV1,
    selection_policy: SelectionPolicyV1,
    limits: Limits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionPolicyV1 {
    PreserveConsumerLock,
    FreshOnly,
}

/// Resolve the exact consumer graph using first-release deterministic policy.
pub fn resolve(request: ResolveRequestV1) -> Result<ResolveOutcomeV1, ResolverError> {
    resolve_with_policy(
        request,
        Limits::default(),
        SelectionPolicyV1::PreserveConsumerLock,
    )
}

/// Resolve a publication verification graph using only fresh-selectable rows.
///
/// Still-valid fresh lock edges retain their normal preference, but yanked,
/// governed-unavailable, and below-quorum selections are treated as ordinary
/// lock changes instead of being preserved for publication.
pub fn resolve_fresh(request: ResolveRequestV1) -> Result<ResolveOutcomeV1, ResolverError> {
    resolve_with_policy(request, Limits::default(), SelectionPolicyV1::FreshOnly)
}

#[cfg(test)]
fn resolve_with_limits(
    request: ResolveRequestV1,
    limits: Limits,
) -> Result<ResolveOutcomeV1, ResolverError> {
    resolve_with_policy(request, limits, SelectionPolicyV1::PreserveConsumerLock)
}

fn resolve_with_policy(
    request: ResolveRequestV1,
    limits: Limits,
    selection_policy: SelectionPolicyV1,
) -> Result<ResolveOutcomeV1, ResolverError> {
    let solver = Solver::new(request, limits, selection_policy)?;
    solver.run()
}

impl Solver {
    #[expect(
        clippy::too_many_lines,
        reason = "solver construction validates and canonicalizes every bounded first-release resolver input in one ordered pass"
    )]
    fn new(
        mut request: ResolveRequestV1,
        limits: Limits,
        selection_policy: SelectionPolicyV1,
    ) -> Result<Self, ResolverError> {
        if request.genesis_hash.iter().all(|byte| *byte == 0) {
            return Err(ResolverError::invalid("genesis hash must not be zero"));
        }
        request
            .snapshot
            .validate()
            .map_err(|error| ResolverError::invalid(error.reason()))?;
        if request.roots.is_empty() {
            return Err(ResolverError::invalid(
                "at least one selected workspace root is required",
            ));
        }
        for root in &mut request.roots {
            root.package
                .validate()
                .map_err(|error| ResolverError::invalid(error.reason()))?;
            if root.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1 {
                return Err(ResolverError::invalid(format!(
                    "workspace root `{}` exceeds the dependency bound",
                    root.package
                )));
            }
            root.dependencies.sort();
            for dependency in &root.dependencies {
                dependency.validate()?;
            }
            if root
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            {
                return Err(ResolverError::invalid(format!(
                    "workspace root `{}` dependency aliases are not unique",
                    root.package
                )));
            }
        }
        request
            .roots
            .sort_by(|left, right| left.package.cmp(&right.package));
        if request
            .roots
            .windows(2)
            .any(|pair| pair[0].package == pair[1].package)
        {
            return Err(ResolverError::invalid(
                "selected workspace root package selectors must be unique",
            ));
        }

        let mut rows = BTreeMap::new();
        for row in request.rows {
            row.validate()
                .map_err(|error| ResolverError::invalid(error.reason()))?;
            if row.index_revision != request.snapshot.index_revision {
                return Err(ResolverError::invalid(format!(
                    "resolver row `{}` is from index revision {}, expected {}",
                    row.release, row.index_revision, request.snapshot.index_revision
                )));
            }
            if row
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            {
                return Err(ResolverError::invalid(format!(
                    "resolver row `{}` dependency aliases are not unique",
                    row.release
                )));
            }
            let release = row.release.clone();
            if rows.insert(release.clone(), row).is_some() {
                return Err(ResolverError::invalid(format!(
                    "duplicate resolver row `{release}`"
                )));
            }
        }

        let mut preserved = BTreeMap::new();
        let mut locked_nodes = BTreeMap::new();
        if let Some(previous) = &request.previous {
            previous
                .validate()
                .map_err(|error| ResolverError::invalid(error.to_string()))?;
            if previous.chain_id != request.chain_id
                || previous.genesis_hash != request.genesis_hash
            {
                return Err(ResolverError::invalid(
                    "existing lock belongs to a different chain or genesis",
                ));
            }
            for root in &previous.roots {
                preserved.insert(
                    ParentKey::Root(root.package.clone()),
                    root.dependencies
                        .iter()
                        .map(|edge| (edge.alias.clone(), edge.clone()))
                        .collect(),
                );
            }
            for node in &previous.nodes {
                preserved.insert(
                    ParentKey::Release(node.release.clone()),
                    node.dependencies
                        .iter()
                        .map(|edge| (edge.alias.clone(), edge.clone()))
                        .collect(),
                );
                locked_nodes.insert(node.release.clone(), node.clone());
            }
        }
        for (release, node) in &locked_nodes {
            if let Some(row) = rows.get(release)
                && !row_matches_locked_node(row, node)
            {
                return Err(ResolverError::invalid(format!(
                    "immutable resolver row `{release}` differs from the existing lock"
                )));
            }
        }

        let update = prepare_update(request.update, request.previous.as_ref())?;
        let precise_occurrences =
            prepare_precise_occurrences(request.previous.as_ref(), update.as_ref())?;
        validate_precise_occurrence_roots(
            request.previous.as_ref(),
            &request.roots,
            &precise_occurrences,
        )?;
        let precise_target_descendants = request
            .previous
            .as_ref()
            .map_or_else(BTreeMap::new, |previous| {
                precise_target_descendants(previous, &precise_occurrences)
            });
        let mut by_package: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for release in rows.keys() {
            by_package
                .entry(release.package.clone())
                .or_default()
                .push(release.clone());
        }
        for releases in by_package.values_mut() {
            releases.sort_by(|left, right| {
                right
                    .version
                    .cmp(&left.version)
                    .then_with(|| left.cmp(right))
            });
        }

        Ok(Self {
            chain_id: request.chain_id,
            genesis_hash: request.genesis_hash,
            snapshot: request.snapshot,
            roots: request.roots,
            rows,
            by_package,
            previous: request.previous,
            preserved,
            locked_nodes,
            update,
            precise_occurrences,
            precise_target_descendants,
            mode: request.mode,
            selection_policy,
            limits,
        })
    }

    fn run(self) -> Result<ResolveOutcomeV1, ResolverError> {
        let pending = self.root_tasks();
        let state = self.search(SearchState::default(), pending)?;
        let proposed = self.build_lock(&state)?;
        if let Some(previous) = &self.previous
            && previous.roots == proposed.roots
            && previous.nodes == proposed.nodes
        {
            return Ok(ResolveOutcomeV1 {
                lockfile: previous.clone(),
                changed: false,
            });
        }
        if self.mode == ResolveModeV1::Locked {
            return Err(ResolverError::LockChangeRequired);
        }
        Ok(ResolveOutcomeV1 {
            lockfile: proposed,
            changed: true,
        })
    }

    fn root_tasks(&self) -> Vec<PendingEdge> {
        self.roots
            .iter()
            .flat_map(|root| {
                let parent = ParentKey::Root(root.package.clone());
                root.dependencies.iter().map(move |dependency| {
                    let chain = vec![PendingEdge::step(
                        &parent,
                        &dependency.alias,
                        &dependency.package,
                        &dependency.requirement,
                    )];
                    PendingEdge {
                        parent: parent.clone(),
                        alias: dependency.alias.clone(),
                        kind: dependency.kind,
                        package: dependency.package.clone(),
                        requirement: dependency.requirement.clone(),
                        depth: 1,
                        chain,
                        origin: self.previous_edge(&parent, &dependency.alias, &dependency.package),
                    }
                })
            })
            .collect()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the recursive resolver state machine keeps candidate selection, conflict evidence, and deterministic backtracking adjacent"
    )]
    fn search(
        &self,
        state: SearchState,
        mut pending: Vec<PendingEdge>,
    ) -> Result<SearchState, ResolverError> {
        if pending.is_empty() {
            if let Some(conflict) = self.precise_conflict(&state) {
                return Err(ResolverError::Conflict(Box::new(conflict)));
            }
            return Ok(state);
        }
        let task = pending.remove(0);
        if task.depth > self.limits.depth {
            return Err(ResolverError::Conflict(Box::new(ResolutionConflictV1 {
                chain: task.chain,
                reason: ConflictReasonV1::DepthLimit,
            })));
        }
        let preserved_candidate = self.preservable_locked_candidate(&task).cloned();
        let candidates = self.candidates(&state, &task);
        if candidates.is_empty() {
            return Err(ResolverError::Conflict(Box::new(ResolutionConflictV1 {
                chain: task.chain,
                reason: ConflictReasonV1::NoCandidate,
            })));
        }

        let minimum_parallel_excess = parallel_version_excess(&state);
        let mut best_conflict = None;
        let mut best_solution = None;
        for candidate in candidates {
            if Self::would_cycle(&state, &task.parent, &candidate) {
                select_better_conflict(
                    &mut best_conflict,
                    ResolutionConflictV1 {
                        chain: task.chain.clone(),
                        reason: ConflictReasonV1::Cycle(candidate),
                    },
                );
                continue;
            }
            if state.selected.contains(&candidate)
                && let Some(chain) = self.selected_subtree_depth_conflict(&state, &task, &candidate)
            {
                select_better_conflict(
                    &mut best_conflict,
                    ResolutionConflictV1 {
                        chain,
                        reason: ConflictReasonV1::DepthLimit,
                    },
                );
                continue;
            }
            let mut next = state.clone();
            let is_new = next.selected.insert(candidate.clone());
            if is_new && next.selected.len() > self.limits.nodes {
                select_better_conflict(
                    &mut best_conflict,
                    ResolutionConflictV1 {
                        chain: task.chain.clone(),
                        reason: ConflictReasonV1::NodeLimit,
                    },
                );
                continue;
            }
            let edge = MusubiExactDependencyEdgeV1 {
                alias: task.alias.clone(),
                kind: task.kind,
                package: task.package.clone(),
                requirement: task.requirement.clone(),
                selected: candidate.clone(),
            };
            next.edges
                .entry(task.parent.clone())
                .or_default()
                .insert(task.alias.clone(), edge);

            let mut next_pending = Vec::new();
            if is_new {
                let row = self.rows.get(&candidate).expect("candidate row exists");
                let parent = ParentKey::Release(candidate.clone());
                let origin_parent = task
                    .origin
                    .as_ref()
                    .map(|origin| ParentKey::Release(origin.selected.clone()));
                for dependency in &row.dependencies {
                    let mut chain = task.chain.clone();
                    chain.push(PendingEdge::step(
                        &parent,
                        &dependency.alias,
                        &dependency.package,
                        &dependency.requirement,
                    ));
                    next_pending.push(PendingEdge {
                        parent: parent.clone(),
                        alias: dependency.alias.clone(),
                        kind: MusubiDependencyKindV1::Normal,
                        package: dependency.package.clone(),
                        requirement: dependency.requirement.clone(),
                        depth: task.depth.saturating_add(1),
                        chain,
                        origin: origin_parent.as_ref().and_then(|origin_parent| {
                            self.previous_edge(
                                origin_parent,
                                &dependency.alias,
                                &dependency.package,
                            )
                        }),
                    });
                }
            }
            next_pending.extend(pending.iter().cloned());
            match self.search(next, next_pending) {
                Ok(solution) => {
                    // A successful still-valid locked branch wins over
                    // duplicate-version minimization. If that branch cannot
                    // resolve, normal candidate backtracking remains active.
                    if preserved_candidate.as_ref() == Some(&candidate) {
                        return Ok(solution);
                    }
                    if preserved_candidate.is_none()
                        && parallel_version_excess(&solution) == minimum_parallel_excess
                    {
                        return Ok(solution);
                    }
                    if best_solution.as_ref().is_none_or(|current| {
                        parallel_version_excess(&solution) < parallel_version_excess(current)
                    }) {
                        best_solution = Some(solution);
                    }
                }
                Err(ResolverError::Conflict(conflict)) => {
                    select_better_conflict(&mut best_conflict, *conflict);
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(solution) = best_solution {
            return Ok(solution);
        }
        Err(ResolverError::Conflict(Box::new(
            best_conflict.unwrap_or_else(|| ResolutionConflictV1 {
                chain: task.chain,
                reason: ConflictReasonV1::NoCandidate,
            }),
        )))
    }

    fn candidates(&self, state: &SearchState, task: &PendingEdge) -> Vec<MusubiReleaseIdV1> {
        let preserved = self.preserved_edge(task);
        let update_edge = self.update.as_ref().is_some_and(|update| {
            task.origin
                .as_ref()
                .is_some_and(|origin| origin.selected == update.target)
        });
        let precise = update_edge
            .then(|| {
                self.update
                    .as_ref()
                    .and_then(|update| update.precise.as_ref())
            })
            .flatten();
        let matches = |release: &MusubiReleaseIdV1| {
            if release.package != task.package
                || !task.requirement.matches(&release.version)
                || precise.is_some_and(|version| &release.version != version)
            {
                return false;
            }
            self.rows.contains_key(release)
        };
        let fresh = |release: &MusubiReleaseIdV1| {
            matches(release)
                && self
                    .rows
                    .get(release)
                    .is_some_and(|row| row.selection.fresh_selectable())
        };
        let locked = |release: &MusubiReleaseIdV1| {
            matches(release)
                && self
                    .rows
                    .get(release)
                    .is_some_and(|row| self.locked_state_is_preservable(row))
        };

        let mut candidates = Vec::new();
        let mut selected = state
            .selected
            .iter()
            .filter(|release| {
                !(update_edge
                    && self
                        .update
                        .as_ref()
                        .is_some_and(|update| **release == update.target))
                    && (fresh(release) || locked(release))
            })
            .cloned()
            .collect::<Vec<_>>();
        selected.sort_by(|left, right| {
            right
                .version
                .cmp(&left.version)
                .then_with(|| left.cmp(right))
        });
        for release in selected {
            push_unique(&mut candidates, release);
        }
        if !update_edge
            && let Some(edge) = preserved
            && (fresh(&edge.selected) || locked(&edge.selected))
        {
            push_unique(&mut candidates, edge.selected.clone());
        }
        let mut globally_locked = self
            .locked_nodes
            .keys()
            .filter(|release| {
                !(update_edge
                    && self
                        .update
                        .as_ref()
                        .is_some_and(|update| **release == update.target))
                    && locked(release)
            })
            .cloned()
            .collect::<Vec<_>>();
        globally_locked.sort_by(|left, right| {
            right
                .version
                .cmp(&left.version)
                .then_with(|| left.cmp(right))
        });
        for release in globally_locked {
            push_unique(&mut candidates, release);
        }
        if let Some(releases) = self.by_package.get(&task.package) {
            for release in releases {
                if fresh(release) {
                    push_unique(&mut candidates, release.clone());
                }
            }
        }
        candidates
    }

    fn precise_target_release(&self) -> Option<MusubiReleaseIdV1> {
        self.update.as_ref().and_then(|update| {
            update.precise.as_ref().map(|version| {
                MusubiReleaseIdV1::new(update.target.package.clone(), version.clone())
            })
        })
    }

    fn previous_edge(
        &self,
        parent: &ParentKey,
        alias: &Name,
        package: &MusubiPackageIdV1,
    ) -> Option<PreviousEdge> {
        self.preserved
            .get(parent)
            .and_then(|edges| edges.get(alias))
            .filter(|edge| &edge.package == package)
            .map(|edge| PreviousEdge {
                selected: edge.selected.clone(),
            })
    }

    fn precise_conflict(&self, state: &SearchState) -> Option<ResolutionConflictV1> {
        let expected = self.precise_target_release()?;
        let replay = self.precise_replay(state);
        let mut best = None;

        for occurrence in &self.precise_occurrences {
            let mapped = replay
                .paths
                .iter()
                .filter(|((previous, _), _)| previous == &occurrence.parent)
                .map(|((_, current), chain)| (current, chain))
                .collect::<Vec<_>>();
            if mapped.is_empty() {
                let chain = replay
                    .missing
                    .get(occurrence)
                    .cloned()
                    .expect("validated precise occurrence has a current structural break");
                select_better_conflict(
                    &mut best,
                    ResolutionConflictV1 {
                        chain,
                        reason: ConflictReasonV1::NoCandidate,
                    },
                );
                continue;
            }

            for (current, prefix) in mapped {
                let selects_expected = state
                    .edges
                    .get(current)
                    .and_then(|edges| edges.get(&occurrence.alias))
                    .is_some_and(|edge| edge.selected == expected);
                if selects_expected {
                    continue;
                }
                let mut chain = prefix.clone();
                chain.push(Self::precise_terminal_step(current, occurrence, &expected));
                select_better_conflict(
                    &mut best,
                    ResolutionConflictV1 {
                        chain,
                        reason: ConflictReasonV1::NoCandidate,
                    },
                );
            }
        }
        best
    }

    fn precise_terminal_step(
        parent: &ParentKey,
        occurrence: &PreviousEdgeKey,
        expected: &MusubiReleaseIdV1,
    ) -> ConflictStepV1 {
        ConflictStepV1 {
            parent: parent.conflict_parent(),
            alias: occurrence.alias.clone(),
            package: expected.package.clone(),
            requirement: MusubiVersionReqV1::Exact(expected.version.clone()),
        }
    }

    fn precise_replay(&self, state: &SearchState) -> PreciseReplay {
        let Some(previous) = self.previous.as_ref() else {
            return PreciseReplay::default();
        };
        let current_paths = self.current_release_paths(state);
        let mut replay = PreciseReplay::default();
        let mut pending = BTreeSet::new();

        for root in &previous.roots {
            if self
                .roots
                .binary_search_by(|candidate| candidate.package.cmp(&root.package))
                .is_ok()
            {
                let key = (
                    ParentKey::Root(root.package.clone()),
                    ParentKey::Root(root.package.clone()),
                );
                replay.paths.insert(key.clone(), Vec::new());
                pending.insert(key);
            }
        }
        for node in &previous.nodes {
            if let Some(chain) = current_paths.get(&node.release) {
                let key = (
                    ParentKey::Release(node.release.clone()),
                    ParentKey::Release(node.release.clone()),
                );
                if insert_better_chain(&mut replay.paths, key.clone(), chain.clone()) {
                    pending.insert(key);
                }
            }
        }

        while let Some(key) = pending.pop_first() {
            let chain = replay
                .paths
                .get(&key)
                .expect("queued precise replay path exists")
                .clone();
            let (previous_parent, current_parent) = (&key.0, &key.1);
            let previous_edges = previous_edges(previous, previous_parent);
            let current_edges = state.edges.get(current_parent);
            for previous_edge in previous_edges {
                let Some(descendants) =
                    self.precise_target_descendants.get(&previous_edge.selected)
                else {
                    continue;
                };
                let current_edge = current_edges
                    .and_then(|edges| edges.get(&previous_edge.alias))
                    .filter(|edge| edge.package == previous_edge.package);
                let Some(current_edge) = current_edge else {
                    let mut missing_chain = chain.clone();
                    missing_chain.push(PendingEdge::step(
                        current_parent,
                        &previous_edge.alias,
                        &previous_edge.package,
                        &previous_edge.requirement,
                    ));
                    for target_parent in descendants {
                        for occurrence in self.precise_occurrences.iter().filter(|occurrence| {
                            occurrence.parent == ParentKey::Release(target_parent.clone())
                        }) {
                            insert_better_chain(
                                &mut replay.missing,
                                occurrence.clone(),
                                missing_chain.clone(),
                            );
                        }
                    }
                    continue;
                };

                let mut next_chain = chain.clone();
                next_chain.push(PendingEdge::step(
                    current_parent,
                    &current_edge.alias,
                    &current_edge.package,
                    &current_edge.requirement,
                ));
                let next = (
                    ParentKey::Release(previous_edge.selected.clone()),
                    ParentKey::Release(current_edge.selected.clone()),
                );
                if insert_better_chain(&mut replay.paths, next.clone(), next_chain) {
                    pending.insert(next);
                }
            }
        }
        replay
    }

    fn current_release_paths(
        &self,
        state: &SearchState,
    ) -> BTreeMap<MusubiReleaseIdV1, Vec<ConflictStepV1>> {
        let mut paths = BTreeMap::new();
        let mut pending = BTreeSet::new();
        for root in &self.roots {
            let parent = ParentKey::Root(root.package.clone());
            if let Some(edges) = state.edges.get(&parent) {
                for edge in edges.values() {
                    let chain = vec![PendingEdge::step(
                        &parent,
                        &edge.alias,
                        &edge.package,
                        &edge.requirement,
                    )];
                    if insert_better_chain(&mut paths, edge.selected.clone(), chain) {
                        pending.insert(edge.selected.clone());
                    }
                }
            }
        }
        while let Some(release) = pending.pop_first() {
            let chain = paths
                .get(&release)
                .expect("queued current release path exists")
                .clone();
            let parent = ParentKey::Release(release);
            if let Some(edges) = state.edges.get(&parent) {
                for edge in edges.values() {
                    let mut next_chain = chain.clone();
                    next_chain.push(PendingEdge::step(
                        &parent,
                        &edge.alias,
                        &edge.package,
                        &edge.requirement,
                    ));
                    if insert_better_chain(&mut paths, edge.selected.clone(), next_chain) {
                        pending.insert(edge.selected.clone());
                    }
                }
            }
        }
        paths
    }

    fn preserved_edge(&self, task: &PendingEdge) -> Option<&MusubiExactDependencyEdgeV1> {
        self.preserved
            .get(&task.parent)
            .and_then(|edges| edges.get(&task.alias))
            .filter(|edge| {
                edge.kind == task.kind
                    && edge.package == task.package
                    && task.requirement.matches(&edge.selected.version)
            })
    }

    fn preservable_locked_candidate(&self, task: &PendingEdge) -> Option<&MusubiReleaseIdV1> {
        let edge = self.preserved_edge(task)?;
        if self
            .update
            .as_ref()
            .is_some_and(|update| edge.selected == update.target)
        {
            return None;
        }
        self.rows
            .get(&edge.selected)
            .is_some_and(|row| self.locked_state_is_preservable(row))
            .then_some(&edge.selected)
    }

    fn locked_state_is_preservable(&self, row: &MusubiResolverReleaseRowV1) -> bool {
        let exact_locked_row = self
            .locked_nodes
            .get(&row.release)
            .is_some_and(|node| row_matches_locked_node(row, node));
        exact_locked_row
            && match self.selection_policy {
                SelectionPolicyV1::PreserveConsumerLock => {
                    matches!(
                        &row.selection.governance,
                        MusubiArtifactGovernanceStateV1::Available
                    ) && row.selection.storage.availability
                        != MusubiStorageAvailabilityV1::Unavailable
                }
                SelectionPolicyV1::FreshOnly => row.selection.fresh_selectable(),
            }
    }

    fn would_cycle(state: &SearchState, parent: &ParentKey, candidate: &MusubiReleaseIdV1) -> bool {
        let ParentKey::Release(parent) = parent else {
            return false;
        };
        if parent == candidate {
            return true;
        }
        let mut seen = BTreeSet::new();
        let mut pending = vec![candidate];
        while let Some(release) = pending.pop() {
            if !seen.insert(release) {
                continue;
            }
            if release == parent {
                return true;
            }
            if let Some(edges) = state.edges.get(&ParentKey::Release(release.clone())) {
                pending.extend(edges.values().map(|edge| &edge.selected));
            }
        }
        false
    }

    fn selected_subtree_depth_conflict(
        &self,
        state: &SearchState,
        task: &PendingEdge,
        candidate: &MusubiReleaseIdV1,
    ) -> Option<Vec<ConflictStepV1>> {
        let mut paths = BTreeMap::new();
        paths.insert((candidate.clone(), task.depth), task.chain.clone());
        let mut pending = BTreeSet::from([(task.depth, task.chain.clone(), candidate.clone())]);

        while let Some((depth, chain, release)) = pending.pop_first() {
            if paths.get(&(release.clone(), depth)) != Some(&chain) {
                continue;
            }
            let parent = ParentKey::Release(release);
            let Some(edges) = state.edges.get(&parent) else {
                continue;
            };
            for edge in edges.values() {
                let next_depth = depth.saturating_add(1);
                let mut next_chain = chain.clone();
                next_chain.push(PendingEdge::step(
                    &parent,
                    &edge.alias,
                    &edge.package,
                    &edge.requirement,
                ));
                if next_depth > self.limits.depth {
                    return Some(next_chain);
                }
                let key = (edge.selected.clone(), next_depth);
                if insert_better_chain(&mut paths, key, next_chain.clone()) {
                    pending.insert((next_depth, next_chain, edge.selected.clone()));
                }
            }
        }
        None
    }

    fn build_lock(&self, state: &SearchState) -> Result<LockfileV1, ResolverError> {
        let roots = self
            .roots
            .iter()
            .map(|root| LockedRootV1 {
                package: root.package.clone(),
                dependencies: state
                    .edges
                    .get(&ParentKey::Root(root.package.clone()))
                    .map(|edges| edges.values().cloned().collect())
                    .unwrap_or_default(),
            })
            .collect();
        let nodes = state
            .selected
            .iter()
            .map(|release| {
                let row = self.rows.get(release).expect("selected row exists");
                MusubiVerificationNodeV1 {
                    release: release.clone(),
                    release_digest: row.release_digest,
                    archive_id: row.archive_id,
                    source_digest: row.source_digest,
                    interface_digest: row.interface_digest,
                    abi: row.abi,
                    dependencies: state
                        .edges
                        .get(&ParentKey::Release(release.clone()))
                        .map(|edges| edges.values().cloned().collect())
                        .unwrap_or_default(),
                }
            })
            .collect();
        LockfileV1::new(
            self.chain_id.clone(),
            self.genesis_hash,
            self.snapshot,
            roots,
            nodes,
        )
        .map_err(|error| ResolverError::invalid(error.to_string()))
    }
}

fn prepare_update(
    update: Option<TargetedUpdateV1>,
    previous: Option<&LockfileV1>,
) -> Result<Option<UpdatePlan>, ResolverError> {
    let Some(update) = update else {
        return Ok(None);
    };
    update
        .package
        .validate()
        .map_err(|error| ResolverError::invalid(error.reason()))?;
    if let Some(version) = &update.locked_version {
        version
            .validate()
            .map_err(|error| ResolverError::invalid(error.reason()))?;
    }
    if let Some(version) = &update.precise {
        version
            .validate()
            .map_err(|error| ResolverError::invalid(error.reason()))?;
    }
    let previous = previous.ok_or_else(|| {
        ResolverError::invalid("a targeted update requires an existing Musubi V1 lock")
    })?;
    let releases = previous
        .nodes
        .iter()
        .filter(|node| node.release.package == update.package)
        .map(|node| node.release.clone())
        .collect::<Vec<_>>();
    let target = match update.locked_version {
        Some(version) => {
            let release = MusubiReleaseIdV1::new(update.package, version);
            if !releases.contains(&release) {
                return Err(ResolverError::invalid(format!(
                    "targeted release `{release}` is not present in the existing lock"
                )));
            }
            release
        }
        None => match releases.as_slice() {
            [release] => release.clone(),
            [] => {
                return Err(ResolverError::invalid(format!(
                    "targeted package `{}` is not present in the existing lock",
                    update.package
                )));
            }
            _ => {
                return Err(ResolverError::invalid(format!(
                    "targeted package `{}` has multiple locked versions; specify PACKAGE@VERSION",
                    update.package
                )));
            }
        },
    };
    Ok(Some(UpdatePlan {
        target,
        precise: update.precise,
    }))
}

fn prepare_precise_occurrences(
    previous: Option<&LockfileV1>,
    update: Option<&UpdatePlan>,
) -> Result<BTreeSet<PreviousEdgeKey>, ResolverError> {
    let Some(update) = update.filter(|update| update.precise.is_some()) else {
        return Ok(BTreeSet::new());
    };
    let previous = previous.ok_or_else(|| {
        ResolverError::invalid("a precise targeted update requires an existing Musubi V1 lock")
    })?;
    let mut occurrences = BTreeSet::new();
    for root in &previous.roots {
        let parent = ParentKey::Root(root.package.clone());
        for edge in &root.dependencies {
            if edge.selected == update.target {
                occurrences.insert(PreviousEdgeKey {
                    parent: parent.clone(),
                    alias: edge.alias.clone(),
                });
            }
        }
    }
    for node in &previous.nodes {
        let parent = ParentKey::Release(node.release.clone());
        for edge in &node.dependencies {
            if edge.selected == update.target {
                occurrences.insert(PreviousEdgeKey {
                    parent: parent.clone(),
                    alias: edge.alias.clone(),
                });
            }
        }
    }
    if occurrences.is_empty() {
        return Err(ResolverError::invalid(format!(
            "targeted release `{}` has no incoming edge in the existing lock",
            update.target
        )));
    }
    Ok(occurrences)
}

fn validate_precise_occurrence_roots(
    previous: Option<&LockfileV1>,
    roots: &[WorkspaceRootReqV1],
    occurrences: &BTreeSet<PreviousEdgeKey>,
) -> Result<(), ResolverError> {
    if occurrences.is_empty() {
        return Ok(());
    }
    let previous = previous.expect("precise occurrences require a previous lock");
    let current_roots = roots
        .iter()
        .map(|root| root.package.clone())
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::new();
    let mut pending = previous
        .roots
        .iter()
        .filter(|root| current_roots.contains(&root.package))
        .flat_map(|root| root.dependencies.iter().map(|edge| edge.selected.clone()))
        .collect::<Vec<_>>();
    while let Some(release) = pending.pop() {
        if !reachable.insert(release.clone()) {
            continue;
        }
        pending.extend(
            previous_edges(previous, &ParentKey::Release(release))
                .iter()
                .map(|edge| edge.selected.clone()),
        );
    }

    for occurrence in occurrences {
        let rooted = match &occurrence.parent {
            ParentKey::Root(root) => current_roots.contains(root),
            ParentKey::Release(release) => reachable.contains(release),
        };
        if !rooted {
            return Err(ResolverError::invalid(format!(
                "precise targeted occurrence `{}` / `{}` is not reachable from the selected workspace roots",
                occurrence.parent.conflict_parent(),
                occurrence.alias
            )));
        }
    }
    Ok(())
}

fn previous_edges<'lock>(
    previous: &'lock LockfileV1,
    parent: &ParentKey,
) -> &'lock [MusubiExactDependencyEdgeV1] {
    match parent {
        ParentKey::Root(root) => previous
            .roots
            .binary_search_by(|candidate| candidate.package.cmp(root))
            .ok()
            .and_then(|index| previous.roots.get(index))
            .map_or(&[], |root| root.dependencies.as_slice()),
        ParentKey::Release(release) => previous
            .nodes
            .binary_search_by(|candidate| candidate.release.cmp(release))
            .ok()
            .and_then(|index| previous.nodes.get(index))
            .map_or(&[], |node| node.dependencies.as_slice()),
    }
}

fn precise_target_descendants(
    previous: &LockfileV1,
    occurrences: &BTreeSet<PreviousEdgeKey>,
) -> BTreeMap<MusubiReleaseIdV1, BTreeSet<MusubiReleaseIdV1>> {
    // The validated lock is acyclic and contains at most 1,024 releases. Each
    // stored ancestor/target-parent pair is unique, so retained replay state is
    // deterministically bounded by the square of the V1 node limit.
    let mut reverse = BTreeMap::<MusubiReleaseIdV1, BTreeSet<MusubiReleaseIdV1>>::new();
    for node in &previous.nodes {
        for edge in &node.dependencies {
            reverse
                .entry(edge.selected.clone())
                .or_default()
                .insert(node.release.clone());
        }
    }

    let target_parents = occurrences
        .iter()
        .filter_map(|occurrence| match &occurrence.parent {
            ParentKey::Root(_) => None,
            ParentKey::Release(release) => Some(release.clone()),
        })
        .collect::<BTreeSet<_>>();
    let mut descendants = BTreeMap::<_, BTreeSet<_>>::new();
    for target_parent in target_parents {
        let mut pending = vec![target_parent.clone()];
        let mut seen = BTreeSet::new();
        while let Some(release) = pending.pop() {
            if !seen.insert(release.clone()) {
                continue;
            }
            descendants
                .entry(release.clone())
                .or_default()
                .insert(target_parent.clone());
            if let Some(parents) = reverse.get(&release) {
                pending.extend(parents.iter().cloned());
            }
        }
    }
    descendants
}

fn insert_better_chain<Key: Ord>(
    paths: &mut BTreeMap<Key, Vec<ConflictStepV1>>,
    key: Key,
    candidate: Vec<ConflictStepV1>,
) -> bool {
    let replace = paths.get(&key).is_none_or(|current| {
        candidate.len() < current.len()
            || (candidate.len() == current.len() && candidate < *current)
    });
    if replace {
        paths.insert(key, candidate);
    }
    replace
}

fn row_matches_locked_node(
    row: &MusubiResolverReleaseRowV1,
    node: &MusubiVerificationNodeV1,
) -> bool {
    row.release == node.release
        && row.release_digest == node.release_digest
        && row.archive_id == node.archive_id
        && row.source_digest == node.source_digest
        && row.interface_digest == node.interface_digest
        && row.abi == node.abi
        && row.dependencies.len() == node.dependencies.len()
        && row
            .dependencies
            .iter()
            .zip(&node.dependencies)
            .all(|(requirement, edge)| {
                edge.kind == MusubiDependencyKindV1::Normal
                    && requirement.alias == edge.alias
                    && requirement.package == edge.package
                    && requirement.requirement == edge.requirement
            })
}

fn push_unique(candidates: &mut Vec<MusubiReleaseIdV1>, release: MusubiReleaseIdV1) {
    if !candidates.contains(&release) {
        candidates.push(release);
    }
}

fn parallel_version_excess(state: &SearchState) -> usize {
    let mut counts = BTreeMap::<&MusubiPackageIdV1, usize>::new();
    for release in &state.selected {
        *counts.entry(&release.package).or_default() += 1;
    }
    counts.values().map(|count| count.saturating_sub(1)).sum()
}

fn select_better_conflict(
    best: &mut Option<ResolutionConflictV1>,
    candidate: ResolutionConflictV1,
) {
    let replace = best.as_ref().is_none_or(|current| {
        candidate.chain.len() < current.chain.len()
            || (candidate.chain.len() == current.chain.len() && candidate < *current)
    });
    if replace {
        *best = Some(candidate);
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        account::AccountId,
        musubi::{
            ArchiveId, MUSUBI_MIN_HEALTHY_REPLICAS_V1, MusubiAbiBindingV1,
            MusubiArchiveAvailabilityV1, MusubiArtifactGovernanceStateV1, MusubiArtifactTakedownV1,
            MusubiContentDigestV1, MusubiGovernanceActionDigestV1, MusubiPackageScopeV1,
            MusubiReasonV1, MusubiReleaseDigestV1, MusubiReleaseSelectionStateV1,
            MusubiReleaseYankV1, MusubiStorageAvailabilityV1,
        },
        nexus::DataSpaceId,
        prelude::{Algorithm, KeyPair},
    };

    use super::*;

    fn account() -> AccountId {
        let keypair =
            KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).expect("fixture keypair");
        AccountId::new(keypair.public_key().clone())
    }

    fn package(name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            name.parse().expect("package name"),
        )
    }

    fn version(raw: &str) -> MusubiVersionV1 {
        raw.parse().expect("version")
    }

    fn requirement(raw: &str) -> MusubiVersionReqV1 {
        raw.parse().expect("requirement")
    }

    fn dependency(alias: &str, package: &MusubiPackageIdV1, req: &str) -> MusubiDependencyReqV1 {
        MusubiDependencyReqV1 {
            alias: alias.parse().expect("alias"),
            package: package.clone(),
            requirement: requirement(req),
        }
    }

    fn root_dependency(
        alias: &str,
        kind: MusubiDependencyKindV1,
        package: &MusubiPackageIdV1,
        req: &str,
    ) -> WorkspaceDependencyReqV1 {
        WorkspaceDependencyReqV1 {
            alias: alias.parse().expect("alias"),
            kind,
            package: package.clone(),
            requirement: requirement(req),
        }
    }

    fn root(dependencies: Vec<WorkspaceDependencyReqV1>) -> WorkspaceRootReqV1 {
        WorkspaceRootReqV1 {
            package: "test/app".parse().expect("root selector"),
            dependencies,
        }
    }

    fn snapshot(height: u64) -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: height,
            finalized_block_hash: [u8::try_from(height).unwrap_or(0xFE); 32],
            index_revision: height,
        }
    }

    fn row(
        package: &MusubiPackageIdV1,
        raw_version: &str,
        mut dependencies: Vec<MusubiDependencyReqV1>,
        snapshot: MusubiRegistrySnapshotV1,
    ) -> MusubiResolverReleaseRowV1 {
        dependencies.sort();
        let version = version(raw_version);
        let release = MusubiReleaseIdV1::new(package.clone(), version.clone());
        let mut seed = package
            .name
            .as_str()
            .bytes()
            .chain(raw_version.bytes())
            .fold(1_u8, u8::wrapping_add);
        if seed == 0 {
            seed = 1;
        }
        let archive_id = ArchiveId::new([seed; 32]);
        MusubiResolverReleaseRowV1 {
            release: release.clone(),
            release_digest: MusubiReleaseDigestV1::new([seed.wrapping_add(1); 32]),
            archive_id,
            source_digest: MusubiContentDigestV1::new([seed.wrapping_add(2); 32]),
            interface_digest: MusubiContentDigestV1::new([seed.wrapping_add(3); 32]),
            abi: MusubiAbiBindingV1::new([0xAB; 32]).expect("ABI"),
            dependencies,
            selection: MusubiReleaseSelectionStateV1 {
                yank: MusubiReleaseYankV1 {
                    release,
                    yanked: false,
                    reason: "fixture".parse::<MusubiReasonV1>().expect("reason"),
                    changed_by: account(),
                    changed_at_height: snapshot.finalized_height,
                    revision: 1,
                },
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                    active_locations: 1,
                    finalized_height: snapshot.finalized_height,
                    finalized_block_hash: snapshot.finalized_block_hash,
                    index_revision: snapshot.index_revision,
                },
                governance: MusubiArtifactGovernanceStateV1::Available,
            },
            index_revision: snapshot.index_revision,
        }
    }

    fn with_storage(
        mut row: MusubiResolverReleaseRowV1,
        availability: MusubiStorageAvailabilityV1,
    ) -> MusubiResolverReleaseRowV1 {
        row.selection.storage.availability = availability;
        match availability {
            MusubiStorageAvailabilityV1::Selectable => {
                row.selection.storage.healthy_replicas = MUSUBI_MIN_HEALTHY_REPLICAS_V1;
                row.selection.storage.active_locations = 1;
            }
            MusubiStorageAvailabilityV1::BelowQuorum => {
                row.selection.storage.healthy_replicas = 1;
                row.selection.storage.active_locations = 1;
            }
            MusubiStorageAvailabilityV1::Unavailable => {
                row.selection.storage.healthy_replicas = 0;
                row.selection.storage.active_locations = 0;
            }
        }
        row
    }

    fn yanked(mut row: MusubiResolverReleaseRowV1) -> MusubiResolverReleaseRowV1 {
        row.selection.yank.yanked = true;
        row
    }

    fn taken_down(mut row: MusubiResolverReleaseRowV1) -> MusubiResolverReleaseRowV1 {
        row.selection.governance =
            MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
                action_digest: MusubiGovernanceActionDigestV1::new([0xDD; 32]),
                reason: "fixture takedown"
                    .parse::<MusubiReasonV1>()
                    .expect("reason"),
                applied_at_height: row.index_revision,
            });
        row
    }

    fn request(
        roots: Vec<WorkspaceRootReqV1>,
        rows: Vec<MusubiResolverReleaseRowV1>,
        snapshot: MusubiRegistrySnapshotV1,
    ) -> ResolveRequestV1 {
        ResolveRequestV1 {
            chain_id: ChainId::from("musubi-test"),
            genesis_hash: [0x42; 32],
            snapshot,
            roots,
            rows,
            previous: None,
            update: None,
            mode: ResolveModeV1::UpdateLock,
        }
    }

    fn root_selection<'a>(lock: &'a LockfileV1, alias: &str) -> &'a MusubiReleaseIdV1 {
        &lock.roots[0]
            .dependencies
            .iter()
            .find(|edge| edge.alias.as_ref() == alias)
            .expect("locked root edge")
            .selected
    }

    #[test]
    fn input_order_is_irrelevant_and_selected_compatible_precedes_newest() {
        let snap = snapshot(3);
        let pkg = package("codec");
        let dependencies = vec![
            root_dependency("broad", MusubiDependencyKindV1::Normal, &pkg, "*"),
            root_dependency("anchor", MusubiDependencyKindV1::Normal, &pkg, "=1.0.0"),
        ];
        let rows = vec![
            row(&pkg, "2.0.0", vec![], snap),
            row(&pkg, "1.0.0", vec![], snap),
        ];

        let first = resolve(request(
            vec![root(dependencies.clone())],
            rows.clone(),
            snap,
        ))
        .expect("resolution");
        let mut reversed_dependencies = dependencies;
        reversed_dependencies.reverse();
        let mut reversed_rows = rows;
        reversed_rows.reverse();
        let second = resolve(request(
            vec![root(reversed_dependencies)],
            reversed_rows,
            snap,
        ))
        .expect("resolution");

        assert_eq!(first.lockfile, second.lockfile);
        assert_eq!(first.lockfile.nodes.len(), 1);
        assert_eq!(
            root_selection(&first.lockfile, "anchor").version,
            version("1.0.0")
        );
        assert_eq!(
            root_selection(&first.lockfile, "broad").version,
            version("1.0.0")
        );
    }

    #[test]
    fn fresh_resolution_uses_descending_semver() {
        let snap = snapshot(3);
        let pkg = package("codec");
        let outcome = resolve(request(
            vec![root(vec![root_dependency(
                "codec",
                MusubiDependencyKindV1::Normal,
                &pkg,
                "*",
            )])],
            vec![
                row(&pkg, "1.0.0", vec![], snap),
                row(&pkg, "2.0.0", vec![], snap),
            ],
            snap,
        ))
        .expect("resolution");
        assert_eq!(
            root_selection(&outcome.lockfile, "codec").version,
            version("2.0.0")
        );
    }

    #[test]
    fn parent_edge_lock_preserves_yanked_below_quorum_release_and_old_snapshot() {
        let old_snapshot = snapshot(3);
        let new_snapshot = snapshot(4);
        let pkg = package("codec");
        let roots = vec![root(vec![root_dependency(
            "codec",
            MusubiDependencyKindV1::Normal,
            &pkg,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![row(&pkg, "1.0.0", vec![], old_snapshot)],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let locked_row = yanked(with_storage(
            row(&pkg, "1.0.0", vec![], new_snapshot),
            MusubiStorageAvailabilityV1::BelowQuorum,
        ));
        let mut next = request(
            roots,
            vec![locked_row, row(&pkg, "2.0.0", vec![], new_snapshot)],
            new_snapshot,
        );
        next.previous = Some(previous.clone());

        let outcome = resolve(next).expect("preserved lock");
        assert!(!outcome.changed);
        assert_eq!(outcome.lockfile, previous);
        assert_eq!(outcome.lockfile.snapshot, old_snapshot);
    }

    #[test]
    fn publication_resolution_replaces_a_yanked_below_quorum_lock() {
        let old_snapshot = snapshot(5);
        let new_snapshot = snapshot(6);
        let pkg = package("codec");
        let roots = vec![root(vec![root_dependency(
            "codec",
            MusubiDependencyKindV1::Normal,
            &pkg,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![row(&pkg, "1.0.0", vec![], old_snapshot)],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let locked_row = yanked(with_storage(
            row(&pkg, "1.0.0", vec![], new_snapshot),
            MusubiStorageAvailabilityV1::BelowQuorum,
        ));
        let mut next = request(
            roots,
            vec![locked_row, row(&pkg, "2.0.0", vec![], new_snapshot)],
            new_snapshot,
        );
        next.previous = Some(previous);

        let outcome = resolve_fresh(next.clone()).expect("fresh publication graph");
        assert!(outcome.changed);
        assert_eq!(
            root_selection(&outcome.lockfile, "codec").version,
            version("2.0.0")
        );

        next.mode = ResolveModeV1::Locked;
        assert!(matches!(
            resolve_fresh(next),
            Err(ResolverError::LockChangeRequired)
        ));
    }

    #[test]
    fn changed_root_ranges_keep_every_still_valid_locked_version() {
        let old_snapshot = snapshot(15);
        let new_snapshot = snapshot(16);
        let pkg = package("codec");
        let previous = resolve(request(
            vec![root(vec![
                root_dependency("a-broad", MusubiDependencyKindV1::Normal, &pkg, "=1.8.0"),
                root_dependency("b-high", MusubiDependencyKindV1::Normal, &pkg, "=1.9.0"),
            ])],
            vec![
                row(&pkg, "1.8.0", vec![], old_snapshot),
                row(&pkg, "1.9.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("parallel initial lock")
        .lockfile;
        assert_eq!(previous.nodes.len(), 2);

        let mut next = request(
            vec![root(vec![
                root_dependency("a-broad", MusubiDependencyKindV1::Normal, &pkg, "^1.0.0"),
                root_dependency(
                    "b-high",
                    MusubiDependencyKindV1::Normal,
                    &pkg,
                    ">=1.9.0,<2.0.0",
                ),
            ])],
            vec![
                row(&pkg, "1.8.0", vec![], new_snapshot),
                row(&pkg, "1.9.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        next.previous = Some(previous);
        let resolved = resolve(next).expect("still-valid selections remain locked");

        assert!(
            resolved.changed,
            "changed requirements rewrite edge metadata"
        );
        assert_eq!(resolved.lockfile.nodes.len(), 2);
        assert_eq!(
            root_selection(&resolved.lockfile, "a-broad").version,
            version("1.8.0")
        );
        assert_eq!(
            root_selection(&resolved.lockfile, "b-high").version,
            version("1.9.0")
        );
    }

    #[test]
    fn compatible_selected_version_does_not_discard_yanked_locked_release() {
        let old_snapshot = snapshot(19);
        let new_snapshot = snapshot(20);
        let pkg = package("codec");
        let previous = resolve(request(
            vec![root(vec![
                root_dependency("a-current", MusubiDependencyKindV1::Normal, &pkg, "=2.0.0"),
                root_dependency("b-legacy", MusubiDependencyKindV1::Normal, &pkg, "=1.0.0"),
            ])],
            vec![
                row(&pkg, "2.0.0", vec![], old_snapshot),
                row(&pkg, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial parallel lock")
        .lockfile;

        let locked_legacy = yanked(with_storage(
            row(&pkg, "1.0.0", vec![], new_snapshot),
            MusubiStorageAvailabilityV1::BelowQuorum,
        ));
        let mut next = request(
            vec![root(vec![
                root_dependency("a-current", MusubiDependencyKindV1::Normal, &pkg, "=2.0.0"),
                root_dependency("b-legacy", MusubiDependencyKindV1::Normal, &pkg, "*"),
            ])],
            vec![row(&pkg, "2.0.0", vec![], new_snapshot), locked_legacy],
            new_snapshot,
        );
        next.previous = Some(previous);

        let resolved = resolve(next).expect("yanked locked release remains fixed");
        assert_eq!(resolved.lockfile.nodes.len(), 2);
        assert_eq!(
            root_selection(&resolved.lockfile, "a-current").version,
            version("2.0.0")
        );
        assert_eq!(
            root_selection(&resolved.lockfile, "b-legacy").version,
            version("1.0.0")
        );
    }

    #[test]
    fn unavailable_locked_descendant_allows_parent_candidate_backtracking() {
        let old_snapshot = snapshot(21);
        let new_snapshot = snapshot(22);
        let parent = package("parent");
        let child = package("child");
        let roots = vec![root(vec![root_dependency(
            "parent",
            MusubiDependencyKindV1::Normal,
            &parent,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("child", &child, "=1.0.0")],
                    old_snapshot,
                ),
                row(&child, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;

        let mut next = request(
            roots,
            vec![
                row(
                    &parent,
                    "2.0.0",
                    vec![dependency("child", &child, "=2.0.0")],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("child", &child, "=1.0.0")],
                    new_snapshot,
                ),
                row(&child, "2.0.0", vec![], new_snapshot),
                with_storage(
                    row(&child, "1.0.0", vec![], new_snapshot),
                    MusubiStorageAvailabilityV1::Unavailable,
                ),
            ],
            new_snapshot,
        );
        next.previous = Some(previous);

        let resolved = resolve(next).expect("failed locked branch backtracks to another parent");
        assert!(resolved.changed);
        assert_eq!(
            root_selection(&resolved.lockfile, "parent").version,
            version("2.0.0")
        );
        assert!(resolved.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(child.clone(), version("2.0.0"))
        }));
        assert!(!resolved.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(child.clone(), version("1.0.0"))
        }));
    }

    #[test]
    fn fresh_selection_excludes_every_unavailable_state() {
        let snap = snapshot(5);
        let pkg = package("codec");
        let rows = vec![
            taken_down(row(&pkg, "5.0.0", vec![], snap)),
            with_storage(
                row(&pkg, "4.0.0", vec![], snap),
                MusubiStorageAvailabilityV1::Unavailable,
            ),
            with_storage(
                row(&pkg, "3.0.0", vec![], snap),
                MusubiStorageAvailabilityV1::BelowQuorum,
            ),
            yanked(row(&pkg, "2.0.0", vec![], snap)),
            row(&pkg, "1.0.0", vec![], snap),
        ];
        let outcome = resolve(request(
            vec![root(vec![root_dependency(
                "codec",
                MusubiDependencyKindV1::Normal,
                &pkg,
                "*",
            )])],
            rows,
            snap,
        ))
        .expect("only healthy release is selectable");

        assert_eq!(
            root_selection(&outcome.lockfile, "codec").version,
            version("1.0.0")
        );
    }

    #[test]
    fn cargo_prerelease_eligibility_comes_from_structured_requirement() {
        let snap = snapshot(6);
        let pkg = package("codec");
        let outcome = resolve(request(
            vec![root(vec![
                root_dependency(
                    "explicit",
                    MusubiDependencyKindV1::Normal,
                    &pkg,
                    "=1.1.0-alpha.1",
                ),
                root_dependency("stable", MusubiDependencyKindV1::Normal, &pkg, "*"),
            ])],
            vec![
                row(&pkg, "1.1.0-alpha.1", vec![], snap),
                row(&pkg, "1.0.0", vec![], snap),
            ],
            snap,
        ))
        .expect("prerelease resolution");

        assert_eq!(
            root_selection(&outcome.lockfile, "explicit").version,
            version("1.1.0-alpha.1")
        );
        assert_eq!(
            root_selection(&outcome.lockfile, "stable").version,
            version("1.0.0")
        );
    }

    #[test]
    fn comparator_prerelease_does_not_admit_a_different_core_prerelease() {
        let snap = snapshot(23);
        let pkg = package("codec");
        let outcome = resolve(request(
            vec![root(vec![root_dependency(
                "codec",
                MusubiDependencyKindV1::Normal,
                &pkg,
                ">=1.2.3-alpha.1,<2.0.0",
            )])],
            vec![
                row(&pkg, "1.3.0-beta.1", vec![], snap),
                row(&pkg, "1.2.3-beta.2", vec![], snap),
                row(&pkg, "1.2.3-alpha.1", vec![], snap),
            ],
            snap,
        ))
        .expect("same-core prerelease resolution");

        assert_eq!(
            root_selection(&outcome.lockfile, "codec").version,
            version("1.2.3-beta.2")
        );
    }

    #[test]
    fn incompatible_ranges_use_parallel_versions_and_dev_is_root_local() {
        let snap = snapshot(7);
        let pkg = package("codec");
        let transitive = package("support");
        let outcome = resolve(request(
            vec![root(vec![
                root_dependency("old", MusubiDependencyKindV1::Normal, &pkg, "=1.0.0"),
                root_dependency("new", MusubiDependencyKindV1::Development, &pkg, "=2.0.0"),
            ])],
            vec![
                row(
                    &pkg,
                    "1.0.0",
                    vec![dependency("support", &transitive, "=1.0.0")],
                    snap,
                ),
                row(&pkg, "2.0.0", vec![], snap),
                row(&transitive, "1.0.0", vec![], snap),
            ],
            snap,
        ))
        .expect("parallel versions");

        assert_eq!(
            root_selection(&outcome.lockfile, "old").version,
            version("1.0.0")
        );
        assert_eq!(
            root_selection(&outcome.lockfile, "new").version,
            version("2.0.0")
        );
        assert_eq!(
            outcome.lockfile.roots[0].dependencies[0].kind,
            MusubiDependencyKindV1::Development
        );
        assert_eq!(
            outcome.lockfile.roots[0].dependencies[1].kind,
            MusubiDependencyKindV1::Normal
        );
        assert!(
            outcome
                .lockfile
                .nodes
                .iter()
                .flat_map(|node| &node.dependencies)
                .all(|edge| edge.kind == MusubiDependencyKindV1::Normal)
        );
    }

    #[test]
    fn transitive_failure_backtracks_to_an_older_parent_release() {
        let snap = snapshot(8);
        let parent = package("parent");
        let child = package("child");
        let outcome = resolve(request(
            vec![root(vec![root_dependency(
                "parent",
                MusubiDependencyKindV1::Normal,
                &parent,
                "*",
            )])],
            vec![
                row(
                    &parent,
                    "2.0.0",
                    vec![dependency("child", &child, "=2.0.0")],
                    snap,
                ),
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("child", &child, "=1.0.0")],
                    snap,
                ),
                row(&child, "1.0.0", vec![], snap),
            ],
            snap,
        ))
        .expect("parent backtracking");

        assert_eq!(
            root_selection(&outcome.lockfile, "parent").version,
            version("1.0.0")
        );
    }

    #[test]
    fn cycles_and_minimal_conflicts_are_deterministic() {
        let snap = snapshot(9);
        let a = package("a");
        let b = package("b");
        let cycle_rows = vec![
            row(&a, "1.0.0", vec![dependency("b", &b, "*")], snap),
            row(&b, "1.0.0", vec![dependency("a", &a, "*")], snap),
        ];
        let cycle = resolve(request(
            vec![root(vec![root_dependency(
                "a",
                MusubiDependencyKindV1::Normal,
                &a,
                "*",
            )])],
            cycle_rows,
            snap,
        ))
        .expect_err("cycle");
        let ResolverError::Conflict(cycle) = cycle else {
            panic!("expected cycle conflict");
        };
        assert!(matches!(cycle.reason, ConflictReasonV1::Cycle(_)));
        assert_eq!(cycle.chain.len(), 3);

        let parent = package("parent");
        let middle = package("middle");
        let missing = package("missing");
        let rows = vec![
            row(
                &parent,
                "2.0.0",
                vec![dependency("short", &missing, "*")],
                snap,
            ),
            row(
                &parent,
                "1.0.0",
                vec![dependency("middle", &middle, "*")],
                snap,
            ),
            row(
                &middle,
                "1.0.0",
                vec![dependency("long", &missing, "*")],
                snap,
            ),
        ];
        let roots = vec![root(vec![root_dependency(
            "parent",
            MusubiDependencyKindV1::Normal,
            &parent,
            "*",
        )])];
        let first = resolve(request(roots.clone(), rows.clone(), snap)).expect_err("conflict");
        let mut reversed = rows;
        reversed.reverse();
        let second = resolve(request(roots, reversed, snap)).expect_err("conflict");
        assert_eq!(first, second);
        let ResolverError::Conflict(conflict) = first else {
            panic!("expected dependency conflict");
        };
        assert_eq!(conflict.chain.len(), 2);
        assert_eq!(conflict.chain[1].alias.as_ref(), "short");
    }

    #[test]
    fn depth_and_node_limits_are_enforced_during_search() {
        let snap = snapshot(10);
        let a = package("a");
        let b = package("b");
        let rows = vec![
            row(&a, "1.0.0", vec![dependency("b", &b, "*")], snap),
            row(&b, "1.0.0", vec![], snap),
        ];
        let roots = vec![root(vec![root_dependency(
            "a",
            MusubiDependencyKindV1::Normal,
            &a,
            "*",
        )])];

        let depth = resolve_with_limits(
            request(roots.clone(), rows.clone(), snap),
            Limits {
                nodes: 10,
                depth: 1,
            },
        )
        .expect_err("depth bound");
        let ResolverError::Conflict(depth) = depth else {
            panic!("expected depth conflict");
        };
        assert_eq!(depth.reason, ConflictReasonV1::DepthLimit);

        let nodes = resolve_with_limits(
            request(roots, rows, snap),
            Limits {
                nodes: 1,
                depth: 10,
            },
        )
        .expect_err("node bound");
        let ResolverError::Conflict(nodes) = nodes else {
            panic!("expected node conflict");
        };
        assert_eq!(nodes.reason, ConflictReasonV1::NodeLimit);
    }

    #[test]
    fn reused_selected_subtree_still_enforces_the_deeper_path_limit() {
        let snap = snapshot(24);
        let parent = package("parent");
        let shared = package("shared");
        let leaf = package("leaf");
        let request = request(
            vec![root(vec![
                root_dependency("a-anchor", MusubiDependencyKindV1::Normal, &shared, "*"),
                root_dependency("z-parent", MusubiDependencyKindV1::Normal, &parent, "*"),
            ])],
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("shared", &shared, "*")],
                    snap,
                ),
                row(&shared, "1.0.0", vec![dependency("leaf", &leaf, "*")], snap),
                row(&leaf, "1.0.0", vec![], snap),
            ],
            snap,
        );

        let mut with_fallback = request.clone();
        with_fallback.rows.push(row(&parent, "0.9.0", vec![], snap));

        let error = resolve_with_limits(request, Limits { nodes: 8, depth: 2 })
            .expect_err("reused subtree creates a three-edge path");
        let ResolverError::Conflict(conflict) = error else {
            panic!("expected depth conflict");
        };
        assert_eq!(conflict.reason, ConflictReasonV1::DepthLimit);
        assert_eq!(conflict.chain.len(), 3);
        assert_eq!(conflict.chain[0].alias.as_ref(), "z-parent");
        assert_eq!(conflict.chain[1].alias.as_ref(), "shared");
        assert_eq!(conflict.chain[2].alias.as_ref(), "leaf");

        let resolved = resolve_with_limits(with_fallback, Limits { nodes: 8, depth: 2 })
            .expect("depth failure backtracks to the older parent");
        assert_eq!(
            root_selection(&resolved.lockfile, "z-parent").version,
            version("0.9.0")
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the fixture keeps parallel locked occurrences and their preservation assertions together"
    )]
    fn version_qualified_update_isolates_parallel_occurrences_and_forced_descendants() {
        let old_snapshot = snapshot(36);
        let new_snapshot = snapshot(37);
        let target = package("target");
        let child = package("child");
        let independent = package("independent");
        let roots = vec![root(vec![
            root_dependency(
                "a-target-one",
                MusubiDependencyKindV1::Normal,
                &target,
                ">=1.0.0,<2.0.0",
            ),
            root_dependency(
                "b-target-two",
                MusubiDependencyKindV1::Normal,
                &target,
                "=2.0.0",
            ),
            root_dependency(
                "z-independent",
                MusubiDependencyKindV1::Normal,
                &independent,
                "*",
            ),
        ])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &target,
                    "1.0.0",
                    vec![dependency("child", &child, "=1.0.0")],
                    old_snapshot,
                ),
                row(
                    &target,
                    "2.0.0",
                    vec![dependency("child", &child, "=2.0.0")],
                    old_snapshot,
                ),
                row(&child, "1.0.0", vec![], old_snapshot),
                row(&child, "2.0.0", vec![], old_snapshot),
                row(&independent, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial parallel lock")
        .lockfile;

        let rows = vec![
            row(
                &target,
                "1.5.0",
                vec![dependency("child", &child, "=1.5.0")],
                new_snapshot,
            ),
            row(
                &target,
                "1.0.0",
                vec![dependency("child", &child, "=1.0.0")],
                new_snapshot,
            ),
            row(
                &target,
                "2.0.0",
                vec![dependency("child", &child, "=2.0.0")],
                new_snapshot,
            ),
            row(&child, "1.5.0", vec![], new_snapshot),
            row(&child, "1.0.0", vec![], new_snapshot),
            row(&child, "2.0.0", vec![], new_snapshot),
            row(&independent, "2.0.0", vec![], new_snapshot),
            row(&independent, "1.0.0", vec![], new_snapshot),
        ];
        let update = TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: None,
        };
        let mut forward = request(roots, rows, new_snapshot);
        forward.previous = Some(previous.clone());
        forward.update = Some(update.clone());

        let updated = resolve(forward.clone()).expect("version-qualified update");
        assert_eq!(
            root_selection(&updated.lockfile, "a-target-one").version,
            version("1.5.0")
        );
        assert_eq!(
            root_selection(&updated.lockfile, "b-target-two").version,
            version("2.0.0")
        );
        assert_eq!(
            root_selection(&updated.lockfile, "z-independent").version,
            version("1.0.0")
        );
        assert!(updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(child.clone(), version("1.5.0"))
        }));
        assert!(updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(child.clone(), version("2.0.0"))
        }));
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(child.clone(), version("1.0.0"))
        }));
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(independent.clone(), version("2.0.0"))
        }));
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(target.clone(), version("1.0.0"))
        }));
        for preserved_release in [
            MusubiReleaseIdV1::new(target.clone(), version("2.0.0")),
            MusubiReleaseIdV1::new(child.clone(), version("2.0.0")),
            MusubiReleaseIdV1::new(independent, version("1.0.0")),
        ] {
            let before = previous
                .nodes
                .iter()
                .find(|node| node.release == preserved_release)
                .expect("preserved node in previous lock");
            let after = updated
                .lockfile
                .nodes
                .iter()
                .find(|node| node.release == preserved_release)
                .expect("preserved node in updated lock");
            assert_eq!(after, before);
        }

        forward.roots[0].dependencies.reverse();
        forward.rows.reverse();
        forward.previous = Some(previous);
        forward.update = Some(update);
        let reversed = resolve(forward).expect("reversed version-qualified update");
        assert_eq!(updated.lockfile, reversed.lockfile);
    }

    #[test]
    fn unqualified_update_rejects_a_parallel_locked_package() {
        let old_snapshot = snapshot(38);
        let new_snapshot = snapshot(39);
        let target = package("target");
        let roots = vec![root(vec![
            root_dependency(
                "target-one",
                MusubiDependencyKindV1::Normal,
                &target,
                "=1.0.0",
            ),
            root_dependency(
                "target-two",
                MusubiDependencyKindV1::Normal,
                &target,
                "=2.0.0",
            ),
        ])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(&target, "1.0.0", vec![], old_snapshot),
                row(&target, "2.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial parallel lock")
        .lockfile;
        let mut update = request(
            roots,
            vec![
                row(&target, "1.0.0", vec![], new_snapshot),
                row(&target, "2.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target,
            locked_version: None,
            precise: None,
        });

        let ResolverError::InvalidInput(message) = resolve(update).expect_err("ambiguous target")
        else {
            panic!("expected invalid targeted update");
        };
        assert!(message.contains("has multiple locked versions; specify PACKAGE@VERSION"));
    }

    #[test]
    fn targeted_precise_update_changes_only_target_and_forced_descendants() {
        let old_snapshot = snapshot(11);
        let new_snapshot = snapshot(12);
        let target = package("target");
        let independent = package("independent");
        let child = package("child");
        let roots = vec![root(vec![
            root_dependency("target", MusubiDependencyKindV1::Normal, &target, "*"),
            root_dependency(
                "independent",
                MusubiDependencyKindV1::Normal,
                &independent,
                "*",
            ),
        ])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &target,
                    "1.0.0",
                    vec![dependency("child", &child, "=1.0.0")],
                    old_snapshot,
                ),
                row(&child, "1.0.0", vec![], old_snapshot),
                row(&independent, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let mut normal = request(
            roots.clone(),
            vec![
                row(
                    &target,
                    "2.0.0",
                    vec![dependency("child", &child, "=2.0.0")],
                    new_snapshot,
                ),
                row(
                    &target,
                    "1.0.0",
                    vec![dependency("child", &child, "=1.0.0")],
                    new_snapshot,
                ),
                row(&child, "2.0.0", vec![], new_snapshot),
                row(&child, "1.0.0", vec![], new_snapshot),
                row(&independent, "2.0.0", vec![], new_snapshot),
                row(&independent, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        normal.previous = Some(previous.clone());
        let unchanged = resolve(normal.clone()).expect("normal preservation");
        assert!(!unchanged.changed);

        let mut unlocked = normal.clone();
        unlocked.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: None,
        });
        let unlocked = resolve(unlocked).expect("targeted update without --precise");
        assert_eq!(
            root_selection(&unlocked.lockfile, "target").version,
            version("2.0.0")
        );
        assert_eq!(
            root_selection(&unlocked.lockfile, "independent").version,
            version("1.0.0")
        );

        normal.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });
        let updated = resolve(normal).expect("targeted update");
        assert!(updated.changed);
        assert_eq!(updated.lockfile.snapshot, new_snapshot);
        assert_eq!(
            root_selection(&updated.lockfile, "target").version,
            version("2.0.0")
        );
        assert_eq!(
            root_selection(&updated.lockfile, "independent").version,
            version("1.0.0")
        );
        assert!(updated.lockfile.nodes.iter().any(|node| node.release == MusubiReleaseIdV1::new(child.clone(), version("2.0.0"))));
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(child.clone(), version("1.0.0"))
        }));
    }

    #[test]
    fn precise_target_survives_parent_candidate_backtracking() {
        let old_snapshot = snapshot(24);
        let new_snapshot = snapshot(25);
        let parent = package("parent");
        let target = package("target");
        let roots = vec![root(vec![root_dependency(
            "parent",
            MusubiDependencyKindV1::Normal,
            &parent,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("target", &target, "=1.0.0")],
                    old_snapshot,
                ),
                row(&target, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let mut update = request(
            roots,
            vec![
                row(
                    &parent,
                    "3.0.0",
                    vec![dependency("target", &target, "=3.0.0")],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "2.0.0",
                    vec![dependency("target", &target, ">=2.0.0,<4.0.0")],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("target", &target, "=1.0.0")],
                    new_snapshot,
                ),
                row(&target, "3.0.0", vec![], new_snapshot),
                row(&target, "2.0.0", vec![], new_snapshot),
                row(&target, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let updated = resolve(update).expect("precise update after parent backtracking");
        assert_eq!(
            root_selection(&updated.lockfile, "parent").version,
            version("2.0.0")
        );
        assert!(updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(target.clone(), version("2.0.0"))
        }));
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(target.clone(), version("3.0.0"))
        }));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the fixture enumerates every parallel candidate needed to verify occurrence binding"
    )]
    fn sibling_parallel_selection_cannot_satisfy_a_precise_target_occurrence() {
        let old_snapshot = snapshot(26);
        let new_snapshot = snapshot(27);
        let parent = package("parent");
        let target = package("target");
        let roots = vec![root(vec![root_dependency(
            "parent",
            MusubiDependencyKindV1::Normal,
            &parent,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![
                        dependency("a-targeted", &target, "=1.0.0"),
                        dependency("b-already-precise", &target, "=2.0.0"),
                        dependency("c-parallel", &target, "=4.0.0"),
                    ],
                    old_snapshot,
                ),
                row(&target, "4.0.0", vec![], old_snapshot),
                row(&target, "2.0.0", vec![], old_snapshot),
                row(&target, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial parallel lock")
        .lockfile;
        let mut update = request(
            roots,
            vec![
                row(
                    &parent,
                    "4.0.0",
                    vec![
                        dependency("b-already-precise", &target, "=2.0.0"),
                        dependency("c-parallel", &target, "=4.0.0"),
                    ],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "3.0.0",
                    vec![
                        dependency("a-targeted", &target, "=3.0.0"),
                        dependency("b-already-precise", &target, "=2.0.0"),
                        dependency("c-parallel", &target, "=4.0.0"),
                    ],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "2.0.0",
                    vec![
                        dependency("a-targeted", &target, ">=2.0.0,<4.0.0"),
                        dependency("b-already-precise", &target, "=2.0.0"),
                        dependency("c-parallel", &target, "=4.0.0"),
                    ],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "1.0.0",
                    vec![
                        dependency("a-targeted", &target, "=1.0.0"),
                        dependency("b-already-precise", &target, "=2.0.0"),
                        dependency("c-parallel", &target, "=4.0.0"),
                    ],
                    new_snapshot,
                ),
                row(&target, "4.0.0", vec![], new_snapshot),
                row(&target, "3.0.0", vec![], new_snapshot),
                row(&target, "2.0.0", vec![], new_snapshot),
                row(&target, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let updated = resolve(update).expect("occurrence-bound precise update");
        assert_eq!(
            root_selection(&updated.lockfile, "parent").version,
            version("2.0.0")
        );
        let selected_parent = root_selection(&updated.lockfile, "parent");
        let parent_node = updated
            .lockfile
            .nodes
            .iter()
            .find(|node| &node.release == selected_parent)
            .expect("selected parent node");
        assert_eq!(
            parent_node
                .dependencies
                .iter()
                .find(|edge| edge.alias.as_ref() == "a-targeted")
                .expect("targeted occurrence")
                .selected
                .version,
            version("2.0.0")
        );
        assert_eq!(
            parent_node
                .dependencies
                .iter()
                .find(|edge| edge.alias.as_ref() == "b-already-precise")
                .expect("unrelated precise sibling")
                .selected
                .version,
            version("2.0.0")
        );
        assert_eq!(
            parent_node
                .dependencies
                .iter()
                .find(|edge| edge.alias.as_ref() == "c-parallel")
                .expect("parallel sibling")
                .selected
                .version,
            version("4.0.0")
        );
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(target.clone(), version("3.0.0"))
        }));
    }

    #[test]
    fn precise_replay_treats_a_still_selected_old_parent_as_itself() {
        let old_snapshot = snapshot(28);
        let new_snapshot = snapshot(29);
        let parent = package("parent");
        let target = package("target");
        let previous = resolve(request(
            vec![root(vec![root_dependency(
                "old-parent",
                MusubiDependencyKindV1::Normal,
                &parent,
                "=1.0.0",
            )])],
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("target", &target, "*")],
                    old_snapshot,
                ),
                row(&target, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let mut update = request(
            vec![root(vec![root_dependency(
                "new-parent",
                MusubiDependencyKindV1::Normal,
                &parent,
                "=1.0.0",
            )])],
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("target", &target, "*")],
                    new_snapshot,
                ),
                row(&target, "2.0.0", vec![], new_snapshot),
                row(&target, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let updated = resolve(update).expect("exact old parent is an implicit self mapping");
        let selected_parent = root_selection(&updated.lockfile, "new-parent");
        let parent_node = updated
            .lockfile
            .nodes
            .iter()
            .find(|node| &node.release == selected_parent)
            .expect("selected parent node");
        assert_eq!(
            parent_node.dependencies[0].selected.version,
            version("2.0.0")
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the fixture builds the complete previously selected graph needed for replay coverage"
    )]
    fn precise_replay_propagates_through_an_already_selected_parent() {
        let old_snapshot = snapshot(30);
        let new_snapshot = snapshot(31);
        let parent = package("parent");
        let middle = package("middle");
        let target = package("target");
        let roots = vec![root(vec![
            root_dependency(
                "a-current",
                MusubiDependencyKindV1::Normal,
                &parent,
                "=2.0.0",
            ),
            root_dependency(
                "b-targeted",
                MusubiDependencyKindV1::Normal,
                &parent,
                "=1.0.0",
            ),
        ])];
        let old_rows = vec![
            row(
                &parent,
                "2.0.0",
                vec![dependency("middle", &middle, "=2.0.0")],
                old_snapshot,
            ),
            row(
                &parent,
                "1.0.0",
                vec![dependency("middle", &middle, "=1.0.0")],
                old_snapshot,
            ),
            row(
                &middle,
                "2.0.0",
                vec![dependency("target", &target, "=2.0.0")],
                old_snapshot,
            ),
            row(
                &middle,
                "1.0.0",
                vec![dependency("target", &target, "=1.0.0")],
                old_snapshot,
            ),
            row(&target, "2.0.0", vec![], old_snapshot),
            row(&target, "1.0.0", vec![], old_snapshot),
        ];
        let previous = resolve(request(roots, old_rows, old_snapshot))
            .expect("initial parallel lock")
            .lockfile;
        let mut update = request(
            vec![root(vec![
                root_dependency(
                    "a-current",
                    MusubiDependencyKindV1::Normal,
                    &parent,
                    "=2.0.0",
                ),
                root_dependency("b-targeted", MusubiDependencyKindV1::Normal, &parent, "*"),
            ])],
            vec![
                row(
                    &parent,
                    "2.0.0",
                    vec![dependency("middle", &middle, "=2.0.0")],
                    new_snapshot,
                ),
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("middle", &middle, "=1.0.0")],
                    new_snapshot,
                ),
                row(
                    &middle,
                    "2.0.0",
                    vec![dependency("target", &target, "=2.0.0")],
                    new_snapshot,
                ),
                row(
                    &middle,
                    "1.0.0",
                    vec![dependency("target", &target, "=1.0.0")],
                    new_snapshot,
                ),
                row(&target, "2.0.0", vec![], new_snapshot),
                row(&target, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let updated = resolve(update).expect("final graph replays through selected parent");
        assert_eq!(
            root_selection(&updated.lockfile, "b-targeted").version,
            version("2.0.0")
        );
        assert!(!updated.lockfile.nodes.iter().any(|node| {
            node.release == MusubiReleaseIdV1::new(target.clone(), version("1.0.0"))
        }));
    }

    #[test]
    fn precise_terminal_conflict_uses_the_selected_current_parent() {
        let old_snapshot = snapshot(32);
        let new_snapshot = snapshot(33);
        let parent = package("parent");
        let target = package("target");
        let roots = vec![root(vec![root_dependency(
            "parent",
            MusubiDependencyKindV1::Normal,
            &parent,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &parent,
                    "1.0.0",
                    vec![dependency("target", &target, "=1.0.0")],
                    old_snapshot,
                ),
                row(&target, "1.0.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let unavailable_parent = with_storage(
            row(
                &parent,
                "1.0.0",
                vec![dependency("target", &target, "=1.0.0")],
                new_snapshot,
            ),
            MusubiStorageAvailabilityV1::Unavailable,
        );
        let mut update = request(
            roots,
            vec![
                row(
                    &parent,
                    "2.0.0",
                    vec![dependency("renamed-target", &target, "=3.0.0")],
                    new_snapshot,
                ),
                unavailable_parent,
                row(&target, "3.0.0", vec![], new_snapshot),
                row(&target, "2.0.0", vec![], new_snapshot),
                row(&target, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target.clone(),
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let ResolverError::Conflict(conflict) = resolve(update).expect_err("renamed occurrence")
        else {
            panic!("expected precise dependency conflict");
        };
        assert_eq!(conflict.chain.len(), 2);
        assert!(matches!(
            &conflict.chain[0].parent,
            ConflictParentV1::Workspace(_)
        ));
        assert_eq!(
            conflict.chain[1].parent,
            ConflictParentV1::Release(MusubiReleaseIdV1::new(parent, version("2.0.0")))
        );
        assert_eq!(conflict.chain[1].alias.as_ref(), "target");
        assert_eq!(
            conflict.chain[1].requirement,
            MusubiVersionReqV1::Exact(version("2.0.0"))
        );
    }

    #[test]
    fn precise_update_rejects_an_occurrence_under_an_omitted_lock_root() {
        let old_snapshot = snapshot(34);
        let new_snapshot = snapshot(35);
        let target = package("target");
        let previous = resolve(request(
            vec![root(vec![root_dependency(
                "target",
                MusubiDependencyKindV1::Normal,
                &target,
                "*",
            )])],
            vec![row(&target, "1.0.0", vec![], old_snapshot)],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let mut update = request(
            vec![WorkspaceRootReqV1 {
                package: "test/other".parse().expect("other root selector"),
                dependencies: Vec::new(),
            }],
            vec![
                row(&target, "2.0.0", vec![], new_snapshot),
                row(&target, "1.0.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: target,
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let ResolverError::InvalidInput(message) = resolve(update).expect_err("omitted root")
        else {
            panic!("expected invalid targeted update");
        };
        assert!(message.contains("not reachable from the selected workspace roots"));
    }

    #[test]
    fn updated_parent_prefers_compatible_globally_locked_child() {
        let old_snapshot = snapshot(17);
        let new_snapshot = snapshot(18);
        let parent = package("parent");
        let child = package("child");
        let roots = vec![root(vec![root_dependency(
            "parent",
            MusubiDependencyKindV1::Normal,
            &parent,
            "*",
        )])];
        let old_parent_dependencies = vec![dependency("child", &child, "^1.0.0")];
        let previous = resolve(request(
            roots.clone(),
            vec![
                row(
                    &parent,
                    "1.0.0",
                    old_parent_dependencies.clone(),
                    old_snapshot,
                ),
                row(&child, "1.5.0", vec![], old_snapshot),
            ],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let mut update = request(
            roots,
            vec![
                row(&parent, "1.0.0", old_parent_dependencies, new_snapshot),
                row(
                    &parent,
                    "2.0.0",
                    vec![dependency("child", &child, "^1.0.0")],
                    new_snapshot,
                ),
                row(&child, "1.9.0", vec![], new_snapshot),
                row(&child, "1.5.0", vec![], new_snapshot),
            ],
            new_snapshot,
        );
        update.previous = Some(previous);
        update.update = Some(TargetedUpdateV1 {
            package: parent,
            locked_version: Some(version("1.0.0")),
            precise: Some(version("2.0.0")),
        });

        let updated = resolve(update).expect("targeted parent update");
        let selected_parent = root_selection(&updated.lockfile, "parent");
        let parent_node = updated
            .lockfile
            .nodes
            .iter()
            .find(|node| &node.release == selected_parent)
            .expect("updated parent node");
        assert_eq!(
            parent_node.dependencies[0].selected.version,
            version("1.5.0")
        );
    }

    #[test]
    fn locked_mode_reports_change_required_without_returning_a_rewrite() {
        let old_snapshot = snapshot(13);
        let new_snapshot = snapshot(14);
        let pkg = package("codec");
        let roots = vec![root(vec![root_dependency(
            "codec",
            MusubiDependencyKindV1::Normal,
            &pkg,
            "*",
        )])];
        let previous = resolve(request(
            roots.clone(),
            vec![row(&pkg, "1.0.0", vec![], old_snapshot)],
            old_snapshot,
        ))
        .expect("initial lock")
        .lockfile;
        let mut locked = request(
            roots,
            vec![row(&pkg, "2.0.0", vec![], new_snapshot)],
            new_snapshot,
        );
        locked.previous = Some(previous);
        locked.mode = ResolveModeV1::Locked;

        assert!(matches!(
            resolve(locked),
            Err(ResolverError::LockChangeRequired)
        ));
    }
}
