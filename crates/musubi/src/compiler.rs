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
    koto_test_driver::discover_declared_test_names_source_v1,
    kotodama::{
        compiler::{CompilerMode, CompilerOptions},
        driver::{
            BuildDriver, BuildStatus, LinkedSourceBuildRequest, PublishLayout, PublishMode,
            discover_source_link_request, discover_source_modules,
        },
        linker::{
            ImportBinding, MAX_MODULE_GRAPH_SOURCE_BYTES, MAX_MODULE_GRAPH_SOURCES,
            ModuleBuildGraph, SourceLinkRequest, SourceModuleUnit, SourcePackageGraphRequest,
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
    manifest::{ConcreteDependency, DependencySpec, LocalTarget, PortablePath, parse_manifest},
    package::PackagePlan,
    workspace::{EffectiveDependency, Workspace, WorkspaceMember},
};

/// Compiler operation requested by the Cargo-style command surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilerActionV1 {
    /// Parse, type-check, link, and lint without writing artifacts.
    Check,
    /// Compile and atomically publish every selected local contract target.
    Build,
}

/// One generated contract artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilerArtifactV1 {
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
pub struct CompilerPackageInterfaceV1 {
    /// Public package selector used by the workspace root.
    pub package: MusubiPackageSelectorV1,
    /// Domain-separated digest of the exact exported function signatures.
    pub digest: MusubiContentDigestV1,
}

/// Successful compiler graph execution summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilerExecutionV1 {
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
pub enum CompilerBridgeErrorV1 {
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
pub fn execute_compiler_graph(
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
pub fn validate_packaged_plan(
    cache: &MusubiCache,
    plan: &PackagePlan,
    verification_lock: &MusubiVerificationLockV1,
    chain_discriminant: u16,
) -> Result<MusubiContentDigestV1, CompilerBridgeErrorV1> {
    validate_packaged_with_source(cache, plan, verification_lock, chain_discriminant)
}

#[allow(
    clippy::too_many_lines,
    reason = "clean-package validation is one fail-closed compiler boundary"
)]
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
        .collect::<Vec<_>>();
    let root = SourcePackageUnit {
        identity: verification_lock.root.to_string(),
        modules,
        exports: library.exports.iter().map(ToString::to_string).collect(),
        imports: imports.clone(),
    };
    let target_packages = std::iter::once(root.clone())
        .chain(dependencies.iter().cloned())
        .collect::<Vec<_>>();
    let options = CompilerOptions {
        chain_discriminant,
        mode: CompilerMode::Production,
        ..CompilerOptions::default()
    };
    validate_exact_registry_interfaces_v1(
        verification_lock.nodes.iter(),
        &dependencies,
        options.clone(),
    )
    .map_err(CompilerBridgeErrorV1::Cache)?;
    let driver = BuildDriver::for_current_executable(CompilerSession::new(options))
        .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
    let validated = driver
        .validate_package_project(SourcePackageGraphRequest {
            package: root,
            dependencies: dependencies.clone(),
        })
        .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
    let interface_digest = MusubiContentDigestV1::new(*validated.interface_fingerprint.as_ref());
    validate_packaged_contract_targets(
        &driver,
        plan,
        &manifest.contracts,
        &imports,
        &target_packages,
    )?;
    validate_packaged_test_targets(
        plan,
        &manifest.tests,
        &imports,
        &target_packages,
        chain_discriminant,
    )?;
    Ok(interface_digest)
}

/// Recompute every authenticated registry package interface against one exact source graph.
///
/// The caller supplies source units copied out of authenticated immutable cache entries. This
/// helper never opens their recorded source paths. Every package is selected as the local package
/// once so its semantic interface can be compared with the corresponding exact lock commitment.
#[allow(
    single_use_lifetimes,
    clippy::needless_lifetimes,
    reason = "stable Rust requires a named lifetime for references nested in impl Trait items"
)]
pub fn validate_exact_registry_interfaces_v1<'node>(
    nodes: impl IntoIterator<Item = &'node MusubiVerificationNodeV1>,
    packages: &[SourcePackageUnit],
    options: CompilerOptions,
) -> Result<(), String> {
    if options.mode != CompilerMode::Production {
        return Err("exact registry interfaces require production compiler mode".to_owned());
    }
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    if nodes.len() != packages.len() {
        return Err("authenticated registry node/package counts disagree".to_owned());
    }
    let packages_by_identity = packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.identity.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    if packages_by_identity.len() != packages.len() {
        return Err("the exact registry graph contains duplicate source identities".to_owned());
    }

    let graph = ModuleBuildGraph::default();
    let session = CompilerSession::new(options);
    for node in nodes {
        let identity = node.release.to_string();
        let package_index = packages_by_identity.get(identity.as_str()).ok_or_else(|| {
            format!(
                "release `{}` has no authenticated source package in the exact graph",
                node.release
            )
        })?;
        let dependencies = packages
            .iter()
            .enumerate()
            .filter(|(index, _)| index != package_index)
            .map(|(_, candidate)| candidate)
            .cloned()
            .collect();
        let validated = session
            .validate_package_graph(
                &graph,
                SourcePackageGraphRequest {
                    package: packages[*package_index].clone(),
                    dependencies,
                },
            )
            .map_err(|error| {
                format!(
                    "release `{}` failed exact typed-interface validation: {error}",
                    node.release
                )
            })?;
        if validated.interface_fingerprint.as_ref() != node.interface_digest.as_bytes() {
            return Err(format!(
                "release `{}` typed interface disagrees with its exact lock digest",
                node.release
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackagedTargetKindV1 {
    Contract,
    Test,
}

impl PackagedTargetKindV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::Test => "test",
        }
    }
}

fn validate_packaged_contract_targets(
    driver: &BuildDriver,
    plan: &PackagePlan,
    targets: &[LocalTarget],
    imports: &[ImportBinding],
    dependencies: &[SourcePackageUnit],
) -> Result<(), CompilerBridgeErrorV1> {
    for target in targets {
        for root in
            packaged_target_source_units(plan, &target.path, PackagedTargetKindV1::Contract)?
        {
            let source_name = root.source_name.clone();
            driver
                .check_project(SourceLinkRequest {
                    root,
                    imports: imports.to_vec(),
                    packages: dependencies.to_vec(),
                })
                .map_err(|error| {
                    CompilerBridgeErrorV1::Compiler(format!(
                        "packaged contract target `{}` source `{source_name}` failed clean validation: {error}",
                        target.name
                    ))
                })?;
        }
    }
    Ok(())
}

fn validate_packaged_test_targets(
    plan: &PackagePlan,
    targets: &[LocalTarget],
    imports: &[ImportBinding],
    dependencies: &[SourcePackageUnit],
    chain_discriminant: u16,
) -> Result<(), CompilerBridgeErrorV1> {
    let graph = ModuleBuildGraph::default();
    for target in targets {
        for root in packaged_target_source_units(plan, &target.path, PackagedTargetKindV1::Test)? {
            let source_name = root.source_name.clone();
            discover_declared_test_names_source_v1(&root).map_err(|error| {
                CompilerBridgeErrorV1::Package(format!(
                    "packaged test target `{}` source `{source_name}` is not a direct V1 test root: {error}",
                    target.name
                ))
            })?;
            graph
                .build_test_project(
                    SourceLinkRequest {
                        root,
                        imports: imports.to_vec(),
                        packages: dependencies.to_vec(),
                    },
                    CompilerOptions {
                        chain_discriminant,
                        mode: CompilerMode::Test,
                        ..CompilerOptions::default()
                    },
                    &source_name,
                )
                .map_err(|diagnostics| {
                    CompilerBridgeErrorV1::Compiler(format!(
                        "packaged test target `{}` source `{source_name}` failed clean validation against normal dependencies only; development dependencies do not propagate: {}",
                        target.name,
                        diagnostics.render_human()
                    ))
                })?;
        }
    }
    Ok(())
}

fn packaged_target_source_units(
    plan: &PackagePlan,
    target: &PortablePath,
    kind: PackagedTargetKindV1,
) -> Result<Vec<SourceModuleUnit>, CompilerBridgeErrorV1> {
    let target_path = target.as_str();
    if target_path != "."
        && let Some(file) = plan.files().iter().find(|file| file.path() == target_path)
    {
        if kind == PackagedTargetKindV1::Test && !has_kotodama_extension(file.path()) {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "packaged test target `{target_path}` must be a `.ko` file or directory"
            )));
        }
        return packaged_source_unit(file.path(), file.path(), file.bytes(), kind)
            .map(|unit| vec![unit]);
    }

    let prefix = (target_path != ".").then(|| format!("{target_path}/"));
    let mut source_bytes = 0usize;
    let mut units = Vec::new();
    for file in plan.files() {
        let relative = match prefix.as_deref() {
            Some(prefix) => match file.path().strip_prefix(prefix) {
                Some(relative) => relative,
                None => continue,
            },
            None => file.path(),
        };
        if relative.is_empty() || !has_kotodama_extension(relative) {
            continue;
        }
        source_bytes = source_bytes
            .checked_add(file.bytes().len())
            .ok_or_else(|| {
                CompilerBridgeErrorV1::Package(format!(
                    "packaged {} target `{target_path}` source byte count overflowed",
                    kind.label()
                ))
            })?;
        let source_name = match kind {
            PackagedTargetKindV1::Contract => relative,
            PackagedTargetKindV1::Test => file.path(),
        };
        units.push(packaged_source_unit(
            file.path(),
            source_name,
            file.bytes(),
            kind,
        )?);
    }
    if units.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "packaged {} target directory `{target_path}` contains no `.ko` sources",
            kind.label()
        )));
    }
    if kind == PackagedTargetKindV1::Contract
        && (units.len() > MAX_MODULE_GRAPH_SOURCES || source_bytes > MAX_MODULE_GRAPH_SOURCE_BYTES)
    {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "packaged contract target directory `{target_path}` exceeds the V1 compiler graph bound of {MAX_MODULE_GRAPH_SOURCES} sources or {MAX_MODULE_GRAPH_SOURCE_BYTES} UTF-8 bytes"
        )));
    }
    Ok(units)
}

fn packaged_source_unit(
    packaged_path: &str,
    source_name: &str,
    bytes: &[u8],
    kind: PackagedTargetKindV1,
) -> Result<SourceModuleUnit, CompilerBridgeErrorV1> {
    let source = std::str::from_utf8(bytes).map_err(|_| {
        CompilerBridgeErrorV1::Package(format!(
            "packaged {} source `{packaged_path}` is not UTF-8",
            kind.label()
        ))
    })?;
    Ok(SourceModuleUnit {
        source_name: source_name.to_owned(),
        source: source.to_owned(),
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "compiler graph authentication and execution form one deterministic workflow"
)]
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
    let local_members =
        collect_local_members(workspace, selected).map_err(|error| graph_error(&error))?;
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

fn graph_error(error: &GraphErrorV1) -> CompilerBridgeErrorV1 {
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
    if manifest.workspace.is_some() || !manifest.dev_dependencies.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "cached release `{}` contains workspace or development state",
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
            "release `{release}` manifest dependency count disagrees with its exact proof"
        )));
    }
    for (alias, dependency) in dependencies {
        let DependencySpec::Concrete(ConcreteDependency::Registry {
            package,
            requirement,
        }) = dependency
        else {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "release `{release}` retains a path or workspace dependency `{alias}`"
            )));
        };
        let edge = edges
            .iter()
            .find(|edge| &edge.alias == alias)
            .ok_or_else(|| {
                CompilerBridgeErrorV1::Package(format!(
                    "release `{release}` dependency `{alias}` has no exact edge"
                ))
            })?;
        if edge.kind != MusubiDependencyKindV1::Normal
            || edge.package.name != package.name
            || edge.requirement != *requirement
        {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "release `{release}` dependency `{alias}` disagrees with its exact edge"
            )));
        }
    }
    Ok(())
}

fn relative_library_source(path: &str, source_dir: &PortablePath) -> Option<String> {
    if source_dir.as_str() == "." {
        return has_kotodama_extension(path).then(|| path.to_owned());
    }
    let prefix = format!("{}/", source_dir.as_str());
    path.strip_prefix(&prefix)
        .filter(|relative| has_kotodama_extension(relative) && !relative.is_empty())
        .map(ToOwned::to_owned)
}

fn has_kotodama_extension(path: &str) -> bool {
    path.strip_suffix(".ko").is_some()
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
            ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiExactDependencyEdgeV1,
            MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1,
            MusubiRegistrySnapshotV1, MusubiReleaseDigestV1, MusubiReleaseIdV1,
            MusubiReleaseMetadataV1, MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1,
            MusubiVerificationNodeV1,
        },
        nexus::DataSpaceId,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::{
        cache::CachedKotodamaSourceV1,
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

    struct FixedRegistry {
        package: CachedCompilerPackageV1,
    }

    impl RegistryCompilerSourceV1 for FixedRegistry {
        fn load(
            &self,
            node: &MusubiVerificationNodeV1,
        ) -> Result<CachedCompilerPackageV1, CompilerBridgeErrorV1> {
            if node.release != self.package.semantic_release.release {
                return Err(CompilerBridgeErrorV1::Cache(format!(
                    "unexpected node `{}`",
                    node.release
                )));
            }
            Ok(self.package.clone())
        }
    }

    fn clean_verification_lock() -> MusubiVerificationLockV1 {
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(9),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("package name"),
        );
        MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: MusubiReleaseIdV1::new(package, "1.0.0".parse().expect("version")),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        }
    }

    fn write_clean_library(root: &std::path::Path) {
        fs::create_dir_all(root.join("src")).expect("source directory");
        fs::write(root.join("src/lib.ko"), "module Demo {}").expect("library source");
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
            std::slice::from_ref(&selector),
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
    fn kotodama_extension_is_an_exact_case_sensitive_portable_suffix() {
        assert!(has_kotodama_extension(".ko"));
        assert!(has_kotodama_extension("src/.ko"));
        assert!(has_kotodama_extension("src/main.ko"));
        assert!(!has_kotodama_extension("src/main.KO"));
        assert!(!has_kotodama_extension("src/main.ko.bak"));
        assert!(!has_kotodama_extension("src/mainko"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the fixture verifies one complete authenticated dependency-interface workflow"
    )]
    fn clean_publication_recomputes_each_locked_dependency_interface() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        let root_package = MusubiPackageIdV1::new(
            DataSpaceId::new(9),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("root package name"),
        );
        let dependency_package = MusubiPackageIdV1::new(
            DataSpaceId::new(10),
            MusubiPackageScopeV1::DataspaceRoot,
            "dep".parse().expect("dependency package name"),
        );
        let dependency_release = MusubiReleaseIdV1::new(
            dependency_package.clone(),
            "1.0.0".parse().expect("dependency version"),
        );
        let dependency_requirement = "^1.0.0".parse().expect("dependency requirement");
        let dependency_edge = MusubiExactDependencyEdgeV1 {
            alias: "dep".parse().expect("dependency alias"),
            kind: MusubiDependencyKindV1::Normal,
            package: dependency_package.clone(),
            requirement: dependency_requirement,
            selected: dependency_release.clone(),
        };
        let expected_abi =
            MusubiAbiBindingV1::new(compute_abi_hash(SyscallPolicy::AbiV1)).expect("ABI binding");
        let substituted_interface = MusubiContentDigestV1::new([99; 32]);
        let dependency_node = MusubiVerificationNodeV1 {
            release: dependency_release.clone(),
            release_digest: MusubiReleaseDigestV1::new([11; 32]),
            archive_id: ArchiveId::new([12; 32]),
            source_digest: MusubiContentDigestV1::new([13; 32]),
            interface_digest: substituted_interface,
            abi: expected_abi,
            dependencies: Vec::new(),
        };
        let verification_lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: MusubiReleaseIdV1::new(root_package, "1.0.0".parse().expect("root version")),
            root_dependencies: vec![dependency_edge],
            nodes: vec![dependency_node],
        };
        let dependency_publication_lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: dependency_release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let registry = FixedRegistry {
            package: CachedCompilerPackageV1 {
                source_path: temp.path().join("authenticated-cache-path-is-not-reopened"),
                manifest: r#"manifest-version = 1
[package]
namespace = "deps.sora"
name = "dep"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = ["value"]
"#
                .to_owned(),
                kotodama_sources: vec![CachedKotodamaSourceV1 {
                    path: "src/lib.ko".to_owned(),
                    source: "module Dep { fn value() -> int { return 1; } }".to_owned(),
                }],
                semantic_release: MusubiSemanticReleaseManifestV1 {
                    release: dependency_release,
                    edition: MusubiKotodamaEditionV1::V1,
                    abi: expected_abi,
                    dependencies: Vec::new(),
                    exports: vec!["value".parse().expect("export name")],
                    interface_digest: substituted_interface,
                    metadata: MusubiReleaseMetadataV1::default(),
                    verification_lock_digest: dependency_publication_lock.digest(),
                },
                publication_lock: dependency_publication_lock,
            },
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
exports = []
[dependencies]
dep = { package = "deps.sora/dep", version = "^1.0.0" }
"#;
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        let plan = plan_package(&layout, manifest, &verification_lock).expect("package plan");

        assert!(matches!(
            validate_packaged_with_source(&registry, &plan, &verification_lock, 1),
            Err(CompilerBridgeErrorV1::Cache(reason))
                if reason.contains("typed interface disagrees with its exact lock digest")
        ));
    }

    #[test]
    fn packaged_directory_targets_expand_deterministically_from_plan_paths() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        for (path, source) in [
            ("contracts/z.ko", "seiyaku Z { hajimari() {} }"),
            ("contracts/nested/a.ko", "seiyaku A { hajimari() {} }"),
            ("contracts/readme.txt", "ignored"),
            ("tests/nested/z.ko", "seiyaku ZTests { #[test] fn z() {} }"),
            ("tests/a.ko", "seiyaku ATests { #[test] fn a() {} }"),
            ("tests/readme.txt", "ignored"),
        ] {
            let path = temp.path().join(path);
            fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
            fs::write(path, source).expect("fixture source");
        }
        let manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
[[contract]]
name = "contracts"
path = "contracts"
[[test]]
name = "tests"
path = "tests"
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_contract("contracts");
        layout.add_test("tests");
        let plan = plan_package(&layout, manifest, &lock).expect("package plan");

        let contracts = packaged_target_source_units(
            &plan,
            &PortablePath::new("contracts").expect("contract target"),
            PackagedTargetKindV1::Contract,
        )
        .expect("contract roots");
        assert_eq!(
            contracts
                .iter()
                .map(|source| source.source_name.as_str())
                .collect::<Vec<_>>(),
            ["nested/a.ko", "z.ko"]
        );
        let tests = packaged_target_source_units(
            &plan,
            &PortablePath::new("tests").expect("test target"),
            PackagedTargetKindV1::Test,
        )
        .expect("test roots");
        assert_eq!(
            tests
                .iter()
                .map(|source| source.source_name.as_str())
                .collect::<Vec<_>>(),
            ["tests/a.ko", "tests/nested/z.ko"]
        );
    }

    #[test]
    fn invalid_packaged_contract_is_not_reopened_from_a_repaired_workspace() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        fs::create_dir_all(temp.path().join("contracts")).expect("contract directory");
        let contract = temp.path().join("contracts/deploy.ko");
        fs::write(&contract, "seiyaku Broken { fn").expect("invalid packaged contract");
        let manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
[[contract]]
name = "deploy"
path = "contracts/deploy.ko"
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_contract("contracts/deploy.ko");
        let plan = plan_package(&layout, manifest, &lock).expect("snapshot invalid contract");
        fs::write(&contract, "seiyaku Repaired { hajimari() {} }")
            .expect("repair ambient contract");

        assert!(matches!(
            validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 1),
            Err(CompilerBridgeErrorV1::Compiler(reason))
                if reason.contains("packaged contract target `deploy`")
        ));
    }

    #[test]
    fn invalid_packaged_test_bytes_are_not_reopened_from_a_repaired_workspace() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        fs::create_dir_all(temp.path().join("tests")).expect("test directory");
        let test = temp.path().join("tests/unit.ko");
        fs::write(&test, [0xff, 0xfe]).expect("invalid packaged test bytes");
        let manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
[[test]]
name = "unit"
path = "tests/unit.ko"
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_test("tests/unit.ko");
        let plan = plan_package(&layout, manifest, &lock).expect("snapshot invalid test");
        fs::write(
            &test,
            "seiyaku Repaired { #[test] fn repaired() { test::assert(true); } }",
        )
        .expect("repair ambient test");

        assert!(matches!(
            validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 1),
            Err(CompilerBridgeErrorV1::Package(reason))
                if reason.contains("packaged test source `tests/unit.ko` is not UTF-8")
        ));
    }

    #[test]
    fn clean_targets_ignore_ambient_mutation_and_do_not_change_library_interface() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        for (path, source) in [
            ("contracts/deploy.ko", "seiyaku Deploy { hajimari() {} }"),
            (
                "tests/unit.ko",
                "seiyaku Tests { #[test] fn compile_only() { test::assert(false); } }",
            ),
            ("ambient/undeclared.ko", "this is not Kotodama"),
        ] {
            let path = temp.path().join(path);
            fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
            fs::write(path, source).expect("fixture source");
        }
        let manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
[[contract]]
name = "deploy"
path = "contracts/deploy.ko"
[[test]]
name = "unit"
path = "tests/unit.ko"
"#;
        let library_only_manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_contract("contracts/deploy.ko");
        layout.add_test("tests/unit.ko");
        let plan = plan_package(&layout, manifest, &lock).expect("package target snapshot");
        assert!(
            !plan
                .files()
                .iter()
                .any(|file| file.path() == "ambient/undeclared.ko")
        );
        fs::write(
            temp.path().join("contracts/deploy.ko"),
            "invalid ambient contract",
        )
        .expect("mutate ambient contract");
        fs::write(temp.path().join("tests/unit.ko"), "invalid ambient test")
            .expect("mutate ambient test");

        let with_targets = validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 1)
            .expect("validate immutable target snapshot");
        let mut library_layout = PackageLayout::new(temp.path());
        library_layout.set_library("src");
        let library_plan = plan_package(&library_layout, library_only_manifest, &lock)
            .expect("library-only package plan");
        let library_only = validate_packaged_with_source(&EmptyRegistry, &library_plan, &lock, 1)
            .expect("validate library-only package");
        assert_eq!(with_targets, library_only);
    }

    #[test]
    fn packaged_test_missing_a_normal_dependency_has_a_dev_boundary_diagnostic() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        fs::create_dir_all(temp.path().join("tests")).expect("test directory");
        fs::write(
            temp.path().join("tests/unit.ko"),
            "seiyaku Tests { #[test] fn needs_dev() { test::assert(helper::truth()); } }",
        )
        .expect("test source");
        let manifest = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "demo"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
source-dir = "src"
exports = []
[[test]]
name = "unit"
path = "tests/unit.ko"
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_test("tests/unit.ko");
        let plan = plan_package(&layout, manifest, &lock).expect("package plan");

        assert!(matches!(
            validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 1),
            Err(CompilerBridgeErrorV1::Compiler(reason))
                if reason.contains("development dependencies do not propagate")
        ));
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
