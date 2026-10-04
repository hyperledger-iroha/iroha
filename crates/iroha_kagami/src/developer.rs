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
    /// Observe local validators and independently verified parent attachment separately.
    Status(ContextShowArgs),
    /// List the independently pinned network profiles supplied by this installation.
    Networks(NetworksArgs),
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
                match up(&store, &args.named.name, args.timeout) {
                    Ok(status) if status.phase == ManagedPhase::Ready => {
                        print_status(writer, &status, args.named.store.json)
                    }
                    Ok(status) => localnet_failure(
                        writer,
                        store.root(),
                        &args.named.name,
                        Some(&status),
                        args.named.store.json,
                        eyre!("localnet is not ready"),
                    ),
                    Err(error) => {
                        // Observation cannot replace the original error or create a generation.
                        let retained = store.status(&args.named.name).ok();
                        localnet_failure(
                            writer,
                            store.root(),
                            &args.named.name,
                            retained.as_ref(),
                            args.named.store.json,
                            error,
                        )
                    }
                }
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
                let store = args.store.open()?;
                store.reset(&args.name)?;
                print_reset(writer, store.root(), &args.name, args.store.json)
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
            Self::Up(args) => {
                let store = args.store.open()?;
                let request = DataspaceRequest {
                    name: args.name.unwrap_or_else(|| args.alias.clone()),
                    network: args.network,
                    alias: args.alias,
                    timeout: Duration::from_secs(args.timeout),
                };
                tui::status("Authenticating the installed parent and preparing private validators");
                match InstalledRuntime::discover()
                    .and_then(|runtime| store.up_dataspace(&runtime, &request))
                {
                    Ok(status) => print_dataspace_status(writer, &status, args.store.json),
                    Err(error) => {
                        let retained = store.dataspace_status(&request.name).ok().flatten();
                        dataspace_failure(
                            writer,
                            store.root(),
                            &request.name,
                            retained.as_ref(),
                            args.store.json,
                            error.into(),
                        )
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
                Ok(store.build_registry(&runtime, &registry_context, deadline)?)
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

fn managed_action(
    root: &Path,
    name: &str,
    command: &str,
    action: &str,
    json: bool,
) -> norito::json::Value {
    // The opened canonical store already binds --workspace and any relative --state input.
    // Never turn a non-UTF-8 native path into different executable arguments through lossiness.
    let argv = root.to_str().map(|root| {
        let mut args = vec![
            "kagami".to_owned(),
            command.to_owned(),
            action.to_owned(),
            name.to_owned(),
            "--state".to_owned(),
            root.to_owned(),
        ];
        if json {
            args.push("--json".to_owned());
        }
        args
    });
    norito::json!({
        "action": action,
        "name": name,
        "argv": argv,
        "argv_unavailable": (root.to_str().is_none().then_some("state_path_not_utf8")),
    })
}

fn print_managed_action(
    writer: &mut impl Write,
    root: &Path,
    name: &str,
    command: &str,
    action: &str,
) -> Outcome {
    // Separate fields are portable across shells and cannot disguise spaces as extra argv.
    writeln!(writer, "action: kagami {command} {action}")?;
    writeln!(writer, "  name: {name}")?;
    writeln!(writer, "  --state (quoted path): {root:?}")?;
    Ok(())
}

fn localnet_failure(
    writer: &mut impl Write,
    root: &Path,
    name: &str,
    status: Option<&ManagedStatus>,
    json: bool,
    original: color_eyre::Report,
) -> Outcome {
    // Even a failed writer must not replace the startup failure with a presentation error.
    let _ = (|| -> Outcome {
        if json {
            return write_json(
                writer,
                &norito::json!({
                    "status": status,
                    "recovery": [
                        (managed_action(root, name, "localnet", "status", true)),
                        (managed_action(root, name, "localnet", "logs", true)),
                    ],
                }),
            );
        }
        if let Some(status) = status {
            print_status(writer, status, false)?;
        }
        writeln!(
            writer,
            "Inspect the requested environment with these action fields:"
        )?;
        print_managed_action(writer, root, name, "localnet", "status")?;
        print_managed_action(writer, root, name, "localnet", "logs")
    })();
    Err(original)
}

fn dataspace_failure(
    writer: &mut impl Write,
    root: &Path,
    name: &str,
    status: Option<&ManagedDataspaceStatus>,
    json: bool,
    original: color_eyre::Report,
) -> Outcome {
    // Parent work keeps its original journals. Observation and output are best-effort;
    // neither may replace the actual attachment failure or suggest a new operation.
    let _ = (|| -> Outcome {
        if json {
            return write_json(
                writer,
                &norito::json!({
                    "status": status,
                    "recovery": [
                        (managed_action(root, name, "dataspace", "status", true)),
                        (managed_action(root, name, "localnet", "logs", true)),
                    ],
                }),
            );
        }
        if let Some(status) = status {
            print_dataspace_status(writer, status, false)?;
        }
        writeln!(
            writer,
            "Inspect the requested dataspace with these action fields:"
        )?;
        print_managed_action(writer, root, name, "dataspace", "status")?;
        print_managed_action(writer, root, name, "localnet", "logs")
    })();
    Err(original)
}

fn print_reset(writer: &mut impl Write, root: &Path, name: &str, json: bool) -> Outcome {
    if json {
        return write_json(
            writer,
            &norito::json!({
                "name": name,
                "state": "reset",
                "next": (managed_action(root, name, "localnet", "up", true)),
            }),
        );
    }
    writeln!(writer, "{name} reset; the next up creates a fresh ledger.")?;
    print_managed_action(writer, root, name, "localnet", "up")
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
    fn failed_start_reports_exact_store_actions_and_retains_the_original_error() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace with spaces");
        let state = temporary.path().join("state with spaces and 'quotes'");
        std::fs::create_dir(&workspace).unwrap();
        let cli = crate::Cli::try_parse_from([
            "kagami",
            "localnet",
            "up",
            "acme-dev",
            "--workspace",
            workspace.to_str().unwrap(),
            "--state",
            state.to_str().unwrap(),
            "--json",
        ])
        .unwrap();
        let crate::Command::Localnet(LocalnetCommand::Up(args)) = cli.command else {
            panic!("expected localnet startup");
        };
        let store = args.named.store.open().unwrap();
        let status = ManagedStatus {
            context: ManagedContext {
                name: args.named.name.clone(),
                chain_id: "public-fixture".into(),
                network_id: "public-network".into(),
                account_id: "public-account".into(),
                dataspace_id: 0,
                dataspace_alias: "universal".into(),
                torii_url: "http://127.0.0.1:8080/".into(),
                client_config: store.root().join("private-client.toml"),
            },
            phase: ManagedPhase::Failed,
            running_peers: 0,
            failure: Some("startup readiness deadline expired".into()),
        };
        for retained in [None, Some(&status)] {
            let original = std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "sensitive upstream detail",
            );
            let mut output = Vec::new();
            let error = localnet_failure(
                &mut output,
                store.root(),
                &args.named.name,
                retained,
                true,
                original.into(),
            )
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            let text = std::str::from_utf8(&output).unwrap();
            assert_eq!(text.lines().count(), 1);
            assert!(!text.contains("sensitive upstream detail"));
            let value: norito::json::Value = norito::json::from_slice(&output).unwrap();
            assert_eq!(
                value.get("status").unwrap(),
                &norito::json::to_value(&retained).unwrap()
            );
            let recovery = value.get("recovery").unwrap().as_array().unwrap();
            assert_eq!(recovery.len(), 2);
            for (action, expected) in recovery.iter().zip(["status", "logs"]) {
                let argv = action.get("argv").unwrap().as_array().unwrap();
                let argv: Vec<_> = argv.iter().map(|arg| arg.as_str().unwrap()).collect();
                assert_eq!(argv[2], expected);
                let parsed = crate::Cli::try_parse_from(argv).unwrap();
                let named = match parsed.command {
                    crate::Command::Localnet(LocalnetCommand::Status(named)) => named,
                    crate::Command::Localnet(LocalnetCommand::Logs(logs)) => logs.named,
                    _ => panic!("expected exact recovery action"),
                };
                assert_eq!(named.name, args.named.name);
                assert_eq!(named.store.state.as_deref(), Some(store.root()));
                assert!(named.store.json);
            }
        }
        let mut human = Vec::new();
        assert!(
            localnet_failure(
                &mut human,
                store.root(),
                &args.named.name,
                Some(&status),
                false,
                eyre!("original failure"),
            )
            .is_err()
        );
        let human = String::from_utf8(human).unwrap();
        assert!(human.contains("acme-dev: Failed"));
        assert!(human.contains("action: kagami localnet status"));
        assert!(human.contains("action: kagami localnet logs"));
        assert!(human.contains(&format!("--state (quoted path): {:?}", store.root())));
        assert!(!human.contains("original failure"));
    }

    #[test]
    fn failed_recovery_output_never_replaces_the_startup_cause() {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "closed",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        for (json, dataspace) in [(false, false), (true, false), (false, true), (true, true)] {
            let original = iroha_deploy::managed::Error::Timeout(Duration::from_secs(9));
            let error = if dataspace {
                dataspace_failure(
                    &mut BrokenWriter,
                    Path::new("state"),
                    "named",
                    None,
                    json,
                    original.into(),
                )
            } else {
                localnet_failure(
                    &mut BrokenWriter,
                    Path::new("state"),
                    "named",
                    None,
                    json,
                    original.into(),
                )
            }
            .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<iroha_deploy::managed::Error>(),
                Some(iroha_deploy::managed::Error::Timeout(duration))
                    if *duration == Duration::from_secs(9)
            ));
        }
    }

    #[test]
    fn reset_next_action_preserves_custom_name_and_space_containing_state() {
        let root = Path::new("/a workspace/private state");
        let mut output = Vec::new();
        print_reset(&mut output, root, "acme-dev", true).unwrap();
        let value: norito::json::Value = norito::json::from_slice(&output).unwrap();
        assert_eq!(value.get("state").and_then(|v| v.as_str()), Some("reset"));
        let argv = value
            .get("next")
            .unwrap()
            .get("argv")
            .unwrap()
            .as_array()
            .unwrap();
        let cli = crate::Cli::try_parse_from(argv.iter().map(|arg| arg.as_str().unwrap())).unwrap();
        let crate::Command::Localnet(LocalnetCommand::Up(args)) = cli.command else {
            panic!("reset must suggest an explicit startup");
        };
        assert_eq!(args.named.name, "acme-dev");
        assert_eq!(args.named.store.state.as_deref(), Some(root));
        assert!(args.named.store.json);
        let mut human = Vec::new();
        print_reset(&mut human, root, "acme-dev", false).unwrap();
        let human = String::from_utf8(human).unwrap();
        assert!(human.contains("acme-dev reset; the next up creates a fresh ledger."));
        assert!(human.contains("action: kagami localnet up"));
        assert!(human.contains("  name: acme-dev"));
        assert!(human.contains(&format!("--state (quoted path): {root:?}")));
    }

    #[cfg(unix)]
    #[test]
    fn recovery_does_not_emit_lossy_arguments_for_non_utf8_state_paths() {
        use std::os::unix::ffi::OsStringExt as _;
        let root = PathBuf::from(std::ffi::OsString::from_vec(b"/state/\xff".to_vec()));
        for command in ["localnet", "dataspace"] {
            let action = managed_action(&root, "named", command, "status", true);
            assert_eq!(action.get("argv"), Some(&norito::json::Value::Null));
            assert_eq!(
                action.get("argv_unavailable").and_then(|v| v.as_str()),
                Some("state_path_not_utf8")
            );
        }
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
        // A ready child must remain distinguishable from the failed parent attachment in
        // recovery output, including when no retained attachment was published at all.
        let root = Path::new("/workspace with spaces/state 'quoted'");
        for retained in [None, Some(&status)] {
            let mut output = Vec::new();
            let error = dataspace_failure(
                &mut output,
                root,
                "custom-local-name",
                retained,
                true,
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "original detail").into(),
            )
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            let text = std::str::from_utf8(&output).unwrap();
            assert_eq!(text.lines().count(), 1);
            assert!(!text.contains("original detail"));
            let value: norito::json::Value = norito::json::from_slice(&output).unwrap();
            assert_eq!(
                value.get("status").unwrap(),
                &norito::json::to_value(&retained).unwrap()
            );
            let recovery = value.get("recovery").unwrap().as_array().unwrap();
            assert_eq!(recovery.len(), 2);
            for (action, expected) in recovery.iter().zip(["dataspace", "localnet"]) {
                let argv = action.get("argv").unwrap().as_array().unwrap();
                let argv: Vec<_> = argv.iter().map(|arg| arg.as_str().unwrap()).collect();
                assert_eq!(argv[1], expected);
                let parsed = crate::Cli::try_parse_from(argv).unwrap();
                let (name, store) = match parsed.command {
                    crate::Command::Dataspace(DataspaceCommand::Status(args)) => {
                        (args.name.unwrap(), args.store)
                    }
                    crate::Command::Localnet(LocalnetCommand::Logs(args)) => {
                        (args.named.name, args.named.store)
                    }
                    _ => panic!("expected read-only dataspace recovery action"),
                };
                assert_eq!(name, "custom-local-name");
                assert_eq!(store.state.as_deref(), Some(root));
                assert!(store.json);
            }
        }
        let mut output = Vec::new();
        assert!(
            dataspace_failure(
                &mut output,
                root,
                "custom-local-name",
                Some(&status),
                false,
                eyre!("original detail"),
            )
            .is_err()
        );
        let human = String::from_utf8(output).unwrap();
        assert!(human.contains("Ready (4 validators)"));
        assert!(human.contains("parent taira: funding"));
        assert!(human.contains("action: kagami dataspace status"));
        assert!(human.contains("action: kagami localnet logs"));
        assert!(human.contains("  name: custom-local-name"));
        assert!(human.contains(&format!("--state (quoted path): {root:?}")));
        assert!(!human.contains("original detail"));
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
