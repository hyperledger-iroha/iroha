//! Cargo-style Musubi V1 command parsing and signer-free local workflows.
//!
//! This module owns the public command grammar and returns logical output. Local
//! and read-only commands never construct a signer; network mutations load one
//! only at their explicit registry boundary. Compiler and remaining graph
//! boundaries report stable diagnostics until their dedicated V1 modules connect.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use iroha_data_model::{
    isi::musubi::{RegisterMusubiAliasV1, SetMusubiReleaseYankV1},
    musubi::{
        MusubiAliasNameV1, MusubiAliasQueryV1, MusubiDependencyKindV1, MusubiExactDependencyEdgeV1,
        MusubiNamespaceV1, MusubiPackageNameV1, MusubiPackagePageQueryV1, MusubiPackageSelectorV1,
        MusubiPageRequestV1, MusubiReasonV1, MusubiReleaseIdV1, MusubiVersionReqV1,
        MusubiVersionV1,
    },
    name::Name,
};
use norito::json::{Map, Value};

use crate::{
    atomic_io::AtomicWriteRoot,
    lockfile::{LockfileError, LockfileV1},
    manifest::{
        ConcreteDependency, DependencyPath, DependencySection, DependencySpec, MANIFEST_FILE_NAME,
        Manifest, PortablePath, parse_manifest, remove_dependency, upsert_dependency,
    },
    output::{CommandOutput, Diagnostic, ErrorCode, OutputFormat},
    publish::{
        PublicationAdvanceV1, PublicationEngine, PublicationError, PublicationJournalStore,
        PublicationOperationIdV1, PublicationStagedCarSourceV1,
    },
    registry::{
        PublicationPollPolicyV1, RegistryErrorV1, RegistryPublicationBackendV1,
        RegistryReadClientV1, RegistrySigningClientV1, UnavailablePublicationRuntimeV1,
        resume_with_bounded_polling,
    },
    workspace::{
        DependencyKind, EffectiveDependency, Workspace, WorkspaceMember, discover_manifest,
        load_workspace,
    },
};

const LOCK_FILE_NAME: &str = "Musubi.lock";

/// Parsed presentation mode and logical command result.
pub(crate) struct Invocation {
    pub(crate) format: OutputFormat,
    pub(crate) output: CommandOutput,
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
}

#[derive(Args, Debug)]
struct BuildArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[command(flatten)]
    mode: GraphModeArgs,
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
    /// List the positive package file set without writing a CAR.
    #[arg(long)]
    list: bool,
}

#[derive(Args, Clone, Debug, Default)]
struct NetworkArgs {
    /// Explicit platform Iroha client configuration path.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Wait for finalized success/failure.
    #[arg(long)]
    wait: bool,
}

impl NetworkArgs {
    fn observe_boundary(&self) {
        let _ = (&self.config, self.wait);
    }
}

#[derive(Args, Debug)]
struct PublishArgs {
    #[command(flatten)]
    mode: GraphModeArgs,
    #[command(flatten)]
    network: NetworkArgs,
    /// Return after persisting a resumable operation journal.
    #[arg(long, conflicts_with = "resume")]
    detach: bool,
    /// Resume one secret-free publication journal by its canonical operation id.
    #[arg(long, value_name = "OPERATION_ID", conflicts_with = "detach")]
    resume: Option<PublicationOperationIdV1>,
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

#[derive(Subcommand, Debug)]
enum OwnerCommand {
    /// Invite an account to a package role.
    Invite {
        package: MusubiPackageSelectorV1,
        account: String,
        #[arg(long, value_enum)]
        role: RoleArg,
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
        #[arg(long)]
        expected_revision: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Remove an accepted package member.
    Remove {
        package: MusubiPackageSelectorV1,
        account: String,
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
    },
    /// Quarantine corrupt trusted descendants and refetch when allowed.
    Repair {
        #[command(flatten)]
        mode: GraphModeArgs,
    },
    /// Remove only unreferenced validated cache descendants.
    Prune {
        /// Print candidates without deleting them.
        #[arg(long)]
        dry_run: bool,
    },
}

struct Success {
    message: String,
    data: Value,
}

type CommandResult = Result<Success, Diagnostic>;

/// Parse and execute an argv sequence without writing process streams.
pub(crate) fn invoke<I, T>(args: I) -> Invocation
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
        Command::Check(args) => deferred_build("check", args),
        Command::Build(args) => deferred_build("build", args),
        Command::Test(args) => deferred_build("test", args),
        Command::Package(args) => run_package(manifest_path, args),
        Command::Publish(args) => deferred_publish(args),
        Command::Search(args) => deferred_search(args),
        Command::Info(args) => run_package_info(args),
        Command::Versions(args) => run_package_versions(args),
        Command::Yank(args) => run_release_yank(args, true),
        Command::Unyank(args) => run_release_yank(args, false),
        Command::Owner(args) => deferred_owner(args),
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
            if let Some(local) = local {
                format!(
                    "{} -> {} v{} (path {})",
                    dependency.alias, local.package.selector, local.package.version, path
                )
            } else if let (Some(package), Some(requirement)) = (package, requirement) {
                format!(
                    "{} -> {package} {requirement} (path {path})",
                    dependency.alias
                )
            } else {
                format!("{} -> path {path}", dependency.alias)
            }
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
    fs::read_to_string(path).map_err(|error| io_diagnostic("read manifest", path, &error))
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
        .map_err(|error| lockfile_diagnostic(&path, error))
}

fn lockfile_diagnostic(path: &Path, error: LockfileError) -> Diagnostic {
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
        ("chain", Value::from(lock.chain_id.to_string())),
        ("genesis_hash", Value::from(hex::encode(lock.genesis_hash))),
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

fn missing_locked_roots(
    lock: &LockfileV1,
    selected_packages: &[MusubiPackageSelectorV1],
) -> Vec<String> {
    selected_packages
        .iter()
        .filter(|package| {
            lock.roots
                .binary_search_by(|root| root.package.cmp(package))
                .is_err()
        })
        .map(ToString::to_string)
        .collect()
}

fn graph_change_unavailable(
    command: &'static str,
    mode: GraphModeArgs,
    lock_path: &Path,
    reason: &str,
) -> Diagnostic {
    let mut diagnostic = if mode.effective_locked() {
        Diagnostic::new(
            ErrorCode::Locked,
            format!("`musubi {command}` requires an exact lock graph change forbidden by --locked"),
        )
        .with_help("rerun without --locked after configuring the finalized V1 registry")
    } else if mode.effective_offline() {
        Diagnostic::new(
            ErrorCode::OfflineMiss,
            format!(
                "`musubi {command}` cannot resolve the required graph from authenticated local data"
            ),
        )
        .with_help("rerun online after configuring the finalized V1 registry")
    } else {
        Diagnostic::new(
            ErrorCode::Registry,
            format!("`musubi {command}` requires finalized universal sparse-index rows"),
        )
        .with_help("configure a Musubi V1 registry query adapter and retry")
    };
    diagnostic = diagnostic
        .with_context("lockfile", lock_path.display().to_string())
        .with_context("reason", reason);
    diagnostic
}

fn run_fetch(explicit_manifest: Option<&Path>, args: &FetchArgs) -> CommandResult {
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let Some(lock) = read_optional_workspace_lock(&workspace)? else {
        return Err(graph_change_unavailable(
            "fetch",
            args.mode,
            &lock_path,
            "the selected workspace has no Musubi V1 lock graph",
        ));
    };
    let missing_roots = missing_locked_roots(&lock, &selected_names);
    if !missing_roots.is_empty() {
        return Err(graph_change_unavailable(
            "fetch",
            args.mode,
            &lock_path,
            &format!(
                "the lock does not cover selected workspace roots: {}",
                missing_roots.join(", ")
            ),
        ));
    }

    // TODO: Inject the finalized registry/archive adapter and call
    // `MusubiCache::install`; never reconstruct its commitment or CAR plan
    // from consumer-owned lock fields.
    let required =
        "finalized MusubiArchiveCommitmentV1, canonical CarBuildPlan, and active archive locations";
    let code = if args.mode.effective_offline() {
        ErrorCode::OfflineMiss
    } else {
        ErrorCode::Registry
    };
    let help = if args.mode.effective_offline() {
        "populate the cache from authenticated registry and SoraFS data before retrying offline"
    } else {
        "configure the Musubi V1 registry/archive adapter; consumer lock fields are not archive proofs"
    };
    Err(Diagnostic::new(
        code,
        "archive fetch requires authenticated registry commitments and canonical SoraFS plans",
    )
    .with_context("lockfile", lock_path.display().to_string())
    .with_context("locked_nodes", lock.nodes.len().to_string())
    .with_context("required_inputs", required)
    .with_context("selected_roots", selected_names.len().to_string())
    .with_help(help))
}

fn deferred_build(command: &'static str, args: &BuildArgs) -> CommandResult {
    let _ = (
        args.selection.workspace,
        args.selection.packages.len(),
        args.selection.exclude.len(),
        args.release,
    );
    deferred_graph(command, args.mode, ErrorCode::Compiler, "resolver/compiler")
}

fn run_package(explicit_manifest: Option<&Path>, args: &PackageArgs) -> CommandResult {
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let lock_path = workspace.root().join(LOCK_FILE_NAME);
    let Some(lock) = read_optional_workspace_lock(&workspace)? else {
        return Err(graph_change_unavailable(
            "package",
            args.mode,
            &lock_path,
            "the selected workspace has no Musubi V1 lock graph",
        ));
    };
    let missing_roots = missing_locked_roots(&lock, &selected_names);
    if !missing_roots.is_empty() {
        return Err(graph_change_unavailable(
            "package",
            args.mode,
            &lock_path,
            &format!(
                "the lock does not cover selected workspace roots: {}",
                missing_roots.join(", ")
            ),
        ));
    }

    // TODO: Feed `plan_package` the registry-normalized publication lock and
    // semantic release manifest once the finalized registry adapter exists.
    // The consumer lock is intentionally inspection-only here.
    let code = if args.mode.effective_offline() {
        ErrorCode::OfflineMiss
    } else {
        ErrorCode::Registry
    };
    Err(Diagnostic::new(
        code,
        "clean packaging requires authenticated publication inputs that are absent from the consumer lock",
    )
    .with_context("consumer_lock", "validated, inspection-only")
    .with_context("list_only", args.list.to_string())
    .with_context("lockfile", lock_path.display().to_string())
    .with_context(
        "required_inputs",
        "structural namespace binding, semantic release manifest, and normalized publication verification lock",
    )
    .with_context("selected_roots", selected_names.len().to_string())
    .with_help(
        "configure the finalized V1 registry adapter; Musubi.lock is never promoted into a publication proof",
    ))
}

fn deferred_publish(args: &PublishArgs) -> CommandResult {
    args.network.observe_boundary();
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
    let _ = args.detach;
    deferred_graph(
        "publish",
        args.mode,
        ErrorCode::Publish,
        "registry/SoraFS publisher",
    )
}

fn resume_publication(args: &PublishArgs, operation_id: PublicationOperationIdV1) -> CommandResult {
    let state_root = publication_state_root()?;
    let store = PublicationJournalStore::open(&state_root).map_err(publication_diagnostic)?;
    let journal = store.load(operation_id).map_err(publication_diagnostic)?;
    let reader = RegistryReadClientV1::load(args.network.config.as_deref())
        .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?;
    let signer = RegistrySigningClientV1::load(args.network.config.as_deref())
        .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?;
    let mut backend = RegistryPublicationBackendV1::new(
        reader,
        signer,
        UnavailablePublicationRuntimeV1,
        &journal.request,
    )
    .map_err(|error| registry_diagnostic(error, ErrorCode::Publish))?;
    let source = PublicationStagedCarSourceV1::new(
        &state_root,
        operation_id,
        journal.request.archive_commitment.car_size,
    );
    let engine = PublicationEngine::new(&store);
    match resume_with_bounded_polling(
        &engine,
        operation_id,
        &source,
        &mut backend,
        PublicationPollPolicyV1::default(),
    )
    .map_err(publication_diagnostic)?
    {
        PublicationAdvanceV1::Complete(result) => Ok(Success {
            message: format!(
                "published {}",
                result.final_evidence.home_release.manifest.release
            ),
            data: Value::Object(Map::from_iter([
                (
                    "operation_id".to_owned(),
                    Value::from(result.operation_id.to_string()),
                ),
                (
                    "release".to_owned(),
                    Value::from(
                        result
                            .final_evidence
                            .home_release
                            .manifest
                            .release
                            .to_string(),
                    ),
                ),
                (
                    "transaction_hash".to_owned(),
                    Value::from(hex::encode(result.submission.transaction_hash)),
                ),
            ])),
        }),
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

    root.ok_or_else(|| {
        Diagnostic::new(
            ErrorCode::Io,
            "platform user state directory is unavailable",
        )
    })
}

fn publication_diagnostic(error: PublicationError) -> Diagnostic {
    let public_code = match &error {
        PublicationError::Backend(backend) => backend.code(),
        PublicationError::CarSource(_) => "PUBLICATION_CAR_NOT_STAGED",
        PublicationError::NotFound(_) => "PUBLICATION_OPERATION_NOT_FOUND",
        PublicationError::ConcurrentJournalUpdate => "PUBLICATION_CONCURRENT_RESUME",
        PublicationError::InvalidEvidence { .. } => "PUBLICATION_EVIDENCE_INVALID",
        PublicationError::InvalidJournal(_) => "PUBLICATION_JOURNAL_INVALID",
        PublicationError::JournalWrite(_) | PublicationError::JournalIo(_) => {
            "PUBLICATION_JOURNAL_IO"
        }
    };
    Diagnostic::new(ErrorCode::Publish, "publication resume failed")
        .with_context("publication_code", public_code)
}

fn deferred_search(args: &SearchArgs) -> CommandResult {
    args.network.observe_boundary();
    let _ = (&args.query, args.limit);
    deferred("search", ErrorCode::Registry, "registry query")
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
    let registry = load_registry_reader(&args.network)?;
    let package = registry
        .resolve_selector(&args.package)
        .map_err(|error| registry_diagnostic(error, ErrorCode::Governance))?;
    let release = MusubiReleaseIdV1::new(package, args.version.clone());
    let reason = args.reason.clone().map_or_else(
        || {
            if yanked {
                "package maintainer requested yank"
            } else {
                "package maintainer requested unyank"
            }
            .parse()
            .expect("built-in Musubi reason is valid")
        },
        |reason| reason,
    );
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

fn deferred_owner(args: &OwnerArgs) -> CommandResult {
    match &args.command {
        OwnerCommand::Invite {
            package,
            account,
            role,
            expected_revision,
            network,
        }
        | OwnerCommand::SetRole {
            package,
            account,
            role,
            expected_revision,
            network,
        } => {
            network.observe_boundary();
            let _ = (package, account, role, expected_revision);
        }
        OwnerCommand::Accept {
            package,
            invitation,
            expected_revision,
            network,
        } => {
            network.observe_boundary();
            let _ = (package, invitation, expected_revision);
        }
        OwnerCommand::List { package, network } => {
            network.observe_boundary();
            let _ = package;
        }
        OwnerCommand::Remove {
            package,
            account,
            expected_revision,
            network,
        } => {
            network.observe_boundary();
            let _ = (package, account, expected_revision);
        }
    }
    deferred(
        "owner",
        ErrorCode::Governance,
        "package-governance registry",
    )
}

fn run_alias(args: &AliasArgs) -> CommandResult {
    match &args.command {
        AliasCommand::Register {
            alias,
            package,
            expected_price_revision,
            network,
        } => {
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
    network.observe_boundary();
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
    let code = match error.class() {
        crate::registry::RegistryFailureClassV1::Retryable => ErrorCode::Network,
        crate::registry::RegistryFailureClassV1::Permanent
        | crate::registry::RegistryFailureClassV1::NotFound
        | crate::registry::RegistryFailureClassV1::StaleCursor => fallback,
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
        |target| match &target.locked_version {
            Some(version) => format!("{}@{version}", target.package),
            None => target.package.to_string(),
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
    if args.mode.effective_offline() {
        return Err(Diagnostic::new(
            ErrorCode::OfflineMiss,
            "targeted update requires authenticated finalized resolver rows that are not cached",
        )
        .with_context("lockfile", lock_path.display().to_string())
        .with_context("target", target)
        .with_help("rerun online after configuring the finalized V1 registry"));
    }

    // TODO: Query a finalized universal sparse-index snapshot and pass its
    // authenticated rows to the deterministic resolver before atomically
    // replacing Musubi.lock.
    let mut diagnostic = Diagnostic::new(
        ErrorCode::Registry,
        "targeted update requires authenticated finalized universal sparse-index rows",
    )
    .with_context("lockfile", lock_path.display().to_string())
    .with_context(
        "locked_nodes",
        lock.as_ref().map_or(0, |lock| lock.nodes.len()).to_string(),
    )
    .with_context("target", target)
    .with_help("configure a Musubi V1 registry query adapter and retry");
    if let Some(precise) = &args.precise {
        diagnostic = diagnostic.with_context("precise", precise.to_string());
    }
    Err(diagnostic)
}

fn run_cache(explicit_manifest: Option<&Path>, args: &CacheArgs) -> CommandResult {
    let project_lock = if let Some(explicit_manifest) = explicit_manifest {
        let manifest_path = project_manifest_path(Some(explicit_manifest))?;
        let workspace = load_workspace(&manifest_path).map_err(workspace_diagnostic)?;
        read_optional_workspace_lock(&workspace)?
    } else {
        None
    };
    let inspected_project_lock = project_lock
        .as_ref()
        .map_or("none", |_| "validated, not authoritative");

    // TODO: Supply authenticated archive commitments/plans and a registry-wide
    // retained-id projection. A single project lock must never authorize cache
    // verification, quarantine, or deletion.
    match &args.command {
        CacheCommand::Verify { all } => Err(Diagnostic::new(
            ErrorCode::Registry,
            "cache verification requires authenticated finalized archive commitments and canonical SoraFS plans",
        )
        .with_context("all", all.to_string())
        .with_context("project_lock", inspected_project_lock)
        .with_context(
            "required_inputs",
            "MusubiArchiveCommitmentV1 and matching CarBuildPlan for each ArchiveId",
        )
        .with_help(
            "configure the registry/archive adapter; a consumer lock is not a cache verification anchor",
        )),
        CacheCommand::Repair { mode } => {
            let code = if mode.effective_offline() {
                ErrorCode::OfflineMiss
            } else {
                ErrorCode::Registry
            };
            Err(Diagnostic::new(
                code,
                "cache repair requires authenticated commitments before any descendant may be quarantined",
            )
            .with_context("locked", mode.effective_locked().to_string())
            .with_context("offline", mode.effective_offline().to_string())
            .with_context("project_lock", inspected_project_lock)
            .with_context(
                "required_inputs",
                "MusubiArchiveCommitmentV1 and matching CarBuildPlan for each repair target",
            )
            .with_help(
                "configure the registry/archive adapter; no cache path has been changed",
            ))
        }
        CacheCommand::Prune { dry_run } => Err(Diagnostic::new(
            ErrorCode::Registry,
            "cache pruning requires a trusted global retained-ArchiveId set",
        )
        .with_context("dry_run", dry_run.to_string())
        .with_context("project_lock", inspected_project_lock)
        .with_context("required_inputs", "global retained ArchiveId projection")
        .with_help(
            "one workspace lock cannot authorize deletion from the shared user cache; no cache path has been changed",
        )),
    }
}

fn deferred_graph(
    command: &'static str,
    mode: GraphModeArgs,
    code: ErrorCode,
    boundary: &'static str,
) -> CommandResult {
    let _ = (mode.effective_locked(), mode.effective_offline());
    deferred(command, code, boundary)
}

fn deferred(command: &'static str, code: ErrorCode, boundary: &'static str) -> CommandResult {
    // TODO: Connect this typed command boundary to the dedicated Musubi V1
    // registry/resolver/compiler/cache/publisher module named in `boundary`.
    Err(Diagnostic::new(
        code,
        format!("`musubi {command}` awaits the Musubi V1 {boundary} implementation"),
    )
    .with_help("the retired pre-release workflow is intentionally unavailable"))
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;
    use iroha_data_model::musubi::MusubiRegistrySnapshotV1;
    use tempfile::TempDir;

    use super::*;
    use crate::lockfile::LockedRootV1;

    fn command_names(command: clap::Command) -> BTreeSet<String> {
        command
            .get_subcommands()
            .map(|command| command.get_name().to_owned())
            .collect()
    }

    fn create_test_package(temp: &TempDir) -> (PathBuf, PathBuf) {
        let root = temp.path().join("demo");
        let invocation = invoke([
            OsString::from("musubi"),
            OsString::from("new"),
            root.as_os_str().to_owned(),
            OsString::from("--namespace"),
            OsString::from("apps.sora"),
            OsString::from("--export"),
            OsString::from("run"),
        ]);
        assert_eq!(invocation.output.exit_code(), 0);
        let manifest = root.join(MANIFEST_FILE_NAME);
        (root, manifest)
    }

    fn write_test_lock(root: &Path) {
        let lock = LockfileV1::new(
            "musubi-cli-test".parse().expect("chain id"),
            [1; 32],
            MusubiRegistrySnapshotV1 {
                finalized_height: 7,
                finalized_block_hash: [2; 32],
                index_revision: 3,
            },
            vec![LockedRootV1 {
                package: "apps.sora/demo".parse().expect("root package selector"),
                dependencies: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("valid test lock");
        let bytes = lock.render().expect("render lock").into_bytes();
        LockfileV1::parse(std::str::from_utf8(&bytes).expect("UTF-8 lock"))
            .expect("rendered lock parses");
        let lock_path = root.join(LOCK_FILE_NAME);
        fs::write(&lock_path, &bytes).expect("write lock");
        LockfileV1::read(&lock_path).expect("written lock parses");
    }

    #[test]
    fn top_level_and_nested_command_inventory_is_exact() {
        let command = Cli::command();
        assert_eq!(
            command_names(command.clone()),
            BTreeSet::from_iter(
                [
                    "add", "alias", "build", "cache", "check", "fetch", "info", "init", "metadata",
                    "new", "owner", "package", "publish", "remove", "search", "test", "tree",
                    "unyank", "update", "versions", "yank",
                ]
                .map(str::to_owned)
            )
        );
        let owner = command
            .get_subcommands()
            .find(|command| command.get_name() == "owner")
            .expect("owner command");
        assert_eq!(
            command_names(owner.clone()),
            BTreeSet::from_iter(
                ["accept", "invite", "list", "remove", "set-role"].map(str::to_owned)
            )
        );
        let alias = command
            .get_subcommands()
            .find(|command| command.get_name() == "alias")
            .expect("alias command");
        assert_eq!(
            command_names(alias.clone()),
            BTreeSet::from_iter(["history", "info", "register", "resolve"].map(str::to_owned))
        );
        let cache = command
            .get_subcommands()
            .find(|command| command.get_name() == "cache")
            .expect("cache command");
        assert_eq!(
            command_names(cache.clone()),
            BTreeSet::from_iter(["prune", "repair", "verify"].map(str::to_owned))
        );
    }

    #[test]
    fn retired_commands_and_alias_set_are_rejected() {
        for argv in [
            vec!["musubi", "install"],
            vec!["musubi", "pack"],
            vec!["musubi", "alias", "set"],
        ] {
            let invocation = invoke(argv);
            assert_eq!(invocation.output.exit_code(), ErrorCode::Usage.exit_code());
        }
    }

    #[test]
    fn frozen_combines_locked_and_offline_at_typed_boundary() {
        let cli = Cli::try_parse_from(["musubi", "fetch", "--frozen"]).expect("parse frozen fetch");
        let Command::Fetch(args) = cli.command else {
            panic!("expected fetch");
        };
        assert!(args.mode.effective_locked());
        assert!(args.mode.effective_offline());
    }

    #[test]
    fn targeted_update_parser_requires_structural_package_and_exact_version() {
        let target = "std/math@1.2.3"
            .parse::<UpdateTarget>()
            .expect("valid targeted update");
        assert_eq!(target.package.to_string(), "std/math");
        assert_eq!(
            target.locked_version.as_ref().map(ToString::to_string),
            Some("1.2.3".to_owned())
        );
        assert!("math@1.2.3".parse::<UpdateTarget>().is_err());
        assert!("std/math@1.2.3+local".parse::<UpdateTarget>().is_err());
    }

    #[test]
    fn json_parse_failure_is_one_stdout_document() {
        let invocation = invoke(["musubi", "--format", "json", "install"]);
        let rendered = invocation
            .output
            .render(invocation.format)
            .expect("render JSON failure");
        assert_eq!(rendered.exit_code(), ErrorCode::Usage.exit_code());
        assert!(rendered.stderr().is_empty());
        assert_eq!(rendered.stdout().matches('\n').count(), 1);
        let value: Value = norito::json::from_str(rendered.stdout()).expect("JSON document");
        assert_eq!(value.get("ok").and_then(Value::as_bool), Some(false));
    }

    #[test]
    fn local_new_add_metadata_tree_remove_roundtrip() {
        let temp = TempDir::new().expect("temporary directory");
        let (_root, manifest_path) = create_test_package(&temp);
        let manifest = fs::read_to_string(&manifest_path).expect("new manifest");
        parse_manifest(&manifest).expect("strict generated manifest");

        let add = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("add"),
            OsString::from("std/math"),
            OsString::from("--version"),
            OsString::from("^1.0.0"),
            OsString::from("--rename"),
            OsString::from("math"),
        ]);
        assert_eq!(add.output.exit_code(), 0);

        let add_local_dev = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("add"),
            OsString::from("local-test-support"),
            OsString::from("--path"),
            OsString::from("."),
            OsString::from("--dev"),
        ]);
        assert_eq!(add_local_dev.output.exit_code(), 0);

        let metadata = invoke([
            OsString::from("musubi"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("metadata"),
        ]);
        assert_eq!(
            metadata.output.exit_code(),
            0,
            "{}",
            metadata
                .output
                .render(OutputFormat::Human)
                .expect("metadata diagnostic")
                .stderr()
        );
        let rendered = metadata
            .output
            .render(metadata.format)
            .expect("metadata JSON");
        let document: Value = norito::json::from_str(rendered.stdout()).expect("metadata document");
        assert_eq!(
            document
                .pointer("/data/packages/0/package")
                .and_then(Value::as_str),
            Some("apps.sora/demo")
        );

        let tree = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("tree"),
        ]);
        assert_eq!(tree.output.exit_code(), 0);
        let tree = tree.output.render(tree.format).expect("tree output");
        assert!(tree.stdout().contains("math -> std/math ^1.0.0"));
        assert!(tree.stdout().contains("[dev] local-test-support ->"));

        let remove = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("remove"),
            OsString::from("math"),
        ]);
        assert_eq!(remove.output.exit_code(), 0);
        let manifest = fs::read_to_string(&manifest_path).expect("edited manifest");
        assert!(
            !parse_manifest(&manifest)
                .expect("manifest after remove")
                .dependencies
                .contains_key("math")
        );
    }

    #[test]
    fn metadata_and_tree_include_only_the_validated_exact_lock_graph() {
        let temp = TempDir::new().expect("temporary directory");
        let (root, manifest_path) = create_test_package(&temp);
        write_test_lock(&root);

        let metadata = invoke([
            OsString::from("musubi"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("metadata"),
        ]);
        assert_eq!(metadata.output.exit_code(), 0);
        let rendered = metadata
            .output
            .render(metadata.format)
            .expect("metadata JSON");
        let document: Value = norito::json::from_str(rendered.stdout()).expect("JSON document");
        assert_eq!(
            document
                .pointer("/data/lock/schema")
                .and_then(Value::as_str),
            Some("musubi-lock")
        );
        assert_eq!(
            document
                .pointer("/data/lock/finalized_height")
                .and_then(Value::as_u64),
            Some(7)
        );
        for forbidden in ["cache_path", "provider_url", "credential", "bearer"] {
            assert!(!rendered.stdout().contains(forbidden));
        }

        let tree = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("tree"),
        ]);
        assert_eq!(tree.output.exit_code(), 0);
        let rendered = tree.output.render(tree.format).expect("tree output");
        assert!(
            rendered
                .stdout()
                .contains("apps.sora/demo exact lock graph")
        );
    }

    #[test]
    fn locked_fetch_rejects_legacy_lock_without_rewriting_it() {
        let temp = TempDir::new().expect("temporary directory");
        let (root, manifest_path) = create_test_package(&temp);
        let lock_path = root.join(LOCK_FILE_NAME);
        let legacy = b"schema = \"musubi-lock\"\nversion = 2\n";
        fs::write(&lock_path, legacy).expect("write legacy lock");

        let fetch = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("fetch"),
            OsString::from("--locked"),
        ]);
        assert_eq!(
            fetch.output.exit_code(),
            ErrorCode::LockfileLegacy.exit_code()
        );
        assert_eq!(fs::read(lock_path).expect("read lock"), legacy);
        let rendered = fetch
            .output
            .render(OutputFormat::Human)
            .expect("legacy diagnostic");
        assert!(
            rendered.stderr().contains("MUSUBI_E_LOCKFILE_LEGACY"),
            "{}",
            rendered.stderr()
        );
        assert!(rendered.stderr().contains("never rewrites retired formats"));
    }

    #[test]
    fn consumer_lock_is_not_used_as_package_or_cache_authentication() {
        let temp = TempDir::new().expect("temporary directory");
        let (root, manifest_path) = create_test_package(&temp);
        write_test_lock(&root);

        let package = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("package"),
            OsString::from("--list"),
        ]);
        assert_eq!(package.output.exit_code(), ErrorCode::Registry.exit_code());
        let rendered = package
            .output
            .render(OutputFormat::Human)
            .expect("package diagnostic");
        assert!(rendered.stderr().contains("publication inputs"));
        assert!(rendered.stderr().contains("never promoted"));

        let verify = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("cache"),
            OsString::from("verify"),
        ]);
        assert_eq!(verify.output.exit_code(), ErrorCode::Registry.exit_code());
        let rendered = verify
            .output
            .render(OutputFormat::Human)
            .expect("cache diagnostic");
        assert!(rendered.stderr().contains("MusubiArchiveCommitmentV1"));
        assert!(rendered.stderr().contains("CarBuildPlan"));
        assert!(rendered.stderr().contains("consumer lock is not"));
    }

    #[test]
    fn offline_fetch_with_a_valid_lock_requires_authenticated_cache_inputs() {
        let temp = TempDir::new().expect("temporary directory");
        let (root, manifest_path) = create_test_package(&temp);
        write_test_lock(&root);

        let fetch = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
            OsString::from("fetch"),
            OsString::from("--offline"),
        ]);
        assert_eq!(fetch.output.exit_code(), ErrorCode::OfflineMiss.exit_code());
        let rendered = fetch
            .output
            .render(OutputFormat::Human)
            .expect("offline diagnostic");
        assert!(rendered.stderr().contains("MusubiArchiveCommitmentV1"));
        assert!(rendered.stderr().contains("canonical SoraFS plans"));
    }

    #[test]
    fn deferred_command_never_falls_back_to_legacy_behavior() {
        let invocation = invoke(["musubi", "check", "--locked"]);
        assert_eq!(
            invocation.output.exit_code(),
            ErrorCode::Compiler.exit_code()
        );
        let rendered = invocation
            .output
            .render(OutputFormat::Human)
            .expect("deferred diagnostic");
        assert!(
            rendered
                .stderr()
                .contains("awaits the Musubi V1 resolver/compiler")
        );
        assert!(rendered.stderr().contains("retired pre-release workflow"));
    }

    #[test]
    fn publish_resume_accepts_only_a_canonical_detached_operation() {
        let operation = "0101010101010101010101010101010101010101010101010101010101010101";
        let parsed = Cli::try_parse_from(["musubi", "publish", "--resume", operation])
            .expect("canonical resume command");
        let Command::Publish(arguments) = parsed.command else {
            panic!("publish command expected");
        };
        assert_eq!(arguments.resume.expect("operation").to_string(), operation);
        assert!(!arguments.detach);

        assert!(
            Cli::try_parse_from(["musubi", "publish", "--resume", operation, "--detach"]).is_err(),
            "detach and resume are mutually exclusive"
        );
        assert!(
            Cli::try_parse_from([
                "musubi",
                "publish",
                "--resume",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ])
            .is_err(),
            "operation id uses canonical lowercase hex"
        );
    }
}
