//! Deterministic dependency resolution for the Musubi V1 universal sparse index.
//!
//! Resolution consumes only finalized [`MusubiResolverReleaseRowV1`] values and
//! produces a consumer-owned exact [`LockfileV1`]. The search order is part of
//! the first-release behavior: reuse a compatible selection already present in
//! the graph, then a still-valid parent-local lock edge, then fresh releases in
//! descending SemVer order.

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

#[derive(Clone, Default)]
struct SearchState {
    selected: BTreeSet<MusubiReleaseIdV1>,
    edges: BTreeMap<ParentKey, BTreeMap<Name, MusubiExactDependencyEdgeV1>>,
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
    mode: ResolveModeV1,
    limits: Limits,
}

/// Resolve the exact consumer graph using first-release deterministic policy.
pub fn resolve(request: ResolveRequestV1) -> Result<ResolveOutcomeV1, ResolverError> {
    resolve_with_limits(request, Limits::default())
}

fn resolve_with_limits(
    request: ResolveRequestV1,
    limits: Limits,
) -> Result<ResolveOutcomeV1, ResolverError> {
    let solver = Solver::new(request, limits)?;
    solver.run()
}

impl Solver {
    fn new(mut request: ResolveRequestV1, limits: Limits) -> Result<Self, ResolverError> {
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
            mode: request.mode,
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
                    }
                })
            })
            .collect()
    }

    fn search(
        &self,
        state: SearchState,
        mut pending: Vec<PendingEdge>,
    ) -> Result<SearchState, ResolverError> {
        if pending.is_empty() {
            return Ok(state);
        }
        let task = pending.remove(0);
        if task.depth > self.limits.depth {
            return Err(ResolverError::Conflict(Box::new(ResolutionConflictV1 {
                chain: task.chain,
                reason: ConflictReasonV1::DepthLimit,
            })));
        }
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
            if self.would_cycle(&state, &task.parent, &candidate) {
                select_better_conflict(
                    &mut best_conflict,
                    ResolutionConflictV1 {
                        chain: task.chain.clone(),
                        reason: ConflictReasonV1::Cycle(candidate),
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
                    });
                }
            }
            next_pending.extend(pending.iter().cloned());
            match self.search(next, next_pending) {
                Ok(solution) => {
                    if parallel_version_excess(&solution) == minimum_parallel_excess {
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
        let update_edge = self
            .update
            .as_ref()
            .is_some_and(|update| preserved.is_some_and(|edge| edge.selected == update.target));
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

    fn locked_state_is_preservable(&self, row: &MusubiResolverReleaseRowV1) -> bool {
        self.locked_nodes
            .get(&row.release)
            .is_some_and(|node| row_matches_locked_node(row, node))
            && matches!(
                &row.selection.governance,
                MusubiArtifactGovernanceStateV1::Available
            )
            && row.selection.storage.availability != MusubiStorageAvailabilityV1::Unavailable
    }

    fn would_cycle(
        &self,
        state: &SearchState,
        parent: &ParentKey,
        candidate: &MusubiReleaseIdV1,
    ) -> bool {
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
                enacted_at_height: row.index_revision,
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
    fn overlapping_requirements_backtrack_to_one_compatible_version() {
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
        let resolved = resolve(next).expect("overlapping requirements converge");

        assert_eq!(resolved.lockfile.nodes.len(), 1);
        assert_eq!(
            root_selection(&resolved.lockfile, "a-broad").version,
            version("1.9.0")
        );
        assert_eq!(
            root_selection(&resolved.lockfile, "b-high").version,
            version("1.9.0")
        );
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

        assert_eq!(resolve(locked), Err(ResolverError::LockChangeRequired));
    }
}
