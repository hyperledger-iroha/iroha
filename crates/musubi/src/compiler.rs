//! Authenticated Musubi lock graph to canonical Kotodama compiler bridge.
//!
//! Registry sources cross this boundary only after the immutable cache has
//! re-authenticated the complete bundle against an exact lock node. Local path
//! packages remain explicit `local:` identities, while every registry import
//! uses the exact structural release identity selected by the consumer lock.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::PathBuf,
};

use iroha_data_model::musubi::{
    MusubiContentDigestV1, MusubiDependencyKindV1, MusubiPackageSelectorV1,
    MusubiVerificationLockV1, MusubiVerificationNodeV1,
};
use ivm::{
    SyscallPolicy,
    kotodama::{
        compiler::{CompilerMode, CompilerOptions},
        driver::{
            BuildDriver, BuildStatus, LinkedSourceBuildRequest, PublishLayout, PublishMode,
            discover_source_link_request, discover_source_modules,
        },
        linker::{
            ImportBinding, SourceLinkRequest, SourceModuleUnit, SourcePackageGraphRequest,
            SourcePackageUnit,
        },
        session::CompilerSession,
    },
    syscalls::compute_abi_hash,
};

use crate::{
    cache::{CachedCompilerPackageV1, MusubiCache},
    graph::{GraphErrorV1, collect_local_members},
    lockfile::LockfileV1,
    manifest::{ConcreteDependency, DependencySpec, PortablePath, parse_manifest},
    package::PackagePlan,
    workspace::{EffectiveDependency, Workspace, WorkspaceMember},
};

/// Compiler operation requested by the Cargo-style command surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompilerActionV1 {
    /// Parse, type-check, link, and lint without writing artifacts.
    Check,
    /// Compile and atomically publish every selected local contract target.
    Build,
}

/// One generated contract artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompilerArtifactV1 {
    /// Selected package declaring the contract target.
    pub package: MusubiPackageSelectorV1,
    /// Manifest target name, with a deterministic ordinal for directory targets.
    pub target: String,
    /// Canonical source path used in diagnostics.
    pub source: String,
    /// Published `.to` path.
    pub artifact: PathBuf,
    /// Canonical code hash.
    pub artifact_hash: String,
    /// Whether compilation ran or authenticated outputs were already fresh.
    pub fresh: bool,
}

/// Canonical typed interface proven for one reusable local package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompilerPackageInterfaceV1 {
    /// Public package selector used by the workspace root.
    pub package: MusubiPackageSelectorV1,
    /// Domain-separated digest of the exact exported function signatures.
    pub digest: MusubiContentDigestV1,
}

/// Successful compiler graph execution summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompilerExecutionV1 {
    /// Number of local reusable packages validated through the typed linker.
    pub validated_packages: usize,
    /// Number of deployable contract roots checked or built.
    pub contract_targets: usize,
    /// Total non-fatal lint findings from checked deployable roots.
    pub warnings: usize,
    /// Generated artifacts (empty for `check`).
    pub artifacts: Vec<CompilerArtifactV1>,
    /// Typed reusable-package interfaces in canonical package order.
    pub package_interfaces: Vec<CompilerPackageInterfaceV1>,
}

/// Stable compiler-bridge failure.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CompilerBridgeErrorV1 {
    /// Local workspace/path graph construction failed.
    Workspace(String),
    /// The exact lock graph is missing a required parent-local edge.
    Lock(String),
    /// An authenticated cache entry is absent, corrupt, or inconsistent.
    Cache(String),
    /// A cached or local manifest/source set is not a valid V1 package.
    Package(String),
    /// The canonical Kotodama compiler rejected the graph.
    Compiler(String),
}

impl fmt::Display for CompilerBridgeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(reason) => write!(formatter, "invalid local compiler graph: {reason}"),
            Self::Lock(reason) => write!(formatter, "invalid exact compiler lock: {reason}"),
            Self::Cache(reason) => write!(formatter, "authenticated package cache error: {reason}"),
            Self::Package(reason) => write!(formatter, "invalid compiler package: {reason}"),
            Self::Compiler(reason) => write!(formatter, "Kotodama compiler failed: {reason}"),
        }
    }
}

impl Error for CompilerBridgeErrorV1 {}

trait RegistryCompilerSourceV1 {
    fn load(
        &self,
        node: &MusubiVerificationNodeV1,
    ) -> Result<CachedCompilerPackageV1, CompilerBridgeErrorV1>;
}

impl RegistryCompilerSourceV1 for MusubiCache {
    fn load(
        &self,
        node: &MusubiVerificationNodeV1,
    ) -> Result<CachedCompilerPackageV1, CompilerBridgeErrorV1> {
        self.load_compiler_package(node)
            .map_err(|error| CompilerBridgeErrorV1::Cache(error.to_string()))
    }
}

/// Execute one authenticated compiler operation for selected workspace packages.
pub(crate) fn execute_compiler_graph(
    cache: &MusubiCache,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    lock: &LockfileV1,
    action: CompilerActionV1,
    release: bool,
    chain_discriminant: u16,
) -> Result<CompilerExecutionV1, CompilerBridgeErrorV1> {
    execute_with_source(
        cache,
        workspace,
        selected,
        lock,
        action,
        release,
        chain_discriminant,
    )
}

/// Rebuild and validate the exact clean source tree that will enter a release bundle.
///
/// Unlike ordinary workspace checking, every root import is taken from the normalized
/// publication lock. A local path dependency therefore cannot influence the packaged build.
pub(crate) fn validate_packaged_plan(
    cache: &MusubiCache,
    plan: &PackagePlan,
    verification_lock: &MusubiVerificationLockV1,
    chain_discriminant: u16,
) -> Result<MusubiContentDigestV1, CompilerBridgeErrorV1> {
    validate_packaged_with_source(cache, plan, verification_lock, chain_discriminant)
}

fn validate_packaged_with_source<S: RegistryCompilerSourceV1>(
    source: &S,
    plan: &PackagePlan,
    verification_lock: &MusubiVerificationLockV1,
    chain_discriminant: u16,
) -> Result<MusubiContentDigestV1, CompilerBridgeErrorV1> {
    if chain_discriminant == 0 {
        return Err(CompilerBridgeErrorV1::Package(
            "account chain discriminant must be non-zero".to_owned(),
        ));
    }
    verification_lock
        .validate()
        .map_err(|error| CompilerBridgeErrorV1::Lock(error.to_string()))?;
    let manifest_source = std::str::from_utf8(plan.canonical_manifest())
        .map_err(|_| CompilerBridgeErrorV1::Package("packaged manifest is not UTF-8".to_owned()))?;
    let manifest = parse_manifest(manifest_source)
        .map_err(|error| CompilerBridgeErrorV1::Package(error.to_string()))?;
    if manifest.workspace.is_some() || !manifest.dev_dependencies.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(
            "clean publication manifest retains workspace or development state".to_owned(),
        ));
    }
    let package = manifest
        .resolve_package(None)
        .map_err(|error| CompilerBridgeErrorV1::Package(error.to_string()))?;
    if package.selector.name != verification_lock.root.package.name
        || package.version != verification_lock.root.version
    {
        return Err(CompilerBridgeErrorV1::Package(
            "clean package identity disagrees with its structural release root".to_owned(),
        ));
    }
    validate_manifest_dependency_edges(
        &verification_lock.root,
        &verification_lock.root_dependencies,
        &manifest.dependencies,
    )?;
    let library = manifest.library.as_ref().ok_or_else(|| {
        CompilerBridgeErrorV1::Package("clean publication manifest has no library".to_owned())
    })?;
    let mut modules = Vec::new();
    for file in plan.files() {
        let Some(source_name) = relative_library_source(file.path(), &library.source_dir) else {
            continue;
        };
        let source = std::str::from_utf8(file.bytes()).map_err(|_| {
            CompilerBridgeErrorV1::Package(format!(
                "packaged Kotodama source `{}` is not UTF-8",
                file.path()
            ))
        })?;
        modules.push(SourceModuleUnit {
            source_name,
            source: source.to_owned(),
        });
    }
    if modules.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(
            "clean publication package has no declared Kotodama library sources".to_owned(),
        ));
    }

    let expected_abi = compute_abi_hash(SyscallPolicy::AbiV1);
    let mut dependencies = Vec::with_capacity(verification_lock.nodes.len());
    for node in &verification_lock.nodes {
        if node.abi.abi_hash != expected_abi {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "release `{}` targets a different IVM ABI V1 hash",
                node.release
            )));
        }
        dependencies.push(cached_source_package(node, source.load(node)?)?);
    }
    dependencies.sort_by(|left, right| left.identity.cmp(&right.identity));
    if dependencies
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return Err(CompilerBridgeErrorV1::Lock(
            "publication lock contains duplicate exact releases".to_owned(),
        ));
    }
    let imports = verification_lock
        .root_dependencies
        .iter()
        .map(|edge| ImportBinding {
            alias: edge.alias.to_string(),
            package: edge.selected.to_string(),
        })
        .collect();
    let root = SourcePackageUnit {
        identity: verification_lock.root.to_string(),
        modules,
        exports: library.exports.iter().map(ToString::to_string).collect(),
        imports,
    };
    let options = CompilerOptions {
        chain_discriminant,
        mode: CompilerMode::Production,
        ..CompilerOptions::default()
    };
    let driver = BuildDriver::for_current_executable(CompilerSession::new(options))
        .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
    let validated = driver
        .validate_package_project(SourcePackageGraphRequest {
            package: root,
            dependencies,
        })
        .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
    Ok(MusubiContentDigestV1::new(
        *validated.interface_fingerprint.as_ref(),
    ))
}

fn execute_with_source<S: RegistryCompilerSourceV1>(
    source: &S,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    lock: &LockfileV1,
    action: CompilerActionV1,
    release: bool,
    chain_discriminant: u16,
) -> Result<CompilerExecutionV1, CompilerBridgeErrorV1> {
    if chain_discriminant == 0 {
        return Err(CompilerBridgeErrorV1::Package(
            "account chain discriminant must be non-zero".to_owned(),
        ));
    }
    lock.validate()
        .map_err(|error| CompilerBridgeErrorV1::Lock(error.to_string()))?;
    let local_members = collect_local_members(workspace, selected).map_err(graph_error)?;
    let selected_set = selected.iter().cloned().collect::<BTreeSet<_>>();
    let local_identities = local_members
        .iter()
        .map(|member| (member.manifest_path.clone(), local_identity(member)))
        .collect::<BTreeMap<_, _>>();
    if local_identities.len() != local_members.len() {
        return Err(CompilerBridgeErrorV1::Package(
            "two local packages share one manifest path".to_owned(),
        ));
    }

    let mut local_units = BTreeMap::new();
    let mut package_interfaces = Vec::with_capacity(local_members.len());
    for member in &local_members {
        let unit = local_source_package(member, lock, &local_identities)?;
        if local_units.insert(unit.identity.clone(), unit).is_some() {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "duplicate local identity `{}`",
                local_identity(member)
            )));
        }
    }

    let expected_abi = compute_abi_hash(SyscallPolicy::AbiV1);
    let mut registry_units = BTreeMap::new();
    for node in &lock.nodes {
        if node.abi.abi_hash != expected_abi {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "release `{}` targets a different IVM ABI V1 hash",
                node.release
            )));
        }
        let cached = source.load(node)?;
        let unit = cached_source_package(node, cached)?;
        if registry_units.insert(unit.identity.clone(), unit).is_some() {
            return Err(CompilerBridgeErrorV1::Lock(format!(
                "duplicate registry release `{}`",
                node.release
            )));
        }
    }

    let all_packages = local_units
        .values()
        .chain(registry_units.values())
        .cloned()
        .collect::<Vec<_>>();
    let options = CompilerOptions {
        chain_discriminant,
        mode: CompilerMode::Production,
        ..CompilerOptions::default()
    };
    let driver = BuildDriver::for_current_executable(CompilerSession::new(options))
        .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;

    for member in &local_members {
        let identity = local_identity(member);
        let package = local_units.get(&identity).cloned().ok_or_else(|| {
            CompilerBridgeErrorV1::Package(format!("local package `{identity}` disappeared"))
        })?;
        let dependencies = all_packages
            .iter()
            .filter(|candidate| candidate.identity != identity)
            .cloned()
            .collect();
        let validated = driver
            .validate_package_project(SourcePackageGraphRequest {
                package,
                dependencies,
            })
            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
        package_interfaces.push(CompilerPackageInterfaceV1 {
            package: member.package.selector.clone(),
            digest: MusubiContentDigestV1::new(*validated.interface_fingerprint.as_ref()),
        });
    }
    package_interfaces.sort_by(|left, right| left.package.cmp(&right.package));

    let profile = if release { "release" } else { "debug" };
    let mut result = CompilerExecutionV1 {
        validated_packages: local_members.len(),
        contract_targets: 0,
        warnings: 0,
        artifacts: Vec::new(),
        package_interfaces,
    };
    for member in local_members
        .iter()
        .filter(|member| selected_set.contains(&member.package.selector))
    {
        let target_root = package_target_root(workspace, member);
        let imports = local_imports(member, lock, &local_identities)?;
        for target in &member.manifest.contracts {
            let roots = target_source_units(member, &target.path)?;
            for (ordinal, root) in roots.into_iter().enumerate() {
                result.contract_targets += 1;
                let graph = SourceLinkRequest {
                    root: root.clone(),
                    imports: imports.clone(),
                    packages: all_packages.clone(),
                };
                match action {
                    CompilerActionV1::Check => {
                        result.warnings += driver
                            .check_project(graph)
                            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?
                            .len();
                    }
                    CompilerActionV1::Build => {
                        let stem = if ordinal == 0 {
                            target.name.to_string()
                        } else {
                            format!("{}-{ordinal}", target.name)
                        };
                        let layout = PublishLayout::standard(&target_root, profile, &stem, true)
                            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
                        let outcome = driver
                            .build_project(LinkedSourceBuildRequest {
                                source_name: root.source_name.clone(),
                                graph,
                                profile: profile.to_owned(),
                                layout,
                                mode: PublishMode::Write,
                            })
                            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
                        result.artifacts.push(CompilerArtifactV1 {
                            package: member.package.selector.clone(),
                            target: stem,
                            source: root.source_name,
                            artifact: outcome.paths.artifact,
                            artifact_hash: outcome.artifact_hash.to_string(),
                            fresh: outcome.status == BuildStatus::Fresh,
                        });
                    }
                }
            }
        }
    }
    result.artifacts.sort_by(|left, right| {
        left.package
            .cmp(&right.package)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.source.cmp(&right.source))
    });
    Ok(result)
}

fn package_target_root(workspace: &Workspace, member: &WorkspaceMember) -> PathBuf {
    workspace
        .root()
        .join("target/kotodama")
        .join(member.package.selector.namespace.as_str())
        .join(member.package.selector.name.as_str())
}

fn graph_error(error: GraphErrorV1) -> CompilerBridgeErrorV1 {
    CompilerBridgeErrorV1::Workspace(error.to_string())
}

fn local_identity(member: &WorkspaceMember) -> String {
    format!(
        "local:{}@{}",
        member.package.selector, member.package.version
    )
}

fn exact_registry_identity(node: &MusubiVerificationNodeV1) -> String {
    node.release.to_string()
}

fn local_source_package(
    member: &WorkspaceMember,
    lock: &LockfileV1,
    local_identities: &BTreeMap<PathBuf, String>,
) -> Result<SourcePackageUnit, CompilerBridgeErrorV1> {
    let library = member.manifest.library.as_ref().ok_or_else(|| {
        CompilerBridgeErrorV1::Package(format!(
            "local package `{}` has no library",
            member.package.selector
        ))
    })?;
    let modules =
        discover_source_modules(&member.package_root.join(library.source_dir.to_path_buf()))
            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
    if modules.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "local package `{}` has no Kotodama library sources",
            member.package.selector
        )));
    }
    Ok(SourcePackageUnit {
        identity: local_identity(member),
        modules,
        exports: library.exports.iter().map(ToString::to_string).collect(),
        imports: local_imports(member, lock, local_identities)?,
    })
}

fn local_imports(
    member: &WorkspaceMember,
    lock: &LockfileV1,
    local_identities: &BTreeMap<PathBuf, String>,
) -> Result<Vec<ImportBinding>, CompilerBridgeErrorV1> {
    let root = lock
        .roots
        .binary_search_by(|root| root.package.cmp(&member.package.selector))
        .ok()
        .and_then(|index| lock.roots.get(index))
        .ok_or_else(|| {
            CompilerBridgeErrorV1::Lock(format!(
                "local package `{}` has no exact lock root",
                member.package.selector
            ))
        })?;
    let mut imports = Vec::new();
    for dependency in member.dependencies.values() {
        let package = match &dependency.local_manifest {
            Some(manifest) => local_identities.get(manifest).cloned().ok_or_else(|| {
                CompilerBridgeErrorV1::Package(format!(
                    "local path dependency `{}` is absent from the compiler graph",
                    dependency.alias
                ))
            })?,
            None => exact_edge_identity(root, dependency)?,
        };
        imports.push(ImportBinding {
            alias: dependency.alias.to_string(),
            package,
        });
    }
    imports.sort_by(|left, right| {
        left.alias
            .cmp(&right.alias)
            .then_with(|| left.package.cmp(&right.package))
    });
    Ok(imports)
}

fn exact_edge_identity(
    root: &crate::lockfile::LockedRootV1,
    dependency: &EffectiveDependency,
) -> Result<String, CompilerBridgeErrorV1> {
    let edge = root
        .dependencies
        .iter()
        .find(|edge| edge.alias == dependency.alias && edge.kind == MusubiDependencyKindV1::Normal)
        .ok_or_else(|| {
            CompilerBridgeErrorV1::Lock(format!(
                "normal dependency `{}` has no exact selected edge",
                dependency.alias
            ))
        })?;
    match &dependency.dependency {
        ConcreteDependency::Registry {
            package,
            requirement,
        } => {
            if edge.requirement != *requirement || edge.selected.package.name != package.name {
                return Err(CompilerBridgeErrorV1::Lock(format!(
                    "normal dependency `{}` disagrees with its exact edge",
                    dependency.alias
                )));
            }
        }
        ConcreteDependency::Path { .. } => {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "path dependency `{}` lost its local manifest",
                dependency.alias
            )));
        }
    }
    Ok(edge.selected.to_string())
}

fn cached_source_package(
    node: &MusubiVerificationNodeV1,
    cached: CachedCompilerPackageV1,
) -> Result<SourcePackageUnit, CompilerBridgeErrorV1> {
    let manifest = parse_manifest(&cached.manifest)
        .map_err(|error| CompilerBridgeErrorV1::Package(error.to_string()))?;
    if manifest.workspace.is_some() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "cached release `{}` contains workspace state",
            node.release
        )));
    }
    validate_manifest_dependency_edges(&node.release, &node.dependencies, &manifest.dependencies)?;
    let package = manifest
        .resolve_package(None)
        .map_err(|error| CompilerBridgeErrorV1::Package(error.to_string()))?;
    if package.selector.name != node.release.package.name
        || package.version != node.release.version
        || package.edition != cached.semantic_release.edition
        || package.abi_version != node.abi.abi_version
    {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "cached manifest identity for `{}` is inconsistent",
            node.release
        )));
    }
    let library = manifest.library.as_ref().ok_or_else(|| {
        CompilerBridgeErrorV1::Package(format!("cached release `{}` has no library", node.release))
    })?;
    let declared_exports = library
        .exports
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    let semantic_exports = cached
        .semantic_release
        .exports
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    if declared_exports != semantic_exports {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "cached release `{}` export table disagrees with its semantic manifest",
            node.release
        )));
    }
    let modules = cached
        .kotodama_sources
        .into_iter()
        .filter_map(|source| {
            relative_library_source(&source.path, &library.source_dir).map(|source_name| {
                SourceModuleUnit {
                    source_name,
                    source: source.source,
                }
            })
        })
        .collect::<Vec<_>>();
    if modules.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "cached release `{}` has no declared library sources",
            node.release
        )));
    }
    let imports = node
        .dependencies
        .iter()
        .map(|edge| ImportBinding {
            alias: edge.alias.to_string(),
            package: edge.selected.to_string(),
        })
        .collect();
    Ok(SourcePackageUnit {
        identity: exact_registry_identity(node),
        modules,
        exports: semantic_exports,
        imports,
    })
}

fn validate_manifest_dependency_edges(
    release: &iroha_data_model::musubi::MusubiReleaseIdV1,
    edges: &[iroha_data_model::musubi::MusubiExactDependencyEdgeV1],
    dependencies: &BTreeMap<iroha_data_model::name::Name, DependencySpec>,
) -> Result<(), CompilerBridgeErrorV1> {
    if dependencies.len() != edges.len() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "release `{}` manifest dependency count disagrees with its exact proof",
            release
        )));
    }
    for (alias, dependency) in dependencies {
        let DependencySpec::Concrete(ConcreteDependency::Registry {
            package,
            requirement,
        }) = dependency
        else {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "release `{}` retains a path or workspace dependency `{alias}`",
                release
            )));
        };
        let edge = edges
            .iter()
            .find(|edge| &edge.alias == alias)
            .ok_or_else(|| {
                CompilerBridgeErrorV1::Package(format!(
                    "release `{}` dependency `{alias}` has no exact edge",
                    release
                ))
            })?;
        if edge.kind != MusubiDependencyKindV1::Normal
            || edge.package.name != package.name
            || edge.requirement != *requirement
        {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "release `{}` dependency `{alias}` disagrees with its exact edge",
                release
            )));
        }
    }
    Ok(())
}

fn relative_library_source(path: &str, source_dir: &PortablePath) -> Option<String> {
    if source_dir.as_str() == "." {
        return path.ends_with(".ko").then(|| path.to_owned());
    }
    let prefix = format!("{}/", source_dir.as_str());
    path.strip_prefix(&prefix)
        .filter(|relative| relative.ends_with(".ko") && !relative.is_empty())
        .map(ToOwned::to_owned)
}

fn target_source_units(
    member: &WorkspaceMember,
    target: &PortablePath,
) -> Result<Vec<SourceModuleUnit>, CompilerBridgeErrorV1> {
    let path = member.package_root.join(target.to_path_buf());
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        CompilerBridgeErrorV1::Package(format!(
            "cannot inspect contract target `{}`: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "contract target `{}` is a symlink",
            path.display()
        )));
    }
    if metadata.is_file() {
        return discover_source_link_request(&path, &member.package_root, Vec::new(), Vec::new())
            .map(|request| vec![request.root])
            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()));
    }
    if metadata.is_dir() {
        let modules = discover_source_modules(&path)
            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
        if modules.is_empty() {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "contract target directory `{}` contains no `.ko` sources",
                path.display()
            )));
        }
        return Ok(modules);
    }
    Err(CompilerBridgeErrorV1::Package(format!(
        "contract target `{}` is not a regular file or directory",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use iroha_data_model::{
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiPackageIdV1, MusubiPackageScopeV1,
            MusubiRegistrySnapshotV1, MusubiReleaseIdV1, MusubiVerificationLockV1,
        },
        nexus::DataSpaceId,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::{
        lockfile::{LockedRootV1, LockfileV1},
        package::{PackageLayout, plan_package},
        workspace::load_workspace,
    };

    struct EmptyRegistry;

    impl RegistryCompilerSourceV1 for EmptyRegistry {
        fn load(
            &self,
            node: &MusubiVerificationNodeV1,
        ) -> Result<CachedCompilerPackageV1, CompilerBridgeErrorV1> {
            Err(CompilerBridgeErrorV1::Cache(format!(
                "unexpected node `{}`",
                node.release
            )))
        }
    }

    #[test]
    fn validates_dependency_free_local_library_without_registry_sources() {
        let temp = TempDir::new().expect("temporary directory");
        fs::create_dir_all(temp.path().join("src")).expect("source directory");
        fs::write(
            temp.path().join("Musubi.toml"),
            r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = ["value"]
"#,
        )
        .expect("manifest");
        fs::write(
            temp.path().join("src/lib.ko"),
            "module Demo { fn value() -> int { return 1; } }",
        )
        .expect("source");
        let workspace = load_workspace(temp.path()).expect("workspace");
        let selector: MusubiPackageSelectorV1 = "apps.sora/demo".parse().expect("selector");
        let lock = LockfileV1::new(
            "musubi-compiler-test".parse().expect("chain"),
            [1; 32],
            MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: [2; 32],
                index_revision: 1,
            },
            vec![LockedRootV1 {
                package: selector.clone(),
                dependencies: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("lock");
        let execution = execute_with_source(
            &EmptyRegistry,
            &workspace,
            &[selector.clone()],
            &lock,
            CompilerActionV1::Check,
            false,
            1,
        )
        .expect("compiler graph");
        assert_eq!(execution.validated_packages, 1);
        assert_eq!(execution.contract_targets, 0);
        assert!(execution.artifacts.is_empty());
        assert_eq!(execution.package_interfaces.len(), 1);
        assert_eq!(execution.package_interfaces[0].package, selector);
        assert!(!execution.package_interfaces[0].digest.is_zero());
    }

    #[test]
    fn library_path_filter_is_component_bounded() {
        let source_dir = PortablePath::new("src").expect("source dir");
        assert_eq!(
            relative_library_source("src/math/add.ko", &source_dir).as_deref(),
            Some("math/add.ko")
        );
        assert_eq!(relative_library_source("src2/add.ko", &source_dir), None);
        assert_eq!(relative_library_source("src/readme.txt", &source_dir), None);
    }

    #[test]
    fn build_outputs_are_partitioned_by_public_package_identity() {
        let temp = TempDir::new().expect("temporary directory");
        fs::create_dir_all(temp.path().join("src")).expect("source directory");
        fs::write(
            temp.path().join("Musubi.toml"),
            r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
"#,
        )
        .expect("manifest");
        fs::write(temp.path().join("src/lib.ko"), "module Demo {}").expect("source");
        let workspace = load_workspace(temp.path()).expect("workspace");
        let member = workspace.members().values().next().expect("member");
        assert_eq!(
            package_target_root(&workspace, member),
            workspace.root().join("target/kotodama/apps.sora/demo")
        );
    }

    #[test]
    fn clean_packaged_tree_produces_a_typed_interface_digest() {
        let temp = TempDir::new().expect("temporary directory");
        fs::create_dir_all(temp.path().join("src")).expect("source directory");
        fs::write(
            temp.path().join("src/lib.ko"),
            "module Demo { fn value() -> int { return 1; } }",
        )
        .expect("source");
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(9),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("package name"),
        );
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: MusubiReleaseIdV1::new(package, "1.0.0".parse().expect("version")),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = ["value"]
"#;
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        let plan = plan_package(&layout, manifest, &lock).expect("package plan");
        let digest = validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 1)
            .expect("clean package validation");
        assert!(!digest.is_zero());
    }
}
