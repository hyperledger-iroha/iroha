//! Config-free developer commands over shared managed environments and deployment services.

use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use clap::{Args, Subcommand};
use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_contract_deploy::{DeploymentPreflight, DeploymentProgress};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard, asset::AssetDefinitionId,
    transaction::FeePaymentIntent,
};
use iroha_deploy::managed::{
    self, InstalledRuntime, ManagedContext, ManagedPhase, ManagedStatus, ManagedStore,
    default_state_root, workspace_state_root,
};
use iroha_primitives::numeric::Quantity;
use musubi::deployment_runtime::{AliasSelection, ContractInput, DeploymentRuntime};

use crate::{Outcome, RunArgs, localnet, tui};

/// Store selection and output formatting for managed developer commands.
#[derive(Debug, Clone, Args)]
pub(crate) struct StoreArgs {
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
    fn open(&self) -> Result<ManagedStore> {
        let root = match &self.state {
            Some(path) => path.clone(),
            None => {
                let workspace = self
                    .workspace
                    .clone()
                    .map_or_else(std::env::current_dir, Ok)?;
                workspace_state_root(&default_state_root()?, &workspace)?
            }
        };
        ManagedStore::open(&root).map_err(Into::into)
    }
}

/// Manage a persistent native four-validator local network.
#[derive(Subcommand)]
pub(crate) enum LocalnetCommand {
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
}

#[derive(Debug, Args)]
pub(crate) struct NamedArgs {
    /// Managed environment name.
    #[arg(default_value = "local")]
    name: String,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Debug, Args)]
pub(crate) struct UpArgs {
    #[command(flatten)]
    named: NamedArgs,
    /// Complete generation and readiness budget in seconds.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=600))]
    timeout: u64,
}

#[derive(Debug, Args)]
pub(crate) struct LogsArgs {
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
pub(crate) struct ResetArgs {
    /// Exact environment to retire; required to make reset deliberate.
    name: String,
    #[command(flatten)]
    store: StoreArgs,
}

/// Select and inspect managed developer identities and endpoints.
#[derive(Debug, Subcommand)]
pub(crate) enum ContextCommand {
    /// List retained contexts in this workspace.
    List(StoreArgs),
    /// Show the selected context, or one exact named context.
    Show(ContextShowArgs),
    /// Select an existing context for subsequent developer commands.
    Use(ContextUseArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ContextShowArgs {
    name: Option<String>,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Debug, Args)]
pub(crate) struct ContextUseArgs {
    name: String,
    #[command(flatten)]
    store: StoreArgs,
}

/// Compile and deploy through the canonical native service.
#[derive(Debug, Subcommand)]
pub(crate) enum ContractCommand {
    /// Deploy source, bytecode, or a Musubi package; automatically start a default localnet if needed.
    Deploy(DeployArgs),
}

#[derive(Debug, Args)]
pub(crate) struct DeployArgs {
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
pub(crate) struct WorkerArgs {
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

impl<T: Write> RunArgs<T> for ContractCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self::Deploy(args) = self;
        // Reject local input mistakes before provisioning a network or selecting an identity.
        let input = if args.resume.is_none() {
            Some(ContractInput::from_path(
                args.input.as_deref().unwrap_or(Path::new(".")),
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
        if args.resume.is_some() {
            store.context(args.context.as_deref())?;
        }
        let context = store
            .ensure_selected(&runtime, args.context.as_deref(), Duration::from_secs(30))?
            .context;
        let config = context.load_client_config()?;
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let journal_root = store.root().join("deployments").join(&context.name);
        let runtime = DeploymentRuntime::new(config, journal_root);
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
        let deployed = if let Some(journal) = args.resume {
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
        if args.store.json {
            write_json(
                writer,
                &norito::json!({
                    "status": "applied",
                    "context": (context.name),
                    "receipt": (deployed.receipt),
                    "journal": (deployed.journal),
                }),
            )
        } else {
            writeln!(writer, "Deployed {}", deployed.receipt.contract_alias)?;
            writeln!(writer, "address: {}", deployed.receipt.contract_address)?;
            writeln!(writer, "code_hash: {}", deployed.receipt.code_hash)?;
            writeln!(writer, "journal: {}", deployed.journal.display())?;
            Ok(())
        }
    }
}

fn up(store: &ManagedStore, name: &str, timeout: u64) -> Result<ManagedStatus> {
    let runtime = InstalledRuntime::discover()?;
    store
        .up(&runtime.localnet_request(name, Duration::from_secs(timeout)))
        .map_err(Into::into)
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
