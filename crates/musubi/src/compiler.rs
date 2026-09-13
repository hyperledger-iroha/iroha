//! Authenticated Musubi lock graph to canonical Kotodama compiler bridge.
//!
//! Registry sources cross this boundary only after the immutable cache has
//! re-authenticated the complete bundle against an exact lock node. Local path
//! packages and registry releases have distinct canonical compiler identities derived from
//! their structural fields. Every registry import binds to the exact consumer-lock selection.
use crate::{
    cache::{CachedCompilerPackageV1, MusubiCache},
    compiler_identity::{local_package, registry_release},
    graph::{GraphErrorV1, collect_local_members, resolve_workspace_local},
    lockfile::{LockContextV1, LockfileV1},
    manifest::{ConcreteDependency, DependencySpec, LocalTarget, PortablePath, parse_manifest},
    package::PackagePlan,
    resolver::ResolveModeV1,
    workspace::{EffectiveDependency, Workspace, WorkspaceMember},
};
use iroha_data_model::musubi::{
    MusubiContentDigestV1, MusubiDependencyKindV1, MusubiPackageSelectorV1,
    MusubiVerificationLockV1, MusubiVerificationNodeV1,
};
use ivm::{
    SyscallPolicy,
    koto_test_driver::{
        declared_test_target_source_v1, discover_declared_test_names_source_set_v1,
    },
    kotodama::{
        compiler::{CompilerMode, CompilerOptions},
        driver::{
            BuildDriver, BuildStatus, LinkedSourceBuildRequest, PublishLayout, PublishMode,
            discover_source_link_request, discover_source_modules,
        },
        linker::{
            ImportBinding, ModuleBuildGraph, SourceLinkRequest, SourceModuleUnit,
            SourcePackageGraphRequest, SourcePackageUnit,
        },
        session::CompilerSession,
    },
    syscalls::compute_abi_hash,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::PathBuf,
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
    /// Exact manifest-declared contract target name.
    pub target: String,
    /// Canonical source path used in diagnostics.
    pub source: String,
    /// Published `.to` path.
    pub artifact: PathBuf,
    /// Published compiler manifest path.
    pub manifest: PathBuf,
    /// Published public interface path.
    pub interface: PathBuf,
    /// Canonical advertised public entrypoint names.
    pub entrypoints: Vec<String>,
    /// Canonical ABI hash of the built artifact.
    pub abi_hash: String,
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
    /// Number of local packages whose declared library and contract sources were validated.
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
impl RegistryCompilerSourceV1 for Option<&MusubiCache> {
    fn load(
        &self,
        node: &MusubiVerificationNodeV1,
    ) -> Result<CachedCompilerPackageV1, CompilerBridgeErrorV1> {
        self.ok_or_else(|| {
            CompilerBridgeErrorV1::Cache(
                "an immutable registry node requires an authenticated cache".to_owned(),
            )
        })?
        .load(node)
    }
}
/// Execute one compiler operation for selected workspace packages.
///
/// Local graphs need no registry cache. Every registry node requires an authenticated cache.
pub fn execute_compiler_graph(
    cache: Option<&MusubiCache>,
    workspace: &Workspace,
    selected: &[MusubiPackageSelectorV1],
    lock: &LockfileV1,
    action: CompilerActionV1,
    chain_discriminant: u16,
) -> Result<CompilerExecutionV1, CompilerBridgeErrorV1> {
    execute_with_source(
        &cache,
        workspace,
        selected,
        lock,
        action,
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
            package: registry_release(&edge.selected),
        })
        .collect::<Vec<_>>();
    let root = manifest
        .library
        .as_ref()
        .map(|library| {
            let mut modules = Vec::new();
            for file in plan.files() {
                let Some(source_name) = relative_library_source(file.path(), &library.source_dir)
                else {
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
            Ok(SourcePackageUnit {
                identity: registry_release(&verification_lock.root),
                modules,
                exports: library.exports.iter().map(ToString::to_string).collect(),
                imports: imports.clone(),
            })
        })
        .transpose()?;
    let target_packages = root
        .iter()
        .cloned()
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
    let interface_digest = match root {
        Some(root) => {
            let validated = driver
                .validate_package_project(SourcePackageGraphRequest {
                    package: root,
                    dependencies: dependencies.clone(),
                })
                .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
            MusubiContentDigestV1::new(*validated.interface_fingerprint.as_ref())
        }
        None => contract_only_interface_digest(),
    };
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
        &manifest.contracts,
        &imports,
        &target_packages,
        chain_discriminant,
    )?;
    Ok(interface_digest)
}
/// Canonical commitment stating that a source package declares no reusable library interface.
pub(crate) fn contract_only_interface_digest() -> MusubiContentDigestV1 {
    MusubiContentDigestV1::new(
        *iroha::crypto::Hash::new(b"musubi-contract-only-interface-v1\0").as_ref(),
    )
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
        let identity = registry_release(&node.release);
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
        let root = packaged_contract_source_unit(plan, &target.path)?;
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
    Ok(())
}
fn validate_packaged_test_targets(
    plan: &PackagePlan,
    targets: &[LocalTarget],
    contract_targets: &[LocalTarget],
    imports: &[ImportBinding],
    dependencies: &[SourcePackageUnit],
    chain_discriminant: u16,
) -> Result<(), CompilerBridgeErrorV1> {
    let graph = ModuleBuildGraph::default();
    let mut contracts = BTreeMap::new();
    for target in contract_targets {
        let source = packaged_contract_source_unit(plan, &target.path)?;
        contracts.insert(source.source_name.clone(), source);
    }
    for target in targets {
        for root in packaged_test_source_units(plan, &target.path)? {
            let source_name = root.source_name.clone();
            let declared_target =
                declared_test_target_source_v1(&root).map_err(CompilerBridgeErrorV1::Package)?;
            let contract = declared_target.as_ref().map(|name| {
                contracts.get(name).ok_or_else(|| CompilerBridgeErrorV1::Package(format!(
                    "packaged test source `{source_name}` targets `{name}`, which is not a packaged manifest-declared contract"
                )))
            }).transpose()?;
            discover_declared_test_names_source_set_v1(&root, contract).map_err(|error| {
                CompilerBridgeErrorV1::Package(format!(
                    "packaged test target `{}` source `{source_name}` is not a valid V1 test source set: {error}",
                    target.name
                ))
            })?;
            let (compile_root, test_sources) = match contract {
                Some(contract) => (contract.clone(), vec![root]),
                None => (root, Vec::new()),
            };
            let compile_source_name = compile_root.source_name.clone();
            graph
                .build_test_project_with_sources(
                    SourceLinkRequest {
                        root: compile_root,
                        imports: imports.to_vec(),
                        packages: dependencies.to_vec(),
                    },
                    &test_sources,
                    CompilerOptions {
                        chain_discriminant,
                        mode: CompilerMode::Test,
                        ..CompilerOptions::default()
                    },
                    &compile_source_name,
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
fn packaged_contract_source_unit(
    plan: &PackagePlan,
    target: &PortablePath,
) -> Result<SourceModuleUnit, CompilerBridgeErrorV1> {
    let path = target.as_str();
    let file = plan.files().iter().find(|file| file.path() == path)
        .filter(|file| has_kotodama_extension(file.path()))
        .ok_or_else(|| CompilerBridgeErrorV1::Package(format!(
            "packaged contract target `{path}` must identify one exact `.ko` source file; contract directory discovery is not supported"
        )))?;
    packaged_source_unit(path, path, file.bytes(), PackagedTargetKindV1::Contract)
}
fn packaged_test_source_units(
    plan: &PackagePlan,
    target: &PortablePath,
) -> Result<Vec<SourceModuleUnit>, CompilerBridgeErrorV1> {
    let target_path = target.as_str();
    if target_path != "."
        && let Some(file) = plan.files().iter().find(|file| file.path() == target_path)
    {
        if !has_kotodama_extension(file.path()) {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "packaged test target `{target_path}` must be a `.ko` file or directory"
            )));
        }
        return packaged_source_unit(
            file.path(),
            file.path(),
            file.bytes(),
            PackagedTargetKindV1::Test,
        )
        .map(|unit| vec![unit]);
    }
    let prefix = (target_path != ".").then(|| format!("{target_path}/"));
    let mut units = Vec::new();
    // PackagePlan already bounds the entire retained source set and has bytewise-sorted paths.
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
        units.push(packaged_source_unit(
            file.path(),
            file.path(),
            file.bytes(),
            PackagedTargetKindV1::Test,
        )?);
    }
    if units.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "packaged test target directory `{target_path}` contains no `.ko` sources"
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
    chain_discriminant: u16,
) -> Result<CompilerExecutionV1, CompilerBridgeErrorV1> {
    if chain_discriminant == 0 {
        return Err(CompilerBridgeErrorV1::Package(
            "account chain discriminant must be non-zero".to_owned(),
        ));
    }
    lock.validate()
        .map_err(|error| CompilerBridgeErrorV1::Lock(error.to_string()))?;
    if matches!(&lock.context, LockContextV1::Local { .. }) {
        let validated = resolve_workspace_local(
            workspace,
            selected,
            Some(lock.clone()),
            ResolveModeV1::Locked,
        )
        .map_err(|error| CompilerBridgeErrorV1::Lock(error.to_string()))?;
        if validated.is_none() {
            return Err(CompilerBridgeErrorV1::Lock(
                "local lock cannot authorize a registry dependency".to_owned(),
            ));
        }
    }
    let local_members =
        collect_local_members(workspace, selected).map_err(|error| graph_error(&error))?;
    let selected_set = selected.iter().cloned().collect::<BTreeSet<_>>();
    let local_identities = local_members
        .iter()
        .map(|member| {
            (
                member.manifest_path.clone(),
                local_package(&member.package.selector, &member.package.version),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if local_identities.len() != local_members.len() {
        return Err(CompilerBridgeErrorV1::Package(
            "two local packages share one manifest path".to_owned(),
        ));
    }
    let mut local_units = BTreeMap::new();
    let mut package_interfaces = Vec::with_capacity(local_members.len());
    for member in &local_members {
        let Some(unit) = local_source_package(member, lock, &local_identities)? else {
            continue;
        };
        if local_units.insert(unit.identity.clone(), unit).is_some() {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "duplicate local identity `{}`",
                local_package(&member.package.selector, &member.package.version)
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
        let identity = local_package(&member.package.selector, &member.package.version);
        let Some(package) = local_units.get(&identity).cloned() else {
            continue;
        };
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
    let profile = "production";
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
            let root = contract_source_unit(member, &target.path)?;
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
                    let stem = target.name.to_string();
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
                        manifest: outcome.paths.manifest,
                        interface: outcome.paths.interface.ok_or_else(|| {
                            CompilerBridgeErrorV1::Compiler(
                                "compiler omitted the requested public interface".to_owned(),
                            )
                        })?,
                        entrypoints: outcome
                            .manifest
                            .entrypoints
                            .unwrap_or_default()
                            .into_iter()
                            .map(|entrypoint| entrypoint.name)
                            .collect(),
                        abi_hash: outcome
                            .manifest
                            .abi_hash
                            .ok_or_else(|| {
                                CompilerBridgeErrorV1::Compiler(
                                    "compiler omitted the canonical ABI hash".to_owned(),
                                )
                            })?
                            .to_string(),
                        artifact: outcome.paths.artifact,
                        artifact_hash: outcome.artifact_hash.to_string(),
                        fresh: outcome.status == BuildStatus::Fresh,
                    });
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
fn local_source_package(
    member: &WorkspaceMember,
    lock: &LockfileV1,
    local_identities: &BTreeMap<PathBuf, String>,
) -> Result<Option<SourcePackageUnit>, CompilerBridgeErrorV1> {
    let Some(library) = member.manifest.library.as_ref() else {
        return Ok(None);
    };
    let modules =
        discover_source_modules(&member.package_root.join(library.source_dir.to_path_buf()))
            .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))?;
    if modules.is_empty() {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "local package `{}` has no Kotodama library sources",
            member.package.selector
        )));
    }
    Ok(Some(SourcePackageUnit {
        identity: local_package(&member.package.selector, &member.package.version),
        modules,
        exports: library.exports.iter().map(ToString::to_string).collect(),
        imports: local_imports(member, lock, local_identities)?,
    }))
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
    Ok(registry_release(&edge.selected))
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
    let Some(library) = manifest.library.as_ref() else {
        if !cached.semantic_release.exports.is_empty()
            || node.interface_digest != contract_only_interface_digest()
        {
            return Err(CompilerBridgeErrorV1::Package(format!(
                "contract-only release `{}` has an inconsistent absent-library interface commitment",
                node.release
            )));
        }
        return Err(CompilerBridgeErrorV1::Package(format!(
            "dependency `{}` is a contract-only release and cannot be imported as a library",
            node.release
        )));
    };
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
            package: registry_release(&edge.selected),
        })
        .collect();
    Ok(SourcePackageUnit {
        identity: registry_release(&node.release),
        modules,
        exports: semantic_exports,
        imports,
    })
}
fn validate_manifest_dependency_edges(
    release: &iroha_data_model::musubi::MusubiReleaseIdV1,
    edges: &[iroha_data_model::musubi::MusubiExactDependencyEdgeV1],
    dependencies: &BTreeMap<iroha_model_base::name::Name, DependencySpec>,
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
fn contract_source_unit(
    member: &WorkspaceMember,
    target: &PortablePath,
) -> Result<SourceModuleUnit, CompilerBridgeErrorV1> {
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
    if !metadata.is_file() || !has_kotodama_extension(target.as_str()) {
        return Err(CompilerBridgeErrorV1::Package(format!(
            "contract target `{}` must identify one regular `.ko` source file; directory targets are supported only for tests",
            target.as_str()
        )));
    }
    discover_source_link_request(&path, &member.package_root, Vec::new(), Vec::new())
        .map(|request| request.root)
        .map_err(|error| CompilerBridgeErrorV1::Compiler(error.to_string()))
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{
        cache::CachedKotodamaSourceV1,
        lockfile::{LockedRootV1, LockfileV1},
        package::{PackageLayout, plan_package},
        workspace::load_workspace,
    };
    use iroha_data_model::musubi::{
        ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiExactDependencyEdgeV1,
        MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1, MusubiRegistrySnapshotV1,
        MusubiReleaseDigestV1, MusubiReleaseIdV1, MusubiReleaseMetadataV1,
        MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1, MusubiVerificationNodeV1,
    };
    use iroha_model_base::topology::DataSpaceId;
    use std::fs;
    use tempfile::TempDir;
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
            LockContextV1::Registry {
                network_id:
                    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                        .parse()
                        .expect("network id"),
                snapshot: MusubiRegistrySnapshotV1 {
                    finalized_height: 1,
                    finalized_block_hash: [2; 32],
                    index_revision: 1,
                },
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
    fn named_contract_targets_keep_one_source_and_artifact_while_tests_expand_directories() {
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
name = "a"
path = "contracts/nested/a.ko"
[[contract]]
name = "z"
path = "contracts/z.ko"
[[test]]
name = "tests"
path = "tests"
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_contract("contracts/nested/a.ko");
        layout.add_contract("contracts/z.ko");
        layout.add_test("tests");
        let plan = plan_package(&layout, manifest, &lock).expect("package plan");
        for path in ["contracts/nested/a.ko", "contracts/z.ko"] {
            let source = packaged_contract_source_unit(
                &plan,
                &PortablePath::new(path).expect("contract source path"),
            )
            .expect("exact contract root");
            assert_eq!(source.source_name, path);
        }
        for invalid in ["contracts", "contracts/nested", "contracts/missing.ko"] {
            assert!(
                packaged_contract_source_unit(
                    &plan,
                    &PortablePath::new(invalid).expect("invalid target fixture")
                )
                .is_err()
            );
        }
        let tests =
            packaged_test_source_units(&plan, &PortablePath::new("tests").expect("test directory"))
                .expect("test roots");
        assert_eq!(
            tests
                .iter()
                .map(|source| source.source_name.as_str())
                .collect::<Vec<_>>(),
            ["tests/a.ko", "tests/nested/z.ko"]
        );
        fs::write(temp.path().join("Musubi.toml"), manifest).expect("workspace manifest");
        let workspace = load_workspace(temp.path()).expect("exact-target workspace");
        let member = workspace.members().values().next().expect("package member");
        let selected = vec![member.package.selector.clone()];
        let local = resolve_workspace_local(&workspace, &selected, None, ResolveModeV1::UpdateLock)
            .expect("local resolution")
            .expect("local graph")
            .lockfile;
        let built = execute_with_source(
            &EmptyRegistry,
            &workspace,
            &selected,
            &local,
            CompilerActionV1::Build,
            1,
        )
        .expect("build exact named targets");
        assert_eq!(built.contract_targets, 2);
        assert_eq!(
            built
                .artifacts
                .iter()
                .map(|artifact| artifact.target.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        for artifact in &built.artifacts {
            assert_eq!(
                artifact.artifact.file_stem().and_then(|stem| stem.to_str()),
                Some(artifact.target.as_str())
            );
            assert!(artifact.artifact.is_file());
        }
        fs::create_dir_all(temp.path().join("contracts/directory.ko/nested"))
            .expect("directory with .ko suffix");
        fs::write(
            temp.path().join("contracts/directory.ko/nested/extra.ko"),
            "seiyaku Extra { hajimari() {} }",
        )
        .expect("nested source");
        let directory_target =
            PortablePath::new("contracts/directory.ko").expect("directory target");
        assert!(
            contract_source_unit(member, &directory_target)
                .expect_err("a .ko directory is not one contract")
                .to_string()
                .contains("one regular `.ko` source file")
        );
        let mut directory_layout = PackageLayout::new(temp.path());
        directory_layout.add_contract(directory_target.to_path_buf());
        assert!(plan_package(&directory_layout, manifest, &lock).is_err());
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
    fn packaged_standalone_tests_use_only_declared_immutable_contract_sources() {
        let temp = TempDir::new().expect("temporary directory");
        write_clean_library(temp.path());
        fs::create_dir_all(temp.path().join("contracts")).expect("contract directory");
        fs::create_dir_all(temp.path().join("tests")).expect("test directory");
        fs::write(
            temp.path().join("contracts/app.ko"),
            "seiyaku App { fn reward() -> int { return 7; } }",
        )
        .expect("contract");
        fs::write(temp.path().join("tests/unit.ko"), r#"module Tests { koto_test { target: "../contracts/app.ko" } #[test] fn correct() { test::assert(reward() == 7); } }"#).expect("standalone tests");
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
name = "app"
path = "contracts/app.ko"
[[test]]
name = "unit"
path = "tests/unit.ko"
"#;
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.set_library("src");
        layout.add_contract("contracts/app.ko");
        layout.add_test("tests/unit.ko");
        let plan = plan_package(&layout, manifest, &lock).expect("immutable package plan");
        fs::write(
            temp.path().join("contracts/app.ko"),
            "invalid replacement contract",
        )
        .expect("mutate ambient target");
        fs::write(
            temp.path().join("tests/unit.ko"),
            "invalid replacement test",
        )
        .expect("mutate ambient test");
        validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 753)
            .expect("validate exact captured standalone sources");
        let parsed = parse_manifest(manifest).expect("manifest");
        assert!(
            matches!(validate_packaged_test_targets(&plan, &parsed.tests, &[], &[], &[], 753), Err(CompilerBridgeErrorV1::Package(reason)) if reason.contains("not a packaged manifest-declared contract"))
        );
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

    #[test]
    fn contract_only_publication_validates_real_sources_and_has_no_library_interface() {
        let temp = TempDir::new().expect("contract package");
        let manifest = include_str!("../../../examples/coffee-club/Musubi.toml")
            .replace("namespace = \"demo\"", "namespace = \"apps.sora\"")
            .replace("name = \"coffee-club\"", "name = \"demo\"")
            .replace("version = \"0.1.0\"", "version = \"1.0.0\"");
        fs::create_dir(temp.path().join("contracts")).expect("contract directory");
        fs::create_dir(temp.path().join("tests")).expect("test directory");
        fs::write(temp.path().join("Musubi.toml"), &manifest).expect("manifest");
        fs::write(
            temp.path().join("contracts/coffee-club.ko"),
            include_str!("../templates/contract.ko"),
        )
        .expect("contract");
        fs::write(
            temp.path().join("tests/coffee-club.test.ko"),
            include_str!("../templates/contract.test.ko"),
        )
        .expect("public tests");
        let lock = clean_verification_lock();
        let mut layout = PackageLayout::new(temp.path());
        layout.add_contract("contracts/coffee-club.ko");
        layout.add_test("tests/coffee-club.test.ko");
        let plan = plan_package(&layout, &manifest, &lock).expect("contract-only package plan");
        let interface = validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 369)
            .expect("clean public contract validation");
        assert_eq!(interface, contract_only_interface_digest());
        assert!(!interface.is_zero());
        fs::write(
            temp.path().join("contracts/coffee-club.ko"),
            "invalid ambient source",
        )
        .expect("replace ambient source");
        assert_eq!(
            validate_packaged_with_source(&EmptyRegistry, &plan, &lock, 369)
                .expect("immutable source plan"),
            interface
        );
        let invalid = plan_package(&layout, &manifest, &lock).expect("capture changed source");
        assert!(validate_packaged_with_source(&EmptyRegistry, &invalid, &lock, 369).is_err());
    }
}
