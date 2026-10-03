//! Config-free developer commands over shared managed environments and deployment services.

use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use clap::{Args, Subcommand};
use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_contract_deploy::{DeploymentPreflight, DeploymentProgress};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard, asset::AssetDefinitionId,
    transaction::FeePaymentIntent,
};
use iroha_deploy::managed::{
    self, DataspaceRequest, InstalledRuntime, ManagedContext, ManagedDataspaceStatus, ManagedPhase,
    ManagedStatus, ManagedStore, default_state_root, workspace_state_root,
};
use iroha_primitives::numeric::Quantity;
use musubi::archive_fetch::{
    MusubiArchiveDiscoveryErrorV1, PreparedProductionSorafsArchiveTransportV1,
};
use musubi::deployment_runtime::{AliasSelection, ContractInput, DeploymentRuntime};

use crate::{Outcome, RunArgs, localnet, tui};

/// Store selection and output formatting for managed developer commands.
#[derive(Debug, Clone, Args)]
pub struct StoreArgs {
    /// Private runtime store. By default each workspace has its own OS application-state directory.
    #[arg(long)]
    state: Option<PathBuf>,
    /// Workspace whose managed context is selected (defaults to the current directory).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Emit one public JSON result; progress remains on stderr.
    #[arg(long)]
    json: bool,
}

impl StoreArgs {
    fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        let workspace = self
            .workspace
            .clone()
            .map_or_else(std::env::current_dir, Ok)?;
        let workspace = workspace
            .canonicalize()
            .wrap_err("open selected workspace")?;
        ensure!(workspace.is_dir(), "selected workspace must be a directory");
        Ok(workspace.join(path))
    }

    fn open(&self) -> Result<ManagedStore> {
        let root = if let Some(path) = &self.state {
            path.clone()
        } else {
            let workspace = self
                .workspace
                .clone()
                .map_or_else(std::env::current_dir, Ok)?;
            workspace_state_root(&default_state_root()?, &workspace)?
        };
        ManagedStore::open(&root).map_err(Into::into)
    }
}

/// Manage a persistent native four-validator local network.
#[derive(Subcommand)]
pub enum LocalnetCommand {
    /// Generate, start, verify, and select a localnet without supplying configuration.
    Up(UpArgs),
    /// Observe the live supervisor and its retained environment.
    Status(NamedArgs),
    /// Read a bounded retained supervisor or validator log tail.
    Logs(LogsArgs),
    /// Stop the owned validators, preserving their identities and ledger.
    Down(NamedArgs),
    /// Explicitly retire one stopped generation so the next up creates a fresh ledger.
    Reset(ResetArgs),
    /// Generate an operator-owned network bundle without starting validators.
    Generate(localnet::Args),
    /// Validate an isolated native beacon configuration and opaque descriptor handoff.
    ValidateBeaconLaunch(localnet::ValidateBeaconLaunchArgs),
}

#[derive(Debug, Args)]
pub struct NamedArgs {
    /// Managed environment name.
    #[arg(default_value = "local")]
    name: String,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Debug, Args)]
pub struct UpArgs {
    #[command(flatten)]
    named: NamedArgs,
    /// Complete generation and readiness budget in seconds.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=600))]
    timeout: u64,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[command(flatten)]
    named: NamedArgs,
    /// Validator index (0..3). Omit to read the supervisor log.
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=3))]
    peer: Option<u8>,
    /// Maximum bytes returned from the end of the log.
    #[arg(long, default_value_t = 16384, value_parser = clap::value_parser!(u32).range(1..=1048576))]
    bytes: u32,
}

#[derive(Debug, Args)]
pub struct ResetArgs {
    /// Exact environment to retire; required to make reset deliberate.
    name: String,
    #[command(flatten)]
    store: StoreArgs,
}

/// Run an owner-private local dataspace attached to one independently installed parent.
#[derive(Debug, Subcommand)]
pub(crate) enum DataspaceCommand {
    /// Create, fund, register, and select four private validators without supplying configuration.
    Up(DataspaceUpArgs),
    /// Prepare a detached private cohort; parent registration remains separate.
    PreparePrivateRoot(PreparePrivateRootArgs),
    /// Capture a fresh live receipt for an independently selected private root.
    StartupReceipt(DataspaceStartupReceiptArgs),
    /// Observe local validators and independently verified parent attachment separately.
    Status(ContextShowArgs),
    /// List the independently pinned network profiles supplied by this installation.
    Networks(NetworksArgs),
}

#[derive(Debug, Args)]
pub(crate) struct PreparePrivateRootArgs {
    /// Exact canonical SNS alias whose native name hash selects the full dataspace identifier.
    alias: String,
    /// Independently selected parent NetworkId; never inferred from a remote response.
    #[arg(long)]
    parent_network_id: iroha_data_model::NetworkId,
    /// Complete local preparation and readiness budget in seconds.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=600))]
    timeout: u64,
    #[command(flatten)]
    store: StoreArgs,
}

fn detached_private_root_spec(
    args: &PreparePrivateRootArgs,
) -> Result<iroha_deploy::localnet::PrivateRootSpec> {
    use iroha_data_model::sns::{DATASPACE_ALIAS_SUFFIX_ID, NameSelectorV1};
    let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, &args.alias)?;
    ensure!(
        selector.normalized_label() == args.alias,
        "detached private alias must be canonical"
    );
    let spec = iroha_deploy::localnet::PrivateRootSpec {
        parent_network_id: args.parent_network_id,
        dataspace_id: iroha_model_base::topology::DataSpaceId::from_hash(&selector.name_hash()),
        dataspace_alias: args.alias.clone(),
    };
    spec.validate()?;
    Ok(spec)
}

#[derive(Debug, Args)]
pub(crate) struct DataspaceStartupReceiptArgs {
    /// Exact existing managed context; no workspace-selection fallback.
    name: String,
    /// Independently authenticated parent NetworkId.
    #[arg(long)]
    parent_network_id: iroha_data_model::NetworkId,
    /// Full-width native SNS-derived private dataspace identifier.
    #[arg(long)]
    dataspace_id: u64,
    /// Independently selected canonical paid alias.
    #[arg(long)]
    dataspace_alias: String,
    /// Fresh independently generated 32-byte lowercase hexadecimal challenge.
    #[arg(long)]
    challenge: String,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Debug, Args)]
pub(crate) struct DataspaceUpArgs {
    /// Canonical private dataspace alias to lease on the parent.
    alias: String,
    /// Exact independently installed parent profile, such as taira.
    #[arg(long)]
    network: String,
    /// Store-local context name (defaults to the dataspace alias).
    #[arg(long)]
    name: Option<String>,
    /// Complete parent authentication, local startup, and attachment budget in seconds.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=60))]
    timeout: u64,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Debug, Args)]
pub(crate) struct NetworksArgs {
    /// Emit the installed profile names as one JSON array.
    #[arg(long)]
    json: bool,
}

/// Select and inspect managed developer identities and endpoints.
#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// List retained contexts in this workspace.
    List(StoreArgs),
    /// Show the selected context, or one exact named context.
    Show(ContextShowArgs),
    /// Select an existing context for subsequent developer commands.
    Use(ContextUseArgs),
}

#[derive(Debug, Args)]
pub struct ContextShowArgs {
    name: Option<String>,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Debug, Args)]
pub struct ContextUseArgs {
    name: String,
    #[command(flatten)]
    store: StoreArgs,
}

/// Compile and deploy through the canonical native service.
#[derive(Debug, Subcommand)]
pub enum ContractCommand {
    /// Deploy source, bytecode, or a Musubi package; automatically start a default localnet if needed.
    Deploy(DeployArgs),
}

#[derive(Debug, Args)]
pub struct DeployArgs {
    /// .ko source, .to artifact, Musubi.toml, or package directory (defaults to the current directory).
    #[arg(conflicts_with = "resume")]
    input: Option<PathBuf>,
    #[command(flatten)]
    store: StoreArgs,
    /// Use an existing managed context without changing the workspace selection.
    #[arg(long)]
    context: Option<String>,
    /// Exact authorized contract alias; otherwise derive the name in the context's dataspace.
    #[arg(long, conflicts_with = "resume")]
    alias: Option<String>,
    /// Exact Musubi package when the input selects a workspace.
    #[arg(long, conflicts_with = "resume")]
    package: Option<String>,
    /// Exact target when a package has more than one contract.
    #[arg(long, conflicts_with = "resume")]
    contract: Option<String>,
    /// Require an unchanged Musubi dependency lock.
    #[arg(long, conflicts_with = "resume")]
    locked: bool,
    /// Recover this exact retained deployment without rebuilding or signing another plan.
    #[arg(long)]
    resume: Option<PathBuf>,
    /// Bound the aggregate quoted fees in their single fee asset before dispatch.
    #[arg(long, conflicts_with = "resume")]
    max_fee: Option<Quantity>,
}

/// Internal entry point for the shared native supervisor.
#[derive(Debug, Args)]
pub struct WorkerArgs {
    #[arg(long)]
    root: PathBuf,
    #[arg(long)]
    name: String,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=600000))]
    startup_timeout_ms: u64,
}

impl<T: Write> RunArgs<T> for WorkerArgs {
    fn run(self, _: &mut BufWriter<T>) -> Outcome {
        let store = ManagedStore::open(&self.root)?;
        managed::run_worker(
            &store,
            &self.name,
            Duration::from_millis(self.startup_timeout_ms),
        )?;
        Ok(())
    }
}

impl<T: Write> RunArgs<T> for LocalnetCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::Generate(args) => args.run(writer),
            Self::ValidateBeaconLaunch(args) => args.run(writer),
            Self::Up(args) => {
                let store = args.named.store.open()?;
                let status = up(&store, &args.named.name, args.timeout)?;
                print_status(writer, &status, args.named.store.json)?;
                ensure!(
                    status.phase == ManagedPhase::Ready,
                    "localnet is not ready; inspect its status and logs"
                );
                Ok(())
            }
            Self::Status(args) => print_status(
                writer,
                &args.store.open()?.status(&args.name)?,
                args.store.json,
            ),
            Self::Down(args) => print_status(
                writer,
                &args.store.open()?.down(&args.name)?,
                args.store.json,
            ),
            Self::Reset(args) => {
                args.store.open()?.reset(&args.name)?;
                if args.store.json {
                    write_json(
                        writer,
                        &norito::json!({"name": (args.name), "state": "reset"}),
                    )
                } else {
                    writeln!(
                        writer,
                        "{} reset; run `kagami localnet up {}` to create a fresh ledger",
                        args.name, args.name
                    )?;
                    Ok(())
                }
            }
            Self::Logs(args) => {
                let logs = args.named.store.open()?.logs(
                    &args.named.name,
                    args.peer.map(usize::from),
                    args.bytes as usize,
                )?;
                if args.named.store.json {
                    write_json(
                        writer,
                        &norito::json!({"name": (args.named.name), "log": logs}),
                    )
                } else {
                    write!(writer, "{logs}")?;
                    Ok(())
                }
            }
        }
    }
}

impl<T: Write> RunArgs<T> for ContextCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::List(args) => {
                let contexts = args.open()?.contexts()?;
                if args.json {
                    return write_json(writer, &contexts);
                }
                for context in contexts {
                    print_context(writer, &context, false)?;
                }
                Ok(())
            }
            Self::Show(args) => print_context(
                writer,
                &args.store.open()?.context(args.name.as_deref())?,
                args.store.json,
            ),
            Self::Use(args) => print_context(
                writer,
                &args.store.open()?.select(&args.name)?,
                args.store.json,
            ),
        }
    }
}

impl<T: Write> RunArgs<T> for DataspaceCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::PreparePrivateRoot(args) => {
                let spec = detached_private_root_spec(&args)?;
                let runtime = InstalledRuntime::discover()?;
                let store = args.store.open()?;
                let request =
                    runtime.localnet_request(&args.alias, Duration::from_secs(args.timeout));
                let status = store.up_private_root(&request, &spec)?;
                ensure!(
                    status.phase == ManagedPhase::Ready,
                    "detached private cohort is not ready"
                );
                write_json(
                    writer,
                    &norito::json!({
                        "schema": "iroha-managed-detached-private-root",
                        "schema_version": 1,
                        "detached": true,
                        "parent_attachment": "parent_unconfirmed",
                        "private_root": spec,
                        "local": status,
                    }),
                )
            }
            Self::Up(args) => {
                let runtime = InstalledRuntime::discover()?;
                let store = args.store.open()?;
                let request = DataspaceRequest {
                    name: args.name.unwrap_or_else(|| args.alias.clone()),
                    network: args.network,
                    alias: args.alias,
                    timeout: Duration::from_secs(args.timeout),
                };
                tui::status("Authenticating the installed parent and preparing private validators");
                match store.up_dataspace(&runtime, &request) {
                    Ok(status) => print_dataspace_status(writer, &status, args.store.json),
                    Err(error) => {
                        // Partial work remains under the same owner and exact journals. Emit a
                        // safe observation when available while preserving a nonzero outcome.
                        if let Ok(Some(status)) = store.dataspace_status(&request.name) {
                            print_dataspace_status(writer, &status, args.store.json)?;
                        }
                        Err(error.into())
                    }
                }
            }
            Self::Status(args) => {
                let store = args.store.open()?;
                let context = store.context(args.name.as_deref())?;
                let status = store.dataspace_status(&context.name)?.ok_or_else(|| {
                    eyre!(
                        "context `{}` has no remote dataspace attachment",
                        context.name
                    )
                })?;
                print_dataspace_status(writer, &status, args.store.json)
            }
            Self::StartupReceipt(args) => {
                let spec = iroha_deploy::localnet::PrivateRootSpec {
                    parent_network_id: args.parent_network_id,
                    dataspace_id: iroha_model_base::topology::DataSpaceId::new(args.dataspace_id),
                    dataspace_alias: args.dataspace_alias,
                };
                let receipt = args.store.open()?.private_startup_receipt(
                    &args.name,
                    &spec,
                    &args.challenge,
                )?;
                write_json(writer, &receipt)
            }
            Self::Networks(args) => {
                let profiles = InstalledRuntime::discover()?.network_profiles()?;
                let names: Vec<_> = profiles.names().collect();
                if args.json {
                    write_json(writer, &names)
                } else {
                    for name in names {
                        writeln!(writer, "{name}")?;
                    }
                    Ok(())
                }
            }
        }
    }
}

impl<T: Write> RunArgs<T> for ContractCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self::Deploy(args) = self;
        // Reject local input mistakes before provisioning a network or selecting an identity.
        let resume = args
            .resume
            .as_ref()
            .map(|path| args.store.resolve_path(path))
            .transpose()?;
        let input = if resume.is_none() {
            let path = args
                .store
                .resolve_path(args.input.as_deref().unwrap_or_else(|| Path::new(".")))?;
            Some(ContractInput::from_path(
                &path,
                args.package,
                args.contract,
                args.locked,
            )?)
        } else {
            None
        };
        let exact_alias = args
            .alias
            .map(|alias| alias.parse().wrap_err("invalid contract alias"))
            .transpose()?;
        let store = args.store.open()?;
        let runtime = InstalledRuntime::discover()?;
        // Recovery must retain an explicit or previously selected identity. It never creates
        // another default environment merely because its original selection disappeared.
        if resume.is_some() {
            store.context(args.context.as_deref())?;
        }
        let context = store
            .ensure_selected(&runtime, args.context.as_deref(), Duration::from_secs(30))?
            .context;
        let target = store.capture_deployment(&context)?;
        let config = context.load_client_config()?;
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let journal_root = store.root().join("deployments").join(&context.name);
        let registry_root = store.root().to_path_buf();
        let registry_context = context.name.clone();
        let runtime = DeploymentRuntime::new(config, journal_root).with_build_registry_resolver(
            Arc::new(move || {
                // The canonical package service calls this only when a graph needs its exact
                // registry identity. Source, bytecode and local packages never enter it.
                let deadline = Instant::now() + Duration::from_secs(60);
                let store = ManagedStore::open(&registry_root)?;
                let Some(registry) = store.build_registry(&runtime, &registry_context, deadline)?
                else {
                    return Ok(None);
                };
                let parent = registry.config().clone();
                let transport = PreparedProductionSorafsArchiveTransportV1::from_account_registry(
                    parent.clone(),
                    Arc::new(move |provider| {
                        registry.discover(provider, deadline).map_err(|error| {
                            if Instant::now() >= deadline {
                                MusubiArchiveDiscoveryErrorV1::Deadline
                            } else {
                                match error {
                                    iroha_deploy::bootstrap::BootstrapError::Busy
                                    | iroha_deploy::bootstrap::BootstrapError::Io(_) => {
                                        MusubiArchiveDiscoveryErrorV1::Unavailable
                                    }
                                    _ => MusubiArchiveDiscoveryErrorV1::Rejected,
                                }
                            }
                        })
                    }),
                    Duration::from_secs(30),
                )?;
                Ok(Some((parent, transport)))
            }),
        );
        let mut progress = deployment_progress;
        let mut review = |preflight: &DeploymentPreflight| {
            ensure!(
                preflight.dataspace_id.as_u64() == context.dataspace_id,
                "deployment resolved outside the selected dataspace"
            );
            check_fee_budget(
                preflight
                    .fee_quotes
                    .iter()
                    .flat_map(|quote| &quote.components)
                    .map(|component| (&component.asset_definition_id, &component.max_amount)),
                args.max_fee.as_ref(),
            )
        };
        let deployed = if let Some(journal) = resume {
            runtime.resume(&journal, &mut review, &mut progress)?
        } else {
            let input = input.ok_or_else(|| eyre!("deployment input is missing"))?;
            let alias = match exact_alias {
                Some(alias) => AliasSelection::Exact(alias),
                None => AliasSelection::Scope {
                    domain: None,
                    dataspace: context.dataspace_alias.clone(),
                },
            };
            runtime.deploy(
                &input,
                &alias,
                FeePaymentIntent::authority(Vec::new(), None),
                &mut review,
                &mut progress,
            )?
        };
        let report = target.finish(&store, deployed.receipt, deployed.journal)?;
        if args.store.json {
            write_json(writer, &report.to_json()?)
        } else {
            writeln!(
                writer,
                "{}: {}",
                report.execution_summary(),
                report.receipt.contract_alias
            )?;
            writeln!(writer, "address: {}", report.receipt.contract_address)?;
            writeln!(writer, "code_hash: {}", report.receipt.code_hash)?;
            writeln!(writer, "journal: {}", report.journal.display())?;
            if let Some(parent) = report.parent_summary() {
                writeln!(writer, "{parent}")?;
            }
            Ok(())
        }
    }
}

fn up(store: &ManagedStore, name: &str, timeout: u64) -> Result<ManagedStatus> {
    let runtime = InstalledRuntime::discover()?;
    let mut request = runtime.localnet_request(name, Duration::from_secs(timeout));
    match store.prepared(name) {
        Ok(prepared) => {
            request.service_profile = prepared.service_profile;
            store.up_retained(&request).map_err(Into::into)
        }
        Err(iroha_deploy::managed::Error::Io(error))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            store.up(&request).map_err(Into::into)
        }
        Err(error) => Err(error.into()),
    }
}

fn check_fee_budget<'a>(
    components: impl Iterator<Item = (&'a AssetDefinitionId, &'a Quantity)>,
    limit: Option<&Quantity>,
) -> Result<()> {
    let Some(limit) = limit else {
        return Ok(());
    };
    let mut asset = None;
    let mut total = Quantity::zero();
    for (asset_definition_id, max_amount) in components {
        if let Some(previous) = asset {
            ensure!(
                previous == asset_definition_id,
                "--max-fee cannot combine different fee assets"
            );
        }
        asset = Some(asset_definition_id);
        total = total
            .checked_add(max_amount)
            .map_err(|_| eyre!("aggregate fee overflow"))?;
    }
    ensure!(
        &total <= limit,
        "aggregate quoted fees {total} exceed --max-fee {limit}"
    );
    Ok(())
}

fn deployment_progress(progress: DeploymentProgress) {
    match progress {
        DeploymentProgress::Prepared(_) => tui::status("Deployment prepared with exact fee quotes"),
        DeploymentProgress::Submitting(stage) => tui::status(format!(
            "Submitting {} ({}/{})",
            stage.name, stage.number, stage.total
        )),
        DeploymentProgress::Recovering(stage) => {
            tui::status(format!("Recovering {} by its retained hash", stage.name))
        }
        DeploymentProgress::Applied { stage, .. } => tui::status(format!("{} applied", stage.name)),
        DeploymentProgress::ReadingBack { .. } => tui::status("Verifying deployed code and alias"),
    }
}

fn print_status(writer: &mut impl Write, status: &ManagedStatus, json: bool) -> Outcome {
    if json {
        return write_json(writer, status);
    }
    writeln!(
        writer,
        "{}: {:?} ({} validators)",
        status.context.name, status.phase, status.running_peers
    )?;
    print_context(writer, &status.context, false)?;
    if let Some(reason) = &status.failure {
        writeln!(writer, "reason: {reason}")?;
    }
    Ok(())
}

fn print_dataspace_status(
    writer: &mut impl Write,
    status: &ManagedDataspaceStatus,
    json: bool,
) -> Outcome {
    if json {
        return write_json(writer, status);
    }
    print_status(writer, &status.local, false)?;
    writeln!(
        writer,
        "parent {}: {}",
        status.attachment.network,
        status.attachment.stage.as_str()
    )?;
    if let Some(wallet) = &status.attachment.wallet_status {
        writeln!(writer, "parent operation: {wallet}")?;
    }
    if let Some(confirmed) = &status.attachment.parent_confirmed {
        writeln!(
            writer,
            "last verified parent receipt: height {}",
            confirmed.parent_height
        )?;
    }
    if let Some(reason) = &status.attachment.failure {
        writeln!(writer, "attachment: {reason}")?;
    }
    Ok(())
}

fn print_context(writer: &mut impl Write, context: &ManagedContext, json: bool) -> Outcome {
    if json {
        return write_json(writer, context);
    }
    writeln!(writer, "{}  {}", context.name, context.torii_url)?;
    writeln!(writer, "network: {}", context.network_id)?;
    writeln!(writer, "account: {}", context.account_id)?;
    Ok(())
}

fn write_json(writer: &mut impl Write, value: &impl norito::json::JsonSerialize) -> Outcome {
    writeln!(writer, "{}", norito::json::to_json(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn relative_inputs_and_journals_use_the_selected_workspace() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("project");
        std::fs::create_dir(&workspace).unwrap();
        let args = StoreArgs {
            workspace: Some(workspace.clone()),
            state: None,
            json: false,
        };
        for relative in ["hello.ko", "Musubi.toml", "journals/original"] {
            assert_eq!(
                args.resolve_path(Path::new(relative)).unwrap(),
                workspace.canonicalize().unwrap().join(relative)
            );
        }
        let absolute = temporary.path().join("elsewhere.to");
        assert_eq!(args.resolve_path(&absolute).unwrap(), absolute);
        let missing = StoreArgs {
            workspace: Some(workspace.join("missing")),
            ..args
        };
        assert!(missing.resolve_path(Path::new("hello.ko")).is_err());
    }

    #[test]
    fn localnet_up_needs_no_configuration() {
        let cli = crate::Cli::try_parse_from(["kagami", "localnet", "up"]).unwrap();
        assert!(matches!(
            cli.command,
            crate::Command::Localnet(LocalnetCommand::Up(_))
        ));
        assert!(crate::Cli::try_parse_from(["kagami", "localnet", "reset"]).is_err());
        assert!(crate::Cli::try_parse_from(["kagami", "localnet-wizard"]).is_err());
    }

    #[test]
    fn dataspace_up_needs_only_alias_and_explicit_installed_parent() {
        let cli = crate::Cli::try_parse_from([
            "kagami",
            "dataspace",
            "up",
            "privateapp",
            "--network",
            "taira",
        ])
        .unwrap();
        let crate::Command::Dataspace(DataspaceCommand::Up(args)) = cli.command else {
            panic!("expected private dataspace startup");
        };
        assert_eq!(args.alias, "privateapp");
        assert_eq!(args.network, "taira");
        assert!(args.name.is_none());
        assert_eq!(args.timeout, 60);
        assert!(args.store.state.is_none());
        assert!(crate::Cli::try_parse_from(["kagami", "dataspace", "up", "privateapp"]).is_err());
        for timeout in ["0", "61"] {
            assert!(
                crate::Cli::try_parse_from([
                    "kagami",
                    "dataspace",
                    "up",
                    "privateapp",
                    "--network",
                    "taira",
                    "--timeout",
                    timeout,
                ])
                .is_err()
            );
        }
        assert!(crate::Cli::try_parse_from(["kagami", "dataspace", "status", "--json"]).is_ok());
        assert!(crate::Cli::try_parse_from(["kagami", "dataspace", "networks", "--json"]).is_ok());
    }

    #[test]
    fn detached_private_candidate_derives_native_is_identity_without_parent_admission() {
        let parent = "hash:B2D63D8AE5A9415319B219D9BC0E72B44FC88ED67F89BF35E400F8E5BFEC6F7B#B780";
        let cli = crate::Cli::try_parse_from([
            "kagami",
            "dataspace",
            "prepare-private-root",
            "is",
            "--parent-network-id",
            parent,
            "--json",
        ])
        .unwrap();
        let crate::Command::Dataspace(DataspaceCommand::PreparePrivateRoot(args)) = cli.command
        else {
            panic!("expected detached private root candidate");
        };
        let spec = detached_private_root_spec(&args).unwrap();
        assert_eq!(spec.dataspace_alias, "is");
        assert_eq!(spec.dataspace_id.as_u64(), 6_647_857_470_246_403_404);
        assert_eq!(spec.parent_network_id.to_string(), parent);
        assert!(
            crate::Cli::try_parse_from([
                "kagami",
                "dataspace",
                "prepare-private-root",
                "is",
                "--json",
            ])
            .is_err()
        );
        assert!(
            crate::Cli::try_parse_from([
                "kagami",
                "dataspace",
                "prepare-private-root",
                "is",
                "--parent-network-id",
                parent,
                "--dataspace-id",
                "7",
            ])
            .is_err()
        );
        let cli = crate::Cli::try_parse_from([
            "kagami",
            "dataspace",
            "prepare-private-root",
            "IS",
            "--parent-network-id",
            parent,
        ])
        .unwrap();
        let crate::Command::Dataspace(DataspaceCommand::PreparePrivateRoot(args)) = cli.command
        else {
            unreachable!()
        };
        assert!(detached_private_root_spec(&args).is_err());
    }

    #[test]
    fn dataspace_output_separates_local_readiness_from_parent_evidence() {
        use iroha_deploy::managed::{
            ManagedAttachmentFailure, ManagedAttachmentPhase, ManagedAttachmentStatus,
        };
        let status = ManagedDataspaceStatus {
            local: ManagedStatus {
                context: ManagedContext {
                    name: "privateapp".into(),
                    chain_id: "fixture".into(),
                    network_id: "fixture-network".into(),
                    account_id: "fixture-account".into(),
                    dataspace_id: 42,
                    dataspace_alias: "privateapp".into(),
                    torii_url: "http://127.0.0.1:8080/".into(),
                    client_config: PathBuf::from("/private/fixture"),
                },
                phase: ManagedPhase::Ready,
                running_peers: 4,
                failure: None,
            },
            attachment: ManagedAttachmentStatus {
                network: "taira".into(),
                stage: ManagedAttachmentPhase::Funding,
                wallet_status: None,
                local_successor: None,
                parent_confirmed: None,
                failure: Some(ManagedAttachmentFailure::PreparationFailed),
            },
        };
        let mut text = Vec::new();
        print_dataspace_status(&mut text, &status, false).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("Ready (4 validators)"));
        assert!(text.contains("parent taira: funding"));
        assert!(text.contains("attachment: operation preparation failed"));
        assert!(!text.contains("last verified parent receipt"));
        let mut json = Vec::new();
        print_dataspace_status(&mut json, &status, true).unwrap();
        assert_eq!(
            norito::json::from_slice::<ManagedDataspaceStatus>(&json).unwrap(),
            status
        );
    }

    #[test]
    fn deployment_source_needs_no_manifest_or_config() {
        for source in ["hello.ko", "hello.to", "."] {
            let cli = crate::Cli::try_parse_from(["kagami", "contract", "deploy", source]).unwrap();
            assert!(matches!(
                cli.command,
                crate::Command::Contract(ContractCommand::Deploy(_))
            ));
        }
        assert!(
            crate::Cli::try_parse_from([
                "kagami", "contract", "deploy", "hello.ko", "--resume", "journal"
            ])
            .is_err()
        );
    }

    #[test]
    fn invalid_local_inputs_do_not_provision_or_select_a_network() {
        let temporary = tempfile::tempdir().unwrap();
        let state = temporary.path().join("state");
        let unsupported = temporary.path().join("contract.txt");
        std::fs::write(&unsupported, "unsupported input").unwrap();
        let source = temporary.path().join("contract.ko");
        std::fs::write(&source, "seiyaku Test {}").unwrap();
        for (path, alias) in [(&unsupported, None), (&source, Some("not an alias"))] {
            let mut args = vec![
                "kagami",
                "contract",
                "deploy",
                path.to_str().unwrap(),
                "--state",
                state.to_str().unwrap(),
            ];
            if let Some(alias) = alias {
                args.extend(["--alias", alias]);
            }
            let cli = crate::Cli::try_parse_from(args).unwrap();
            let mut output = BufWriter::new(Vec::new());
            assert!(cli.command.run(&mut output).is_err());
            assert!(!state.exists());
        }
    }

    #[test]
    fn fee_budget_bounds_the_whole_deployment_and_rejects_mixed_assets() {
        let mut bytes = [0; 16];
        bytes[6] = 0x40;
        bytes[8] = 0x80;
        let xor = AssetDefinitionId::from_uuid_bytes(bytes).unwrap();
        bytes[0] = 1;
        let other = AssetDefinitionId::from_uuid_bytes(bytes).unwrap();
        let first = Quantity::from(7_u32);
        let second = Quantity::from(5_u32);
        let exact = Quantity::from(12_u32);
        assert!(
            check_fee_budget([(&xor, &first), (&xor, &second)].into_iter(), Some(&exact)).is_ok()
        );
        assert!(
            check_fee_budget(
                [(&xor, &first), (&xor, &second)].into_iter(),
                Some(&Quantity::from(11_u32))
            )
            .is_err()
        );
        assert!(
            check_fee_budget(
                [(&xor, &first), (&other, &second)].into_iter(),
                Some(&exact)
            )
            .is_err()
        );
        assert!(check_fee_budget(std::iter::empty(), Some(&Quantity::zero())).is_ok());
        assert!(check_fee_budget([(&xor, &first)].into_iter(), None).is_ok());
    }
}
