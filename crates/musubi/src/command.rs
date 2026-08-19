//! Cargo-style Musubi V1 command parsing and exact-network authenticated workflows.
//!
//! This module owns the public command grammar and returns logical output. Purely local commands
//! never construct a signer; network reads and mutations load one only at their explicit registry
//! boundary. Resolution, authenticated fetch,
//! compiler, test, cache, and publication work stays in dedicated V1 modules.
use crate::{
    archive_fetch::{
        ArchiveFetchErrorV1, ArchiveFetchFailureClassV1, ArchiveTransportErrorV1,
        MusubiArchiveFetchAdapterV1, PreparedProductionSorafsArchiveTransportV1,
        ProductionSorafsArchiveTransportV1, build_production_archive_transport_v1,
        prepare_production_archive_transport_v1,
    },
    atomic_io::{AtomicWriteErrorCode, AtomicWriteRoot},
    cache::{CacheError, InstallOutcome, MusubiCache, RepairOutcome, platform_cache_root_v1},
    compiler::{
        CompilerActionV1, CompilerBridgeErrorV1, execute_compiler_graph, validate_packaged_plan,
    },
    graph::{
        GraphErrorV1, GraphUpdateV1, resolve_workspace_offline_cached,
        resolve_workspace_online_cached, resolve_workspace_online_cached_fresh,
    },
    local_file::read_bounded_single_link_regular_file_v1,
    lockfile::{LockfileError, LockfileV1},
    manifest::{
        ConcreteDependency, DependencyPath, DependencySection, DependencySpec, MANIFEST_FILE_NAME,
        Manifest, PortablePath, parse_manifest, remove_dependency, upsert_dependency,
    },
    output::{CommandOutput, Diagnostic, ErrorCode, OutputFormat},
    package::{
        PackageError, package_layout_for_member, plan_package, publication_claim,
        publication_manifest_toml, semantic_release_manifest,
    },
    publication_runtime::{
        load_bound_production_publication_runtime_v1, load_production_publication_runtime_v1,
    },
    publish::{
        PublicationAdvanceV1, PublicationBackendError, PublicationEngine, PublicationError,
        PublicationJournalStore, PublicationOperationIdV1, PublicationRequestV1,
        PublicationResultV1, PublicationStagedCarSourceV1, PublicationValidationEvidenceV1,
    },
    registry::{
        PlatformConfigProvenanceV1, PublicationPollPolicyV1, RegistryErrorV1,
        RegistryPublicationBackendV1, RegistryReadClientV1, RegistrySigningClientV1,
        resume_with_bounded_polling,
    },
    registry_cache::{CachedResolverSourceV1, ResolverIndexCacheV1},
    resolver::{ConflictReasonV1, ResolveModeV1, ResolverError},
    test_runner::{WorkspaceTestErrorV1, WorkspaceTestOptionsV1, execute_workspace_tests_v1},
    workspace::{
        DependencyKind, EffectiveDependency, MAX_MANIFEST_BYTES, Workspace, WorkspaceErrorKind,
        WorkspaceMember, discover_manifest, load_workspace,
    },
};
use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use iroha_data_model::{
    isi::musubi::{
        AcceptMusubiPackageMaintainerV1, InviteMusubiPackageMaintainerV1, RegisterMusubiAliasV1,
        RemoveMusubiPackageMaintainerV1, RevokeMusubiPackageMaintainerInvitationV1,
        SetMusubiPackageMaintainerRoleV1, SetMusubiReleaseYankV1,
    },
    musubi::{
        MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1, MUSUBI_MAX_PACKAGE_MEMBERS_V1,
        MUSUBI_MAX_PAGE_SIZE_V1, MUSUBI_MAX_PENDING_INVITATIONS_V1, MusubiAliasNameV1,
        MusubiAliasQueryV1, MusubiArchiveRetentionDispositionV1, MusubiArchiveRetentionQueryV1,
        MusubiContentDigestV1, MusubiDependencyKindV1, MusubiExactDependencyEdgeV1,
        MusubiInviteIdV1, MusubiMaintainerDirectoryEntryV1, MusubiMaintainerPermissionsV1,
        MusubiNamespaceV1, MusubiPackageNameV1, MusubiPackagePageQueryV1, MusubiPackageRoleV1,
        MusubiPackageSelectorV1, MusubiPageRequestV1, MusubiReasonV1, MusubiRegistrySnapshotV1,
        MusubiReleaseIdV1, MusubiSearchPageRequestV1, MusubiSearchQueryV1,
        MusubiStorageAvailabilityV1, MusubiVersionReqV1, MusubiVersionV1,
    },
    name::Name,
};
use norito::json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fmt::Write as _,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    str::FromStr,
};
const LOCK_FILE_NAME: &str = "Musubi.lock";
/// Parsed presentation mode and logical command result.
pub struct Invocation {
    pub format: OutputFormat,
    pub output: CommandOutput,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum OutputArg {
    #[default]
    Human,
    Json,
}
impl From<OutputArg> for OutputFormat {
    fn from(value: OutputArg) -> Self {
        match value {
            OutputArg::Human => Self::Human,
            OutputArg::Json => Self::Json,
        }
    }
}
#[derive(Parser, Debug)]
#[command(
    name = "musubi",
    version = env!("CARGO_PKG_VERSION"),
    about = "Cargo-style Kotodama package manager",
    disable_help_subcommand = true
)]
struct Cli {
    /// Render human text or one versioned JSON document.
    #[arg(long, global = true, value_enum, default_value_t)]
    format: OutputArg,
    /// Use this manifest instead of ancestor discovery.
    #[arg(long, global = true, value_name = "PATH")]
    manifest_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new package directory.
    New(NewArgs),
    /// Initialize an existing directory as a package.
    Init(InitArgs),
    /// Add a registry, path, development, or inherited dependency.
    Add(AddArgs),
    /// Remove a dependency by parent-local alias.
    Remove(RemoveArgs),
    /// Print deterministic workspace/package metadata.
    Metadata(MetadataArgs),
    /// Print the selected local dependency tree.
    Tree(TreeArgs),
    /// Resolve and fetch missing archives.
    Fetch(FetchArgs),
    /// Resolve and compiler-check selected packages.
    Check(BuildArgs),
    /// Resolve and build selected packages.
    Build(BuildArgs),
    /// Resolve and test selected packages.
    Test(BuildArgs),
    /// Build a clean canonical source package.
    Package(PackageArgs),
    /// Run the resumable publication workflow.
    Publish(PublishArgs),
    /// Search the finalized package directory.
    Search(SearchArgs),
    /// Inspect one canonical package.
    Info(PackageQueryArgs),
    /// List finalized versions of one package.
    Versions(PackageQueryArgs),
    /// Yank one exact release.
    Yank(ReleaseMutationArgs),
    /// Reverse a yank on one exact release.
    Unyank(ReleaseMutationArgs),
    /// Manage package owners and maintainers.
    Owner(OwnerArgs),
    /// Register or inspect permanent global aliases.
    Alias(AliasArgs),
    /// Perform a targeted dependency graph update.
    Update(UpdateArgs),
    /// Verify, repair, or prune the immutable source cache.
    Cache(CacheArgs),
}
impl Command {
    const fn name(&self) -> &'static str {
        match self {
            Self::New(_) => "new",
            Self::Init(_) => "init",
            Self::Add(_) => "add",
            Self::Remove(_) => "remove",
            Self::Metadata(_) => "metadata",
            Self::Tree(_) => "tree",
            Self::Fetch(_) => "fetch",
            Self::Check(_) => "check",
            Self::Build(_) => "build",
            Self::Test(_) => "test",
            Self::Package(_) => "package",
            Self::Publish(_) => "publish",
            Self::Search(_) => "search",
            Self::Info(_) => "info",
            Self::Versions(_) => "versions",
            Self::Yank(_) => "yank",
            Self::Unyank(_) => "unyank",
            Self::Owner(_) => "owner",
            Self::Alias(_) => "alias",
            Self::Update(_) => "update",
            Self::Cache(_) => "cache",
        }
    }
}
#[derive(Args, Clone, Debug)]
struct PackageTemplateArgs {
    /// Canonical public namespace.
    #[arg(long)]
    namespace: MusubiNamespaceV1,
    /// Override the package name inferred from the directory.
    #[arg(long)]
    name: Option<MusubiPackageNameV1>,
    /// Initial exact version.
    #[arg(long, default_value = "0.1.0")]
    version: MusubiVersionV1,
    /// Library source directory relative to the package root.
    #[arg(long, default_value = "src")]
    source_dir: PortablePath,
    /// Explicit exported Kotodama interface name; repeat as needed.
    #[arg(long = "export", value_name = "NAME")]
    exports: Vec<Name>,
    /// Bounded package description.
    #[arg(long)]
    description: Option<String>,
    /// Readme file relative to the package root.
    #[arg(long)]
    readme: Option<PortablePath>,
    /// SPDX-like license metadata.
    #[arg(long)]
    license: Option<String>,
    /// License file relative to the package root.
    #[arg(long)]
    license_file: Option<PortablePath>,
    /// Canonical HTTP(S) repository URL.
    #[arg(long)]
    repository: Option<String>,
    /// Canonical lowercase keyword; repeat as needed.
    #[arg(long = "keyword")]
    keywords: Vec<String>,
    /// Positive package include addition; repeat as needed.
    #[arg(long = "include", value_name = "PATH")]
    includes: Vec<PortablePath>,
}
#[derive(Args, Debug)]
struct NewArgs {
    /// New package directory. Its parent must already exist.
    #[arg(value_name = "PATH")]
    path: PathBuf,
    #[command(flatten)]
    package: PackageTemplateArgs,
}
#[derive(Args, Debug)]
struct InitArgs {
    /// Existing directory to initialize.
    #[arg(default_value = ".", value_name = "PATH")]
    path: PathBuf,
    #[command(flatten)]
    package: PackageTemplateArgs,
    /// Replace an existing regular `Musubi.toml` atomically.
    #[arg(long)]
    force: bool,
}
#[allow(clippy::struct_excessive_bools)]
#[derive(Args, Debug)]
struct AddArgs {
    /// Canonical package, or an alias for local-only path/workspace inheritance.
    #[arg(value_name = "PACKAGE_OR_ALIAS")]
    package: String,
    /// Registry version requirement.
    #[arg(long)]
    version: Option<MusubiVersionReqV1>,
    /// Parent-local import alias (renamed dependency).
    #[arg(long, value_name = "ALIAS")]
    rename: Option<Name>,
    /// Local package directory relative to the defining package/root.
    #[arg(long)]
    path: Option<DependencyPath>,
    /// Add to `[dev-dependencies]`.
    #[arg(long, conflicts_with = "workspace_dependency")]
    dev: bool,
    /// Add a concrete entry to `[workspace.dependencies]` at the owning root.
    #[arg(long = "workspace-dependency", conflicts_with = "dev")]
    workspace_dependency: bool,
    /// Inherit this alias using exactly `{ workspace = true }`.
    #[arg(long, conflicts_with = "workspace_dependency")]
    workspace: bool,
    /// Replace an existing entry with the same alias.
    #[arg(long)]
    replace: bool,
}
#[derive(Args, Debug)]
struct RemoveArgs {
    /// Parent-local dependency alias.
    alias: Name,
    /// Remove from `[dev-dependencies]`.
    #[arg(long, conflicts_with = "workspace_dependency")]
    dev: bool,
    /// Remove from `[workspace.dependencies]` at the owning root.
    #[arg(long = "workspace-dependency", conflicts_with = "dev")]
    workspace_dependency: bool,
}
#[derive(Args, Clone, Debug, Default)]
struct SelectionArgs {
    /// Select every active workspace member.
    #[arg(long)]
    workspace: bool,
    /// Select a canonical package; repeat as needed.
    #[arg(short = 'p', long = "package", value_name = "PACKAGE")]
    packages: Vec<MusubiPackageSelectorV1>,
    /// Exclude a canonical package from `--workspace`.
    #[arg(long, value_name = "PACKAGE", requires = "workspace")]
    exclude: Vec<MusubiPackageSelectorV1>,
}
#[derive(Args, Clone, Copy, Debug, Default)]
struct GraphModeArgs {
    /// Fail instead of changing `Musubi.lock`.
    #[arg(long)]
    locked: bool,
    /// Use only cached index and archive data.
    #[arg(long)]
    offline: bool,
    /// Combine `--locked` and `--offline`.
    #[arg(long)]
    frozen: bool,
}
impl GraphModeArgs {
    const fn effective_locked(self) -> bool {
        self.locked || self.frozen
    }
    const fn effective_offline(self) -> bool {
        self.offline || self.frozen
    }
}
#[derive(Args, Debug)]
struct MetadataArgs {
    #[command(flatten)]
    selection: SelectionArgs,
}
#[derive(Args, Debug)]
struct TreeArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Omit selected-root development dependencies.
    #[arg(long)]
    no_dev: bool,
}
#[derive(Args, Debug)]
struct FetchArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[command(flatten)]
    mode: GraphModeArgs,
    #[command(flatten)]
    registry: RegistryReadArgs,
}
#[derive(Args, Debug)]
struct BuildArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[command(flatten)]
    mode: GraphModeArgs,
    #[command(flatten)]
    registry: RegistryReadArgs,
    /// Select release compiler settings.
    #[arg(long)]
    release: bool,
}
#[derive(Args, Debug)]
struct PackageArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[command(flatten)]
    mode: GraphModeArgs,
    #[command(flatten)]
    registry: RegistryReadArgs,
    /// List the positive package file set without writing a CAR.
    #[arg(long)]
    list: bool,
}
#[derive(Args, Clone, Debug, Default)]
struct RegistryReadArgs {
    /// Explicit platform Iroha client configuration path for authenticated registry reads.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
}
#[derive(Args, Clone, Debug, Default)]
struct NetworkArgs {
    /// Explicit platform Iroha client configuration path.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
}
#[derive(Args, Debug)]
struct PublishArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[command(flatten)]
    mode: GraphModeArgs,
    #[command(flatten)]
    network: NetworkArgs,
    /// Return after persisting a resumable operation journal.
    #[arg(long, conflicts_with_all = ["resume", "recover"])]
    detach: bool,
    /// Resume one secret-free publication journal by its canonical operation id.
    #[arg(
        long,
        value_name = "OPERATION_ID",
        conflicts_with_all = ["detach", "recover"]
    )]
    resume: Option<PublicationOperationIdV1>,
    /// Rebuild missing immutable sidecars for one pristine pre-ingress journal.
    #[arg(
        long,
        value_name = "OPERATION_ID",
        conflicts_with_all = ["detach", "resume"]
    )]
    recover: Option<PublicationOperationIdV1>,
}
#[derive(Args, Debug)]
struct SearchArgs {
    /// Search text for the finalized event projection.
    query: String,
    /// Maximum results in this page.
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=100))]
    limit: u32,
    #[command(flatten)]
    network: NetworkArgs,
}
#[derive(Args, Debug)]
struct PackageQueryArgs {
    /// Canonical namespaced package.
    package: MusubiPackageSelectorV1,
    #[command(flatten)]
    network: NetworkArgs,
}
#[derive(Args, Debug)]
struct ReleaseMutationArgs {
    /// Canonical namespaced package.
    package: MusubiPackageSelectorV1,
    /// Exact release version.
    version: MusubiVersionV1,
    /// Expected release yank-state revision.
    #[arg(long)]
    expected_revision: u64,
    /// Public bounded audit reason. A stable command-specific reason is used when omitted.
    #[arg(long)]
    reason: Option<MusubiReasonV1>,
    #[command(flatten)]
    network: NetworkArgs,
}
#[derive(Args, Debug)]
struct OwnerArgs {
    #[command(subcommand)]
    command: OwnerCommand,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum RoleArg {
    Owner,
    Maintainer,
}
#[allow(
    clippy::struct_excessive_bools,
    reason = "the four independent permission switches are the stable CLI shape"
)]
#[derive(Args, Clone, Copy, Debug, Default)]
struct MaintainerPermissionArgs {
    /// Allow publishing immutable releases.
    #[arg(long)]
    publish: bool,
    /// Allow yanking and unyanking releases.
    #[arg(long)]
    yank: bool,
    /// Allow changing mutable package metadata.
    #[arg(long)]
    metadata: bool,
    /// Allow adding, renewing, and retiring archive locations.
    #[arg(long)]
    archive_locations: bool,
}
#[derive(Subcommand, Debug)]
enum OwnerCommand {
    /// Invite an account to a package role.
    Invite {
        package: MusubiPackageSelectorV1,
        account: String,
        #[arg(long, value_enum)]
        role: RoleArg,
        /// Explicit stable invitation identifier (64 lowercase hex digits).
        #[arg(long, value_name = "INVITATION_ID")]
        invitation: String,
        /// Final block height at which the invitation may be accepted.
        #[arg(long)]
        expires_at_height: u64,
        #[command(flatten)]
        permissions: MaintainerPermissionArgs,
        #[arg(long)]
        expected_revision: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Accept a pending package invitation.
    Accept {
        package: MusubiPackageSelectorV1,
        invitation: String,
        #[arg(long)]
        expected_revision: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// List accepted owners/maintainers and pending invitations.
    List {
        package: MusubiPackageSelectorV1,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Change an accepted member role.
    SetRole {
        package: MusubiPackageSelectorV1,
        account: String,
        #[arg(long, value_enum)]
        role: RoleArg,
        #[command(flatten)]
        permissions: MaintainerPermissionArgs,
        #[arg(long)]
        expected_revision: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Remove an accepted member or revoke a pending invitation.
    Remove {
        package: MusubiPackageSelectorV1,
        /// Accepted member account to remove.
        #[arg(required_unless_present = "invitation", conflicts_with = "invitation")]
        account: Option<String>,
        /// Pending invitation to revoke instead of removing an accepted member.
        #[arg(
            long,
            value_name = "INVITATION_ID",
            required_unless_present = "account",
            conflicts_with = "account"
        )]
        invitation: Option<String>,
        #[arg(long)]
        expected_revision: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
}
#[derive(Args, Debug)]
struct AliasArgs {
    #[command(subcommand)]
    command: AliasCommand,
}
#[derive(Subcommand, Debug)]
enum AliasCommand {
    /// Buy and permanently register a global alias.
    Register {
        alias: MusubiAliasNameV1,
        package: MusubiPackageSelectorV1,
        #[arg(long)]
        expected_price_revision: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Resolve a global alias to its current structural target.
    Resolve {
        alias: MusubiAliasNameV1,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Inspect one alias and pricing/history metadata.
    Info {
        alias: MusubiAliasNameV1,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// List immutable retarget history.
    History {
        alias: MusubiAliasNameV1,
        #[command(flatten)]
        network: NetworkArgs,
    },
}
#[derive(Args, Debug)]
struct UpdateArgs {
    /// Unlock only this package, optionally at one currently locked version.
    #[arg(short = 'p', value_name = "PACKAGE[@VERSION]")]
    package: Option<UpdateTarget>,
    /// Add an exact version constraint to the targeted update.
    #[arg(long, requires = "package")]
    precise: Option<MusubiVersionV1>,
    #[command(flatten)]
    mode: GraphModeArgs,
    #[command(flatten)]
    registry: RegistryReadArgs,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct UpdateTarget {
    package: MusubiPackageSelectorV1,
    locked_version: Option<MusubiVersionV1>,
}
impl FromStr for UpdateTarget {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let (package, locked_version) = match raw.rsplit_once('@') {
            Some((package, version)) => {
                if package.is_empty() || version.is_empty() || package.contains('@') {
                    return Err("target must be `namespace/package[@VERSION]`".to_owned());
                }
                let version = version
                    .parse()
                    .map_err(|error| format!("invalid targeted locked version: {error}"))?;
                (package, Some(version))
            }
            None => (raw, None),
        };
        Ok(Self {
            package: package
                .parse()
                .map_err(|error| format!("invalid targeted package: {error}"))?,
            locked_version,
        })
    }
}
#[derive(Args, Debug)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheCommand,
}
#[derive(Subcommand, Debug)]
enum CacheCommand {
    /// Verify immutable archive commitments and extracted trees.
    Verify {
        /// Verify every trusted descendant.
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        registry: RegistryReadArgs,
    },
    /// Quarantine corrupt trusted descendants and refetch when allowed.
    Repair {
        #[command(flatten)]
        mode: GraphModeArgs,
        #[command(flatten)]
        registry: RegistryReadArgs,
    },
    /// Classify exact descendants; non-empty live prune remains fail-closed before mutation.
    Prune {
        /// Print finalized candidates without attempting live mutation.
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        registry: RegistryReadArgs,
    },
}
struct Success {
    message: String,
    data: Value,
}
type CommandResult = Result<Success, Diagnostic>;
/// Parse and execute an argv sequence without writing process streams.
pub fn invoke<I, T>(args: I) -> Invocation
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let raw_arguments = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let detected_format = detect_output_format(&raw_arguments);
    match Cli::try_parse_from(&raw_arguments) {
        Ok(cli) => {
            let format = cli.format.into();
            let command_name = cli.command.name();
            let result = dispatch(cli.manifest_path.as_deref(), &cli.command);
            let output = match result {
                Ok(success) => CommandOutput::success(command_name, success.message, success.data),
                Err(diagnostic) => CommandOutput::failure(command_name, diagnostic),
            };
            Invocation { format, output }
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            Invocation {
                format: detected_format,
                output: CommandOutput::success("help", error.to_string(), Value::Null),
            }
        }
        Err(error) => Invocation {
            format: detected_format,
            output: CommandOutput::failure(
                "cli",
                Diagnostic::new(ErrorCode::Usage, error.to_string())
                    .with_help("run `musubi --help` for the first-release command surface"),
            ),
        },
    }
}
fn detect_output_format(argv: &[OsString]) -> OutputFormat {
    for (index, argument) in argv.iter().enumerate() {
        let argument = argument.to_string_lossy();
        if argument == "--format=json"
            || (argument == "--format" && argv.get(index + 1).is_some_and(|value| value == "json"))
        {
            return OutputFormat::Json;
        }
    }
    OutputFormat::Human
}
fn dispatch(manifest_path: Option<&Path>, command: &Command) -> CommandResult {
    match command {
        Command::New(args) => run_new(args),
        Command::Init(args) => run_init(args),
        Command::Add(args) => run_add(manifest_path, args),
        Command::Remove(args) => run_remove(manifest_path, args),
        Command::Metadata(args) => run_metadata(manifest_path, args),
        Command::Tree(args) => run_tree(manifest_path, args),
        Command::Fetch(args) => run_fetch(manifest_path, args),
        Command::Check(args) => run_build(manifest_path, "check", args),
        Command::Build(args) => run_build(manifest_path, "build", args),
        Command::Test(args) => run_build(manifest_path, "test", args),
        Command::Package(args) => run_package(manifest_path, args),
        Command::Publish(args) => run_publish(manifest_path, args),
        Command::Search(args) => run_search(args),
        Command::Info(args) => run_package_info(args),
        Command::Versions(args) => run_package_versions(args),
        Command::Yank(args) => run_release_yank(args, true),
        Command::Unyank(args) => run_release_yank(args, false),
        Command::Owner(args) => run_owner(args),
        Command::Alias(args) => run_alias(args),
        Command::Update(args) => run_update(manifest_path, args),
        Command::Cache(args) => run_cache(manifest_path, args),
    }
}
fn run_new(args: &NewArgs) -> CommandResult {
    match fs::symlink_metadata(&args.path) {
        Ok(_) => {
            return Err(
                Diagnostic::new(ErrorCode::Io, "new package destination already exists")
                    .with_context("path", args.path.display().to_string()),
            );
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(io_diagnostic(
                "inspect new package destination",
                &args.path,
                &error,
            ));
        }
    }
    let name = package_name_for_root(&args.path, args.package.name.as_ref())?;
    let manifest = render_package_manifest(&args.package, &name)?;
    fs::create_dir(&args.path)
        .map_err(|error| io_diagnostic("create package directory", &args.path, &error))?;
    initialize_package_files(&args.path, &args.package.source_dir, &manifest, false)?;
    Ok(Success {
        message: format!("created {}", args.path.display()),
        data: object([
            (
                "manifest",
                Value::from(args.path.join(MANIFEST_FILE_NAME).display().to_string()),
            ),
            (
                "package",
                Value::from(format!("{}/{}", args.package.namespace, name)),
            ),
        ]),
    })
}
fn run_init(args: &InitArgs) -> CommandResult {
    let metadata = fs::symlink_metadata(&args.path)
        .map_err(|error| io_diagnostic("inspect package directory", &args.path, &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Diagnostic::new(
            ErrorCode::Io,
            "init target must be an existing non-symlink directory",
        )
        .with_context("path", args.path.display().to_string()));
    }
    let manifest_path = args.path.join(MANIFEST_FILE_NAME);
    if !args.force && fs::symlink_metadata(&manifest_path).is_ok() {
        return Err(Diagnostic::new(
            ErrorCode::ManifestInvalid,
            "package manifest already exists",
        )
        .with_context("path", manifest_path.display().to_string())
        .with_help("pass `--force` to atomically replace a regular manifest"));
    }
    let name = package_name_for_root(&args.path, args.package.name.as_ref())?;
    let manifest = render_package_manifest(&args.package, &name)?;
    initialize_package_files(&args.path, &args.package.source_dir, &manifest, true)?;
    Ok(Success {
        message: format!("initialized {}", args.path.display()),
        data: object([
            ("manifest", Value::from(manifest_path.display().to_string())),
            (
                "package",
                Value::from(format!("{}/{}", args.package.namespace, name)),
            ),
        ]),
    })
}
fn package_name_for_root(
    root: &Path,
    explicit: Option<&MusubiPackageNameV1>,
) -> Result<MusubiPackageNameV1, Diagnostic> {
    if let Some(name) = explicit {
        return Ok(name.clone());
    }
    let raw = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::Usage,
                "cannot infer a package name from this directory",
            )
            .with_context("path", root.display().to_string())
            .with_help("pass `--name LOWERCASE-KEBAB` explicitly")
        })?;
    raw.parse::<MusubiPackageNameV1>().map_err(|error| {
        Diagnostic::new(ErrorCode::Usage, error.to_string())
            .with_context("inferred_name", raw)
            .with_help("pass `--name LOWERCASE-KEBAB` explicitly")
    })
}
fn render_package_manifest(
    package: &PackageTemplateArgs,
    name: &MusubiPackageNameV1,
) -> Result<String, Diagnostic> {
    let mut output = String::from("manifest-version = 1\n\n[package]\n");
    push_toml_string(&mut output, "namespace", &package.namespace.to_string());
    push_toml_string(&mut output, "name", &name.to_string());
    push_toml_string(&mut output, "version", &package.version.to_string());
    push_toml_string(&mut output, "edition", "1");
    output.push_str("abi-version = 1\n");
    if let Some(value) = &package.description {
        push_toml_string(&mut output, "description", value);
    }
    if let Some(value) = &package.readme {
        push_toml_string(&mut output, "readme", value.as_str());
    }
    if let Some(value) = &package.license {
        push_toml_string(&mut output, "license", value);
    }
    if let Some(value) = &package.license_file {
        push_toml_string(&mut output, "license-file", value.as_str());
    }
    if let Some(value) = &package.repository {
        push_toml_string(&mut output, "repository", value);
    }
    push_toml_array(
        &mut output,
        "keywords",
        package.keywords.iter().map(String::as_str),
    );
    push_toml_array(
        &mut output,
        "include",
        package.includes.iter().map(PortablePath::as_str),
    );
    output.push_str("\n[lib]\n");
    push_toml_string(&mut output, "source-dir", package.source_dir.as_str());
    let mut exports = package
        .exports
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>();
    exports.sort_unstable();
    exports.dedup();
    push_toml_array(&mut output, "exports", exports);
    parse_manifest(&output)
        .map_err(|error| manifest_diagnostic(Path::new(MANIFEST_FILE_NAME), &error))?;
    Ok(output)
}
fn push_toml_string(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = ");
    output.push_str(&toml_quote(value));
    output.push('\n');
}
fn push_toml_array<I, S>(output: &mut String, key: &str, values: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let values = values
        .into_iter()
        .map(|value| toml_quote(value.as_ref()))
        .collect::<Vec<_>>();
    if values.is_empty() && !matches!(key, "exports") {
        return;
    }
    output.push_str(key);
    output.push_str(" = [");
    output.push_str(&values.join(", "));
    output.push_str("]\n");
}
fn toml_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            character => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}
fn initialize_package_files(
    root: &Path,
    source_dir: &PortablePath,
    manifest: &str,
    preserve_source: bool,
) -> Result<(), Diagnostic> {
    let source_path = root.join(source_dir.to_path_buf());
    fs::create_dir_all(&source_path)
        .map_err(|error| io_diagnostic("create library source directory", &source_path, &error))?;
    let writer = AtomicWriteRoot::new(root).map_err(atomic_diagnostic)?;
    let library = source_dir.to_path_buf().join("lib.ko");
    let library_path = root.join(&library);
    if preserve_source {
        match fs::symlink_metadata(&library_path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(Diagnostic::new(
                    ErrorCode::Io,
                    "existing library target is not a regular non-symlink file",
                )
                .with_context("path", library_path.display().to_string()));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => writer
                .replace(&library, b"// Musubi V1 library source.\n")
                .map_err(atomic_diagnostic)?,
            Err(error) => {
                return Err(io_diagnostic(
                    "inspect library source",
                    &library_path,
                    &error,
                ));
            }
        }
    } else {
        writer
            .replace(&library, b"// Musubi V1 library source.\n")
            .map_err(atomic_diagnostic)?;
    }
    writer
        .replace(Path::new(MANIFEST_FILE_NAME), manifest.as_bytes())
        .map_err(atomic_diagnostic)
}
fn run_add(explicit_manifest: Option<&Path>, args: &AddArgs) -> CommandResult {
    let initial_manifest = project_manifest_path(explicit_manifest)?;
    let section = if args.dev {
        DependencySection::Development
    } else if args.workspace_dependency {
        DependencySection::Workspace
    } else {
        DependencySection::Normal
    };
    let manifest_path = if section == DependencySection::Workspace {
        load_workspace(&initial_manifest)
            .map_err(workspace_diagnostic)?
            .root_manifest_path()
            .to_path_buf()
    } else {
        initial_manifest
    };
    let source = read_manifest_source(&manifest_path)?;
    let parsed =
        parse_manifest(&source).map_err(|error| manifest_diagnostic(&manifest_path, &error))?;
    let (alias, dependency) = dependency_from_add(args)?;
    if section == DependencySection::Workspace && matches!(dependency, DependencySpec::Workspace) {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "a workspace dependency entry must be concrete",
        ));
    }
    let exists = match section {
        DependencySection::Normal => parsed.dependencies.contains_key(alias.as_ref()),
        DependencySection::Development => parsed.dev_dependencies.contains_key(alias.as_ref()),
        DependencySection::Workspace => workspace_dependency_exists(&parsed, &alias),
    };
    if exists && !args.replace {
        return Err(Diagnostic::new(
            ErrorCode::ManifestInvalid,
            "dependency alias already exists",
        )
        .with_context("alias", alias.to_string())
        .with_help("pass `--replace` to update this one focused entry"));
    }
    if matches!(dependency, DependencySpec::Workspace) {
        validate_workspace_inheritance(&manifest_path, &alias)?;
    }
    if let DependencySpec::Concrete(concrete) = &dependency {
        validate_added_path_dependency(&manifest_path, section, &alias, concrete)?;
    }
    let edited = upsert_dependency(&source, section, alias.as_ref(), &dependency)
        .map_err(|error| manifest_diagnostic(&manifest_path, &error))?;
    atomic_replace_manifest(&manifest_path, edited.as_bytes())?;
    let kind = match section {
        DependencySection::Normal => "normal",
        DependencySection::Development => "development",
        DependencySection::Workspace => "workspace",
    };
    Ok(Success {
        message: format!("added {alias} ({kind})"),
        data: object([
            ("alias", Value::from(alias.to_string())),
            ("kind", Value::from(kind)),
            ("manifest", Value::from(manifest_path.display().to_string())),
        ]),
    })
}
fn dependency_from_add(args: &AddArgs) -> Result<(Name, DependencySpec), Diagnostic> {
    if args.workspace {
        if args.version.is_some() || args.path.is_some() || args.rename.is_some() {
            return Err(Diagnostic::new(
                ErrorCode::Usage,
                "`--workspace` accepts only the inherited dependency alias",
            ));
        }
        let alias = args.package.parse::<Name>().map_err(|error| {
            Diagnostic::new(ErrorCode::Usage, error.to_string())
                .with_context("alias", args.package.clone())
        })?;
        return Ok((alias, DependencySpec::Workspace));
    }
    let parse_package = || {
        args.package
            .parse::<MusubiPackageSelectorV1>()
            .map_err(|error| {
                Diagnostic::new(ErrorCode::Usage, error.to_string())
                    .with_context("package", args.package.clone())
            })
    };
    let (default_alias, dependency) = match (&args.path, &args.version) {
        (None, Some(requirement)) => {
            let package = parse_package()?;
            let alias = dependency_alias_from_package(&package)?;
            (
                alias,
                ConcreteDependency::Registry {
                    package,
                    requirement: requirement.clone(),
                },
            )
        }
        (None, None) => {
            return Err(Diagnostic::new(
                ErrorCode::Usage,
                "registry dependencies require `--version REQUIREMENT`",
            ));
        }
        (Some(path), Some(requirement)) => {
            let package = parse_package()?;
            let alias = dependency_alias_from_package(&package)?;
            (
                alias,
                ConcreteDependency::Path {
                    path: path.clone(),
                    package: Some(package),
                    requirement: Some(requirement.clone()),
                },
            )
        }
        (Some(path), None) => {
            let alias = args.package.parse::<Name>().map_err(|error| {
                Diagnostic::new(ErrorCode::Usage, error.to_string())
                    .with_context("alias", args.package.clone())
                    .with_help(
                        "for a publishable path dependency, use `namespace/package --version REQUIREMENT`",
                    )
            })?;
            (
                alias,
                ConcreteDependency::Path {
                    path: path.clone(),
                    package: None,
                    requirement: None,
                },
            )
        }
    };
    let alias = args.rename.clone().unwrap_or(default_alias);
    Ok((alias, DependencySpec::Concrete(dependency)))
}
fn dependency_alias_from_package(package: &MusubiPackageSelectorV1) -> Result<Name, Diagnostic> {
    package.name.as_str().parse::<Name>().map_err(|error| {
        Diagnostic::new(ErrorCode::Internal, error.to_string())
            .with_context("package", package.to_string())
    })
}
fn workspace_dependency_exists(manifest: &Manifest, alias: &Name) -> bool {
    manifest
        .workspace
        .as_ref()
        .is_some_and(|workspace| workspace.dependencies.contains_key(alias.as_ref()))
}
fn validate_workspace_inheritance(manifest_path: &Path, alias: &Name) -> Result<(), Diagnostic> {
    let workspace = load_workspace(manifest_path).map_err(workspace_diagnostic)?;
    let declaration = workspace
        .root_manifest()
        .workspace
        .as_ref()
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::WorkspaceInvalid,
                "dependency requests workspace inheritance outside a workspace",
            )
        })?;
    if !declaration.dependencies.contains_key(alias.as_ref()) {
        return Err(Diagnostic::new(
            ErrorCode::WorkspaceInvalid,
            "inherited dependency is absent from `[workspace.dependencies]`",
        )
        .with_context("alias", alias.to_string()));
    }
    Ok(())
}
fn validate_added_path_dependency(
    manifest_path: &Path,
    section: DependencySection,
    alias: &Name,
    dependency: &ConcreteDependency,
) -> Result<(), Diagnostic> {
    let ConcreteDependency::Path {
        path,
        package,
        requirement,
    } = dependency
    else {
        return Ok(());
    };
    let workspace = load_workspace(manifest_path).map_err(workspace_diagnostic)?;
    let base = if section == DependencySection::Workspace {
        workspace.root()
    } else {
        workspace
            .members()
            .values()
            .find(|member| member.manifest_path == manifest_path)
            .map(|member| member.package_root.as_path())
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorCode::WorkspaceInvalid,
                    "manifest is not an active workspace member",
                )
            })?
    };
    let candidate = fs::canonicalize(base.join(path.to_path_buf())).map_err(|error| {
        io_diagnostic(
            "resolve local dependency directory",
            &base.join(path.to_path_buf()),
            &error,
        )
    })?;
    if !candidate.starts_with(workspace.root()) {
        return Err(Diagnostic::new(
            ErrorCode::WorkspaceInvalid,
            "local dependency path escapes the workspace root",
        )
        .with_context("alias", alias.to_string())
        .with_context("path", path.to_string()));
    }
    let target = load_workspace(&candidate).map_err(workspace_diagnostic)?;
    let target_member = target
        .members()
        .values()
        .find(|member| member.package_root == candidate)
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::WorkspaceInvalid,
                "local dependency path does not identify a package member",
            )
            .with_context("path", candidate.display().to_string())
        })?;
    if let (Some(package), Some(requirement)) = (package, requirement)
        && (package != &target_member.package.selector
            || !requirement.matches(&target_member.package.version))
    {
        return Err(Diagnostic::new(
            ErrorCode::ManifestInvalid,
            "local dependency does not match its declared registry package/range",
        )
        .with_context("alias", alias.to_string())
        .with_context("local_package", target_member.package.selector.to_string())
        .with_context("local_version", target_member.package.version.to_string()));
    }
    Ok(())
}
fn run_remove(explicit_manifest: Option<&Path>, args: &RemoveArgs) -> CommandResult {
    let initial_manifest = project_manifest_path(explicit_manifest)?;
    let section = if args.dev {
        DependencySection::Development
    } else if args.workspace_dependency {
        DependencySection::Workspace
    } else {
        DependencySection::Normal
    };
    let manifest_path = if section == DependencySection::Workspace {
        load_workspace(&initial_manifest)
            .map_err(workspace_diagnostic)?
            .root_manifest_path()
            .to_path_buf()
    } else {
        initial_manifest
    };
    let source = read_manifest_source(&manifest_path)?;
    let edited = remove_dependency(&source, section, args.alias.as_ref())
        .map_err(|error| manifest_diagnostic(&manifest_path, &error))?;
    atomic_replace_manifest(&manifest_path, edited.as_bytes())?;
    Ok(Success {
        message: format!("removed {}", args.alias),
        data: object([
            ("alias", Value::from(args.alias.to_string())),
            ("manifest", Value::from(manifest_path.display().to_string())),
        ]),
    })
}
fn run_metadata(explicit_manifest: Option<&Path>, args: &MetadataArgs) -> CommandResult {
    let manifest_path = project_manifest_path(explicit_manifest)?;
    let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
    let members = select_members(&workspace, &args.selection)?;
    let lock = read_optional_workspace_lock(&workspace)?;
    let mut human = format!("workspace_root = {}\n", workspace.root().display());
    let mut values = Vec::with_capacity(members.len());
    for member in members {
        writeln!(
            human,
            "package = {} {} ({})",
            member.package.selector, member.package.version, member.workspace_path
        )
        .expect("writing to a String cannot fail");
        for dependency in member
            .dependencies
            .values()
            .chain(member.dev_dependencies.values())
        {
            writeln!(
                human,
                "  {} {}",
                dependency_kind_text(dependency.kind),
                dependency_text(dependency, &workspace)
            )
            .expect("writing to a String cannot fail");
        }
        values.push(member_json(member, &workspace));
    }
    if let Some(lock) = &lock {
        writeln!(
            human,
            "lock = {}@{} (height {}, index revision {}, {} nodes)",
            lock.schema,
            lock.version,
            lock.snapshot.finalized_height,
            lock.snapshot.index_revision,
            lock.nodes.len()
        )
        .expect("writing to a String cannot fail");
    }
    Ok(Success {
        message: human.trim_end().to_owned(),
        data: object([
            (
                "workspace_root",
                Value::from(workspace.root().display().to_string()),
            ),
            ("packages", Value::Array(values)),
            ("lock", lock.as_ref().map_or(Value::Null, lockfile_json)),
        ]),
    })
}
fn run_tree(explicit_manifest: Option<&Path>, args: &TreeArgs) -> CommandResult {
    let manifest_path = project_manifest_path(explicit_manifest)?;
    let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
    let roots = select_members(&workspace, &args.selection)?;
    let lock = read_optional_workspace_lock(&workspace)?;
    let mut human = String::new();
    let mut root_values = Vec::with_capacity(roots.len());
    for (index, root) in roots.iter().enumerate() {
        if index > 0 {
            human.push('\n');
        }
        writeln!(
            human,
            "{} v{} ({})",
            root.package.selector, root.package.version, root.workspace_path
        )
        .expect("writing to a String cannot fail");
        let mut visiting = BTreeSet::from([root.manifest_path.clone()]);
        render_tree_dependencies(
            &workspace,
            root,
            "",
            !args.no_dev,
            &mut visiting,
            &mut human,
        );
        root_values.push(member_json(root, &workspace));
    }
    if let Some(lock) = &lock {
        render_locked_roots(lock, &roots, !args.no_dev, &mut human);
    }
    Ok(Success {
        message: human.trim_end().to_owned(),
        data: object([
            ("roots", Value::Array(root_values)),
            ("lock", lock.as_ref().map_or(Value::Null, lockfile_json)),
        ]),
    })
}
fn render_locked_roots(
    lock: &LockfileV1,
    selected: &[&WorkspaceMember],
    include_dev: bool,
    output: &mut String,
) {
    for member in selected {
        let Some(root) = lock
            .roots
            .iter()
            .find(|root| root.package == member.package.selector)
        else {
            continue;
        };
        writeln!(output, "\n{} exact lock graph", member.package.selector)
            .expect("writing to a String cannot fail");
        let mut visiting = BTreeSet::new();
        render_locked_edges(
            lock,
            &root.dependencies,
            "",
            include_dev,
            &mut visiting,
            output,
        );
    }
}
fn render_locked_edges(
    lock: &LockfileV1,
    edges: &[MusubiExactDependencyEdgeV1],
    prefix: &str,
    include_dev: bool,
    visiting: &mut BTreeSet<MusubiReleaseIdV1>,
    output: &mut String,
) {
    let edges = edges
        .iter()
        .filter(|edge| include_dev || edge.kind != MusubiDependencyKindV1::Development)
        .collect::<Vec<_>>();
    for (index, edge) in edges.iter().enumerate() {
        let last = index + 1 == edges.len();
        output.push_str(prefix);
        output.push_str(if last { "└──" } else { "├──" });
        output.push(' ');
        if edge.kind == MusubiDependencyKindV1::Development {
            output.push_str("[dev] ");
        }
        write!(output, "{} -> {}", edge.alias, edge.selected)
            .expect("writing to a String cannot fail");
        let cycle = !visiting.insert(edge.selected.clone());
        if cycle {
            output.push_str(" (*)\n");
            continue;
        }
        output.push('\n');
        if let Some(node) = lock.nodes.iter().find(|node| node.release == edge.selected) {
            let child_prefix = format!("{prefix}{}   ", if last { " " } else { "│" });
            render_locked_edges(
                lock,
                &node.dependencies,
                &child_prefix,
                false,
                visiting,
                output,
            );
        }
        visiting.remove(&edge.selected);
    }
}
fn render_tree_dependencies(
    workspace: &Workspace,
    member: &WorkspaceMember,
    prefix: &str,
    include_dev: bool,
    visiting: &mut BTreeSet<PathBuf>,
    output: &mut String,
) {
    let mut dependencies = member.dependencies.values().collect::<Vec<_>>();
    if include_dev {
        dependencies.extend(member.dev_dependencies.values());
    }
    for (index, dependency) in dependencies.iter().enumerate() {
        let last = index + 1 == dependencies.len();
        let connector = if last { "└──" } else { "├──" };
        output.push_str(prefix);
        output.push_str(connector);
        output.push(' ');
        if dependency.kind == DependencyKind::Development {
            output.push_str("[dev] ");
        }
        output.push_str(&dependency_text(dependency, workspace));
        let child = dependency.local_manifest.as_ref().and_then(|manifest| {
            workspace
                .members()
                .values()
                .find(|member| &member.manifest_path == manifest)
        });
        let cycle = child.is_some_and(|child| visiting.contains(&child.manifest_path));
        if cycle {
            output.push_str(" (*)");
        }
        output.push('\n');
        if let Some(child) = child.filter(|_| !cycle) {
            visiting.insert(child.manifest_path.clone());
            let child_prefix = format!("{prefix}{}   ", if last { " " } else { "│" });
            render_tree_dependencies(workspace, child, &child_prefix, false, visiting, output);
            visiting.remove(&child.manifest_path);
        }
    }
}
fn select_members<'a>(
    workspace: &'a Workspace,
    selection: &SelectionArgs,
) -> Result<Vec<&'a WorkspaceMember>, Diagnostic> {
    workspace
        .select_members(selection.workspace, &selection.packages, &selection.exclude)
        .map_err(workspace_diagnostic)
}
fn member_json(member: &WorkspaceMember, workspace: &Workspace) -> Value {
    let dependencies = member
        .dependencies
        .values()
        .chain(member.dev_dependencies.values())
        .map(|dependency| {
            object([
                ("alias", Value::from(dependency.alias.to_string())),
                ("kind", Value::from(dependency_kind_text(dependency.kind))),
                (
                    "source",
                    Value::from(dependency_text(dependency, workspace)),
                ),
            ])
        })
        .collect();
    object([
        ("path", Value::from(member.workspace_path.to_string())),
        ("package", Value::from(member.package.selector.to_string())),
        ("version", Value::from(member.package.version.to_string())),
        ("dependencies", Value::Array(dependencies)),
    ])
}
const fn dependency_kind_text(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Development => "development",
    }
}
fn dependency_text(dependency: &EffectiveDependency, workspace: &Workspace) -> String {
    match &dependency.dependency {
        ConcreteDependency::Registry {
            package,
            requirement,
        } => format!("{} -> {package} {requirement}", dependency.alias),
        ConcreteDependency::Path {
            path,
            package,
            requirement,
        } => {
            let local = dependency.local_manifest.as_ref().and_then(|manifest| {
                workspace
                    .members()
                    .values()
                    .find(|member| &member.manifest_path == manifest)
            });
            local.map_or_else(
                || {
                    if let (Some(package), Some(requirement)) = (package, requirement) {
                        format!(
                            "{} -> {package} {requirement} (path {path})",
                            dependency.alias
                        )
                    } else {
                        format!("{} -> path {path}", dependency.alias)
                    }
                },
                |local| {
                    format!(
                        "{} -> {} v{} (path {})",
                        dependency.alias, local.package.selector, local.package.version, path
                    )
                },
            )
        }
    }
}
fn project_manifest_path(explicit: Option<&Path>) -> Result<PathBuf, Diagnostic> {
    if let Some(path) = explicit {
        let candidate = if path.is_dir() {
            path.join(MANIFEST_FILE_NAME)
        } else {
            path.to_path_buf()
        };
        if candidate.file_name().and_then(|name| name.to_str()) != Some(MANIFEST_FILE_NAME) {
            return Err(Diagnostic::new(
                ErrorCode::Usage,
                "`--manifest-path` must name `Musubi.toml` or its directory",
            )
            .with_context("path", candidate.display().to_string()));
        }
        discover_manifest(&candidate).map_err(workspace_diagnostic)
    } else {
        let current = std::env::current_dir()
            .map_err(|error| io_diagnostic("read current directory", Path::new("."), &error))?;
        discover_manifest(&current).map_err(workspace_diagnostic)
    }
}
fn read_manifest_source(path: &Path) -> Result<String, Diagnostic> {
    let bytes = read_bounded_single_link_regular_file_v1(path, MAX_MANIFEST_BYTES)
        .map_err(|error| io_diagnostic("read bounded manifest", path, &error))?;
    String::from_utf8(bytes).map_err(|error| {
        io_diagnostic(
            "read manifest as UTF-8",
            path,
            &io::Error::new(io::ErrorKind::InvalidData, error),
        )
    })
}
fn atomic_replace_manifest(path: &Path, contents: &[u8]) -> Result<(), Diagnostic> {
    let root = path.parent().ok_or_else(|| {
        Diagnostic::new(ErrorCode::Io, "manifest path has no parent")
            .with_context("path", path.display().to_string())
    })?;
    let writer = AtomicWriteRoot::new(root).map_err(atomic_diagnostic)?;
    writer
        .replace(Path::new(MANIFEST_FILE_NAME), contents)
        .map_err(atomic_diagnostic)
}
fn manifest_diagnostic(path: &Path, error: &crate::manifest::ManifestError) -> Diagnostic {
    Diagnostic::new(ErrorCode::ManifestInvalid, error.to_string())
        .with_context("path", path.display().to_string())
        .with_context("field", error.location())
}
#[allow(clippy::needless_pass_by_value)]
fn workspace_diagnostic(error: crate::workspace::WorkspaceError) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(ErrorCode::WorkspaceInvalid, error.message());
    if let Some(path) = error.path() {
        diagnostic = diagnostic.with_context("path", path.display().to_string());
    }
    diagnostic
}
#[allow(clippy::needless_pass_by_value)]
fn atomic_diagnostic(error: crate::atomic_io::AtomicWriteError) -> Diagnostic {
    Diagnostic::new(ErrorCode::Io, error.to_string())
        .with_context("path", error.path().display().to_string())
}
fn io_diagnostic(operation: &str, path: &Path, error: &io::Error) -> Diagnostic {
    Diagnostic::new(ErrorCode::Io, format!("failed to {operation}: {error}"))
        .with_context("path", path.display().to_string())
}
fn read_optional_workspace_lock(workspace: &Workspace) -> Result<Option<LockfileV1>, Diagnostic> {
    let path = workspace.root().join(LOCK_FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_diagnostic("inspect lockfile", &path, &error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Diagnostic::new(
            ErrorCode::LockfileInvalid,
            "Musubi.lock must be a regular non-symlink file",
        )
        .with_context("path", path.display().to_string()));
    }
    LockfileV1::read(&path)
        .map(Some)
        .map_err(|error| lockfile_diagnostic(&path, &error))
}
fn lockfile_diagnostic(path: &Path, error: &LockfileError) -> Diagnostic {
    let code = if matches!(error, LockfileError::Legacy) {
        ErrorCode::LockfileLegacy
    } else {
        ErrorCode::LockfileInvalid
    };
    let help = if code == ErrorCode::LockfileLegacy {
        "regenerate the lock with Musubi V1; --locked never rewrites retired formats"
    } else {
        "repair or regenerate the consumer-owned exact lock graph"
    };
    Diagnostic::new(code, error.to_string())
        .with_context("path", path.display().to_string())
        .with_help(help)
}
fn lockfile_json(lock: &LockfileV1) -> Value {
    object([
        ("schema", Value::from(lock.schema.clone())),
        ("version", Value::from(u64::from(lock.version))),
        ("network_id", Value::from(lock.network_id.to_string())),
        (
            "finalized_height",
            Value::from(lock.snapshot.finalized_height),
        ),
        (
            "finalized_block_hash",
            Value::from(hex::encode(lock.snapshot.finalized_block_hash)),
        ),
        ("index_revision", Value::from(lock.snapshot.index_revision)),
        (
            "roots",
            Value::Array(
                lock.roots
                    .iter()
                    .map(|root| {
                        object([
                            ("package", Value::from(root.package.to_string())),
                            (
                                "dependencies",
                                Value::Array(
                                    root.dependencies.iter().map(lock_edge_json).collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "nodes",
            Value::Array(
                lock.nodes
                    .iter()
                    .map(|node| {
                        object([
                            ("release", Value::from(node.release.to_string())),
                            (
                                "release_digest",
                                Value::from(hex::encode(node.release_digest.as_bytes())),
                            ),
                            (
                                "archive_id",
                                Value::from(hex::encode(node.archive_id.as_bytes())),
                            ),
                            (
                                "source_digest",
                                Value::from(hex::encode(node.source_digest.as_bytes())),
                            ),
                            (
                                "interface_digest",
                                Value::from(hex::encode(node.interface_digest.as_bytes())),
                            ),
                            (
                                "dependencies",
                                Value::Array(
                                    node.dependencies.iter().map(lock_edge_json).collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}
fn lock_edge_json(edge: &MusubiExactDependencyEdgeV1) -> Value {
    object([
        ("alias", Value::from(edge.alias.to_string())),
        (
            "kind",
            Value::from(match edge.kind {
                MusubiDependencyKindV1::Normal => "normal",
                MusubiDependencyKindV1::Development => "development",
            }),
        ),
        ("package", Value::from(edge.package.to_string())),
        ("requirement", Value::from(edge.requirement.to_string())),
        ("selected", Value::from(edge.selected.to_string())),
    ])
}
fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<Map>(),
    )
}
fn load_selected_workspace(
    explicit_manifest: Option<&Path>,
    selection: &SelectionArgs,
) -> Result<(Workspace, Vec<MusubiPackageSelectorV1>), Diagnostic> {
    let manifest_path = project_manifest_path(explicit_manifest)?;
    let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
    let selected = select_members(&workspace, selection)?;
    let selected_packages = selected
        .iter()
        .map(|member| member.package.selector.clone())
        .collect();
    Ok((workspace, selected_packages))
}
struct ResolvedWorkspaceGraphV1 {
    lock: LockfileV1,
    registry: Option<RegistryReadClientV1>,
    cached_source: Option<CachedResolverSourceV1>,
    prepared_archive_fetch:
        Option<Result<PreparedProductionSorafsArchiveTransportV1, ArchiveTransportErrorV1>>,
    platform_config_provenance: Option<PlatformConfigProvenanceV1>,
    account_chain_discriminant: u16,
}
impl ResolvedWorkspaceGraphV1 {
    const fn account_chain_discriminant(&self) -> u16 {
        self.account_chain_discriminant
    }
    fn online_registry(&self) -> Result<&RegistryReadClientV1, Diagnostic> {
        self.registry.as_ref().ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::OfflineMiss,
                "this operation requires a live finalized Musubi registry",
            )
        })
    }
    fn bind_selector_namespace(
        &self,
        selector: &MusubiPackageSelectorV1,
    ) -> Result<iroha_data_model::musubi::MusubiPackageIdV1, Diagnostic> {
        if let Some(source) = &self.cached_source {
            return source.bind_selector_namespace(selector).map_err(|error| {
                Diagnostic::new(
                    ErrorCode::OfflineMiss,
                    "cached namespace binding is unavailable for the selected package",
                )
                .with_context("package", selector.to_string())
                .with_context("reason", error.to_string())
            });
        }
        self.online_registry()?
            .bind_selector_namespace(selector)
            .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "preserves resolver transaction ordering"
)]
fn resolve_and_update_workspace_lock(
    workspace: &Workspace,
    selected_packages: &[MusubiPackageSelectorV1],
    mode: GraphModeArgs,
    previous: Option<LockfileV1>,
    update: Option<GraphUpdateV1>,
    config: Option<&Path>,
    fresh_only: bool,
) -> Result<ResolvedWorkspaceGraphV1, Diagnostic> {
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let cache_root = platform_cache_root_v1().map_err(|error| {
        Diagnostic::new(
            ErrorCode::CacheCorrupt,
            "platform Musubi resolver cache root is unavailable",
        )
        .with_context("reason", error.to_string())
    })?;
    let resolver_cache = ResolverIndexCacheV1::open(&cache_root).map_err(|error| {
        Diagnostic::new(
            ErrorCode::CacheCorrupt,
            "Musubi V1 resolver cache could not be opened",
        )
        .with_context("reason", error.to_string())
    })?;
    let resolve_mode = if mode.effective_locked() {
        ResolveModeV1::Locked
    } else {
        ResolveModeV1::UpdateLock
    };
    let (
        outcome,
        registry,
        cached_source,
        prepared_archive_fetch,
        platform_config_provenance,
        account_chain_discriminant,
    ) = if mode.effective_offline() {
        let cached = resolve_workspace_offline_cached(
            &resolver_cache,
            workspace,
            selected_packages,
            previous,
            update,
            resolve_mode,
        )
        .map_err(graph_diagnostic)?;
        let account_chain_discriminant = cached.source.account_chain_discriminant();
        (
            cached.outcome,
            None,
            Some(cached.source),
            None,
            None,
            account_chain_discriminant,
        )
    } else {
        let (registry, config_image) = RegistryReadClientV1::load_with_config_image(config)
            .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
        let prepared_archive_fetch =
            prepare_production_archive_transport_v1(config_image.path(), config_image.bytes());
        let platform_config_provenance = config_image.provenance();
        drop(config_image);
        let account_chain_discriminant = registry.account_chain_discriminant();
        let mut snapshot_mismatches = 0_u8;
        let outcome = loop {
            let result = if fresh_only {
                resolve_workspace_online_cached_fresh(
                    &registry,
                    &resolver_cache,
                    workspace,
                    selected_packages,
                    previous.clone(),
                    update.clone(),
                    resolve_mode,
                )
            } else {
                resolve_workspace_online_cached(
                    &registry,
                    &resolver_cache,
                    workspace,
                    selected_packages,
                    previous.clone(),
                    update.clone(),
                    resolve_mode,
                )
            };
            match result {
                Err(GraphErrorV1::SnapshotChanged) if snapshot_mismatches < 2 => {
                    snapshot_mismatches += 1;
                }
                result => break result.map_err(graph_diagnostic)?,
            }
        };
        (
            outcome,
            Some(registry),
            None,
            Some(prepared_archive_fetch),
            Some(platform_config_provenance),
            account_chain_discriminant,
        )
    };
    if outcome.changed {
        let writer = AtomicWriteRoot::new(workspace.root()).map_err(atomic_diagnostic)?;
        outcome
            .lockfile
            .write_atomic(&writer, Path::new(LOCK_FILE_NAME))
            .map_err(|error| lockfile_diagnostic(&lock_path, &error))?;
    }
    Ok(ResolvedWorkspaceGraphV1 {
        lock: outcome.lockfile,
        registry,
        cached_source,
        prepared_archive_fetch,
        platform_config_provenance,
        account_chain_discriminant,
    })
}
fn graph_diagnostic(error: GraphErrorV1) -> Diagnostic {
    match error {
        GraphErrorV1::Workspace(error) => workspace_diagnostic(error),
        GraphErrorV1::Registry(code) => Diagnostic::new(
            ErrorCode::Registry,
            "finalized Musubi registry query failed",
        )
        .with_context("registry_code", code),
        GraphErrorV1::Cache(reason) => Diagnostic::new(
            ErrorCode::CacheCorrupt,
            "authenticated Musubi resolver cache failed",
        )
        .with_context("reason", reason),
        GraphErrorV1::OfflineMiss(reason) => Diagnostic::new(
            ErrorCode::OfflineMiss,
            "cached Musubi resolver index does not cover the requested graph",
        )
        .with_context("reason", reason)
        .with_help("run the same selection once online to refresh the resolver cache"),
        GraphErrorV1::PackageNotFound(package) => Diagnostic::new(
            ErrorCode::Registry,
            "a manifest dependency has no exact canonical registry package",
        )
        .with_context("package", package.to_string()),
        GraphErrorV1::SnapshotChanged => Diagnostic::new(
            ErrorCode::Network,
            "the finalized Musubi registry snapshot changed during resolution",
        )
        .with_help("retry after finality advances less frequently"),
        GraphErrorV1::InvalidRegistryData(reason) => Diagnostic::new(
            ErrorCode::Registry,
            "the finalized Musubi registry returned inconsistent resolver data",
        )
        .with_context("reason", reason),
        GraphErrorV1::CandidateLimit => Diagnostic::new(
            ErrorCode::ResolutionConflict,
            "the bounded resolver collection exceeds its resource corridor",
        ),
        GraphErrorV1::Resolver(ResolverError::LockChangeRequired) => Diagnostic::new(
            ErrorCode::Locked,
            "the exact dependency graph must change, but --locked forbids rewriting Musubi.lock",
        ),
        GraphErrorV1::Resolver(ResolverError::Conflict(conflict)) => {
            let code = if matches!(conflict.reason, ConflictReasonV1::Cycle(_)) {
                ErrorCode::DependencyCycle
            } else {
                ErrorCode::ResolutionConflict
            };
            Diagnostic::new(code, conflict.to_string())
        }
        GraphErrorV1::Resolver(ResolverError::SearchLimitExceeded { limit }) => Diagnostic::new(
            ErrorCode::ResolutionConflict,
            "dependency resolution exhausted its deterministic search corridor",
        )
        .with_context("candidate_branch_attempt_limit", limit.to_string()),
        GraphErrorV1::Resolver(ResolverError::InvalidInput(reason)) => Diagnostic::new(
            ErrorCode::Registry,
            "validated lock, workspace, and registry inputs disagree",
        )
        .with_context("reason", reason),
    }
}
fn run_fetch(explicit_manifest: Option<&Path>, args: &FetchArgs) -> CommandResult {
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let previous = read_optional_workspace_lock(&workspace)?;
    let graph = resolve_and_update_workspace_lock(
        &workspace,
        &selected_names,
        args.mode,
        previous,
        None,
        args.registry.config.as_deref(),
        false,
    )?;
    let cache = open_user_cache()?;
    let fetched = ensure_graph_archives(&cache, &graph, args.mode)?;
    Ok(Success {
        message: format!("fetched {} archive(s)", fetched.len()),
        data: object([
            ("lockfile", Value::from(lock_path.display().to_string())),
            ("archives", Value::Array(fetched)),
            ("graph", lockfile_json(&graph.lock)),
        ]),
    })
}
fn ensure_graph_archives(
    cache: &MusubiCache,
    graph: &ResolvedWorkspaceGraphV1,
    mode: GraphModeArgs,
) -> Result<Vec<Value>, Diagnostic> {
    let mut nodes_by_archive = BTreeMap::new();
    for node in &graph.lock.nodes {
        nodes_by_archive
            .entry(node.archive_id)
            .or_insert_with(Vec::new)
            .push(node);
    }
    let mut transport: Option<ProductionSorafsArchiveTransportV1> = None;
    let mut fetched = Vec::with_capacity(nodes_by_archive.len());
    for (archive_id, nodes) in nodes_by_archive {
        if nodes
            .iter()
            .all(|node| cache.load_compiler_package(node).is_ok())
        {
            fetched.push(object([
                (
                    "archive_id",
                    Value::from(hex::encode(archive_id.as_bytes())),
                ),
                ("location_id", Value::Null),
                ("provider", Value::Null),
                ("cache", Value::from("already-present")),
            ]));
            continue;
        }
        if mode.effective_offline() {
            return Err(Diagnostic::new(
                ErrorCode::OfflineMiss,
                "an exact locked archive is absent or invalid in the immutable cache",
            )
            .with_context("archive_id", hex::encode(archive_id.as_bytes()))
            .with_help("rerun online, or repair a corrupt cache entry first"));
        }
        if transport.is_none() {
            let prepared = graph
                .prepared_archive_fetch
                .as_ref()
                .ok_or_else(|| {
                    Diagnostic::new(
                        ErrorCode::OfflineMiss,
                        "authenticated archive-fetch configuration is unavailable offline",
                    )
                })?
                .as_ref()
                .map_err(|error| archive_transport_diagnostic(*error))?;
            transport = Some(
                build_production_archive_transport_v1(prepared)
                    .map_err(archive_transport_diagnostic)?,
            );
        }
        let adapter = MusubiArchiveFetchAdapterV1::new(graph.online_registry()?, cache)
            .with_expected_deployment(graph.lock.network_id, graph.lock.snapshot);
        let outcome = adapter
            .fetch_exact(
                archive_id,
                transport
                    .as_mut()
                    .expect("production transport was initialized above"),
            )
            .map_err(|error| {
                archive_fetch_diagnostic(error)
                    .with_context("archive_id", hex::encode(archive_id.as_bytes()))
            })?;
        for node in nodes {
            cache.load_compiler_package(node).map_err(|error| {
                cache_maintenance_diagnostic(&error)
                    .with_context("archive_id", hex::encode(archive_id.as_bytes()))
            })?;
        }
        let cache_status = match outcome.cache {
            InstallOutcome::Installed(_) => "installed",
            InstallOutcome::AlreadyPresent(_) => "already-present",
        };
        fetched.push(object([
            (
                "archive_id",
                Value::from(hex::encode(outcome.archive_id.as_bytes())),
            ),
            (
                "location_id",
                Value::from(hex::encode(outcome.location_id.as_bytes())),
            ),
            ("provider", Value::from(outcome.provider.to_string())),
            ("cache", Value::from(cache_status)),
        ]));
    }
    Ok(fetched)
}
fn archive_transport_diagnostic(error: ArchiveTransportErrorV1) -> Diagnostic {
    let code = match error.class() {
        ArchiveFetchFailureClassV1::Retryable => ErrorCode::Network,
        ArchiveFetchFailureClassV1::Integrity => ErrorCode::ArchiveInvalid,
        ArchiveFetchFailureClassV1::Unavailable | ArchiveFetchFailureClassV1::Permanent => {
            ErrorCode::Registry
        }
    };
    Diagnostic::new(
        code,
        "authenticated SoraFS archive transport is unavailable",
    )
    .with_context("archive_code", error.code())
}
fn archive_fetch_diagnostic(error: ArchiveFetchErrorV1) -> Diagnostic {
    let code = match error.class() {
        ArchiveFetchFailureClassV1::Retryable => ErrorCode::Network,
        ArchiveFetchFailureClassV1::Integrity
        | ArchiveFetchFailureClassV1::Unavailable
        | ArchiveFetchFailureClassV1::Permanent => ErrorCode::ArchiveInvalid,
    };
    Diagnostic::new(code, "authenticated SoraFS archive fetch failed")
        .with_context("archive_code", error.code())
}
#[allow(
    clippy::too_many_lines,
    reason = "the CLI handler keeps one auditable build/test orchestration sequence"
)]
fn run_build(
    explicit_manifest: Option<&Path>,
    command: &'static str,
    args: &BuildArgs,
) -> CommandResult {
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let previous = read_optional_workspace_lock(&workspace)?;
    let graph = resolve_and_update_workspace_lock(
        &workspace,
        &selected_names,
        args.mode,
        previous,
        None,
        args.registry.config.as_deref(),
        false,
    )?;
    let cache = open_user_cache()?;
    let archives = ensure_graph_archives(&cache, &graph, args.mode)?;
    let action = if command == "build" {
        CompilerActionV1::Build
    } else {
        CompilerActionV1::Check
    };
    let execution = execute_compiler_graph(
        &cache,
        &workspace,
        &selected_names,
        &graph.lock,
        action,
        args.release,
        graph.account_chain_discriminant(),
    )
    .map_err(|error| graph_mode_compiler_diagnostic(&error, args.mode))?;
    if command == "test" {
        let report = execute_workspace_tests_v1(
            &cache,
            &workspace,
            &selected_names,
            &graph.lock,
            &WorkspaceTestOptionsV1::new(graph.account_chain_discriminant()),
        )
        .map_err(|error| graph_mode_test_diagnostic(&error, args.mode))?;
        if !report.is_success() {
            let first_failure = report
                .targets
                .iter()
                .flat_map(|target| {
                    target
                        .report
                        .cases
                        .iter()
                        .filter(|case| !case.passed)
                        .map(move |case| {
                            format!(
                                "{}::{}::{} at line {}",
                                target.package, target.target, case.name, case.line
                            )
                        })
                })
                .next()
                .unwrap_or_else(|| "unknown failing test".to_owned());
            return Err(
                Diagnostic::new(ErrorCode::Compiler, "one or more Kotodama tests failed")
                    .with_context("passed", report.passed().to_string())
                    .with_context("failed", report.failed().to_string())
                    .with_context("first_failure", first_failure),
            );
        }
        let passed = report.passed();
        let targets = report
            .targets
            .into_iter()
            .map(|target| {
                let cases = target
                    .report
                    .cases
                    .into_iter()
                    .map(|case| {
                        object([
                            ("name", Value::from(case.name)),
                            ("line", Value::from(u64::from(case.line))),
                            ("passed", Value::from(case.passed)),
                        ])
                    })
                    .collect();
                object([
                    ("package", Value::from(target.package.to_string())),
                    ("target", Value::from(target.target)),
                    ("source", Value::from(target.source)),
                    ("cases", Value::Array(cases)),
                ])
            })
            .collect();
        return Ok(Success {
            message: format!("test completed: {passed} passed; 0 failed"),
            data: object([
                (
                    "passed",
                    Value::from(u64::try_from(passed).expect("test count fits u64")),
                ),
                ("failed", Value::from(0_u64)),
                (
                    "validated_packages",
                    Value::from(
                        u64::try_from(execution.validated_packages)
                            .expect("validated package count fits u64"),
                    ),
                ),
                (
                    "compiler_warnings",
                    Value::from(u64::try_from(execution.warnings).expect("warning count fits u64")),
                ),
                ("targets", Value::Array(targets)),
                ("archives", Value::Array(archives)),
                ("lock", lockfile_json(&graph.lock)),
            ]),
        });
    }
    let artifacts = execution
        .artifacts
        .iter()
        .map(|artifact| {
            object([
                ("package", Value::from(artifact.package.to_string())),
                ("target", Value::from(artifact.target.clone())),
                ("source", Value::from(artifact.source.clone())),
                (
                    "artifact",
                    Value::from(artifact.artifact.display().to_string()),
                ),
                ("artifact_hash", Value::from(artifact.artifact_hash.clone())),
                ("fresh", Value::from(artifact.fresh)),
            ])
        })
        .collect();
    let interfaces = execution
        .package_interfaces
        .iter()
        .map(|interface| {
            object([
                ("package", Value::from(interface.package.to_string())),
                (
                    "digest",
                    Value::from(hex::encode(interface.digest.as_bytes())),
                ),
            ])
        })
        .collect();
    Ok(Success {
        message: format!(
            "{command} completed for {} package(s) and {} contract target(s)",
            execution.validated_packages, execution.contract_targets
        ),
        data: object([
            (
                "validated_packages",
                Value::from(execution.validated_packages as u64),
            ),
            (
                "contract_targets",
                Value::from(execution.contract_targets as u64),
            ),
            ("warnings", Value::from(execution.warnings as u64)),
            ("artifacts", Value::Array(artifacts)),
            ("interfaces", Value::Array(interfaces)),
            ("archives", Value::Array(archives)),
            ("lock", lockfile_json(&graph.lock)),
        ]),
    })
}
fn open_user_cache() -> Result<MusubiCache, Diagnostic> {
    let root = platform_cache_root_v1().map_err(|error| {
        Diagnostic::new(ErrorCode::Io, "platform Musubi cache root is unavailable")
            .with_context("reason", error.to_string())
    })?;
    MusubiCache::open(root).map_err(|error| {
        let code = if matches!(
            &error,
            CacheError::UnsupportedPlatform | CacheError::Io { .. }
        ) {
            ErrorCode::Io
        } else {
            ErrorCode::CacheCorrupt
        };
        Diagnostic::new(code, "Musubi V1 cache could not be opened")
            .with_context("reason", error.to_string())
    })
}
fn compiler_bridge_diagnostic(error: &CompilerBridgeErrorV1) -> Diagnostic {
    let code = match error {
        CompilerBridgeErrorV1::Workspace(_) => ErrorCode::WorkspaceInvalid,
        CompilerBridgeErrorV1::Lock(_) => ErrorCode::LockfileInvalid,
        CompilerBridgeErrorV1::Cache(_) => ErrorCode::CacheCorrupt,
        CompilerBridgeErrorV1::Package(_) => ErrorCode::PackageInvalid,
        CompilerBridgeErrorV1::Compiler(_) => ErrorCode::Compiler,
    };
    Diagnostic::new(code, error.to_string())
}
fn graph_mode_compiler_diagnostic(
    error: &CompilerBridgeErrorV1,
    mode: GraphModeArgs,
) -> Diagnostic {
    if mode.effective_offline() && matches!(&error, CompilerBridgeErrorV1::Cache(_)) {
        return Diagnostic::new(
            ErrorCode::OfflineMiss,
            "an exact dependency archive is unavailable in the authenticated local cache",
        )
        .with_help("rerun the command online to fetch it, or repair a corrupt cache entry first");
    }
    compiler_bridge_diagnostic(error)
}
fn test_runner_diagnostic(error: &WorkspaceTestErrorV1) -> Diagnostic {
    let code = match error {
        WorkspaceTestErrorV1::UnsupportedPlatform => ErrorCode::Io,
        WorkspaceTestErrorV1::Workspace(_) | WorkspaceTestErrorV1::Target(_) => {
            ErrorCode::WorkspaceInvalid
        }
        WorkspaceTestErrorV1::Lock(_) => ErrorCode::LockfileInvalid,
        WorkspaceTestErrorV1::Cache(_) => ErrorCode::CacheCorrupt,
        WorkspaceTestErrorV1::ExternalModules(_) | WorkspaceTestErrorV1::Runner(_) => {
            ErrorCode::Compiler
        }
    };
    Diagnostic::new(code, error.to_string())
}
fn graph_mode_test_diagnostic(error: &WorkspaceTestErrorV1, mode: GraphModeArgs) -> Diagnostic {
    if mode.effective_offline() && matches!(&error, WorkspaceTestErrorV1::Cache(_)) {
        return Diagnostic::new(
            ErrorCode::OfflineMiss,
            "an exact test dependency is unavailable in the authenticated local cache",
        )
        .with_help("rerun `musubi test` online, or repair a corrupt cache entry first");
    }
    test_runner_diagnostic(error)
}
#[allow(
    clippy::too_many_lines,
    reason = "the CLI handler keeps clean-package validation and receipt assembly together"
)]
fn run_package(explicit_manifest: Option<&Path>, args: &PackageArgs) -> CommandResult {
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let previous = read_optional_workspace_lock(&workspace)?;
    let graph = resolve_and_update_workspace_lock(
        &workspace,
        &selected_names,
        args.mode,
        previous,
        None,
        args.registry.config.as_deref(),
        false,
    )?;
    let (cache, archives) = if args.list {
        (None, Vec::new())
    } else {
        let cache = open_user_cache()?;
        let archives = ensure_graph_archives(&cache, &graph, args.mode)?;
        (Some(cache), archives)
    };
    let mut listed = Vec::new();
    let mut packaged = Vec::new();
    let mut output_writer = None;
    for selector in &selected_names {
        let member = workspace
            .members()
            .values()
            .find(|member| &member.package.selector == selector)
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorCode::WorkspaceInvalid,
                    "selected package disappeared from the loaded workspace",
                )
                .with_context("package", selector.to_string())
            })?;
        let structural = graph.bind_selector_namespace(selector)?;
        let release = MusubiReleaseIdV1::new(structural, member.package.version.clone());
        let verification_lock = graph
            .lock
            .verification_lock(selector, release.clone())
            .map_err(|error| lockfile_diagnostic(&lock_path, &error))?;
        let manifest =
            publication_manifest_toml(member).map_err(|error| package_diagnostic(&error))?;
        let layout = package_layout_for_member(workspace.root(), member);
        let plan = plan_package(&layout, &manifest, &verification_lock)
            .map_err(|error| package_diagnostic(&error))?;
        let file_paths = plan
            .files()
            .iter()
            .map(|file| Value::from(file.path().to_owned()))
            .collect::<Vec<_>>();
        if args.list {
            listed.push(object([
                ("package", Value::from(selector.to_string())),
                ("release", Value::from(release.to_string())),
                ("files", Value::Array(file_paths)),
                ("source_bytes", Value::from(plan.source_bytes())),
            ]));
            continue;
        }
        let cache = cache.as_ref().expect("non-list package opens the cache");
        let interface_digest = validate_packaged_plan(
            cache,
            &plan,
            &verification_lock,
            graph.account_chain_discriminant(),
        )
        .map_err(|error| graph_mode_compiler_diagnostic(&error, args.mode))?;
        let semantic = semantic_release_manifest(
            member,
            release.clone(),
            &verification_lock,
            interface_digest,
        )
        .map_err(|error| package_diagnostic(&error))?;
        let car = plan
            .into_car(&semantic, &verification_lock)
            .map_err(|error| package_diagnostic(&error))?;
        let archive_commitment = car
            .archive_commitment()
            .map_err(|error| package_diagnostic(&error))?;
        let publication = publication_claim(
            &semantic,
            &archive_commitment,
            graph.lock.snapshot,
            verification_lock,
        )
        .map_err(|error| package_diagnostic(&error))?;
        if output_writer.is_none() {
            output_writer = Some(package_output_writer(&workspace)?);
        }
        let writer = output_writer
            .as_ref()
            .expect("package output writer was initialized");
        let file_name = format!(
            "{}-{}-{}.car",
            selector.namespace, selector.name, member.package.version
        );
        let relative = PathBuf::from("target/package").join(&file_name);
        writer
            .replace(&relative, car.bytes())
            .map_err(atomic_diagnostic)?;
        packaged.push(object([
            ("package", Value::from(selector.to_string())),
            ("release", Value::from(release.to_string())),
            (
                "path",
                Value::from(workspace.root().join(&relative).display().to_string()),
            ),
            ("files", Value::Array(file_paths)),
            ("source_bytes", Value::from(car.source_bytes())),
            (
                "source_tree_digest",
                Value::from(hex::encode(
                    car.commitments().source_tree_digest().as_bytes(),
                )),
            ),
            (
                "interface_digest",
                Value::from(hex::encode(interface_digest.as_bytes())),
            ),
            ("car_bytes", Value::from(car.stats().car_size)),
            (
                "car_digest",
                Value::from(hex::encode(car.stats().car_archive_digest.as_bytes())),
            ),
            (
                "bundle_digest",
                Value::from(hex::encode(car.commitments().bundle_digest().as_bytes())),
            ),
            (
                "archive_id",
                Value::from(hex::encode(archive_commitment.archive_id().as_bytes())),
            ),
            (
                "release_digest",
                Value::from(hex::encode(
                    publication.manifest.release_digest().as_bytes(),
                )),
            ),
            (
                "chunk_count",
                Value::from(u64::from(archive_commitment.chunk_count)),
            ),
        ]));
    }
    if args.list {
        return Ok(Success {
            message: format!("listed {} clean package(s)", listed.len()),
            data: object([
                ("packages", Value::Array(listed)),
                ("lockfile", Value::from(lock_path.display().to_string())),
            ]),
        });
    }
    Ok(Success {
        message: format!("packaged {} clean archive(s)", packaged.len()),
        data: object([
            ("packages", Value::Array(packaged)),
            ("archives", Value::Array(archives)),
            ("lockfile", Value::from(lock_path.display().to_string())),
        ]),
    })
}
fn package_output_writer(workspace: &Workspace) -> Result<AtomicWriteRoot, Diagnostic> {
    let mut current = workspace.root().to_path_buf();
    for component in ["target", "package"] {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(Diagnostic::new(
                    ErrorCode::Io,
                    "package output ancestor is not a real directory",
                )
                .with_context("path", current.display().to_string()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if let Err(create_error) = fs::create_dir(&current)
                    && create_error.kind() != io::ErrorKind::AlreadyExists
                {
                    return Err(io_diagnostic(
                        "create package output directory",
                        &current,
                        &create_error,
                    ));
                }
                let metadata = fs::symlink_metadata(&current).map_err(|error| {
                    io_diagnostic("inspect package output directory", &current, &error)
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(Diagnostic::new(
                        ErrorCode::Io,
                        "package output ancestor changed during creation",
                    )
                    .with_context("path", current.display().to_string()));
                }
            }
            Err(error) => {
                return Err(io_diagnostic(
                    "inspect package output directory",
                    &current,
                    &error,
                ));
            }
        }
    }
    AtomicWriteRoot::new(workspace.root()).map_err(atomic_diagnostic)
}
fn package_diagnostic(error: &PackageError) -> Diagnostic {
    let code = if matches!(
        error,
        PackageError::UnsupportedPlatform | PackageError::Io { .. }
    ) {
        ErrorCode::Io
    } else {
        ErrorCode::PackageInvalid
    };
    Diagnostic::new(code, error.to_string())
}
#[allow(
    clippy::too_many_lines,
    reason = "publication setup is one security-sensitive validation and staging workflow"
)]
fn run_publish(explicit_manifest: Option<&Path>, args: &PublishArgs) -> CommandResult {
    if let Some(operation_id) = args.recover {
        return recover_publication_sidecars(explicit_manifest, args, operation_id);
    }
    if let Some(operation_id) = args.resume {
        if args.mode.effective_offline() {
            return Err(Diagnostic::new(
                ErrorCode::OfflineMiss,
                "publication resume requires finalized registry and SoraFS evidence",
            )
            .with_context("operation_id", operation_id.to_string()));
        }
        return resume_publication(args, operation_id);
    }
    if args.mode.effective_offline() {
        return Err(Diagnostic::new(
            ErrorCode::OfflineMiss,
            "publication requires authenticated seed ingress and finalized registry evidence",
        ));
    }
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let [selector] = selected_names.as_slice() else {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "publish requires exactly one selected workspace package",
        )
        .with_context("selected_packages", selected_names.len().to_string())
        .with_help("select one package with `-p namespace/package`"));
    };
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let previous = read_optional_workspace_lock(&workspace)?;
    let graph = resolve_and_update_workspace_lock(
        &workspace,
        &selected_names,
        args.mode,
        previous,
        None,
        args.network.config.as_deref(),
        true,
    )?;
    let member = workspace
        .members()
        .values()
        .find(|member| &member.package.selector == selector)
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::WorkspaceInvalid,
                "selected package disappeared from the loaded workspace",
            )
            .with_context("package", selector.to_string())
        })?;
    let structural = graph.bind_selector_namespace(selector)?;
    let expected_governance_revision = graph
        .online_registry()?
        .exact_package(structural.clone())
        .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?
        .map(|package| package.revisions.governance);
    let release = MusubiReleaseIdV1::new(structural, member.package.version.clone());
    let verification_lock = graph
        .lock
        .verification_lock(selector, release.clone())
        .map_err(|error| lockfile_diagnostic(&lock_path, &error))?;
    let manifest = publication_manifest_toml(member).map_err(|error| package_diagnostic(&error))?;
    let layout = package_layout_for_member(workspace.root(), member);
    let plan = plan_package(&layout, &manifest, &verification_lock)
        .map_err(|error| package_diagnostic(&error))?;
    let cache = open_user_cache()?;
    ensure_graph_archives(&cache, &graph, args.mode)?;
    let interface_digest = validate_packaged_plan(
        &cache,
        &plan,
        &verification_lock,
        graph.account_chain_discriminant(),
    )
    .map_err(|error| compiler_bridge_diagnostic(&error))?;
    let semantic = semantic_release_manifest(member, release, &verification_lock, interface_digest)
        .map_err(|error| package_diagnostic(&error))?;
    let car = plan
        .into_car(&semantic, &verification_lock)
        .map_err(|error| package_diagnostic(&error))?;
    let archive_commitment = car
        .archive_commitment()
        .map_err(|error| package_diagnostic(&error))?;
    let publication = publication_claim(
        &semantic,
        &archive_commitment,
        graph.lock.snapshot,
        verification_lock,
    )
    .map_err(|error| package_diagnostic(&error))?;
    let compiler_output_digest = publication_compiler_output_digest(
        interface_digest,
        publication.manifest.release_digest(),
        publication.manifest.verification_lock_digest,
    );
    let expected_interface_digest = interface_digest;
    let validator = move |operation_id: PublicationOperationIdV1,
                          request: &PublicationRequestV1,
                          input: &mut dyn Read| {
        validate_prepared_publication_car(
            operation_id,
            request,
            input,
            expected_interface_digest,
            compiler_output_digest,
        )
    };
    let platform_config_provenance =
        graph.platform_config_provenance.as_ref().ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::Publish,
                "publication configuration provenance is unavailable",
            )
        })?;
    let loaded =
        load_bound_production_publication_runtime_v1(platform_config_provenance, validator)
            .map_err(publication_configuration_diagnostic)?;
    let registry = loaded.registry_reader();
    let bindings = loaded.bindings().clone();
    let (signing, mut services, _) = loaded.into_parts();
    if signing.network_id() != graph.lock.network_id {
        return Err(Diagnostic::new(
            ErrorCode::Publish,
            "signing configuration belongs to a different network than the resolved graph",
        ));
    }
    let request = PublicationRequestV1 {
        network_id: graph.lock.network_id,
        publisher: signing.authority().clone(),
        ingress_broker: bindings.ingress_broker,
        seed_provider: bindings.seed_provider,
        namespace: selector.namespace.clone(),
        publication,
        archive_commitment: archive_commitment.clone(),
        namespace_delegation: bindings.namespace_delegation,
        expected_policy_revision: bindings.expected_policy_revision,
        expected_governance_revision,
        nonce: unpredictable_publication_nonce(),
    };
    request
        .validate()
        .map_err(|error| publication_diagnostic(&error))?;
    let operation_id = request.operation_id();
    let state_root = publication_state_root()?;
    let store = PublicationJournalStore::open(&state_root)
        .map_err(|error| publication_diagnostic(&error))?;
    services
        .bind_publication_state_root(&state_root)
        .map_err(publication_configuration_diagnostic)?;
    let engine = PublicationEngine::new(&store);
    let (_, source) = engine
        .begin_detached_with_car(request.clone(), car.plan(), car.bytes())
        .map_err(|error| {
            let diagnostic = publication_diagnostic(&error)
                .with_context("operation_id", operation_id.to_string());
            match store.load(operation_id) {
                Ok(journal) if journal.request == request => {
                    if journal.phase == crate::publish::PublicationPhaseV1::Validation
                        && journal.revision == 1
                    {
                        diagnostic.with_help(format!(
                            "a matching pristine request journal is durable; after correcting local packaging or storage, run `musubi publish --recover {operation_id}` and then resume it"
                        ))
                    } else {
                        diagnostic.with_help(format!(
                            "the matching operation has already advanced; continue with `musubi publish --resume {operation_id}`"
                        ))
                    }
                }
                Ok(_) | Err(_) => diagnostic,
            }
        })?;
    let mut backend = RegistryPublicationBackendV1::new(registry, signing, services, &request)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?;
    match engine
        .advance_once(operation_id, &source, &mut backend)
        .map_err(|error| publication_diagnostic(&error))?
    {
        PublicationAdvanceV1::Complete(result) => {
            return Ok(publication_result(&request.namespace, result));
        }
        PublicationAdvanceV1::Progressed(crate::publish::PublicationPhaseV1::SeedIngress) => {}
        PublicationAdvanceV1::Pending(phase) | PublicationAdvanceV1::Progressed(phase) => {
            return Err(Diagnostic::new(
                ErrorCode::Publish,
                "publication validation did not reach the durable seed-ingress boundary",
            )
            .with_context("operation_id", operation_id.to_string())
            .with_context("phase", format!("{phase:?}")));
        }
    }
    if args.detach {
        return Ok(detached_publication_result(
            &request.namespace,
            &request.publication.manifest.release,
            operation_id,
        ));
    }
    finish_publication(
        &engine,
        operation_id,
        &request.namespace,
        &source,
        &mut backend,
    )
}
#[allow(
    clippy::too_many_lines,
    reason = "recovery keeps the journal-derived clean rebuild and exact sidecar comparison adjacent"
)]
fn recover_publication_sidecars(
    explicit_manifest: Option<&Path>,
    args: &PublishArgs,
    operation_id: PublicationOperationIdV1,
) -> CommandResult {
    if args.selection.workspace
        || !args.selection.packages.is_empty()
        || !args.selection.exclude.is_empty()
    {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "publication recovery derives its exact package selection from the journal",
        )
        .with_context("operation_id", operation_id.to_string())
        .with_help("remove `--workspace`, `--exclude`, and `-p/--package` from `--recover`"));
    }
    let state_root = publication_state_root()?;
    recover_publication_sidecars_at(explicit_manifest, args, operation_id, &state_root, None)
}
#[allow(
    clippy::too_many_lines,
    reason = "recovery keeps the journal-derived clean rebuild and exact sidecar comparison adjacent"
)]
fn recover_publication_sidecars_at(
    explicit_manifest: Option<&Path>,
    args: &PublishArgs,
    operation_id: PublicationOperationIdV1,
    state_root: &Path,
    injected_cache: Option<&MusubiCache>,
) -> CommandResult {
    let store = PublicationJournalStore::open(state_root)
        .map_err(|error| publication_diagnostic(&error))?;
    let journal = store.load(operation_id).map_err(|error| {
        publication_diagnostic(&error).with_context("operation_id", operation_id.to_string())
    })?;
    if journal.phase != crate::publish::PublicationPhaseV1::Validation || journal.revision != 1 {
        return Err(publication_diagnostic(&PublicationError::InvalidJournal(
            "pre-ingress sidecar recovery requires the pristine validation revision".to_owned(),
        ))
        .with_context("operation_id", operation_id.to_string())
        .with_help("advanced publication operations must continue with `musubi publish --resume OPERATION_ID`"));
    }
    let expected_release = &journal.request.publication.manifest.release;
    let selector = MusubiPackageSelectorV1 {
        namespace: journal.request.namespace.clone(),
        name: expected_release.package.name.clone(),
    };
    let manifest_path = project_manifest_path(explicit_manifest)?;
    let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
    let member = workspace
        .members()
        .values()
        .find(|member| member.package.selector == selector)
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::WorkspaceInvalid,
                "the journal package is not a member of the selected workspace",
            )
            .with_context("operation_id", operation_id.to_string())
            .with_context("package", selector.to_string())
        })?;
    if member.package.version != expected_release.version {
        return Err(Diagnostic::new(
            ErrorCode::WorkspaceInvalid,
            "the workspace package version differs from the immutable recovery journal",
        )
        .with_context("operation_id", operation_id.to_string())
        .with_context("package", selector.to_string())
        .with_context("journal_version", expected_release.version.to_string())
        .with_context("workspace_version", member.package.version.to_string()));
    }
    let verification_lock = journal.request.publication.resolution.lock.clone();
    let graph_lock = LockfileV1::new(
        journal.request.network_id(),
        journal.request.publication.resolution.snapshot,
        vec![crate::lockfile::LockedRootV1 {
            package: selector.clone(),
            dependencies: verification_lock.root_dependencies.clone(),
        }],
        verification_lock.nodes.clone(),
    )
    .map_err(|error| {
        Diagnostic::new(
            ErrorCode::Publish,
            "the journal exact verification graph cannot be reconstructed safely",
        )
        .with_context("operation_id", operation_id.to_string())
        .with_context("reason", error.to_string())
    })?;
    let manifest = publication_manifest_toml(member).map_err(|error| package_diagnostic(&error))?;
    let layout = package_layout_for_member(workspace.root(), member);
    let plan = plan_package(&layout, &manifest, &verification_lock)
        .map_err(|error| package_diagnostic(&error))?;
    let (registry, config_image) =
        RegistryReadClientV1::load_with_config_image(args.network.config.as_deref())
            .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?;
    let prepared_archive_fetch =
        prepare_production_archive_transport_v1(config_image.path(), config_image.bytes());
    drop(config_image);
    let account_chain_discriminant = registry.account_chain_discriminant();
    let graph = ResolvedWorkspaceGraphV1 {
        lock: graph_lock,
        registry: Some(registry),
        cached_source: None,
        prepared_archive_fetch: Some(prepared_archive_fetch),
        platform_config_provenance: None,
        account_chain_discriminant,
    };
    let platform_cache;
    let cache = if let Some(cache) = injected_cache {
        cache
    } else {
        platform_cache = open_user_cache()?;
        &platform_cache
    };
    ensure_graph_archives(cache, &graph, args.mode)?;
    let interface_digest = validate_packaged_plan(
        cache,
        &plan,
        &verification_lock,
        graph.account_chain_discriminant(),
    )
    .map_err(|error| graph_mode_compiler_diagnostic(&error, args.mode))?;
    let semantic = semantic_release_manifest(
        member,
        expected_release.clone(),
        &verification_lock,
        interface_digest,
    )
    .map_err(|error| package_diagnostic(&error))?;
    let car = plan
        .into_car(&semantic, &verification_lock)
        .map_err(|error| package_diagnostic(&error))?;
    let archive_commitment = car
        .archive_commitment()
        .map_err(|error| package_diagnostic(&error))?;
    let publication = publication_claim(
        &semantic,
        &archive_commitment,
        journal.request.publication.resolution.snapshot,
        verification_lock,
    )
    .map_err(|error| package_diagnostic(&error))?;
    let engine = PublicationEngine::new(&store);
    engine
        .recover_pre_ingress_sidecars(
            &journal,
            &publication,
            &archive_commitment,
            car.plan(),
            car.bytes(),
        )
        .map_err(|error| {
            publication_diagnostic(&error).with_context("operation_id", operation_id.to_string())
        })?;
    Ok(recovered_publication_result(
        &journal.request.namespace,
        expected_release,
        operation_id,
    ))
}
fn resume_publication(args: &PublishArgs, operation_id: PublicationOperationIdV1) -> CommandResult {
    let state_root = publication_state_root()?;
    let store = PublicationJournalStore::open(&state_root)
        .map_err(|error| publication_diagnostic(&error))?;
    let journal = store
        .load(operation_id)
        .map_err(|error| publication_diagnostic(&error))?;
    let loaded = load_production_publication_runtime_v1(
        args.network.config.as_deref(),
        validate_resumable_publication_car,
    )
    .map_err(publication_configuration_diagnostic)?;
    let reader = loaded.registry_reader();
    let (signer, mut services, _) = loaded.into_parts();
    services
        .bind_publication_state_root(&state_root)
        .map_err(publication_configuration_diagnostic)?;
    let mut backend = RegistryPublicationBackendV1::new(reader, signer, services, &journal.request)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?;
    let source = PublicationStagedCarSourceV1::new(
        &state_root,
        operation_id,
        journal.request.archive_commitment.car_size,
    );
    let engine = PublicationEngine::new(&store);
    finish_publication(
        &engine,
        operation_id,
        &journal.request.namespace,
        &source,
        &mut backend,
    )
}
fn finish_publication(
    engine: &PublicationEngine<'_>,
    operation_id: PublicationOperationIdV1,
    namespace: &MusubiNamespaceV1,
    source: &PublicationStagedCarSourceV1,
    backend: &mut dyn crate::publish::PublicationBackend,
) -> CommandResult {
    match resume_with_bounded_polling(
        engine,
        operation_id,
        source,
        backend,
        PublicationPollPolicyV1::default(),
    )
    .map_err(|error| publication_diagnostic(&error))?
    {
        PublicationAdvanceV1::Complete(result) => Ok(publication_result(namespace, result)),
        PublicationAdvanceV1::Pending(phase) | PublicationAdvanceV1::Progressed(phase) => {
            Err(Diagnostic::new(
                ErrorCode::Publish,
                "publication did not reach exact finalized release verification",
            )
            .with_context("operation_id", operation_id.to_string())
            .with_context("phase", format!("{phase:?}"))
            .with_help("rerun `musubi publish --resume OPERATION_ID` to continue safely"))
        }
    }
}
fn recovered_publication_result(
    namespace: &MusubiNamespaceV1,
    release: &MusubiReleaseIdV1,
    operation_id: PublicationOperationIdV1,
) -> Success {
    Success {
        message: format!("recovered publication sidecars {operation_id}"),
        data: object([
            ("status", Value::from("recovered")),
            ("operation_id", Value::from(operation_id.to_string())),
            (
                "release",
                Value::from(namespaced_release(namespace, release)),
            ),
            ("structural_release", Value::from(release.to_string())),
            ("phase", Value::from("validation")),
            (
                "next",
                Value::from(format!("musubi publish --resume {operation_id}")),
            ),
        ]),
    }
}
fn detached_publication_result(
    namespace: &MusubiNamespaceV1,
    release: &MusubiReleaseIdV1,
    operation_id: PublicationOperationIdV1,
) -> Success {
    Success {
        message: format!("prepared detached publication {operation_id}"),
        data: object([
            ("status", Value::from("detached")),
            ("operation_id", Value::from(operation_id.to_string())),
            (
                "release",
                Value::from(namespaced_release(namespace, release)),
            ),
            ("structural_release", Value::from(release.to_string())),
            ("phase", Value::from("seed-ingress")),
        ]),
    }
}
fn publication_result(namespace: &MusubiNamespaceV1, result: PublicationResultV1) -> Success {
    let PublicationResultV1 {
        operation_id,
        submission,
        final_checkpoint: checkpoint,
    } = result;
    Success {
        message: format!(
            "published {}",
            namespaced_release(namespace, &checkpoint.release)
        ),
        data: object([
            ("status", Value::from("complete")),
            ("operation_id", Value::from(operation_id.to_string())),
            (
                "release",
                Value::from(namespaced_release(namespace, &checkpoint.release)),
            ),
            (
                "structural_release",
                Value::from(checkpoint.release.to_string()),
            ),
            ("network_id", Value::from(checkpoint.network_id.to_string())),
            (
                "snapshot",
                object([
                    (
                        "finalized_height",
                        Value::from(checkpoint.snapshot.finalized_height),
                    ),
                    (
                        "finalized_block_hash",
                        Value::from(hex::encode(checkpoint.snapshot.finalized_block_hash)),
                    ),
                    (
                        "index_revision",
                        Value::from(checkpoint.snapshot.index_revision),
                    ),
                ]),
            ),
            (
                "release_digest",
                Value::from(hex::encode(checkpoint.release_digest.as_bytes())),
            ),
            (
                "archive_id",
                Value::from(hex::encode(checkpoint.archive_id.as_bytes())),
            ),
            (
                "home_release_digest",
                Value::from(hex::encode(checkpoint.home_release_digest)),
            ),
            (
                "universal_release_digest",
                Value::from(hex::encode(checkpoint.universal_release_digest)),
            ),
            (
                "checkpoint_digest",
                Value::from(hex::encode(checkpoint.checkpoint_digest)),
            ),
            (
                "amx_submission",
                object([
                    (
                        "instruction_digest",
                        Value::from(hex::encode(submission.instruction_digest)),
                    ),
                    (
                        "transaction_hash",
                        Value::from(hex::encode(submission.transaction_hash)),
                    ),
                    ("applied_height", Value::from(submission.applied_height)),
                ]),
            ),
        ]),
    }
}
fn namespaced_release(namespace: &MusubiNamespaceV1, release: &MusubiReleaseIdV1) -> String {
    format!("{namespace}/{}@{}", release.package.name, release.version)
}
fn publication_compiler_output_digest(
    interface_digest: MusubiContentDigestV1,
    release_digest: iroha_data_model::musubi::MusubiReleaseDigestV1,
    verification_lock_digest: iroha_data_model::musubi::MusubiVerificationLockDigestV1,
) -> MusubiContentDigestV1 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"musubi-publication-compiler-output-v1\0");
    hasher.update(interface_digest.as_bytes());
    hasher.update(release_digest.as_bytes());
    hasher.update(verification_lock_digest.as_bytes());
    MusubiContentDigestV1::new(*hasher.finalize().as_bytes())
}
fn validate_prepared_publication_car(
    operation_id: PublicationOperationIdV1,
    request: &PublicationRequestV1,
    input: &mut dyn Read,
    expected_interface_digest: MusubiContentDigestV1,
    compiler_output_digest: MusubiContentDigestV1,
) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
    if operation_id != request.operation_id()
        || request.publication.manifest.interface_digest != expected_interface_digest
    {
        return Err(PublicationBackendError::permanent(
            "PACKAGE_VALIDATION_BINDING_MISMATCH",
        ));
    }
    validate_prepared_car_stream(
        input,
        request.archive_commitment.car_size,
        request.archive_commitment.car_digest,
    )?;
    Ok(PublicationValidationEvidenceV1 {
        archive_id: request.archive_commitment.archive_id(),
        semantic_release_digest: request.publication.manifest.semantic_digest(),
        release_digest: request.publication.manifest.release_digest(),
        source_tree_digest: request.archive_commitment.source_tree_digest,
        descriptor_digest: request.archive_commitment.descriptor_digest,
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
        car_digest: request.archive_commitment.car_digest,
        car_size: request.archive_commitment.car_size,
        compiler_output_digest,
        resolution_snapshot: request.publication.resolution.snapshot,
    })
}
fn validate_resumable_publication_car(
    operation_id: PublicationOperationIdV1,
    request: &PublicationRequestV1,
    input: &mut dyn Read,
) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
    let manifest = &request.publication.manifest;
    validate_prepared_publication_car(
        operation_id,
        request,
        input,
        manifest.interface_digest,
        publication_compiler_output_digest(
            manifest.interface_digest,
            manifest.release_digest(),
            manifest.verification_lock_digest,
        ),
    )
}
fn validate_prepared_car_stream(
    input: &mut dyn Read,
    expected_size: u64,
    expected_digest: MusubiContentDigestV1,
) -> Result<(), PublicationBackendError> {
    let mut size = 0_u64;
    let mut digest = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|_| PublicationBackendError::retryable("PACKAGE_VALIDATION_READ_FAILED"))?;
        if read == 0 {
            break;
        }
        size = size
            .checked_add(u64::try_from(read).expect("read buffer length fits u64"))
            .ok_or_else(|| {
                PublicationBackendError::permanent("PACKAGE_VALIDATION_LENGTH_INVALID")
            })?;
        if size > expected_size {
            return Err(PublicationBackendError::permanent(
                "PACKAGE_VALIDATION_LENGTH_INVALID",
            ));
        }
        digest.update(&buffer[..read]);
    }
    if size != expected_size || digest.finalize().as_bytes() != expected_digest.as_bytes() {
        return Err(PublicationBackendError::permanent(
            "PACKAGE_VALIDATION_CAR_MISMATCH",
        ));
    }
    Ok(())
}
fn unpredictable_publication_nonce() -> [u8; 32] {
    loop {
        let key_pair = iroha::crypto::KeyPair::random();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"musubi-publication-nonce-v1\0");
        hasher.update(key_pair.public_key().to_string().as_bytes());
        let nonce = *hasher.finalize().as_bytes();
        if nonce.iter().any(|byte| *byte != 0) {
            return nonce;
        }
    }
}
fn publication_configuration_diagnostic(
    error: crate::publication_runtime::ProductionPublicationConfigurationErrorV1,
) -> Diagnostic {
    let diagnostic = Diagnostic::new(
        ErrorCode::Publish,
        if error.code() == "MUSUBI_PUBLICATION_CONFIG_CHANGED" {
            "platform client configuration changed during publication preparation"
        } else {
            "production publication services are not configured"
        },
    )
    .with_context("publication_code", error.code());
    if error.code() == "MUSUBI_PUBLICATION_CONFIG_CHANGED" {
        diagnostic.with_help(
            "rerun publish so dependency resolution and authenticated publication use one configuration image",
        )
    } else {
        diagnostic
    }
}
fn publication_state_root() -> Result<PathBuf, Diagnostic> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Iroha").join("musubi"));
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME").map(PathBuf::from).map(|path| {
        path.join("Library")
            .join("Application Support")
            .join("Iroha")
            .join("musubi")
    });
    #[cfg(all(unix, not(target_os = "macos")))]
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .map(|path| path.join("iroha").join("musubi"))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".local/state/iroha/musubi"))
        });
    #[cfg(not(any(unix, target_os = "windows")))]
    let root: Option<PathBuf> = None;
    let requested = root.ok_or_else(|| {
        Diagnostic::new(
            ErrorCode::Io,
            "platform user state directory is unavailable",
        )
    })?;
    AtomicWriteRoot::open_or_create_private(&requested)
        .map(|root| root.path().to_path_buf())
        .map_err(atomic_diagnostic)
}
fn publication_diagnostic(error: &PublicationError) -> Diagnostic {
    let public_code = match error {
        PublicationError::Backend(backend) => backend.code(),
        PublicationError::CarSource(_) => "PUBLICATION_SIDECAR_UNAVAILABLE",
        PublicationError::NotFound(_) => "PUBLICATION_OPERATION_NOT_FOUND",
        PublicationError::ConcurrentJournalUpdate => "PUBLICATION_CONCURRENT_RESUME",
        PublicationError::InvalidEvidence { .. } => "PUBLICATION_EVIDENCE_INVALID",
        PublicationError::InvalidJournal(_) => "PUBLICATION_JOURNAL_INVALID",
        PublicationError::JournalWrite(error)
            if matches!(
                error.code(),
                AtomicWriteErrorCode::ImmutableConflict | AtomicWriteErrorCode::UnsafeTarget
            ) =>
        {
            "PUBLICATION_STATE_INTEGRITY_INVALID"
        }
        PublicationError::JournalWrite(_) | PublicationError::JournalIo(_) => {
            "PUBLICATION_JOURNAL_IO"
        }
    };
    Diagnostic::new(ErrorCode::Publish, "publication operation failed")
        .with_context("publication_code", public_code)
}
fn run_search(args: &SearchArgs) -> CommandResult {
    let registry = load_registry_reader(&args.network)?;
    let page = registry
        .search(&MusubiSearchQueryV1 {
            query: args.query.clone(),
            page: MusubiSearchPageRequestV1 {
                limit: args.limit,
                cursor: None,
            },
        })
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
    Ok(Success {
        message: format!("{} finalized search result(s)", page.items.len()),
        data: registry_json(&page)?,
    })
}
fn run_package_info(args: &PackageQueryArgs) -> CommandResult {
    let registry = load_registry_reader(&args.network)?;
    let package = registry
        .resolve_selector(&args.package)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
    let record = registry
        .exact_package(package)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?
        .ok_or_else(|| {
            Diagnostic::new(ErrorCode::Registry, "canonical package was not found")
                .with_context("package", args.package.to_string())
        })?;
    Ok(Success {
        message: format!("{}", args.package),
        data: registry_json(&record)?,
    })
}
fn run_package_versions(args: &PackageQueryArgs) -> CommandResult {
    let registry = load_registry_reader(&args.network)?;
    let package = registry
        .resolve_selector(&args.package)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
    let page = registry
        .versions(&MusubiPackagePageQueryV1 {
            package,
            page: MusubiPageRequestV1 {
                limit: 50,
                cursor: None,
            },
        })
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
    Ok(Success {
        message: format!(
            "{} finalized version(s) for {}",
            page.items.len(),
            args.package
        ),
        data: registry_json(&page)?,
    })
}
fn run_release_yank(args: &ReleaseMutationArgs, yanked: bool) -> CommandResult {
    require_nonzero_revision(args.expected_revision, "expected yank revision")?;
    let registry = load_registry_reader(&args.network)?;
    let package = registry
        .resolve_selector(&args.package)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
    let release = MusubiReleaseIdV1::new(package, args.version.clone());
    let reason = args.reason.clone().unwrap_or_else(|| {
        if yanked {
            "package maintainer requested yank"
        } else {
            "package maintainer requested unyank"
        }
        .parse()
        .expect("built-in Musubi reason is valid")
    });
    let signer = RegistrySigningClientV1::load(args.network.config.as_deref())
        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
    let transaction_hash = signer
        .submit_v1(SetMusubiReleaseYankV1::new(
            release.clone(),
            yanked,
            reason,
            args.expected_revision,
        ))
        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
    Ok(Success {
        message: format!("{} {release}", if yanked { "yanked" } else { "unyanked" }),
        data: Value::Object(Map::from_iter([
            ("release".to_owned(), Value::from(release.to_string())),
            ("yanked".to_owned(), Value::from(yanked)),
            (
                "transaction_hash".to_owned(),
                Value::from(hex::encode(transaction_hash)),
            ),
        ])),
    })
}
#[allow(
    clippy::too_many_lines,
    reason = "the command preserves one explicit match arm per governance operation"
)]
fn run_owner(args: &OwnerArgs) -> CommandResult {
    match &args.command {
        OwnerCommand::Invite {
            package,
            account,
            role,
            invitation,
            expires_at_height,
            permissions,
            expected_revision,
            network,
        } => {
            require_nonzero_revision(*expected_revision, "expected governance revision")?;
            if *expires_at_height == 0 {
                return Err(Diagnostic::new(
                    ErrorCode::Usage,
                    "invitation expiry height must be non-zero",
                ));
            }
            let role = owner_role(*role, *permissions)?;
            let invite_id = parse_invite_id(invitation)?;
            let reader = RegistryReadClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let package = reader
                .resolve_selector(package)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let signer = RegistrySigningClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let account = signer
                .parse_account_id(account)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let transaction_hash = signer
                .submit_v1(InviteMusubiPackageMaintainerV1 {
                    package: package.clone(),
                    invite_id,
                    invited_account: account.clone(),
                    role,
                    expires_at_height: *expires_at_height,
                    expected_governance_revision: *expected_revision,
                })
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            Ok(Success {
                message: format!("invited {account} to {package}"),
                data: object([
                    ("package", Value::from(package.to_string())),
                    ("account", Value::from(account.to_string())),
                    ("invitation", Value::from(hex::encode(invite_id.as_bytes()))),
                    ("expires_at_height", Value::from(*expires_at_height)),
                    (
                        "transaction_hash",
                        Value::from(hex::encode(transaction_hash)),
                    ),
                ]),
            })
        }
        OwnerCommand::SetRole {
            package,
            account,
            role,
            permissions,
            expected_revision,
            network,
        } => {
            require_nonzero_revision(*expected_revision, "expected governance revision")?;
            let role = owner_role(*role, *permissions)?;
            let reader = RegistryReadClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let package = reader
                .resolve_selector(package)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let signer = RegistrySigningClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let account = signer
                .parse_account_id(account)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let transaction_hash = signer
                .submit_v1(SetMusubiPackageMaintainerRoleV1 {
                    package: package.clone(),
                    account: account.clone(),
                    role,
                    expected_governance_revision: *expected_revision,
                })
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            Ok(Success {
                message: format!("updated {account} in {package}"),
                data: owner_mutation_json(&package, Some(&account), transaction_hash),
            })
        }
        OwnerCommand::Accept {
            package,
            invitation,
            expected_revision,
            network,
        } => {
            require_nonzero_revision(*expected_revision, "expected governance revision")?;
            let invite_id = parse_invite_id(invitation)?;
            let reader = RegistryReadClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let package = reader
                .resolve_selector(package)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let signer = RegistrySigningClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let transaction_hash = signer
                .submit_v1(AcceptMusubiPackageMaintainerV1 {
                    package: package.clone(),
                    invite_id,
                    expected_governance_revision: *expected_revision,
                })
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            Ok(Success {
                message: format!("accepted invitation for {package}"),
                data: object([
                    ("package", Value::from(package.to_string())),
                    ("invitation", Value::from(hex::encode(invite_id.as_bytes()))),
                    (
                        "transaction_hash",
                        Value::from(hex::encode(transaction_hash)),
                    ),
                ]),
            })
        }
        OwnerCommand::List { package, network } => run_owner_list(package, network),
        OwnerCommand::Remove {
            package,
            account,
            invitation,
            expected_revision,
            network,
        } => {
            require_nonzero_revision(*expected_revision, "expected governance revision")?;
            let reader = RegistryReadClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let package = reader
                .resolve_selector(package)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let signer = RegistrySigningClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            match (account.as_deref(), invitation.as_deref()) {
                (Some(account), None) => {
                    let account = signer
                        .parse_account_id(account)
                        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
                    let transaction_hash = signer
                        .submit_v1(RemoveMusubiPackageMaintainerV1 {
                            package: package.clone(),
                            account: account.clone(),
                            expected_governance_revision: *expected_revision,
                        })
                        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
                    Ok(Success {
                        message: format!("removed {account} from {package}"),
                        data: owner_mutation_json(&package, Some(&account), transaction_hash),
                    })
                }
                (None, Some(invitation)) => {
                    let invite_id = parse_invite_id(invitation)?;
                    let transaction_hash = signer
                        .submit_v1(RevokeMusubiPackageMaintainerInvitationV1 {
                            package: package.clone(),
                            invite_id,
                            expected_governance_revision: *expected_revision,
                        })
                        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
                    Ok(Success {
                        message: format!("revoked invitation for {package}"),
                        data: object([
                            ("package", Value::from(package.to_string())),
                            ("invitation", Value::from(hex::encode(invite_id.as_bytes()))),
                            (
                                "transaction_hash",
                                Value::from(hex::encode(transaction_hash)),
                            ),
                        ]),
                    })
                }
                _ => Err(Diagnostic::new(
                    ErrorCode::Usage,
                    "owner remove requires exactly one account or --invitation",
                )),
            }
        }
    }
}
fn owner_role(
    role: RoleArg,
    permissions: MaintainerPermissionArgs,
) -> Result<MusubiPackageRoleV1, Diagnostic> {
    let permissions = MusubiMaintainerPermissionsV1 {
        publish: permissions.publish,
        yank: permissions.yank,
        metadata: permissions.metadata,
        archive_locations: permissions.archive_locations,
    };
    match role {
        RoleArg::Owner if permissions.is_empty() => Ok(MusubiPackageRoleV1::Owner),
        RoleArg::Owner => Err(Diagnostic::new(
            ErrorCode::Usage,
            "owner roles do not accept maintainer permission flags",
        )),
        RoleArg::Maintainer if permissions.is_empty() => Err(Diagnostic::new(
            ErrorCode::Usage,
            "maintainer roles require at least one explicit permission",
        )),
        RoleArg::Maintainer => Ok(MusubiPackageRoleV1::Maintainer(permissions)),
    }
}
fn parse_invite_id(raw: &str) -> Result<MusubiInviteIdV1, Diagnostic> {
    if raw.len() != 64
        || raw
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "invitation id must be 64 lowercase hexadecimal digits",
        ));
    }
    let bytes = hex::decode(raw)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .filter(|bytes| bytes.iter().any(|byte| *byte != 0))
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::Usage,
                "invitation id must be non-zero canonical hexadecimal",
            )
        })?;
    Ok(MusubiInviteIdV1::new(bytes))
}
fn require_nonzero_revision(revision: u64, label: &str) -> Result<(), Diagnostic> {
    if revision == 0 {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            format!("{label} must be non-zero"),
        ));
    }
    Ok(())
}
fn owner_mutation_json(
    package: &iroha_data_model::musubi::MusubiPackageIdV1,
    account: Option<&iroha_data_model::account::AccountId>,
    transaction_hash: [u8; 32],
) -> Value {
    let mut map = Map::new();
    map.insert("package".to_owned(), Value::from(package.to_string()));
    if let Some(account) = account {
        map.insert("account".to_owned(), Value::from(account.to_string()));
    }
    map.insert(
        "transaction_hash".to_owned(),
        Value::from(hex::encode(transaction_hash)),
    );
    Value::Object(map)
}
fn run_owner_list(selector: &MusubiPackageSelectorV1, network: &NetworkArgs) -> CommandResult {
    let registry = load_registry_reader(network)?;
    let package = registry
        .resolve_selector(selector)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
    let maximum_entries = MUSUBI_MAX_PACKAGE_MEMBERS_V1
        .checked_add(MUSUBI_MAX_PENDING_INVITATIONS_V1)
        .expect("Musubi member-directory bounds fit usize");
    let page_limit = u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1)
        .expect("Musubi page maximum fits the wire limit field");
    let mut cursor = None;
    let mut seen_cursor_keys = BTreeSet::new();
    let mut entries = Vec::new();
    let mut snapshot = None;
    loop {
        let page = registry
            .maintainers(&MusubiPackagePageQueryV1 {
                package: package.clone(),
                page: MusubiPageRequestV1 {
                    limit: page_limit,
                    cursor,
                },
            })
            .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
        if snapshot
            .as_ref()
            .is_some_and(|expected| expected != &page.snapshot)
        {
            return Err(Diagnostic::new(
                ErrorCode::Registry,
                "maintainer directory pages crossed finalized snapshots",
            ));
        }
        snapshot.get_or_insert(page.snapshot);
        entries.extend(page.items);
        if entries.len() > maximum_entries {
            return Err(Diagnostic::new(
                ErrorCode::Registry,
                "maintainer directory exceeds its consensus entry bound",
            )
            .with_context("package", package.to_string()));
        }
        let Some(next) = page.next_cursor else {
            break;
        };
        if !seen_cursor_keys.insert(next.last_key.clone()) {
            return Err(Diagnostic::new(
                ErrorCode::Registry,
                "maintainer directory pagination did not advance",
            )
            .with_context("package", package.to_string()));
        }
        cursor = Some(next);
    }
    let accepted = entries
        .iter()
        .filter(|entry| matches!(entry, MusubiMaintainerDirectoryEntryV1::Accepted(_)))
        .count();
    let pending = entries.len().saturating_sub(accepted);
    let entries = entries
        .iter()
        .map(registry_json)
        .collect::<Result<Vec<_>, _>>()?;
    let snapshot = snapshot
        .as_ref()
        .map(registry_json)
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(Success {
        message: format!("{accepted} accepted member(s), {pending} pending invitation(s)"),
        data: object([
            ("package", Value::from(package.to_string())),
            ("accepted", Value::from(accepted as u64)),
            ("pending", Value::from(pending as u64)),
            ("entries", Value::Array(entries)),
            ("snapshot", snapshot),
        ]),
    })
}
fn run_alias(args: &AliasArgs) -> CommandResult {
    match &args.command {
        AliasCommand::Register {
            alias,
            package,
            expected_price_revision,
            network,
        } => {
            require_nonzero_revision(*expected_price_revision, "expected pricing-policy revision")?;
            let registry = load_registry_reader(network)?;
            let target = registry
                .resolve_selector(package)
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let signer = RegistrySigningClientV1::load(network.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            let transaction_hash = signer
                .submit_v1(RegisterMusubiAliasV1::new(
                    alias.clone(),
                    target.clone(),
                    *expected_price_revision,
                ))
                .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
            Ok(Success {
                message: format!("registered permanent alias {alias} -> {package}"),
                data: Value::Object(Map::from_iter([
                    ("alias".to_owned(), Value::from(alias.to_string())),
                    ("target".to_owned(), Value::from(target.to_string())),
                    (
                        "transaction_hash".to_owned(),
                        Value::from(hex::encode(transaction_hash)),
                    ),
                ])),
            })
        }
        AliasCommand::Resolve { alias, network } | AliasCommand::Info { alias, network } => {
            let registry = load_registry_reader(network)?;
            let record = registry
                .alias(&MusubiAliasQueryV1 {
                    alias: alias.clone(),
                    page: MusubiPageRequestV1 {
                        limit: 0,
                        cursor: None,
                    },
                })
                .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?
                .ok_or_else(|| {
                    Diagnostic::new(ErrorCode::Registry, "permanent alias was not found")
                        .with_context("alias", alias.to_string())
                })?;
            Ok(Success {
                message: format!("{alias} -> {}", record.target),
                data: registry_json(&record)?,
            })
        }
        AliasCommand::History { alias, network } => {
            let registry = load_registry_reader(network)?;
            let page = registry
                .alias_history(&MusubiAliasQueryV1 {
                    alias: alias.clone(),
                    page: MusubiPageRequestV1 {
                        limit: 50,
                        cursor: None,
                    },
                })
                .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
            Ok(Success {
                message: format!("{} history entries for {alias}", page.items.len()),
                data: registry_json(&page)?,
            })
        }
    }
}
fn load_registry_reader(network: &NetworkArgs) -> Result<RegistryReadClientV1, Diagnostic> {
    RegistryReadClientV1::load(network.config.as_deref())
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))
}
fn registry_json<T: norito::json::JsonSerialize + ?Sized>(value: &T) -> Result<Value, Diagnostic> {
    norito::json::to_value(value).map_err(|_| {
        Diagnostic::new(
            ErrorCode::Internal,
            "validated registry result could not be rendered",
        )
    })
}
fn registry_diagnostic(error: RegistryErrorV1, fallback: ErrorCode) -> Diagnostic {
    let code = match (error.code(), error.class()) {
        ("MUSUBI_PUBLICATION_AUTHORITY_MISMATCH", _) => ErrorCode::Unauthorized,
        (_, crate::registry::RegistryFailureClassV1::Retryable) => ErrorCode::Network,
        (
            _,
            crate::registry::RegistryFailureClassV1::Permanent
            | crate::registry::RegistryFailureClassV1::NotFound
            | crate::registry::RegistryFailureClassV1::StaleCursor,
        ) => fallback,
    };
    Diagnostic::new(code, "Musubi registry operation failed")
        .with_context("registry_code", error.code())
}
fn run_update(explicit_manifest: Option<&Path>, args: &UpdateArgs) -> CommandResult {
    let manifest_path = project_manifest_path(explicit_manifest)?;
    let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let lock = read_optional_workspace_lock(&workspace)?;
    let target = args.package.as_ref().map_or_else(
        || "all locked packages".to_owned(),
        |target| {
            target.locked_version.as_ref().map_or_else(
                || target.package.to_string(),
                |version| format!("{}@{version}", target.package),
            )
        },
    );
    if args.mode.effective_locked() {
        return Err(Diagnostic::new(
            ErrorCode::Locked,
            "`musubi update` requests a graph change forbidden by --locked",
        )
        .with_context("lockfile", lock_path.display().to_string())
        .with_context("target", target)
        .with_help("rerun without --locked to authorize an atomic Musubi.lock update"));
    }
    if args.package.is_some() && lock.is_none() {
        return Err(Diagnostic::new(
            ErrorCode::LockfileInvalid,
            "a targeted update requires an existing Musubi V1 lock graph",
        )
        .with_context("lockfile", lock_path.display().to_string())
        .with_context("target", target));
    }
    let member_packages = workspace
        .members()
        .values()
        .map(|member| member.package.selector.clone())
        .collect::<BTreeSet<_>>();
    let mut selected = lock
        .as_ref()
        .map(|lock| {
            lock.roots
                .iter()
                .filter(|root| member_packages.contains(&root.package))
                .map(|root| root.package.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if selected.is_empty() {
        selected = workspace
            .select_members(false, &[], &[])
            .map_err(workspace_diagnostic)?
            .into_iter()
            .map(|member| member.package.selector.clone())
            .collect();
    }
    selected.sort();
    selected.dedup();
    let graph_update = args.package.as_ref().map(|target| GraphUpdateV1 {
        package: target.package.clone(),
        locked_version: target.locked_version.clone(),
        precise: args.precise.clone(),
    });
    let previous_for_resolution = if graph_update.is_some() {
        lock.clone()
    } else {
        None
    };
    let updated = resolve_and_update_workspace_lock(
        &workspace,
        &selected,
        args.mode,
        previous_for_resolution,
        graph_update,
        args.registry.config.as_deref(),
        false,
    )?;
    Ok(Success {
        message: format!("updated {target}"),
        data: object([
            ("target", Value::from(target)),
            ("lockfile", Value::from(lock_path.display().to_string())),
            ("nodes", Value::from(updated.lock.nodes.len() as u64)),
            ("graph", lockfile_json(&updated.lock)),
        ]),
    })
}
fn run_cache(explicit_manifest: Option<&Path>, args: &CacheArgs) -> CommandResult {
    match &args.command {
        CacheCommand::Verify { all, registry } => {
            let project_lock = optional_cache_project_lock(explicit_manifest)?;
            let cache = open_user_cache()?;
            let targets = if *all {
                cache
                    .archive_ids()
                    .map_err(|error| cache_maintenance_diagnostic(&error))?
                    .into_iter()
                    .collect()
            } else {
                let lock = project_lock.as_ref().ok_or_else(|| {
                    Diagnostic::new(
                        ErrorCode::Usage,
                        "cache verify requires an ancestor Musubi.lock or --all",
                    )
                    .with_help(
                        "run inside a Musubi workspace, pass --manifest-path, or select --all",
                    )
                })?;
                // A consumer lock selects identities only. `prepare_exact` below
                // supplies the finalized commitment and canonical provider plan.
                lock.nodes.iter().map(|node| node.archive_id).collect()
            };
            verify_cache_targets(&cache, &targets, registry.config.as_deref())
        }
        CacheCommand::Repair { mode, registry } => {
            let project_lock = optional_cache_project_lock(explicit_manifest)?;
            let inspected_project_lock = project_lock
                .as_ref()
                .map_or("none", |_| "validated, not authoritative");
            if mode.effective_offline() {
                return Err(Diagnostic::new(
                    ErrorCode::OfflineMiss,
                    "cache repair requires finalized archive commitments and provider plans",
                )
                .with_context("project_lock", inspected_project_lock)
                .with_help(
                    "rerun without --offline after configuring authenticated SoraFS fetch",
                ));
            }
            let cache = open_user_cache()?;
            let mut targets = cache
                .archive_ids()
                .map_err(|error| cache_maintenance_diagnostic(&error))?
                .into_iter()
                .collect::<BTreeSet<_>>();
            if let Some(lock) = &project_lock {
                targets.extend(lock.nodes.iter().map(|node| node.archive_id));
            }
            repair_cache_targets(
                &cache,
                &targets,
                registry.config.as_deref(),
                mode.effective_locked(),
            )
        }
        CacheCommand::Prune { dry_run, registry } => {
            let cache = open_user_cache()?;
            let archive_ids = cache
                .archive_ids()
                .map_err(|error| cache_maintenance_diagnostic(&error))?;
            if archive_ids.is_empty() {
                return Ok(empty_cache_prune_success(*dry_run));
            }
            let registry = RegistryReadClientV1::load(registry.config.as_deref())
                .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
            prune_cache_targets(&cache, &archive_ids, &registry, *dry_run)
        }
    }
}
fn optional_cache_project_lock(
    explicit_manifest: Option<&Path>,
) -> Result<Option<LockfileV1>, Diagnostic> {
    let manifest_path = if explicit_manifest.is_some() {
        Some(project_manifest_path(explicit_manifest)?)
    } else {
        let current = std::env::current_dir()
            .map_err(|error| io_diagnostic("read current directory", Path::new("."), &error))?;
        match discover_manifest(&current) {
            Ok(path) => Some(path),
            Err(error) if error.kind() == WorkspaceErrorKind::NotFound => None,
            Err(error) => return Err(workspace_diagnostic(error)),
        }
    };
    let Some(manifest_path) = manifest_path else {
        return Ok(None);
    };
    let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
    read_optional_workspace_lock(&workspace)
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheRetentionDeploymentV1 {
    network_id: iroha_data_model::NetworkId,
    snapshot: MusubiRegistrySnapshotV1,
}
fn empty_cache_prune_success(dry_run: bool) -> Success {
    Success {
        message: if dry_run {
            "would prune 0 cached archive(s)".to_owned()
        } else {
            "pruned 0 cached archive(s)".to_owned()
        },
        data: object([
            ("dry_run", Value::from(dry_run)),
            ("network_id", Value::Null),
            ("snapshot", Value::Null),
            ("queried", Value::from(0_u64)),
            ("retained", Value::from(0_u64)),
            ("prunable", Value::from(0_u64)),
            ("removed", Value::Array(Vec::new())),
            ("candidates", Value::Array(Vec::new())),
            ("decisions", Value::Array(Vec::new())),
        ]),
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "finalized classification and the fail-closed non-empty live-prune handoff remain one auditable workflow"
)]
fn prune_cache_targets(
    cache: &MusubiCache,
    archive_ids: &[iroha_data_model::musubi::ArchiveId],
    registry: &RegistryReadClientV1,
    dry_run: bool,
) -> CommandResult {
    if archive_ids.is_empty() {
        return Ok(empty_cache_prune_success(dry_run));
    }
    if archive_ids
        .iter()
        .any(iroha_data_model::musubi::ArchiveId::is_zero)
        || archive_ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(cache_retention_diagnostic(
            "local cache inventory is not a strictly ordered set of non-zero ArchiveIds",
        ));
    }
    let mut deployment = None::<CacheRetentionDeploymentV1>;
    let mut decisions = Vec::with_capacity(archive_ids.len());
    for archive_batch in archive_ids.chunks(MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1) {
        let request = MusubiArchiveRetentionQueryV1 {
            archive_ids: archive_batch.to_vec(),
            expected_snapshot: deployment.as_ref().map(|binding| binding.snapshot),
        };
        let page = registry
            .archive_retention(&request)
            .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
        let response_deployment = CacheRetentionDeploymentV1 {
            network_id: page.network_id,
            snapshot: page.snapshot,
        };
        if let Some(expected) = &deployment {
            if &response_deployment != expected {
                return Err(cache_retention_diagnostic(
                    "archive-retention batches disagree on network or finalized snapshot",
                ));
            }
        } else {
            deployment = Some(response_deployment);
        }
        if page
            .items
            .iter()
            .map(|decision| decision.archive_id)
            .ne(archive_batch.iter().copied())
        {
            return Err(cache_retention_diagnostic(
                "archive-retention response identities differ from the exact request batch",
            ));
        }
        decisions.extend(page.items);
    }
    if decisions.len() != archive_ids.len()
        || decisions
            .iter()
            .map(|decision| decision.archive_id)
            .ne(archive_ids.iter().copied())
    {
        return Err(cache_retention_diagnostic(
            "archive-retention responses do not cover the complete cache inventory",
        ));
    }
    let deployment = deployment.ok_or_else(|| {
        cache_retention_diagnostic("archive-retention proof has no finalized deployment binding")
    })?;
    let prunable = decisions
        .iter()
        .filter(|decision| !decision.must_retain())
        .map(|decision| decision.archive_id)
        .collect::<BTreeSet<_>>();
    let retained_count = decisions.len().saturating_sub(prunable.len());
    let queried_count = u64::try_from(decisions.len()).map_err(|_| {
        cache_retention_diagnostic("archive-retention decision count exceeds JSON output bounds")
    })?;
    let retained_count = u64::try_from(retained_count).map_err(|_| {
        cache_retention_diagnostic("archive-retention retained count exceeds JSON output bounds")
    })?;
    let prunable_count = u64::try_from(prunable.len()).map_err(|_| {
        cache_retention_diagnostic("archive-retention prunable count exceeds JSON output bounds")
    })?;
    // Dry-run preserves the complete finalized classification. A non-empty live prune reaches
    // the cache boundary only after that proof is coherent, then fails before inspecting or
    // mutating any candidate until atomic handle-relative compare-and-delete exists.
    let removed = if dry_run {
        Vec::new()
    } else {
        cache
            .prune_exact(&prunable)
            .map_err(|error| cache_maintenance_diagnostic(&error))?
            .removed
    };
    let decision_values = decisions
        .iter()
        .map(cache_retention_decision_json)
        .collect::<Vec<_>>();
    let candidates = prunable
        .iter()
        .map(|archive_id| Value::from(hex::encode(archive_id.as_bytes())))
        .collect::<Vec<_>>();
    let removed_values = removed
        .iter()
        .map(|archive_id| Value::from(hex::encode(archive_id.as_bytes())))
        .collect::<Vec<_>>();
    let message_count = if dry_run {
        prunable.len()
    } else {
        removed.len()
    };
    Ok(Success {
        message: if dry_run {
            format!("would prune {message_count} cached archive(s)")
        } else {
            format!("pruned {message_count} cached archive(s)")
        },
        data: object([
            ("dry_run", Value::from(dry_run)),
            ("network_id", Value::from(deployment.network_id.to_string())),
            (
                "snapshot",
                object([
                    (
                        "finalized_height",
                        Value::from(deployment.snapshot.finalized_height),
                    ),
                    (
                        "finalized_block_hash",
                        Value::from(hex::encode(deployment.snapshot.finalized_block_hash)),
                    ),
                    (
                        "index_revision",
                        Value::from(deployment.snapshot.index_revision),
                    ),
                ]),
            ),
            ("queried", Value::from(queried_count)),
            ("retained", Value::from(retained_count)),
            ("prunable", Value::from(prunable_count)),
            ("removed", Value::Array(removed_values)),
            ("candidates", Value::Array(candidates)),
            ("decisions", Value::Array(decision_values)),
        ]),
    })
}
fn cache_retention_decision_json(
    decision: &iroha_data_model::musubi::MusubiArchiveRetentionDecisionV1,
) -> Value {
    let disposition = match decision.disposition {
        MusubiArchiveRetentionDispositionV1::RetainUnknown => "retain-unknown",
        MusubiArchiveRetentionDispositionV1::RetainReferenced => "retain-referenced",
        MusubiArchiveRetentionDispositionV1::PruneUnreferenced => "prune-unreferenced",
        MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown => "prune-governed-takedown",
    };
    let storage = decision.storage.map(|storage| match storage.availability {
        MusubiStorageAvailabilityV1::Selectable => "selectable",
        MusubiStorageAvailabilityV1::BelowQuorum => "below-quorum",
        MusubiStorageAvailabilityV1::Unavailable => "unavailable",
    });
    object([
        (
            "archive_id",
            Value::from(hex::encode(decision.archive_id.as_bytes())),
        ),
        ("disposition", Value::from(disposition)),
        ("must_retain", Value::from(decision.must_retain())),
        (
            "active_releases",
            Value::from(u64::from(decision.active_releases)),
        ),
        (
            "yanked_releases",
            Value::from(u64::from(decision.yanked_releases)),
        ),
        (
            "taken_down_releases",
            Value::from(u64::from(decision.taken_down_releases)),
        ),
        ("storage", storage.map_or(Value::Null, Value::from)),
    ])
}
fn cache_retention_diagnostic(reason: &'static str) -> Diagnostic {
    Diagnostic::new(
        ErrorCode::Registry,
        "authoritative Musubi cache-retention proof is inconsistent",
    )
    .with_context("reason", reason)
    .with_help("no cache path has been changed")
}
fn load_registry_and_archive_transport(
    config: Option<&Path>,
) -> Result<(RegistryReadClientV1, ProductionSorafsArchiveTransportV1), Diagnostic> {
    let (registry, config_image) = RegistryReadClientV1::load_with_config_image(config)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Registry))?;
    let prepared =
        prepare_production_archive_transport_v1(config_image.path(), config_image.bytes())
            .map_err(archive_transport_diagnostic)?;
    drop(config_image);
    let transport =
        build_production_archive_transport_v1(&prepared).map_err(archive_transport_diagnostic)?;
    Ok((registry, transport))
}
fn verify_cache_targets(
    cache: &MusubiCache,
    targets: &BTreeSet<iroha_data_model::musubi::ArchiveId>,
    config: Option<&Path>,
) -> CommandResult {
    if targets.is_empty() {
        return Ok(Success {
            message: "verified 0 cached archive(s)".to_owned(),
            data: object([("archives", Value::Array(Vec::new()))]),
        });
    }
    let (registry, mut transport) = load_registry_and_archive_transport(config)?;
    let adapter = MusubiArchiveFetchAdapterV1::new(&registry, cache);
    let mut verified = Vec::with_capacity(targets.len());
    for archive_id in targets {
        let prepared = adapter
            .prepare_exact(*archive_id, &mut transport)
            .map_err(|error| {
                archive_fetch_diagnostic(error)
                    .with_context("archive_id", hex::encode(archive_id.as_bytes()))
            })?;
        cache
            .verify(&prepared.commitment, &prepared.plan)
            .map_err(|error| cache_maintenance_diagnostic(&error))?;
        verified.push(object([
            (
                "archive_id",
                Value::from(hex::encode(archive_id.as_bytes())),
            ),
            (
                "location_id",
                Value::from(hex::encode(prepared.location_id.as_bytes())),
            ),
            ("provider", Value::from(prepared.provider.to_string())),
            ("status", Value::from("healthy")),
        ]));
    }
    Ok(Success {
        message: format!("verified {} cached archive(s)", verified.len()),
        data: object([("archives", Value::Array(verified))]),
    })
}
fn repair_cache_targets(
    cache: &MusubiCache,
    targets: &BTreeSet<iroha_data_model::musubi::ArchiveId>,
    config: Option<&Path>,
    locked: bool,
) -> CommandResult {
    if targets.is_empty() {
        return Ok(Success {
            message: "repaired 0 cached archive(s)".to_owned(),
            data: object([
                ("locked", Value::from(locked)),
                ("archives", Value::Array(Vec::new())),
            ]),
        });
    }
    let (registry, mut transport) = load_registry_and_archive_transport(config)?;
    let adapter = MusubiArchiveFetchAdapterV1::new(&registry, cache);
    let mut repaired = Vec::with_capacity(targets.len());
    for archive_id in targets {
        let prepared = adapter
            .prepare_exact(*archive_id, &mut transport)
            .map_err(|error| {
                archive_fetch_diagnostic(error)
                    .with_context("archive_id", hex::encode(archive_id.as_bytes()))
            })?;
        let repair = cache
            .repair(&prepared.commitment, &prepared.plan)
            .map_err(|error| cache_maintenance_diagnostic(&error))?;
        let (location_id, provider, status) = match repair {
            RepairOutcome::Healthy(_) => (
                prepared.location_id,
                prepared.provider,
                "healthy".to_owned(),
            ),
            RepairOutcome::Missing => {
                let fetched =
                    adapter
                        .fetch_exact(*archive_id, &mut transport)
                        .map_err(|error| {
                            archive_fetch_diagnostic(error)
                                .with_context("archive_id", hex::encode(archive_id.as_bytes()))
                        })?;
                (
                    fetched.location_id,
                    fetched.provider,
                    "missing-and-fetched".to_owned(),
                )
            }
            RepairOutcome::Quarantined { .. } => {
                let fetched =
                    adapter
                        .fetch_exact(*archive_id, &mut transport)
                        .map_err(|error| {
                            archive_fetch_diagnostic(error)
                                .with_context("archive_id", hex::encode(archive_id.as_bytes()))
                        })?;
                (
                    fetched.location_id,
                    fetched.provider,
                    "quarantined-and-refetched".to_owned(),
                )
            }
        };
        repaired.push(object([
            (
                "archive_id",
                Value::from(hex::encode(archive_id.as_bytes())),
            ),
            (
                "location_id",
                Value::from(hex::encode(location_id.as_bytes())),
            ),
            ("provider", Value::from(provider.to_string())),
            ("status", Value::from(status)),
        ]));
    }
    Ok(Success {
        message: format!("repaired {} cached archive(s)", repaired.len()),
        data: object([
            ("locked", Value::from(locked)),
            ("archives", Value::Array(repaired)),
        ]),
    })
}
fn cache_maintenance_diagnostic(error: &CacheError) -> Diagnostic {
    let code = if matches!(
        error,
        CacheError::UnsupportedPlatform | CacheError::Io { .. }
    ) {
        ErrorCode::Io
    } else {
        ErrorCode::CacheCorrupt
    };
    Diagnostic::new(code, "immutable Musubi cache validation failed")
        .with_context("reason", error.to_string())
}
#[cfg(all(test, unix))]
mod tests {
    include!("command_tests.rs");
}
