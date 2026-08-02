//! Finalized registry collection and workspace-to-resolver graph construction.
//!
//! This module is deliberately separate from command parsing. It turns selected
//! workspace packages (including recursively reachable local path packages)
//! into deterministic resolver roots, binds every public selector to a stable
//! structural package identity, and collects one coherent finalized sparse
//! index snapshot before invoking the pure backtracking resolver.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use iroha_data_model::musubi::{
    MUSUBI_MAX_PAGE_SIZE_V1, MUSUBI_MAX_RESOLUTION_NODES_V1, MusubiDependencyKindV1,
    MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1,
    MusubiPackageIdV1, MusubiPackageSelectorV1, MusubiPageRequestV1, MusubiRegistrySnapshotV1,
    MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1, MusubiResolverReleaseRowV1,
    MusubiVersionReqV1,
};

use crate::{
    lockfile::LockfileV1,
    manifest::ConcreteDependency,
    registry::{RegistryErrorV1, RegistryReadClientV1},
    registry_cache::{CachedResolverSourceV1, RecordingResolverSourceV1, ResolverIndexCacheV1},
    resolver::{
        ResolveModeV1, ResolveOutcomeV1, ResolveRequestV1, ResolverError, TargetedUpdateV1,
        WorkspaceDependencyReqV1, WorkspaceRootReqV1, resolve, resolve_fresh,
    },
    workspace::{EffectiveDependency, Workspace, WorkspaceError, WorkspaceMember},
};

const MAX_COLLECTED_RESOLVER_ROWS_V1: usize = MUSUBI_MAX_RESOLUTION_NODES_V1 * 16;

/// Read-only finalized registry surface needed by dependency resolution.
pub(crate) trait ResolverRegistrySourceV1 {
    /// Concrete source error returned by a network reader or cache replay.
    type Error: Error;

    /// Classify one source error at the graph boundary.
    fn map_error(error: Self::Error) -> GraphErrorV1;

    /// Read one exact finalized public-directory page.
    fn ordered_prefix(
        &self,
        request: &MusubiOrderedPrefixQueryV1,
    ) -> Result<MusubiOrderedPackagePageV1, Self::Error>;

    /// Read one exact finalized sparse resolver-index page.
    fn resolver_index(
        &self,
        request: &MusubiResolverIndexQueryV1,
    ) -> Result<MusubiResolverIndexPageV1, Self::Error>;
}

impl ResolverRegistrySourceV1 for RegistryReadClientV1 {
    type Error = RegistryErrorV1;

    fn map_error(error: Self::Error) -> GraphErrorV1 {
        GraphErrorV1::Registry(error.to_string())
    }

    fn ordered_prefix(
        &self,
        request: &MusubiOrderedPrefixQueryV1,
    ) -> Result<MusubiOrderedPackagePageV1, Self::Error> {
        RegistryReadClientV1::ordered_prefix(self, request)
    }

    fn resolver_index(
        &self,
        request: &MusubiResolverIndexQueryV1,
    ) -> Result<MusubiResolverIndexPageV1, Self::Error> {
        RegistryReadClientV1::resolver_index(self, request)
    }
}

/// Stable resolution-collection failure.
#[derive(Debug)]
pub(crate) enum GraphErrorV1 {
    /// Workspace path packages or their dependency graph are invalid.
    Workspace(WorkspaceError),
    /// A public registry query failed with a redacted stable code.
    Registry(String),
    /// Durable resolver-cache publication or validation failed.
    Cache(String),
    /// No complete coherent cached snapshot covered the requested offline graph.
    OfflineMiss(String),
    /// A namespace/package selector had no exact immutable binding.
    PackageNotFound(MusubiPackageSelectorV1),
    /// Independent pages did not represent one exact chain/genesis/snapshot.
    SnapshotChanged,
    /// Finalized query output was internally inconsistent.
    InvalidRegistryData(String),
    /// The bounded candidate collection exceeded its fixed safety ceiling.
    CandidateLimit,
    /// The deterministic solver rejected the collected graph.
    Resolver(ResolverError),
}

impl fmt::Display for GraphErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(error) => write!(formatter, "{error}"),
            Self::Registry(code) => write!(formatter, "registry query failed: {code}"),
            Self::Cache(reason) => write!(formatter, "resolver cache failed: {reason}"),
            Self::OfflineMiss(reason) => write!(formatter, "{reason}"),
            Self::PackageNotFound(package) => {
                write!(formatter, "registry package `{package}` was not found")
            }
            Self::SnapshotChanged => formatter.write_str(
                "registry pages changed finalized chain, genesis, or sparse-index snapshot",
            ),
            Self::InvalidRegistryData(reason) => {
                write!(
                    formatter,
                    "registry returned invalid resolver data: {reason}"
                )
            }
            Self::CandidateLimit => write!(
                formatter,
                "resolver candidate collection exceeds {MAX_COLLECTED_RESOLVER_ROWS_V1} rows"
            ),
            Self::Resolver(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for GraphErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Workspace(error) => Some(error),
            Self::Resolver(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkspaceError> for GraphErrorV1 {
    fn from(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

impl From<ResolverError> for GraphErrorV1 {
    fn from(error: ResolverError) -> Self {
        Self::Resolver(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegistryAnchorV1 {
    chain_id: iroha_data_model::ChainId,
    genesis_hash: [u8; 32],
    snapshot: MusubiRegistrySnapshotV1,
}

impl RegistryAnchorV1 {
    fn observe_ordered(
        anchor: &mut Option<Self>,
        page: &MusubiOrderedPackagePageV1,
    ) -> Result<(), GraphErrorV1> {
        Self::observe(
            anchor,
            Self {
                chain_id: page.chain_id.clone(),
                genesis_hash: page.genesis_hash,
                snapshot: page.snapshot,
            },
        )
    }

    fn observe_resolver(
        anchor: &mut Option<Self>,
        page: &MusubiResolverIndexPageV1,
    ) -> Result<(), GraphErrorV1> {
        Self::observe(
            anchor,
            Self {
                chain_id: page.chain_id.clone(),
                genesis_hash: page.genesis_hash,
                snapshot: page.snapshot,
            },
        )
    }

    fn observe(anchor: &mut Option<Self>, candidate: Self) -> Result<(), GraphErrorV1> {
        match anchor {
            Some(current) if current != &candidate => Err(GraphErrorV1::SnapshotChanged),
            Some(_) => Ok(()),
            None => {
                *anchor = Some(candidate);
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug)]
struct LocalRootSpecV1 {
    package: MusubiPackageSelectorV1,
    dependencies: Vec<LocalDependencySpecV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LocalDependencySpecV1 {
    alias: iroha_data_model::name::Name,
    kind: MusubiDependencyKindV1,
    package: MusubiPackageSelectorV1,
    requirement: MusubiVersionReqV1,
}

/// User-facing targeted update before the selector is normalized structurally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphUpdateV1 {
    /// Canonical namespace/package selector from `-p`.
    pub package: MusubiPackageSelectorV1,
    /// Optional currently locked version from `PACKAGE@VERSION`.
    pub locked_version: Option<iroha_data_model::musubi::MusubiVersionV1>,
    /// Optional exact replacement requested with `--precise`.
    pub precise: Option<iroha_data_model::musubi::MusubiVersionV1>,
}

/// Resolve the selected workspace graph against one coherent finalized registry snapshot.
pub(crate) fn resolve_workspace_online(
    registry: &RegistryReadClientV1,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
) -> Result<ResolveOutcomeV1, GraphErrorV1> {
    resolve_workspace_from_source(registry, workspace, selected, previous, update, mode)
}

/// Resolve online and atomically publish only the coherent validated pages consumed.
pub(crate) fn resolve_workspace_online_cached(
    registry: &RegistryReadClientV1,
    cache: &ResolverIndexCacheV1,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
) -> Result<ResolveOutcomeV1, GraphErrorV1> {
    resolve_workspace_online_cached_with_policy(
        registry, cache, workspace, selected, previous, update, mode, false,
    )
}

/// Resolve and cache a publication graph that contains only fresh-selectable releases.
pub(crate) fn resolve_workspace_online_cached_fresh(
    registry: &RegistryReadClientV1,
    cache: &ResolverIndexCacheV1,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
) -> Result<ResolveOutcomeV1, GraphErrorV1> {
    resolve_workspace_online_cached_with_policy(
        registry, cache, workspace, selected, previous, update, mode, true,
    )
}

fn resolve_workspace_online_cached_with_policy(
    registry: &RegistryReadClientV1,
    cache: &ResolverIndexCacheV1,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
    fresh_only: bool,
) -> Result<ResolveOutcomeV1, GraphErrorV1> {
    let recorder = RecordingResolverSourceV1::new(registry);
    let outcome = resolve_workspace_from_source_with_policy(
        &recorder, workspace, selected, previous, update, mode, fresh_only,
    )?;
    let snapshot = recorder
        .finish()
        .map_err(|error| GraphErrorV1::Cache(error.to_string()))?;
    cache
        .publish(snapshot)
        .map_err(|error| GraphErrorV1::Cache(error.to_string()))?;
    Ok(outcome)
}

/// Successful offline resolution and the one coherent cached source it consumed.
pub(crate) struct CachedResolveOutcomeV1 {
    /// Exact lock outcome produced by the deterministic resolver.
    pub outcome: ResolveOutcomeV1,
    /// Snapshot source retained for local namespace binding during packaging.
    pub source: CachedResolverSourceV1,
}

/// Resolve entirely from the newest complete coherent cached snapshot.
///
/// A missing query in a newer snapshot permits trying an older compatible
/// snapshot. Any semantic resolver result (including governed unavailability,
/// yank-aware selection, or a dependency conflict) is final and is never
/// weakened by falling back to older data.
pub(crate) fn resolve_workspace_offline_cached(
    cache: &ResolverIndexCacheV1,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
) -> Result<CachedResolveOutcomeV1, GraphErrorV1> {
    let sources = cache
        .sources(previous.as_ref())
        .map_err(|error| GraphErrorV1::OfflineMiss(error.to_string()))?;
    let mut last_miss = None;
    for source in sources {
        match resolve_workspace_from_source(
            &source,
            workspace,
            selected,
            previous.clone(),
            update.clone(),
            mode,
        ) {
            Ok(outcome) => return Ok(CachedResolveOutcomeV1 { outcome, source }),
            Err(GraphErrorV1::OfflineMiss(reason)) => last_miss = Some(reason),
            Err(error) => return Err(error),
        }
    }
    Err(GraphErrorV1::OfflineMiss(last_miss.unwrap_or_else(|| {
        "MUSUBI_E_OFFLINE_MISS: no cached snapshot covers the requested graph".to_owned()
    })))
}

fn resolve_workspace_from_source<S: ResolverRegistrySourceV1>(
    source: &S,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
) -> Result<ResolveOutcomeV1, GraphErrorV1> {
    resolve_workspace_from_source_with_policy(
        source, workspace, selected, previous, update, mode, false,
    )
}

fn resolve_workspace_from_source_with_policy<S: ResolverRegistrySourceV1>(
    source: &S,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    mode: ResolveModeV1,
    fresh_only: bool,
) -> Result<ResolveOutcomeV1, GraphErrorV1> {
    let local_roots = collect_local_roots(workspace, selected)?;
    let mut anchor = None;
    let mut bindings = BTreeMap::new();
    let mut selectors = local_roots
        .iter()
        .flat_map(|root| {
            root.dependencies
                .iter()
                .map(|dependency| dependency.package.clone())
        })
        .collect::<BTreeSet<_>>();
    if let Some(update) = &update {
        selectors.insert(update.package.clone());
    }

    for selector in selectors {
        let package = resolve_selector(source, &selector, &mut anchor)?;
        bindings.insert(selector, package);
    }

    if anchor.is_none() {
        let first = local_roots.first().ok_or_else(|| {
            GraphErrorV1::InvalidRegistryData("selected workspace graph has no roots".to_owned())
        })?;
        observe_namespace_anchor(source, &first.package, &mut anchor)?;
    }

    let roots = local_roots
        .into_iter()
        .map(|root| {
            let dependencies = root
                .dependencies
                .into_iter()
                .map(|dependency| {
                    let package = bindings.get(&dependency.package).cloned().ok_or_else(|| {
                        GraphErrorV1::InvalidRegistryData(format!(
                            "missing structural binding for `{}`",
                            dependency.package
                        ))
                    })?;
                    Ok(WorkspaceDependencyReqV1 {
                        alias: dependency.alias,
                        kind: dependency.kind,
                        package,
                        requirement: dependency.requirement,
                    })
                })
                .collect::<Result<Vec<_>, GraphErrorV1>>()?;
            Ok(WorkspaceRootReqV1 {
                package: root.package,
                dependencies,
            })
        })
        .collect::<Result<Vec<_>, GraphErrorV1>>()?;

    let targeted_update = update
        .map(|update| {
            bindings
                .get(&update.package)
                .cloned()
                .ok_or_else(|| GraphErrorV1::PackageNotFound(update.package.clone()))
                .map(|package| TargetedUpdateV1 {
                    package,
                    locked_version: update.locked_version,
                    precise: update.precise,
                })
        })
        .transpose()?;

    let mut requirements = BTreeSet::new();
    for root in &roots {
        requirements.extend(
            root.dependencies
                .iter()
                .map(|dependency| (dependency.package.clone(), dependency.requirement.clone())),
        );
    }
    if let Some(lock) = &previous {
        requirements.extend(lock.nodes.iter().map(|node| {
            (
                node.release.package.clone(),
                MusubiVersionReqV1::Exact(node.release.version.clone()),
            )
        }));
    }

    let mut queried = BTreeSet::new();
    let mut rows = BTreeMap::new();
    while let Some(query_key) = requirements.pop_first() {
        if !queried.insert(query_key.clone()) {
            continue;
        }
        let (package, requirement) = query_key;
        let fetched = collect_requirement_rows(source, package, requirement, &mut anchor)?;
        for row in fetched {
            for dependency in &row.dependencies {
                requirements.insert((dependency.package.clone(), dependency.requirement.clone()));
            }
            match rows.insert(row.release.clone(), row.clone()) {
                Some(previous) if previous != row => {
                    return Err(GraphErrorV1::InvalidRegistryData(format!(
                        "conflicting rows for `{}`",
                        row.release
                    )));
                }
                _ => {}
            }
            if rows.len() > MAX_COLLECTED_RESOLVER_ROWS_V1 {
                return Err(GraphErrorV1::CandidateLimit);
            }
        }
    }

    let anchor = anchor.expect("one ordered or resolver query establishes an anchor");
    let request = ResolveRequestV1 {
        chain_id: anchor.chain_id,
        genesis_hash: anchor.genesis_hash,
        snapshot: anchor.snapshot,
        roots,
        rows: rows.into_values().collect(),
        previous,
        update: targeted_update,
        mode,
    };
    if fresh_only {
        resolve_fresh(request)
    } else {
        resolve(request)
    }
    .map_err(GraphErrorV1::from)
}

fn collect_local_roots(
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
) -> Result<Vec<LocalRootSpecV1>, GraphErrorV1> {
    let packages = collect_local_members(workspace, selected)?;
    let selected = selected.iter().cloned().collect::<BTreeSet<_>>();
    let mut roots = Vec::with_capacity(packages.len());
    for member in packages {
        let include_dev = selected.contains(&member.package.selector);
        let mut dependencies = member
            .dependencies
            .values()
            .filter_map(local_requirement)
            .collect::<Result<Vec<_>, _>>()?;
        if include_dev {
            dependencies.extend(
                member
                    .dev_dependencies
                    .values()
                    .filter_map(local_requirement)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        dependencies.sort();
        if dependencies
            .windows(2)
            .any(|pair| pair[0].alias == pair[1].alias)
        {
            return Err(GraphErrorV1::InvalidRegistryData(format!(
                "local package `{}` has duplicate effective dependency aliases",
                member.package.selector
            )));
        }
        roots.push(LocalRootSpecV1 {
            package: member.package.selector,
            dependencies,
        });
    }
    roots.sort_by(|left, right| left.package.cmp(&right.package));
    Ok(roots)
}

/// Collect selected workspace members and recursively reachable local path packages.
pub(crate) fn collect_local_members(
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
) -> Result<Vec<WorkspaceMember>, GraphErrorV1> {
    let selected = selected.iter().cloned().collect::<BTreeSet<_>>();
    if selected.is_empty() {
        return Err(GraphErrorV1::InvalidRegistryData(
            "at least one workspace package must be selected".to_owned(),
        ));
    }
    let by_selector = workspace
        .members()
        .values()
        .map(|member| (member.package.selector.clone(), member))
        .collect::<BTreeMap<_, _>>();
    let mut pending = selected
        .iter()
        .map(|selector| {
            by_selector
                .get(selector)
                .map(|member| (*member).clone())
                .ok_or_else(|| {
                    GraphErrorV1::InvalidRegistryData(format!(
                        "selected package `{selector}` is not a workspace member"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    pending.sort_by(|left, right| right.package.selector.cmp(&left.package.selector));

    let mut packages = BTreeMap::<MusubiPackageSelectorV1, WorkspaceMember>::new();
    let mut manifests = BTreeMap::new();
    while let Some(member) = pending.pop() {
        if let Some(previous_path) = manifests.insert(
            member.manifest_path.clone(),
            member.package.selector.clone(),
        ) && previous_path != member.package.selector
        {
            return Err(GraphErrorV1::InvalidRegistryData(format!(
                "manifest `{}` changed package identity",
                member.manifest_path.display()
            )));
        }
        if let Some(previous) = packages.get(&member.package.selector) {
            if previous.manifest_path != member.manifest_path {
                return Err(GraphErrorV1::InvalidRegistryData(format!(
                    "local package `{}` is declared by both `{}` and `{}`",
                    member.package.selector,
                    previous.manifest_path.display(),
                    member.manifest_path.display()
                )));
            }
            continue;
        }
        let include_dev = selected.contains(&member.package.selector);
        for dependency in member
            .dependencies
            .values()
            .chain(member.dev_dependencies.values().filter(|_| include_dev))
        {
            if let Some(manifest_path) = &dependency.local_manifest {
                pending.push(workspace.load_path_package(manifest_path)?);
            }
        }
        pending.sort_by(|left, right| right.package.selector.cmp(&left.package.selector));
        packages.insert(member.package.selector.clone(), member);
    }

    Ok(packages.into_values().collect())
}

fn local_requirement(
    dependency: &EffectiveDependency,
) -> Option<Result<LocalDependencySpecV1, GraphErrorV1>> {
    let (package, requirement) = match &dependency.dependency {
        ConcreteDependency::Registry {
            package,
            requirement,
        } => (package.clone(), requirement.clone()),
        ConcreteDependency::Path {
            package: Some(package),
            requirement: Some(requirement),
            ..
        } => (package.clone(), requirement.clone()),
        ConcreteDependency::Path {
            package: None,
            requirement: None,
            ..
        } => return None,
        ConcreteDependency::Path { .. } => {
            return Some(Err(GraphErrorV1::InvalidRegistryData(format!(
                "path dependency `{}` has only one publication identity field",
                dependency.alias
            ))));
        }
    };
    Some(Ok(LocalDependencySpecV1 {
        alias: dependency.alias.clone(),
        kind: match dependency.kind {
            crate::workspace::DependencyKind::Normal => MusubiDependencyKindV1::Normal,
            crate::workspace::DependencyKind::Development => MusubiDependencyKindV1::Development,
        },
        package,
        requirement,
    }))
}

fn resolve_selector<S: ResolverRegistrySourceV1>(
    source: &S,
    selector: &MusubiPackageSelectorV1,
    anchor: &mut Option<RegistryAnchorV1>,
) -> Result<MusubiPackageIdV1, GraphErrorV1> {
    let page = source
        .ordered_prefix(&MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new(&selector.to_string())
                .map_err(|error| GraphErrorV1::InvalidRegistryData(error.reason().to_owned()))?,
            page: MusubiPageRequestV1 {
                limit: 2,
                cursor: None,
            },
        })
        .map_err(S::map_error)?;
    RegistryAnchorV1::observe_ordered(anchor, &page)?;
    let entry = page
        .items
        .into_iter()
        .find(|entry| &entry.selector == selector)
        .ok_or_else(|| GraphErrorV1::PackageNotFound(selector.clone()))?;
    if entry.package.name != selector.name || entry.index_revision != page.snapshot.index_revision {
        return Err(GraphErrorV1::InvalidRegistryData(format!(
            "directory binding for `{selector}` is inconsistent"
        )));
    }
    Ok(entry.package)
}

fn observe_namespace_anchor<S: ResolverRegistrySourceV1>(
    source: &S,
    selector: &MusubiPackageSelectorV1,
    anchor: &mut Option<RegistryAnchorV1>,
) -> Result<(), GraphErrorV1> {
    let prefix = format!("{}/", selector.namespace);
    let page = source
        .ordered_prefix(&MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new(&prefix)
                .map_err(|error| GraphErrorV1::InvalidRegistryData(error.reason().to_owned()))?,
            page: MusubiPageRequestV1 {
                limit: 1,
                cursor: None,
            },
        })
        .map_err(S::map_error)?;
    if page.namespace_binding.namespace != selector.namespace {
        return Err(GraphErrorV1::InvalidRegistryData(format!(
            "namespace binding for `{selector}` does not match its ordered-prefix query"
        )));
    }
    RegistryAnchorV1::observe_ordered(anchor, &page)
}

fn collect_requirement_rows<S: ResolverRegistrySourceV1>(
    source: &S,
    package: MusubiPackageIdV1,
    requirement: MusubiVersionReqV1,
    anchor: &mut Option<RegistryAnchorV1>,
) -> Result<Vec<MusubiResolverReleaseRowV1>, GraphErrorV1> {
    let mut cursor = None;
    let mut seen_cursor_keys = BTreeSet::new();
    let mut rows = Vec::new();
    loop {
        let page = source
            .resolver_index(&MusubiResolverIndexQueryV1 {
                package: package.clone(),
                requirement: Some(requirement.clone()),
                page: MusubiPageRequestV1 {
                    limit: u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1)
                        .expect("Musubi page maximum fits u32"),
                    cursor,
                },
            })
            .map_err(S::map_error)?;
        RegistryAnchorV1::observe_resolver(anchor, &page)?;
        for row in page.items {
            if row.release.package != package
                || !requirement.matches(&row.release.version)
                || row.index_revision != page.snapshot.index_revision
            {
                return Err(GraphErrorV1::InvalidRegistryData(format!(
                    "resolver row `{}` does not match its query",
                    row.release
                )));
            }
            rows.push(row);
            if rows.len() > MAX_COLLECTED_RESOLVER_ROWS_V1 {
                return Err(GraphErrorV1::CandidateLimit);
            }
        }
        let Some(next) = page.next_cursor else {
            break;
        };
        if !seen_cursor_keys.insert(next.last_key.clone()) {
            return Err(GraphErrorV1::InvalidRegistryData(
                "resolver pagination cursor did not advance".to_owned(),
            ));
        }
        cursor = Some(next);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use iroha_data_model::musubi::{MusubiNamespaceBindingV1, MusubiPackageScopeV1};
    use tempfile::TempDir;

    use super::*;
    use crate::workspace::load_workspace;

    const APP: &str = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "app"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
exports = []
"#;

    fn write(path: &Path, source: &str) {
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create parent");
        fs::write(path, source).expect("write fixture");
    }

    #[derive(Debug)]
    struct FakeError;

    impl fmt::Display for FakeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("fake registry failure")
        }
    }

    impl Error for FakeError {}

    struct AnchorOnlyRegistry {
        page: MusubiOrderedPackagePageV1,
    }

    struct LoopingResolverRegistry {
        page: MusubiResolverIndexPageV1,
    }

    impl ResolverRegistrySourceV1 for LoopingResolverRegistry {
        type Error = FakeError;

        fn map_error(error: Self::Error) -> GraphErrorV1 {
            GraphErrorV1::Registry(error.to_string())
        }

        fn ordered_prefix(
            &self,
            _request: &MusubiOrderedPrefixQueryV1,
        ) -> Result<MusubiOrderedPackagePageV1, Self::Error> {
            Err(FakeError)
        }

        fn resolver_index(
            &self,
            _request: &MusubiResolverIndexQueryV1,
        ) -> Result<MusubiResolverIndexPageV1, Self::Error> {
            Ok(self.page.clone())
        }
    }

    impl ResolverRegistrySourceV1 for AnchorOnlyRegistry {
        type Error = FakeError;

        fn map_error(error: Self::Error) -> GraphErrorV1 {
            GraphErrorV1::Registry(error.to_string())
        }

        fn ordered_prefix(
            &self,
            _request: &MusubiOrderedPrefixQueryV1,
        ) -> Result<MusubiOrderedPackagePageV1, Self::Error> {
            Ok(self.page.clone())
        }

        fn resolver_index(
            &self,
            _request: &MusubiResolverIndexQueryV1,
        ) -> Result<MusubiResolverIndexPageV1, Self::Error> {
            Err(FakeError)
        }
    }

    fn anchor_page(hash: u8) -> MusubiOrderedPackagePageV1 {
        MusubiOrderedPackagePageV1 {
            chain_id: "musubi-graph-test".parse().expect("chain id"),
            genesis_hash: [7; 32],
            namespace_binding: MusubiNamespaceBindingV1 {
                namespace: "apps.sora".parse().expect("namespace"),
                home_dataspace: iroha_data_model::nexus::DataSpaceId::new(7),
                scope: MusubiPackageScopeV1::Domain("apps".parse().expect("domain")),
                generation: 1,
            },
            items: Vec::new(),
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 10,
                finalized_block_hash: [hash; 32],
                index_revision: 4,
            },
        }
    }

    #[test]
    fn dependency_free_workspace_uses_ordered_page_lock_identity() {
        let temp = TempDir::new().expect("temporary directory");
        write(&temp.path().join("Musubi.toml"), APP);
        let workspace = load_workspace(temp.path()).expect("workspace");
        let selected = vec!["apps.sora/app".parse().expect("selector")];
        let result = resolve_workspace_from_source(
            &AnchorOnlyRegistry {
                page: anchor_page(8),
            },
            &workspace,
            &selected,
            None,
            None,
            ResolveModeV1::UpdateLock,
        )
        .expect("dependency-free resolution");
        assert!(result.changed);
        assert_eq!(result.lockfile.chain_id.as_str(), "musubi-graph-test");
        assert_eq!(result.lockfile.genesis_hash, [7; 32]);
        assert_eq!(result.lockfile.roots.len(), 1);
        assert!(result.lockfile.nodes.is_empty());
    }

    #[test]
    fn dependency_free_workspace_rejects_a_different_namespace_binding() {
        let temp = TempDir::new().expect("temporary directory");
        write(&temp.path().join("Musubi.toml"), APP);
        let workspace = load_workspace(temp.path()).expect("workspace");
        let selected = vec!["apps.sora/app".parse().expect("selector")];
        let mut page = anchor_page(8);
        page.namespace_binding = MusubiNamespaceBindingV1 {
            namespace: "other.sora".parse().expect("namespace"),
            home_dataspace: iroha_data_model::nexus::DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::Domain("other".parse().expect("domain")),
            generation: 1,
        };
        assert!(matches!(
            resolve_workspace_from_source(
                &AnchorOnlyRegistry { page },
                &workspace,
                &selected,
                None,
                None,
                ResolveModeV1::UpdateLock,
            ),
            Err(GraphErrorV1::InvalidRegistryData(_))
        ));
    }

    #[test]
    fn selected_dev_path_is_local_but_its_dev_dependencies_do_not_propagate() {
        let temp = TempDir::new().expect("temporary directory");
        write(
            &temp.path().join("Musubi.toml"),
            &APP.replace(
                "[lib]\nexports = []",
                "[lib]\nexports = []\n[dev-dependencies]\nhelper = { path = \"helper\" }",
            ),
        );
        write(
            &temp.path().join("helper/Musubi.toml"),
            r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "helper"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
exports = []
[dependencies]
core = { package = "libs.sora/core", version = "^1.0.0" }
[dev-dependencies]
ignored = { package = "libs.sora/ignored", version = "^1.0.0" }
"#,
        );
        let workspace = load_workspace(temp.path()).expect("workspace");
        let selected = vec!["apps.sora/app".parse().expect("selector")];
        let roots = collect_local_roots(&workspace, &selected).expect("local roots");
        assert_eq!(
            roots
                .iter()
                .map(|root| root.package.to_string())
                .collect::<Vec<_>>(),
            ["apps.sora/app", "apps.sora/helper"]
        );
        let helper = roots
            .iter()
            .find(|root| root.package.to_string() == "apps.sora/helper")
            .expect("helper root");
        assert_eq!(helper.dependencies.len(), 1);
        assert_eq!(helper.dependencies[0].alias.as_ref(), "core");
        assert_eq!(helper.dependencies[0].kind, MusubiDependencyKindV1::Normal);
    }

    #[test]
    fn registry_anchor_rejects_snapshot_mixing() {
        let mut anchor = None;
        RegistryAnchorV1::observe_ordered(&mut anchor, &anchor_page(1)).expect("first anchor");
        assert!(matches!(
            RegistryAnchorV1::observe_ordered(&mut anchor, &anchor_page(2)),
            Err(GraphErrorV1::SnapshotChanged)
        ));
    }

    #[test]
    fn resolver_collection_rejects_a_repeated_cursor() {
        use iroha_data_model::musubi::{
            MusubiFinalizedCursorV1, MusubiQueryHashV1, MusubiVersionReqV1,
        };

        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 10,
            finalized_block_hash: [8; 32],
            index_revision: 4,
        };
        let page = MusubiResolverIndexPageV1 {
            chain_id: "musubi-graph-test".parse().expect("chain id"),
            genesis_hash: [7; 32],
            items: Vec::new(),
            next_cursor: Some(MusubiFinalizedCursorV1 {
                snapshot: snapshot.clone(),
                query_hash: MusubiQueryHashV1::new([6; 32]),
                last_key: "same-cursor".to_owned(),
                caller: None,
            }),
            snapshot,
        };
        let package = MusubiPackageIdV1::new(
            iroha_data_model::nexus::DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("package name"),
        );
        let error = collect_requirement_rows(
            &LoopingResolverRegistry { page },
            package,
            MusubiVersionReqV1::Any,
            &mut None,
        )
        .expect_err("a repeating cursor must not create an infinite query loop");
        assert!(matches!(error, GraphErrorV1::InvalidRegistryData(_)));
    }
}
