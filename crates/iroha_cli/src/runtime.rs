//! Runtime upgrade helpers for CLI

use eyre::Result;

use crate::{CliOutputFormat, Run, RunContext};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Runtime ABI helpers
    #[command(subcommand)]
    Abi(AbiCommand),
    /// Runtime upgrade management
    #[command(subcommand)]
    Upgrade(UpgradeCommand),
    /// Show runtime metrics/status summary
    Status,
    /// Fetch node capability advert (ABI + crypto manifest)
    Capabilities,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Abi(cmd) => cmd.run(context),
            Command::Upgrade(cmd) => cmd.run(context),
            Command::Status => {
                let client = context.client_from_config();
                let value = client.get_runtime_metrics_json()?;
                // Compose a short one-line summary
                let abi_version = value
                    .get("abi_version")
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                let up = value
                    .get("upgrade_events_total")
                    .and_then(|v| v.as_object());
                let proposed = up
                    .and_then(|m| m.get("proposed"))
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                let activated = up
                    .and_then(|m| m.get("activated"))
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                let canceled = up
                    .and_then(|m| m.get("canceled"))
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                let summary = format!(
                    "runtime: abi_version={abi_version} proposed={proposed} activated={activated} canceled={canceled}"
                );
                match context.output_format() {
                    CliOutputFormat::Text => context.println(summary)?,
                    CliOutputFormat::Json => context.print_data(&value)?,
                }
                Ok(())
            }
            Command::Capabilities => {
                let client = context.client_from_config();
                let value = client.get_node_capabilities_json()?;
                let sm_enabled = value
                    .get("crypto")
                    .and_then(|v| v.get("sm"))
                    .and_then(|v| v.get("enabled"))
                    .and_then(norito::json::Value::as_bool)
                    .unwrap_or(false);
                let policy = value
                    .get("crypto")
                    .and_then(|v| v.get("sm"))
                    .and_then(|v| v.get("acceleration"))
                    .and_then(|v| v.get("policy"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let openssl_preview = value
                    .get("crypto")
                    .and_then(|v| v.get("sm"))
                    .and_then(|v| v.get("openssl_preview"))
                    .and_then(norito::json::Value::as_bool)
                    .unwrap_or(false);
                let summary = format!(
                    "node capabilities: sm_enabled={sm_enabled} policy={policy} openssl_preview={openssl_preview}"
                );
                match context.output_format() {
                    CliOutputFormat::Text => context.println(summary)?,
                    CliOutputFormat::Json => context.print_data(&value)?,
                }
                Ok(())
            }
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum AbiCommand {
    /// Fetch the active ABI version from the node
    Active,
    /// Fetch the active ABI version via signed Norito query (core /query)
    ActiveQuery,
    /// Fetch the node's canonical ABI hash for the active policy
    Hash,
}

impl Run for AbiCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            AbiCommand::Active => {
                let client = context.client_from_config();
                let value = client.get_runtime_abi_active_json()?;
                context.print_data(&value)
            }
            AbiCommand::ActiveQuery => {
                let client = context.client_from_config();
                let out: iroha::data_model::query::runtime::AbiVersion =
                    client.query_single(iroha::data_model::query::runtime::prelude::FindAbiVersion)?;
                context.print_data(&out)
            }
            AbiCommand::Hash => {
                let client = context.client_from_config();
                let value = client.get_runtime_abi_hash_json()?;
                context.print_data(&value)
            }
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum UpgradeCommand {
    /// List proposed/activated runtime upgrades
    List,
    /// Build a `ProposeRuntimeUpgrade` instruction skeleton via Torii
    Propose(ProposeArgs),
    /// Build an `ActivateRuntimeUpgrade` instruction skeleton via Torii
    Activate(ActivateArgs),
    /// Build a `CancelRuntimeUpgrade` instruction skeleton via Torii
    Cancel(CancelArgs),
}

impl Run for UpgradeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            UpgradeCommand::List => {
                let client = context.client_from_config();
                let value = client.get_runtime_upgrades_json()?;
                context.print_data(&value)
            }
            UpgradeCommand::Propose(args) => args.run(context),
            UpgradeCommand::Activate(args) => args.run(context),
            UpgradeCommand::Cancel(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ProposeArgs {
    /// Path to a JSON file with `RuntimeUpgradeManifest` fields
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
}

impl Run for ProposeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let s = std::fs::read_to_string(&self.file)?;
        let value: norito::json::Value = norito::json::from_str(&s)?;
        let out = client.post_runtime_propose_upgrade_json(&value)?;
        context.print_data(&out)
    }
}

#[derive(clap::Args, Debug)]
pub struct ActivateArgs {
    /// Upgrade id (hex)
    #[arg(long, value_name = "HEX")]
    id: String,
}

impl Run for ActivateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let out = client.post_runtime_activate_upgrade_json(&self.id)?;
        context.print_data(&out)
    }
}

#[derive(clap::Args, Debug)]
pub struct CancelArgs {
    /// Upgrade id (hex)
    #[arg(long, value_name = "HEX")]
    id: String,
}

impl Run for CancelArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let out = client.post_runtime_cancel_upgrade_json(&self.id)?;
        context.print_data(&out)
    }
}
