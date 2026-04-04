//! Contracts helpers.

use std::{collections::BTreeMap, path::PathBuf, str::FromStr, sync::Arc};

use base64::Engine as _;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    data_model::{
        isi::contract_alias::SetContractAlias,
        metadata::Metadata,
        prelude::*,
        transaction::{IvmBytecode, TransactionBuilder},
    },
};
use iroha_core::{
    pipeline::overlay::build_overlay_for_transaction_with_accounts,
    smartcontracts::ivm::{cache::ProgramSummary, host::CoreHost},
};
use iroha_crypto::{Hash, KeyPair, PrivateKey};
use ivm::{PointerType, host::IVMHost};
use reqwest::StatusCode;

use crate::{Run, RunContext, TransactionWaitArgs, wait_for_transaction_status};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Contract code helpers
    #[command(subcommand)]
    Code(CodeCommand),
    /// Contract alias helpers
    #[command(subcommand)]
    Alias(AliasCommand),
    /// Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)
    Deploy(DeployArgs),
    /// Derive a canonical contract address locally from authority, deploy nonce, and dataspace
    DeriveAddress(DeriveAddressArgs),
    /// Submit a contract call through Torii (POST /v1/contracts/call)
    Call(CallArgs),
    /// Execute a read-only contract view through Torii (POST /v1/contracts/view)
    View(ViewArgs),
    /// Execute a read-only contract view locally against compiled bytecode and optional fixtures
    DebugView(DebugViewArgs),
    /// Execute a public contract entrypoint locally against compiled bytecode and optional fixtures
    DebugCall(DebugCallArgs),
    /// Contract manifest helpers
    #[command(subcommand)]
    Manifest(ManifestCommand),
    /// Run an offline simulation of IVM bytecode to see the queued ISIs and header metadata
    Simulate(SimulateArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Code(cmd) => cmd.run(context),
            Command::Alias(cmd) => cmd.run(context),
            Command::Deploy(args) => args.run(context),
            Command::DeriveAddress(args) => args.run(context),
            Command::Call(args) => args.run(context),
            Command::View(args) => args.run(context),
            Command::DebugView(args) => args.run(context),
            Command::DebugCall(args) => args.run(context),
            Command::Manifest(cmd) => cmd.run(context),
            Command::Simulate(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum CodeCommand {
    /// Fetch on-chain contract code bytes by code hash and write to a file
    Get(CodeBytesGetArgs),
}

#[derive(clap::Subcommand, Debug)]
pub enum AliasCommand {
    /// Lease or renew an on-chain contract alias for a contract address
    Lease(ContractAliasLeaseArgs),
    /// Release the current on-chain alias binding for a contract address
    Release(ContractAliasReleaseArgs),
    /// Resolve an on-chain contract alias to its current canonical contract address
    Resolve(ContractAliasResolveArgs),
}

impl Run for AliasCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            AliasCommand::Lease(args) => args.run(context),
            AliasCommand::Release(args) => args.run(context),
            AliasCommand::Resolve(args) => args.run(context),
        }
    }
}

impl Run for CodeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            CodeCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ManifestCommand {
    /// Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)
    Get(ManifestArgs),
    /// Inspect the manifest embedded in compiled bytecode (with optional signing)
    Build(BuildManifestArgs),
}

impl Run for ManifestCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ManifestCommand::Get(args) => args.run(context),
            ManifestCommand::Build(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct CodeBytesGetArgs {
    /// Hex-encoded 32-byte code hash (0x optional)
    #[arg(long, value_name = "HEX64")]
    pub code_hash: String,
    /// Output path to write the `.to` bytes
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
}

impl Run for CodeBytesGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let code_hash = self.code_hash.trim_start_matches("0x");
        let bytes = client.get_contract_code_bytes(code_hash)?;
        std::fs::write(&self.out, &bytes)?;
        context.println(format_args!(
            "Wrote {} bytes to {}",
            bytes.len(),
            self.out.display()
        ))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ContractAliasLeaseArgs {
    /// Canonical contract address to bind.
    #[arg(long)]
    pub contract_address: String,
    /// Alias literal in `name::domain.dataspace` or `name::dataspace` format.
    #[arg(long)]
    pub contract_alias: String,
    /// Optional lease expiry timestamp in unix milliseconds. Omit for a permanent binding.
    #[arg(long)]
    pub lease_expiry_ms: Option<u64>,
}

impl Run for ContractAliasLeaseArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let contract_address = self
            .contract_address
            .parse()
            .wrap_err("invalid --contract-address")?;
        let contract_alias = self
            .contract_alias
            .parse()
            .wrap_err("invalid --contract-alias")?;
        context.submit(vec![InstructionBox::from(SetContractAlias::bind(
            contract_address,
            contract_alias,
            self.lease_expiry_ms,
        ))])
    }
}

#[derive(clap::Args, Debug)]
pub struct ContractAliasReleaseArgs {
    /// Canonical contract address whose alias binding should be cleared.
    #[arg(long)]
    pub contract_address: String,
}

impl Run for ContractAliasReleaseArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let contract_address = self
            .contract_address
            .parse()
            .wrap_err("invalid --contract-address")?;
        context.submit(vec![InstructionBox::from(SetContractAlias::clear(
            contract_address,
        ))])
    }
}

#[derive(clap::Args, Debug)]
pub struct ContractAliasResolveArgs {
    /// Alias literal in `name::domain.dataspace` or `name::dataspace` format.
    pub contract_alias: String,
}

impl Run for ContractAliasResolveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let contract_alias: iroha::data_model::smart_contract::ContractAlias = self
            .contract_alias
            .parse()
            .wrap_err("invalid contract alias")?;
        let client: Client = context.client_from_config();
        let response = client
            .post_contract_alias_resolve(&contract_alias)
            .wrap_err("failed to call `/v1/contracts/aliases/resolve`")?;
        let status = response.status();
        let body = response.into_body();

        match status {
            StatusCode::OK => {
                let value: norito::json::Value =
                    norito::json::from_slice(&body).wrap_err("decode contract alias response")?;
                context.print_data(&value)
            }
            StatusCode::NOT_FOUND => Err(eyre!("contract alias `{contract_alias}` not found")),
            status => Err(eyre!(
                "contract alias resolve request failed with HTTP {}: {}",
                status,
                std::str::from_utf8(&body).unwrap_or("")
            )),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct DeployArgs {
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Stable on-chain contract alias (`name::domain.dataspace` or `name::dataspace`)
    #[arg(long)]
    pub contract_alias: String,
    /// Optional lease expiry timestamp (unix ms) for the alias binding
    #[arg(long)]
    pub lease_expiry_ms: Option<u64>,
    /// Hex-encoded private key for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    #[command(flatten)]
    pub wait: TransactionWaitArgs,
}

impl Run for DeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        // Parse authority and key
        let authority = crate::resolve_account_id(context, &self.authority)
            .wrap_err("failed to resolve --authority")?;
        let private_key: iroha_crypto::PrivateKey =
            self.private_key.parse().wrap_err("invalid --private-key")?;
        // Obtain base64 code
        let code_b64 = if let Some(p) = self.code_file {
            let bytes = std::fs::read(&p).wrap_err("read --code-file")?;
            base64::engine::general_purpose::STANDARD.encode(bytes)
        } else if let Some(s) = self.code_b64 {
            s
        } else {
            return Err(eyre!("either --code-file or --code-b64 must be provided"));
        };
        let contract_alias: iroha::data_model::smart_contract::ContractAlias = self
            .contract_alias
            .parse()
            .wrap_err("invalid --contract-alias")?;
        let v = client.post_contract_deploy_json(
            &authority,
            &private_key,
            &code_b64,
            &contract_alias,
            self.lease_expiry_ms,
        )?;
        if self.wait.is_enabled() {
            let tx_hash = extract_submitted_transaction_hash(&v)
                .wrap_err("deploy response missing canonical `tx_hash_hex`")?;
            let status = wait_for_transaction_status(&client, tx_hash, &self.wait)?;
            context.print_data(&ContractSubmissionWaitResponse {
                submit: v,
                terminal_kind: status.terminal_kind,
                attempts: status.attempts,
                elapsed_ms: status.elapsed_ms,
                r#final: status.r#final,
            })?;
        } else {
            context.print_data(&v)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct DeriveAddressArgs {
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Target dataspace alias or numeric dataspace id (defaults to `universal`)
    #[arg(long, default_value = "universal")]
    pub dataspace: String,
    /// Successful deploy nonce consumed for address derivation
    #[arg(long)]
    pub deploy_nonce: u64,
    /// Explicit chain discriminant used for Bech32m contract-address derivation
    #[arg(long)]
    pub chain_discriminant: u16,
    /// Optional numeric dataspace id override for non-default dataspaces
    #[arg(long)]
    pub dataspace_id: Option<u64>,
}

impl Run for DeriveAddressArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let authority = parse_account_address(&self.authority, Some(self.chain_discriminant))
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to resolve --authority")?
            .address
            .to_account_id()
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to decode --authority")?;
        let dataspace_id = resolve_contract_dataspace_id_hint(&self.dataspace, self.dataspace_id)?;
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            self.chain_discriminant,
            &authority,
            self.deploy_nonce,
            dataspace_id,
        )
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err("failed to derive contract address")?;

        context.print_data(&norito::json!({
            "authority": (authority),
            "dataspace": (self.dataspace),
            "dataspace_id": (dataspace_id.as_u64()),
            "deploy_nonce": (self.deploy_nonce),
            "chain_discriminant": (self.chain_discriminant),
            "contract_address": (contract_address),
        }))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct ContractTargetArgs {
    /// Canonical contract address.
    #[arg(long, conflicts_with = "contract_alias")]
    pub contract_address: Option<String>,
    /// On-chain contract alias (`name::domain.dataspace` or `name::dataspace`).
    #[arg(long, conflicts_with = "contract_address")]
    pub contract_alias: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ContractPayloadArgs {
    /// Inline Norito JSON payload object or value.
    #[arg(long, value_name = "JSON", conflicts_with = "payload_file")]
    pub payload_json: Option<String>,
    /// File containing a Norito JSON payload object or value.
    #[arg(long, value_name = "PATH", conflicts_with = "payload_json")]
    pub payload_file: Option<PathBuf>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct ContractSubmissionWaitResponse {
    submit: norito::json::Value,
    terminal_kind: String,
    attempts: u64,
    elapsed_ms: u64,
    r#final: iroha_torii_shared::PipelineTransactionStatusResponse,
}

fn extract_submitted_transaction_hash(
    value: &norito::json::Value,
) -> Result<HashOf<iroha::data_model::transaction::SignedTransaction>> {
    let tx_hash_hex = value
        .as_object()
        .and_then(|map| map.get("tx_hash_hex"))
        .and_then(norito::json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| eyre!("response missing `tx_hash_hex`"))?;
    tx_hash_hex
        .parse::<HashOf<iroha::data_model::transaction::SignedTransaction>>()
        .map_err(|err| eyre!("invalid `tx_hash_hex`: {err}"))
}

#[derive(clap::Args, Debug)]
pub struct CallArgs {
    /// Authority account identifier. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Hex-encoded private key override used to sign and submit the call directly.
    #[arg(long, value_name = "HEX", conflicts_with = "scaffold_only")]
    pub private_key: Option<String>,
    /// Request an unsigned transaction scaffold instead of direct submission.
    #[arg(long, conflicts_with = "simulate")]
    pub scaffold_only: bool,
    /// Simulate the contract call locally on Torii without submitting a transaction.
    #[arg(long, conflicts_with_all = ["scaffold_only", "private_key", "wait"])]
    pub simulate: bool,
    /// Optional contract entrypoint selector (defaults to `main`).
    #[arg(long)]
    pub entrypoint: Option<String>,
    /// Optional gas asset id forwarded to transaction metadata.
    #[arg(long)]
    pub gas_asset_id: Option<String>,
    /// Optional fee sponsor account charged for gas/fees when supported.
    #[arg(long)]
    pub fee_sponsor: Option<String>,
    /// Gas limit metadata forwarded to the contract call.
    #[arg(long, default_value_t = 100_000)]
    pub gas_limit: u64,
    #[command(flatten)]
    pub target: ContractTargetArgs,
    #[command(flatten)]
    pub payload: ContractPayloadArgs,
    #[command(flatten)]
    pub wait: TransactionWaitArgs,
}

impl Run for CallArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let authority = resolve_contract_authority(context, self.authority.as_deref())?;
        let private_key = if self.simulate {
            None
        } else {
            resolve_contract_call_private_key(
                context,
                &authority,
                self.private_key.as_deref(),
                self.scaffold_only,
            )?
        };
        let fee_sponsor = self
            .fee_sponsor
            .as_deref()
            .map(|value| crate::resolve_account_id(context, value))
            .transpose()
            .wrap_err("failed to resolve --fee-sponsor")?;
        let target = resolve_contract_target(self.target)?;
        let payload = load_contract_payload_value(
            self.payload.payload_json.as_deref(),
            self.payload.payload_file.as_deref(),
        )?;
        if self.simulate {
            let value = client.post_contract_call_simulate_json(
                &authority,
                target.contract_address.as_ref(),
                target.contract_alias.as_ref(),
                self.entrypoint.as_deref(),
                payload.as_ref(),
                self.gas_asset_id.as_deref(),
                fee_sponsor.as_ref(),
                self.gas_limit,
            )?;
            context.print_data(&value)?;
            return Ok(());
        }
        let value = client.post_contract_call_json(
            &authority,
            private_key.as_ref(),
            target.contract_address.as_ref(),
            target.contract_alias.as_ref(),
            self.entrypoint.as_deref(),
            payload.as_ref(),
            None,
            self.gas_asset_id.as_deref(),
            fee_sponsor.as_ref(),
            self.gas_limit,
        )?;
        if self.wait.is_enabled() {
            let tx_hash = extract_submitted_transaction_hash(&value)
                .wrap_err("contract call response missing canonical `tx_hash_hex`")?;
            let status = wait_for_transaction_status(&client, tx_hash, &self.wait)?;
            context.print_data(&ContractSubmissionWaitResponse {
                submit: value,
                terminal_kind: status.terminal_kind,
                attempts: status.attempts,
                elapsed_ms: status.elapsed_ms,
                r#final: status.r#final,
            })?;
        } else {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ViewArgs {
    /// Authority account identifier used as the read context. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Optional contract entrypoint selector (defaults to `main`).
    #[arg(long)]
    pub entrypoint: Option<String>,
    /// Gas limit applied to the local view execution.
    #[arg(long, default_value_t = 100_000)]
    pub gas_limit: u64,
    #[command(flatten)]
    pub target: ContractTargetArgs,
    #[command(flatten)]
    pub payload: ContractPayloadArgs,
}

impl Run for ViewArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let authority = resolve_contract_authority(context, self.authority.as_deref())?;
        let target = resolve_contract_target(self.target)?;
        let payload = load_contract_payload_value(
            self.payload.payload_json.as_deref(),
            self.payload.payload_file.as_deref(),
        )?;
        let value = client.post_contract_view_json(
            &authority,
            target.contract_address.as_ref(),
            target.contract_alias.as_ref(),
            self.entrypoint.as_deref(),
            payload.as_ref(),
            self.gas_limit,
        )?;
        context.print_data(&value)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct DebugViewArgs {
    /// Authority account identifier used as the local read context. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Optional contract entrypoint selector (defaults to `main`).
    #[arg(long)]
    pub entrypoint: Option<String>,
    /// Gas limit applied to the local view execution.
    #[arg(long, default_value_t = 100_000)]
    pub gas_limit: u64,
    /// Optional source file used to render snippet context for trapped debug locations.
    #[arg(long, value_name = "PATH")]
    pub source_file: Option<PathBuf>,
    /// Optional JSON array of canonical account ids available to iterator helpers.
    #[arg(long, value_name = "JSON", conflicts_with = "accounts_file")]
    pub accounts_json: Option<String>,
    /// File containing a JSON array of canonical account ids available to iterator helpers.
    #[arg(long, value_name = "PATH", conflicts_with = "accounts_json")]
    pub accounts_file: Option<PathBuf>,
    /// Optional JSON object mapping durable state keys to encoded values (`0x...` hex or base64).
    #[arg(long, value_name = "JSON", conflicts_with = "durable_state_file")]
    pub durable_state_json: Option<String>,
    /// File containing a JSON object mapping durable state keys to encoded values (`0x...` hex or base64).
    #[arg(long, value_name = "PATH", conflicts_with = "durable_state_json")]
    pub durable_state_file: Option<PathBuf>,
    #[command(flatten)]
    pub payload: ContractPayloadArgs,
}

impl Run for DebugViewArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let authority = resolve_contract_authority(context, self.authority.as_deref())?;
        let report = execute_local_contract_debug_view(context, self, authority)?;
        context.print_data(&report)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct DebugCallArgs {
    /// Authority account identifier used as the local call context. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Optional contract entrypoint selector (defaults to `main`).
    #[arg(long)]
    pub entrypoint: Option<String>,
    /// Gas limit applied to the local call execution.
    #[arg(long, default_value_t = 100_000)]
    pub gas_limit: u64,
    /// Optional source file used to render snippet context for trapped debug locations.
    #[arg(long, value_name = "PATH")]
    pub source_file: Option<PathBuf>,
    /// Optional JSON array of canonical account ids available to iterator helpers.
    #[arg(long, value_name = "JSON", conflicts_with = "accounts_file")]
    pub accounts_json: Option<String>,
    /// File containing a JSON array of canonical account ids available to iterator helpers.
    #[arg(long, value_name = "PATH", conflicts_with = "accounts_json")]
    pub accounts_file: Option<PathBuf>,
    /// Optional JSON object mapping durable state keys to encoded values (`0x...` hex or base64).
    #[arg(long, value_name = "JSON", conflicts_with = "durable_state_file")]
    pub durable_state_json: Option<String>,
    /// File containing a JSON object mapping durable state keys to encoded values (`0x...` hex or base64).
    #[arg(long, value_name = "PATH", conflicts_with = "durable_state_json")]
    pub durable_state_file: Option<PathBuf>,
    #[command(flatten)]
    pub payload: ContractPayloadArgs,
}

impl Run for DebugCallArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let authority = resolve_contract_authority(context, self.authority.as_deref())?;
        let report = execute_local_contract_debug_call(context, self, authority)?;
        context.print_data(&report)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct BuildManifestArgs {
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Hex-encoded private key for signing the manifest (optional)
    #[arg(long, value_name = "HEX")]
    pub sign_with: Option<String>,
    /// Optional output path; if omitted, prints to stdout
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

impl Run for BuildManifestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let code = load_code_bytes(self.code_file.clone(), self.code_b64.clone())?;
        let verified = verify_contract_from_bytes(&code)?;
        let mut manifest = verified.manifest;
        if let Some(hex_key) = self.sign_with {
            let private: PrivateKey = hex_key.parse().wrap_err("invalid --sign-with")?;
            let kp =
                KeyPair::from_private_key(private).wrap_err("derive signing keypair failed")?;
            manifest = manifest.signed(&kp);
        }
        let rendered = norito::json::to_json_pretty(&manifest)?;
        if let Some(path) = self.out {
            std::fs::write(&path, rendered.as_bytes())
                .wrap_err_with(|| format!("write manifest to {}", path.display()))?;
            context.println(format_args!("Wrote manifest to {}", path.display()))?;
        } else {
            context.println(rendered)?;
        }
        Ok(())
    }
}

fn load_code_bytes(code_file: Option<PathBuf>, code_b64: Option<String>) -> Result<Vec<u8>> {
    if let Some(path) = code_file {
        let bytes = std::fs::read(&path).wrap_err_with(|| format!("read {}", path.display()))?;
        Ok(bytes)
    } else if let Some(s) = code_b64 {
        base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .wrap_err("decode base64 code payload")
    } else {
        Err(eyre!("either --code-file or --code-b64 must be provided"))
    }
}

fn resolve_contract_dataspace_id_hint(
    dataspace: &str,
    dataspace_id: Option<u64>,
) -> Result<iroha::data_model::nexus::DataSpaceId> {
    if let Some(dataspace_id) = dataspace_id {
        return Ok(iroha::data_model::nexus::DataSpaceId::new(dataspace_id));
    }

    let trimmed = dataspace.trim();
    if trimmed.is_empty() {
        return Err(eyre!("--dataspace must not be empty"));
    }

    if let Ok(raw) = trimmed.parse::<u64>() {
        return Ok(iroha::data_model::nexus::DataSpaceId::new(raw));
    }

    let raw = match trimmed {
        "universal" => 0,
        "governance" => 1,
        "zk" => 2,
        _ => {
            return Err(eyre!(
                "unknown dataspace alias `{trimmed}`; pass --dataspace-id for non-default dataspaces"
            ));
        }
    };
    Ok(iroha::data_model::nexus::DataSpaceId::new(raw))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedContractTarget {
    contract_address: Option<iroha::data_model::smart_contract::ContractAddress>,
    contract_alias: Option<iroha::data_model::smart_contract::ContractAlias>,
}

fn resolve_contract_target(
    args: ContractTargetArgs,
) -> Result<ResolvedContractTarget> {
    match (args.contract_address.as_deref(), args.contract_alias.as_deref()) {
        (Some(address), None) => Ok(
            ResolvedContractTarget {
                contract_address: Some(
                    address
                        .parse()
                        .wrap_err("invalid --contract-address canonical literal")?,
                ),
                contract_alias: None,
            },
        ),
        (None, Some(alias)) => Ok(
            ResolvedContractTarget {
                contract_address: None,
                contract_alias: Some(alias.parse().wrap_err("invalid --contract-alias")?),
            },
        ),
        (None, None) => Err(eyre!(
            "provide exactly one contract target via --contract-address or --contract-alias"
        )),
        _ => Err(eyre!(
            "provide exactly one contract target via --contract-address or --contract-alias"
        )),
    }
}

fn resolve_optional_contract_address<C: RunContext>(
    context: &C,
    args: &ContractTargetArgs,
) -> Result<Option<iroha::data_model::smart_contract::ContractAddress>> {
    match (args.contract_address.as_deref(), args.contract_alias.as_deref()) {
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(eyre!(
            "provide exactly one contract target via --contract-address or --contract-alias"
        )),
        (Some(contract_address), None) => Ok(Some(
            contract_address
                .parse()
                .wrap_err("invalid --contract-address canonical literal")?,
        )),
        (None, Some(contract_alias_raw)) => {
            let contract_alias: iroha::data_model::smart_contract::ContractAlias = contract_alias_raw
                .parse()
                .wrap_err("invalid --contract-alias")?;
            let client: Client = context.client_from_config();
            let response = client
                .post_contract_alias_resolve(&contract_alias)
                .wrap_err("failed to call `/v1/contracts/aliases/resolve`")?;
            let status = response.status();
            let body = response.into_body();

            match status {
                StatusCode::OK => {
                    let value: norito::json::Value =
                        norito::json::from_slice(&body).wrap_err("decode contract alias response")?;
                    let resolved = value
                        .get("contract_address")
                        .and_then(norito::json::Value::as_str)
                        .ok_or_else(|| eyre!("contract alias response missing `contract_address`"))?;
                    Ok(Some(
                        resolved
                            .parse()
                            .wrap_err("resolved contract address is invalid")?,
                    ))
                }
                StatusCode::NOT_FOUND => Err(eyre!("contract alias `{contract_alias}` not found")),
                status => Err(eyre!(
                    "contract alias resolve request failed with HTTP {}: {}",
                    status,
                    std::str::from_utf8(&body).unwrap_or("")
                )),
            }
        }
    }
}

fn load_contract_payload_value(
    payload_json: Option<&str>,
    payload_file: Option<&std::path::Path>,
) -> Result<Option<norito::json::Value>> {
    match (payload_json, payload_file) {
        (Some(raw), None) => norito::json::from_str(raw)
            .map(Some)
            .wrap_err("invalid --payload-json"),
        (None, Some(path)) => {
            let contents = std::fs::read_to_string(path)
                .wrap_err_with(|| format!("read {}", path.display()))?;
            norito::json::from_str(&contents)
                .map(Some)
                .wrap_err_with(|| format!("invalid JSON in {}", path.display()))
        }
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(eyre!(
            "--payload-json and --payload-file are mutually exclusive"
        )),
    }
}

fn resolve_contract_authority<C: RunContext>(
    context: &mut C,
    authority: Option<&str>,
) -> Result<AccountId> {
    match authority {
        Some(authority) => {
            crate::resolve_account_id(context, authority).wrap_err("failed to resolve --authority")
        }
        None => Ok(context.config().account.clone()),
    }
}

fn resolve_contract_call_private_key<C: RunContext>(
    context: &C,
    authority: &AccountId,
    private_key_hex: Option<&str>,
    scaffold_only: bool,
) -> Result<Option<PrivateKey>> {
    if scaffold_only {
        return Ok(None);
    }
    if let Some(private_key_hex) = private_key_hex {
        return private_key_hex
            .parse()
            .map(Some)
            .wrap_err("invalid --private-key");
    }
    if authority == &context.config().account {
        return Ok(Some(context.config().key_pair.private_key().clone()));
    }
    Err(eyre!(
        "--private-key is required when --authority does not match client.toml authority"
    ))
}

fn verify_contract_from_bytes(bytes: &[u8]) -> Result<ivm::VerifiedContractArtifact> {
    ivm::verify_contract_artifact(bytes).map_err(|err| eyre!(err.to_string()))
}

fn program_summary_from_bytes(bytes: &[u8]) -> Result<ProgramSummary> {
    let parsed = ivm::ProgramMetadata::parse(bytes)
        .map_err(|err| eyre!("failed to parse IVM metadata: {err}"))?;
    let code_hash = Hash::new(&bytes[parsed.header_len..]);
    let metadata = parsed.metadata;
    let policy = match metadata.abi_version {
        1 => ivm::SyscallPolicy::AbiV1,
        v => {
            return Err(eyre!(
                "unsupported abi_version {v}; expected 1 for the first release"
            ));
        }
    };
    let abi_hash = Hash::prehashed(ivm::syscalls::compute_abi_hash(policy));
    let meta_hash = Hash::new(metadata.encode());
    Ok(ProgramSummary {
        metadata,
        code_hash,
        abi_hash,
        meta_hash,
        header_len: parsed.header_len,
        code_offset: parsed.code_offset,
    })
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractDebugViewResponse {
    ok: bool,
    code_hash_hex: String,
    abi_hash_hex: String,
    entrypoint: LocalContractDebugEntrypoint,
    budget: LocalContractDebugBudget,
    syscall_trace: Vec<LocalContractSyscallTrace>,
    result: Option<norito::json::Value>,
    error: Option<String>,
    vm_diagnostic: Option<LocalContractDebugVmDiagnostic>,
    source_snippet: Option<LocalContractSourceSnippet>,
    queued_instruction_count: usize,
    durable_state_mutation_count: usize,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractDebugEntrypoint {
    name: String,
    kind: String,
    pc: u64,
    return_type: Option<String>,
    params: Vec<LocalContractDebugParam>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractDebugParam {
    name: String,
    type_name: String,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractDebugBudget {
    gas_limit: u64,
    gas_remaining: u64,
    gas_used: u64,
    cycles: u64,
    max_cycles: u64,
    stack_limit_bytes: u64,
    stack_bytes_used: u64,
    entrypoint_pc: u64,
    final_pc: u64,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractDebugVmDiagnostic {
    trap_kind: String,
    message: String,
    pc: u64,
    function: Option<String>,
    source_path: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    gas_limit: u64,
    gas_remaining: u64,
    gas_used: u64,
    cycles: u64,
    max_cycles: u64,
    stack_limit_bytes: u64,
    stack_bytes_used: u64,
    entrypoint_pc: Option<u64>,
    current_function: Option<String>,
    opcode: Option<u16>,
    syscall: Option<u32>,
    predecoded_loaded: bool,
    predecoded_hit: Option<bool>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractSyscallTrace {
    pc: u64,
    syscall: u32,
    gas_remaining_at_call: u64,
    additional_gas: Option<u64>,
    error: Option<String>,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractSourceSnippet {
    path: String,
    line: u32,
    column: u32,
    excerpt: String,
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct LocalContractDebugCallResponse {
    ok: bool,
    code_hash_hex: String,
    abi_hash_hex: String,
    entrypoint: LocalContractDebugEntrypoint,
    budget: LocalContractDebugBudget,
    syscall_trace: Vec<LocalContractSyscallTrace>,
    result: Option<norito::json::Value>,
    error: Option<String>,
    vm_diagnostic: Option<LocalContractDebugVmDiagnostic>,
    source_snippet: Option<LocalContractSourceSnippet>,
    queued_instruction_count: usize,
    queued_instructions: norito::json::Value,
    durable_state_mutation_count: usize,
    durable_state_overlay: norito::json::Value,
}

struct TracingHost<H> {
    inner: H,
    syscall_trace: Vec<LocalContractSyscallTrace>,
}

impl<H> TracingHost<H> {
    fn new(inner: H) -> Self {
        Self {
            inner,
            syscall_trace: Vec::new(),
        }
    }

    fn into_parts(self) -> (H, Vec<LocalContractSyscallTrace>) {
        (self.inner, self.syscall_trace)
    }
}

impl<H> IVMHost for TracingHost<H>
where
    H: IVMHost + 'static,
{
    fn syscall(&mut self, number: u32, vm: &mut ivm::IVM) -> Result<u64, ivm::VMError> {
        let record = LocalContractSyscallTrace {
            pc: vm.pc(),
            syscall: number,
            gas_remaining_at_call: vm.gas_remaining,
            additional_gas: None,
            error: None,
        };
        match self.inner.syscall(number, vm) {
            Ok(additional_gas) => {
                let mut record = record;
                record.additional_gas = Some(additional_gas);
                self.syscall_trace.push(record);
                Ok(additional_gas)
            }
            Err(err) => {
                let mut record = record;
                record.error = Some(err.to_string());
                self.syscall_trace.push(record);
                Err(err)
            }
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any
    where
        Self: 'static,
    {
        self
    }

    fn supports_concurrent_blocks(&self) -> bool {
        self.inner.supports_concurrent_blocks()
    }

    fn begin_tx(&mut self, declared: &ivm::parallel::StateAccessSet) -> Result<(), ivm::VMError> {
        self.inner.begin_tx(declared)
    }

    fn finish_tx(&mut self) -> Result<ivm::host::AccessLog, ivm::VMError> {
        self.inner.finish_tx()
    }

    fn set_external_vk_bytes(&mut self, backend: String, bytes: Vec<u8>) {
        self.inner.set_external_vk_bytes(backend, bytes);
    }

    fn checkpoint(&self) -> Option<Box<dyn std::any::Any + Send>> {
        self.inner.checkpoint()
    }

    fn restore(&mut self, snapshot: &dyn std::any::Any) -> bool {
        self.inner.restore(snapshot)
    }

    fn access_logging_supported(&self) -> bool {
        self.inner.access_logging_supported()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LocalContractSchemaType {
    Unit,
    Int,
    Numeric,
    Bool,
    String,
    Json,
    Name,
    AccountId,
    AssetDefinitionId,
    AssetId,
    DomainId,
    NftId,
    Blob,
    Bytes,
    DataSpaceId,
    AxtDescriptor,
    AssetHandle,
    ProofBlob,
    Tuple(Vec<LocalContractSchemaType>),
}

fn execute_local_contract_debug_view<C: RunContext>(
    context: &C,
    args: DebugViewArgs,
    authority: AccountId,
) -> Result<LocalContractDebugViewResponse> {
    let code = load_code_bytes(args.code_file.clone(), args.code_b64.clone())?;
    let verified = verify_contract_from_bytes(&code)?;
    let summary = program_summary_from_bytes(&code)?;
    let selector = args.entrypoint.unwrap_or_else(|| "main".to_owned());
    let descriptor = resolve_local_view_entrypoint(&verified, &selector)?;
    let entrypoint_pc = resolve_local_contract_entrypoint_pc(&code, descriptor)?;
    let payload = load_contract_payload_value(
        args.payload.payload_json.as_deref(),
        args.payload.payload_file.as_deref(),
    )?;
    let payload = normalize_local_contract_payload(descriptor, payload.as_ref())?;
    let accounts = load_debug_accounts_fixture(
        &authority,
        args.accounts_json.as_deref(),
        args.accounts_file.as_deref(),
    )?;
    let durable_state = load_debug_durable_state_fixture(
        args.durable_state_json.as_deref(),
        args.durable_state_file.as_deref(),
    )?;

    let mut host = if let Some(payload) = payload {
        CoreHost::with_accounts_and_args(authority, Arc::clone(&accounts), payload)
    } else {
        CoreHost::with_accounts(authority, Arc::clone(&accounts))
    };
    host.set_chain_id(&context.config().chain);
    host.set_durable_state_snapshot(durable_state);

    let mut tracing_host = TracingHost::new(host);
    let mut vm = ivm::IVM::new(args.gas_limit);
    vm.load_program(&code)
        .map_err(|err| eyre!("failed to load contract debug view bytecode: {err}"))?;
    vm.set_gas_limit(args.gas_limit);
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entrypoint_pc)
        .map_err(|err| eyre!("failed to seek to contract debug entrypoint: {err}"))?;

    let run_result = vm.run_with_host(&mut tracing_host);
    let (mut host, syscall_trace) = tracing_host.into_parts();
    let queued = host.drain_instructions();
    let durable_state_overlay = host.drain_durable_state_overlay();
    let budget = build_local_debug_budget(&vm, args.gas_limit, entrypoint_pc);
    let vm_diagnostic = vm.last_diagnostic().map(map_local_vm_diagnostic);
    let source_snippet =
        maybe_render_source_snippet(args.source_file.as_deref(), vm_diagnostic.as_ref());
    let entrypoint = build_local_debug_entrypoint(descriptor, entrypoint_pc);

    if let Err(err) = run_result {
        return Ok(LocalContractDebugViewResponse {
            ok: false,
            code_hash_hex: hex::encode(summary.code_hash.as_ref()),
            abi_hash_hex: hex::encode(summary.abi_hash.as_ref()),
            entrypoint,
            budget,
            syscall_trace,
            result: None,
            error: Some(format!("contract debug view execution failed: {err}")),
            vm_diagnostic,
            source_snippet,
            queued_instruction_count: queued.len(),
            durable_state_mutation_count: durable_state_overlay.len(),
        });
    }

    if !queued.is_empty() {
        return Ok(LocalContractDebugViewResponse {
            ok: false,
            code_hash_hex: hex::encode(summary.code_hash.as_ref()),
            abi_hash_hex: hex::encode(summary.abi_hash.as_ref()),
            entrypoint,
            budget,
            syscall_trace,
            result: None,
            error: Some("view entrypoint attempted to emit instructions".to_owned()),
            vm_diagnostic,
            source_snippet,
            queued_instruction_count: queued.len(),
            durable_state_mutation_count: durable_state_overlay.len(),
        });
    }

    if !durable_state_overlay.is_empty() {
        return Ok(LocalContractDebugViewResponse {
            ok: false,
            code_hash_hex: hex::encode(summary.code_hash.as_ref()),
            abi_hash_hex: hex::encode(summary.abi_hash.as_ref()),
            entrypoint,
            budget,
            syscall_trace,
            result: None,
            error: Some("view entrypoint attempted to mutate durable state".to_owned()),
            vm_diagnostic,
            source_snippet,
            queued_instruction_count: queued.len(),
            durable_state_mutation_count: durable_state_overlay.len(),
        });
    }

    let schema = descriptor
        .return_type
        .as_deref()
        .map(parse_local_contract_schema_type)
        .transpose()?
        .unwrap_or(LocalContractSchemaType::Unit);
    let (result, _) = decode_local_contract_view_result_value(&vm, 10, &schema)
        .map_err(|err| eyre!("failed to decode contract debug view return value: {err}"))?;

    Ok(LocalContractDebugViewResponse {
        ok: true,
        code_hash_hex: hex::encode(summary.code_hash.as_ref()),
        abi_hash_hex: hex::encode(summary.abi_hash.as_ref()),
        entrypoint,
        budget,
        syscall_trace,
        result: Some(result),
        error: None,
        vm_diagnostic,
        source_snippet,
        queued_instruction_count: 0,
        durable_state_mutation_count: 0,
    })
}

fn execute_local_contract_debug_call<C: RunContext>(
    context: &C,
    args: DebugCallArgs,
    authority: AccountId,
) -> Result<LocalContractDebugCallResponse> {
    let code = load_code_bytes(args.code_file.clone(), args.code_b64.clone())?;
    let verified = verify_contract_from_bytes(&code)?;
    let summary = program_summary_from_bytes(&code)?;
    let selector = args.entrypoint.unwrap_or_else(|| "main".to_owned());
    let descriptor = resolve_local_public_entrypoint(&verified, &selector)?;
    let entrypoint_pc = resolve_local_contract_entrypoint_pc(&code, descriptor)?;
    let payload = load_contract_payload_value(
        args.payload.payload_json.as_deref(),
        args.payload.payload_file.as_deref(),
    )?;
    let payload = normalize_local_contract_payload(descriptor, payload.as_ref())?;
    let accounts = load_debug_accounts_fixture(
        &authority,
        args.accounts_json.as_deref(),
        args.accounts_file.as_deref(),
    )?;
    let durable_state = load_debug_durable_state_fixture(
        args.durable_state_json.as_deref(),
        args.durable_state_file.as_deref(),
    )?;

    let mut host = if let Some(payload) = payload {
        CoreHost::with_accounts_and_args(authority, Arc::clone(&accounts), payload)
    } else {
        CoreHost::with_accounts(authority, Arc::clone(&accounts))
    };
    host.set_chain_id(&context.config().chain);
    host.set_durable_state_snapshot(durable_state);

    let mut tracing_host = TracingHost::new(host);
    let mut vm = ivm::IVM::new(args.gas_limit);
    vm.load_program(&code)
        .map_err(|err| eyre!("failed to load contract debug call bytecode: {err}"))?;
    vm.set_gas_limit(args.gas_limit);
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entrypoint_pc)
        .map_err(|err| eyre!("failed to seek to contract debug entrypoint: {err}"))?;

    let run_result = vm.run_with_host(&mut tracing_host);
    let (mut host, syscall_trace) = tracing_host.into_parts();
    let queued = host.drain_instructions();
    let durable_state_overlay = host.drain_durable_state_overlay();
    let queued_instruction_count = queued.len();
    let durable_state_mutation_count = durable_state_overlay.len();
    let queued_instructions = render_queued_instructions(&queued)?;
    let durable_state_overlay_json = render_durable_state_overlay(&durable_state_overlay)?;
    let budget = build_local_debug_budget(&vm, args.gas_limit, entrypoint_pc);
    let vm_diagnostic = vm.last_diagnostic().map(map_local_vm_diagnostic);
    let source_snippet =
        maybe_render_source_snippet(args.source_file.as_deref(), vm_diagnostic.as_ref());
    let entrypoint = build_local_debug_entrypoint(descriptor, entrypoint_pc);

    if let Err(err) = run_result {
        return Ok(LocalContractDebugCallResponse {
            ok: false,
            code_hash_hex: hex::encode(summary.code_hash.as_ref()),
            abi_hash_hex: hex::encode(summary.abi_hash.as_ref()),
            entrypoint,
            budget,
            syscall_trace,
            result: None,
            error: Some(format!("contract debug call execution failed: {err}")),
            vm_diagnostic,
            source_snippet,
            queued_instruction_count,
            queued_instructions,
            durable_state_mutation_count,
            durable_state_overlay: durable_state_overlay_json,
        });
    }

    let schema = descriptor
        .return_type
        .as_deref()
        .map(parse_local_contract_schema_type)
        .transpose()?
        .unwrap_or(LocalContractSchemaType::Unit);
    let result = if schema == LocalContractSchemaType::Unit {
        None
    } else {
        let (value, _) = decode_local_contract_view_result_value(&vm, 10, &schema)
            .map_err(|err| eyre!("failed to decode contract debug call return value: {err}"))?;
        Some(value)
    };

    Ok(LocalContractDebugCallResponse {
        ok: true,
        code_hash_hex: hex::encode(summary.code_hash.as_ref()),
        abi_hash_hex: hex::encode(summary.abi_hash.as_ref()),
        entrypoint,
        budget,
        syscall_trace,
        result,
        error: None,
        vm_diagnostic,
        source_snippet,
        queued_instruction_count,
        queued_instructions,
        durable_state_mutation_count,
        durable_state_overlay: durable_state_overlay_json,
    })
}

fn build_local_debug_entrypoint(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    entrypoint_pc: u64,
) -> LocalContractDebugEntrypoint {
    LocalContractDebugEntrypoint {
        name: descriptor.name.clone(),
        kind: format!("{:?}", descriptor.kind),
        pc: entrypoint_pc,
        return_type: descriptor.return_type.clone(),
        params: descriptor
            .params
            .iter()
            .map(|param| LocalContractDebugParam {
                name: param.name.clone(),
                type_name: param.type_name.clone(),
            })
            .collect(),
    }
}

fn render_queued_instructions(
    queued: &[iroha::data_model::isi::InstructionBox],
) -> Result<norito::json::Value> {
    let values = queued
        .iter()
        .map(norito::json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .wrap_err("failed to serialize queued instructions")?;
    Ok(norito::json::Value::Array(values))
}

fn render_durable_state_overlay(
    overlay: &BTreeMap<Name, Option<Vec<u8>>>,
) -> Result<norito::json::Value> {
    let mut object = norito::json::Map::new();
    for (path, value) in overlay {
        object.insert(
            path.as_ref().to_owned(),
            value.as_ref().map_or(norito::json::Value::Null, |bytes| {
                norito::json::Value::from(format!("0x{}", hex::encode(bytes)))
            }),
        );
    }
    Ok(norito::json::Value::Object(object))
}

fn build_local_debug_budget(
    vm: &ivm::IVM,
    gas_limit: u64,
    entrypoint_pc: u64,
) -> LocalContractDebugBudget {
    let stack_top = vm.memory.stack_top();
    let stack_pointer = vm.register(31);
    let stack_bytes_used = if stack_pointer <= stack_top {
        stack_top.saturating_sub(stack_pointer)
    } else {
        0
    };
    LocalContractDebugBudget {
        gas_limit,
        gas_remaining: vm.gas_remaining,
        gas_used: gas_limit.saturating_sub(vm.gas_remaining),
        cycles: vm.get_cycle_count(),
        max_cycles: vm.metadata().max_cycles,
        stack_limit_bytes: vm.memory.stack_limit(),
        stack_bytes_used,
        entrypoint_pc,
        final_pc: vm.pc(),
    }
}

fn map_local_vm_diagnostic(diag: &ivm::VmExecutionDiagnostic) -> LocalContractDebugVmDiagnostic {
    LocalContractDebugVmDiagnostic {
        trap_kind: format!("{:?}", diag.trap_kind),
        message: diag.message.clone(),
        pc: diag.pc,
        function: diag
            .source
            .as_ref()
            .and_then(|source| source.function.clone()),
        source_path: diag.source.as_ref().and_then(|source| source.path.clone()),
        line: diag.source.as_ref().and_then(|source| source.line),
        column: diag.source.as_ref().and_then(|source| source.column),
        gas_limit: diag.budget.gas_limit,
        gas_remaining: diag.budget.gas_remaining,
        gas_used: diag.budget.gas_used,
        cycles: diag.budget.cycles,
        max_cycles: diag.budget.max_cycles,
        stack_limit_bytes: diag.budget.stack_limit_bytes,
        stack_bytes_used: diag.budget.stack_bytes_used,
        entrypoint_pc: diag.context.entrypoint_pc,
        current_function: diag.context.current_function.clone(),
        opcode: diag.context.opcode,
        syscall: diag.context.syscall,
        predecoded_loaded: diag.context.predecoded_loaded,
        predecoded_hit: diag.context.predecoded_hit,
    }
}

fn maybe_render_source_snippet(
    source_file: Option<&std::path::Path>,
    diagnostic: Option<&LocalContractDebugVmDiagnostic>,
) -> Option<LocalContractSourceSnippet> {
    let diagnostic = diagnostic?;
    let line = diagnostic.line?;
    let column = diagnostic.column.unwrap_or(1);
    let resolved_path = if let Some(source_file) = source_file {
        source_file.to_path_buf()
    } else {
        PathBuf::from(diagnostic.source_path.as_deref()?)
    };
    let contents = std::fs::read_to_string(&resolved_path).ok()?;
    let lines = contents.lines().collect::<Vec<_>>();
    let idx = usize::try_from(line.saturating_sub(1)).ok()?;
    let start = idx.saturating_sub(1);
    let end = std::cmp::min(idx + 2, lines.len());
    let mut excerpt = String::new();
    for (offset, text) in lines[start..end].iter().enumerate() {
        let current = start + offset + 1;
        if !excerpt.is_empty() {
            excerpt.push('\n');
        }
        excerpt.push_str(&format!("{current:>4} | {text}"));
    }
    Some(LocalContractSourceSnippet {
        path: resolved_path.display().to_string(),
        line,
        column,
        excerpt,
    })
}

fn load_debug_accounts_fixture(
    authority: &AccountId,
    accounts_json: Option<&str>,
    accounts_file: Option<&std::path::Path>,
) -> Result<Arc<Vec<AccountId>>> {
    let mut accounts = match (accounts_json, accounts_file) {
        (Some(raw), None) => parse_debug_account_list(raw)?,
        (None, Some(path)) => {
            let contents = std::fs::read_to_string(path)
                .wrap_err_with(|| format!("read {}", path.display()))?;
            parse_debug_account_list(&contents)?
        }
        (None, None) => vec![authority.clone()],
        (Some(_), Some(_)) => {
            return Err(eyre!(
                "--accounts-json and --accounts-file are mutually exclusive"
            ));
        }
    };
    if !accounts.iter().any(|candidate| candidate == authority) {
        accounts.push(authority.clone());
    }
    Ok(Arc::new(accounts))
}

fn parse_debug_account_list(raw: &str) -> Result<Vec<AccountId>> {
    let parsed: norito::json::Value =
        norito::json::from_str(raw).wrap_err("invalid account fixture JSON")?;
    let array = parsed
        .as_array()
        .ok_or_else(|| eyre!("account fixture must be a JSON array"))?;
    array
        .iter()
        .map(|value| {
            let literal = value
                .as_str()
                .ok_or_else(|| eyre!("account fixture entries must be strings"))?;
            AccountId::parse_encoded(literal)
                .map(|parsed| parsed.into_account_id())
                .map_err(|err| eyre!("invalid account fixture literal `{literal}`: {err}"))
        })
        .collect()
}

fn load_debug_durable_state_fixture(
    durable_state_json: Option<&str>,
    durable_state_file: Option<&std::path::Path>,
) -> Result<BTreeMap<Name, Vec<u8>>> {
    match (durable_state_json, durable_state_file) {
        (Some(raw), None) => parse_debug_durable_state_fixture(raw),
        (None, Some(path)) => {
            let contents = std::fs::read_to_string(path)
                .wrap_err_with(|| format!("read {}", path.display()))?;
            parse_debug_durable_state_fixture(&contents)
        }
        (None, None) => Ok(BTreeMap::new()),
        (Some(_), Some(_)) => Err(eyre!(
            "--durable-state-json and --durable-state-file are mutually exclusive"
        )),
    }
}

fn parse_debug_durable_state_fixture(raw: &str) -> Result<BTreeMap<Name, Vec<u8>>> {
    let parsed: norito::json::Value =
        norito::json::from_str(raw).wrap_err("invalid durable state fixture JSON")?;
    let object = parsed
        .as_object()
        .ok_or_else(|| eyre!("durable state fixture must be a JSON object"))?;
    object
        .iter()
        .map(|(path, value)| {
            let name = Name::from_str(path)
                .map_err(|err| eyre!("invalid durable state key `{path}`: {err}"))?;
            let encoded = value
                .as_str()
                .ok_or_else(|| eyre!("durable state values must be strings"))?;
            let bytes = decode_debug_fixture_bytes(encoded)?;
            Ok((name, bytes))
        })
        .collect()
}

fn decode_debug_fixture_bytes(raw: &str) -> Result<Vec<u8>> {
    if let Some(hex_raw) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        return hex::decode(hex_raw).wrap_err("invalid hex fixture bytes");
    }
    base64::engine::general_purpose::STANDARD
        .decode(raw.as_bytes())
        .wrap_err("invalid base64 fixture bytes")
}

fn resolve_local_entrypoint<'a>(
    artifact: &'a ivm::VerifiedContractArtifact,
    selector: &str,
    expected_kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
    expected_label: &str,
) -> Result<&'a ivm::EmbeddedEntrypointDescriptor> {
    let descriptor = artifact
        .contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
        .ok_or_else(|| eyre!("unknown contract entrypoint `{selector}`"))?;
    if descriptor.kind != expected_kind {
        return Err(eyre!(
            "contract entrypoint `{selector}` is not a {expected_label} entrypoint"
        ));
    }
    Ok(descriptor)
}

fn resolve_local_view_entrypoint<'a>(
    artifact: &'a ivm::VerifiedContractArtifact,
    selector: &str,
) -> Result<&'a ivm::EmbeddedEntrypointDescriptor> {
    resolve_local_entrypoint(
        artifact,
        selector,
        iroha_data_model::smart_contract::manifest::EntryPointKind::View,
        "read-only view",
    )
}

fn resolve_local_public_entrypoint<'a>(
    artifact: &'a ivm::VerifiedContractArtifact,
    selector: &str,
) -> Result<&'a ivm::EmbeddedEntrypointDescriptor> {
    resolve_local_entrypoint(
        artifact,
        selector,
        iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
        "public",
    )
}

fn resolve_local_contract_entrypoint_pc(
    code_bytes: &[u8],
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
) -> Result<u64> {
    let parsed = ivm::ProgramMetadata::parse(code_bytes)
        .map_err(|err| eyre!("invalid contract artifact: {err}"))?;
    Ok(parsed.prefix_len() as u64 + descriptor.entry_pc)
}

fn split_local_schema_list(input: &str) -> Result<Vec<String>> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    for ch in input.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err(eyre!("invalid contract schema type `{input}`"));
                }
                current.push(ch);
            }
            ',' if depth == 0 => {
                items.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if depth != 0 {
        return Err(eyre!("invalid contract schema type `{input}`"));
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_owned());
    }
    Ok(items)
}

fn parse_local_contract_schema_type(raw: &str) -> Result<LocalContractSchemaType> {
    let trimmed = raw.trim();
    if trimmed == "()" {
        return Ok(LocalContractSchemaType::Unit);
    }
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.trim().is_empty() {
            return Ok(LocalContractSchemaType::Tuple(Vec::new()));
        }
        let items = split_local_schema_list(inner)?
            .into_iter()
            .map(|item| parse_local_contract_schema_type(&item))
            .collect::<Result<Vec<_>>>()?;
        return Ok(LocalContractSchemaType::Tuple(items));
    }
    match trimmed {
        "int" => Ok(LocalContractSchemaType::Int),
        "fixed_u128" | "Amount" | "Balance" => Ok(LocalContractSchemaType::Numeric),
        "bool" => Ok(LocalContractSchemaType::Bool),
        "string" => Ok(LocalContractSchemaType::String),
        "Json" => Ok(LocalContractSchemaType::Json),
        "Name" => Ok(LocalContractSchemaType::Name),
        "AccountId" => Ok(LocalContractSchemaType::AccountId),
        "AssetDefinitionId" => Ok(LocalContractSchemaType::AssetDefinitionId),
        "AssetId" => Ok(LocalContractSchemaType::AssetId),
        "DomainId" => Ok(LocalContractSchemaType::DomainId),
        "NftId" => Ok(LocalContractSchemaType::NftId),
        "Blob" => Ok(LocalContractSchemaType::Blob),
        "bytes" => Ok(LocalContractSchemaType::Bytes),
        "DataSpaceId" => Ok(LocalContractSchemaType::DataSpaceId),
        "AxtDescriptor" => Ok(LocalContractSchemaType::AxtDescriptor),
        "AssetHandle" => Ok(LocalContractSchemaType::AssetHandle),
        "ProofBlob" => Ok(LocalContractSchemaType::ProofBlob),
        _ => Err(eyre!("unsupported contract schema type `{trimmed}`")),
    }
}

fn validate_local_numeric_json_value(value: &norito::json::Value) -> bool {
    match value {
        norito::json::Value::String(raw) => {
            raw.parse::<iroha_primitives::numeric::Numeric>().is_ok()
        }
        norito::json::Value::Number(norito::json::native::Number::I64(_))
        | norito::json::Value::Number(norito::json::native::Number::U64(_)) => true,
        _ => false,
    }
}

fn validate_local_contract_value(
    schema: &LocalContractSchemaType,
    value: &norito::json::Value,
    field_name: &str,
) -> Result<()> {
    let ok = match schema {
        LocalContractSchemaType::Unit => matches!(value, norito::json::Value::Null),
        LocalContractSchemaType::Int => matches!(
            value,
            norito::json::Value::Number(norito::json::native::Number::I64(_))
                | norito::json::Value::Number(norito::json::native::Number::U64(_))
        ),
        LocalContractSchemaType::Numeric => validate_local_numeric_json_value(value),
        LocalContractSchemaType::Bool => matches!(value, norito::json::Value::Bool(_)),
        LocalContractSchemaType::String => matches!(value, norito::json::Value::String(_)),
        LocalContractSchemaType::Json => true,
        LocalContractSchemaType::Name => match value {
            norito::json::Value::String(raw) => Name::from_str(raw).is_ok(),
            _ => false,
        },
        LocalContractSchemaType::AccountId => match value {
            norito::json::Value::String(raw) => AccountId::parse_encoded(raw).is_ok(),
            _ => false,
        },
        LocalContractSchemaType::AssetDefinitionId => match value {
            norito::json::Value::String(raw) => raw
                .parse::<iroha_data_model::asset::AssetDefinitionId>()
                .is_ok(),
            _ => false,
        },
        LocalContractSchemaType::AssetId => match value {
            norito::json::Value::String(raw) => {
                raw.parse::<iroha_data_model::asset::AssetId>().is_ok()
            }
            _ => false,
        },
        LocalContractSchemaType::DomainId => match value {
            norito::json::Value::String(raw) => {
                iroha_data_model::domain::DomainId::parse_fully_qualified(raw).is_ok()
            }
            _ => false,
        },
        LocalContractSchemaType::NftId => match value {
            norito::json::Value::String(raw) => raw.parse::<iroha_data_model::nft::NftId>().is_ok(),
            _ => false,
        },
        LocalContractSchemaType::Blob | LocalContractSchemaType::Bytes => match value {
            norito::json::Value::String(raw) => {
                let raw = raw.strip_prefix("0x").unwrap_or(raw);
                raw.len() % 2 == 0 && hex::decode(raw).is_ok()
            }
            _ => false,
        },
        LocalContractSchemaType::DataSpaceId => match value {
            norito::json::Value::String(raw) => raw.parse::<u64>().is_ok(),
            norito::json::Value::Number(norito::json::native::Number::I64(v)) => *v >= 0,
            norito::json::Value::Number(norito::json::native::Number::U64(_)) => true,
            _ => false,
        },
        LocalContractSchemaType::AxtDescriptor
        | LocalContractSchemaType::AssetHandle
        | LocalContractSchemaType::ProofBlob => matches!(value, norito::json::Value::String(_)),
        LocalContractSchemaType::Tuple(items) => match value {
            norito::json::Value::Array(values) if values.len() == items.len() => {
                items.iter().zip(values.iter()).all(|(schema, value)| {
                    validate_local_contract_value(schema, value, field_name).is_ok()
                })
            }
            _ => false,
        },
    };
    if ok {
        Ok(())
    } else {
        Err(eyre!(
            "contract payload field `{field_name}` does not match the declared schema"
        ))
    }
}

fn normalize_local_contract_payload(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    payload: Option<&norito::json::Value>,
) -> Result<Option<iroha_primitives::json::Json>> {
    if descriptor.params.is_empty() {
        if let Some(payload) = payload {
            match payload {
                norito::json::Value::Object(map) if map.is_empty() => {}
                _ => {
                    return Err(eyre!(
                        "contract payload must be an empty JSON object for zero-parameter entrypoints"
                    ));
                }
            }
        }
        return Ok(None);
    }

    let payload = payload
        .ok_or_else(|| eyre!("contract payload is required for parameterized entrypoints"))?;
    let object = payload
        .as_object()
        .ok_or_else(|| eyre!("contract payload must be a JSON object keyed by parameter name"))?;

    let mut normalized = norito::json::Map::new();
    for param in &descriptor.params {
        let value = object.get(&param.name).ok_or_else(|| {
            eyre!(
                "missing contract payload field `{}` for entrypoint `{}`",
                param.name,
                descriptor.name
            )
        })?;
        let schema = parse_local_contract_schema_type(&param.type_name)?;
        validate_local_contract_value(&schema, value, &param.name)?;
        normalized.insert(param.name.clone(), value.clone());
    }

    for key in object.keys() {
        if !descriptor.params.iter().any(|param| param.name == *key) {
            return Err(eyre!(
                "unexpected contract payload field `{key}` for entrypoint `{}`",
                descriptor.name
            ));
        }
    }

    Ok(Some(iroha_primitives::json::Json::from(
        norito::json::Value::Object(normalized),
    )))
}

fn decode_local_contract_view_result_value(
    vm: &ivm::IVM,
    start_register: usize,
    schema: &LocalContractSchemaType,
) -> Result<(norito::json::Value, usize)> {
    let pointer_string = |ptr: u64| -> Result<String> {
        if ptr == 0 {
            return Err(eyre!(
                "contract view returned a null pointer for a non-nullable type"
            ));
        }
        Ok(ptr.to_string())
    };

    match schema {
        LocalContractSchemaType::Unit => Ok((norito::json::Value::Null, 0)),
        LocalContractSchemaType::Int => {
            let raw = vm.register(start_register);
            let value =
                i64::try_from(raw).map_err(|_| eyre!("contract view int return overflowed i64"))?;
            Ok((norito::json::Value::from(value), 1))
        }
        LocalContractSchemaType::Bool => Ok((
            norito::json::Value::Bool(vm.register(start_register) != 0),
            1,
        )),
        LocalContractSchemaType::Numeric => {
            let ptr = vm.register(start_register);
            let value: iroha_primitives::numeric::Numeric =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)
                    .map_err(|err| eyre!("failed to decode numeric return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::String => {
            Err(eyre!("string contract view returns are not supported yet"))
        }
        LocalContractSchemaType::Json => {
            let ptr = vm.register(start_register);
            let value = CoreHost::decode_tlv_json(vm, ptr)
                .map_err(|err| eyre!("failed to decode JSON return: {err}"))?;
            let parsed = norito::json::parse_value(value.get())
                .map_err(|err| eyre!("invalid JSON return payload: {err}"))?;
            Ok((parsed, 1))
        }
        LocalContractSchemaType::Name => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::name::Name =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::Name)
                    .map_err(|err| eyre!("failed to decode Name return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::AccountId => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::account::AccountId =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::AccountId)
                    .map_err(|err| eyre!("failed to decode AccountId return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::AssetDefinitionId => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::asset::AssetDefinitionId =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::AssetDefinitionId)
                    .map_err(|err| eyre!("failed to decode AssetDefinitionId return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::AssetId => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::asset::AssetId =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::AssetId)
                    .map_err(|err| eyre!("failed to decode AssetId return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::DomainId => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::domain::DomainId =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::DomainId)
                    .map_err(|err| eyre!("failed to decode DomainId return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::NftId => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::nft::NftId =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::NftId)
                    .map_err(|err| eyre!("failed to decode NftId return: {err}"))?;
            Ok((norito::json::Value::from(value.to_string()), 1))
        }
        LocalContractSchemaType::Blob | LocalContractSchemaType::Bytes => {
            let ptr = vm.register(start_register);
            let value = CoreHost::decode_tlv_blob(vm, ptr)
                .map_err(|err| eyre!("failed to decode blob return: {err}"))?;
            Ok((
                norito::json::Value::from(format!("0x{}", hex::encode(value))),
                1,
            ))
        }
        LocalContractSchemaType::DataSpaceId => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::nexus::DataSpaceId =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::DataSpaceId)
                    .map_err(|err| eyre!("failed to decode DataSpaceId return: {err}"))?;
            Ok((norito::json::Value::from(value.as_u64()), 1))
        }
        LocalContractSchemaType::AxtDescriptor => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::nexus::AxtDescriptor =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::AxtDescriptor)
                    .map_err(|err| eyre!("failed to decode AxtDescriptor return: {err}"))?;
            Ok((norito::json::to_value(&value)?, 1))
        }
        LocalContractSchemaType::AssetHandle => {
            let ptr = vm.register(start_register);
            let value: iroha_data_model::nexus::AssetHandle =
                CoreHost::decode_tlv_typed(vm, ptr, PointerType::AssetHandle)
                    .map_err(|err| eyre!("failed to decode AssetHandle return: {err}"))?;
            Ok((norito::json::to_value(&value)?, 1))
        }
        LocalContractSchemaType::ProofBlob => {
            let ptr = vm.register(start_register);
            let value = pointer_string(ptr)?;
            Ok((norito::json::Value::from(value), 1))
        }
        LocalContractSchemaType::Tuple(items) => {
            let mut values = Vec::with_capacity(items.len());
            let mut consumed = 0_usize;
            for item in items {
                let (value, used) =
                    decode_local_contract_view_result_value(vm, start_register + consumed, item)?;
                values.push(value);
                consumed += used;
            }
            Ok((norito::json::Value::Array(values), consumed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, ExposedPrivateKey};
    use iroha_i18n::{Bundle, Language, Localizer};
    use ivm::kotodama::compiler::CompilerOptions;
    use url::Url;

    fn minimal_program() -> Vec<u8> {
        let meta = ivm::ProgramMetadata {
            max_cycles: 1,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }

    fn minimal_view_contract_program() -> Vec<u8> {
        let source = r#"
            seiyaku Demo {
                view fn inspect() -> int {
                    return 7;
                }
            }
        "#;
        let compiler = ivm::KotodamaCompiler::new();
        let (program, _manifest) = compiler
            .compile_source_with_manifest(source)
            .expect("compile view contract");
        program
    }

    fn compile_contract_program(source: &str) -> Vec<u8> {
        let compiler = ivm::KotodamaCompiler::new();
        let (program, _manifest) = compiler
            .compile_source_with_manifest(source)
            .expect("compile contract");
        program
    }

    fn compile_contract_program_with_source_path(source: &str, source_path: &str) -> Vec<u8> {
        let compiler = ivm::KotodamaCompiler::new_with_options(CompilerOptions {
            debug_source_name: Some(source_path.to_owned()),
            ..CompilerOptions::default()
        });
        let (program, _manifest) = compiler
            .compile_source_with_manifest(source)
            .expect("compile contract with source path");
        program
    }

    #[test]
    fn program_summary_reports_hashes() {
        let program = minimal_program();
        let parsed = ivm::ProgramMetadata::parse(&program).expect("parse");
        let expected_code_hash = Hash::new(&program[parsed.header_len..]);
        let summary = program_summary_from_bytes(&program).expect("summary");
        assert_eq!(summary.code_hash, expected_code_hash);
        assert_eq!(
            summary.abi_hash,
            Hash::prehashed(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
        );
    }

    #[test]
    fn simulate_emits_gas_limit_metadata_key() {
        let key_pair = KeyPair::from_seed(vec![1u8; 32], Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let mut ctx = TestContext::new(authority.clone());
        let authority_literal = authority.to_string();
        let program = minimal_program();
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let private_key = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        let args = SimulateArgs {
            authority: authority_literal,
            private_key,
            code_file: None,
            code_b64: Some(code_b64),
            gas_limit: 42,
            target: ContractTargetArgs {
                contract_address: None,
                contract_alias: None,
            },
        };
        args.run(&mut ctx).expect("simulate");
        let output = ctx.take_output().expect("output");
        let metadata_keys = output
            .get("metadata_keys")
            .and_then(norito::json::Value::as_array)
            .expect("metadata_keys");
        let has_gas_limit = metadata_keys
            .iter()
            .any(|value| value.as_str() == Some("gas_limit"));
        assert!(
            has_gas_limit,
            "metadata_keys missing gas_limit: {metadata_keys:?}"
        );
    }

    #[test]
    fn debug_view_executes_local_view_and_decodes_result() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let mut ctx = TestContext::new(authority);
        let program = minimal_view_contract_program();
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let args = DebugViewArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: Some("inspect".to_owned()),
            gas_limit: 50_000,
            source_file: None,
            accounts_json: None,
            accounts_file: None,
            durable_state_json: None,
            durable_state_file: None,
            payload: ContractPayloadArgs {
                payload_json: None,
                payload_file: None,
            },
        };
        args.run(&mut ctx).expect("debug view");
        let output = ctx.take_output().expect("output");
        assert_eq!(
            output.get("ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            output.get("result").and_then(norito::json::Value::as_i64),
            Some(7)
        );
        assert_eq!(
            output
                .get("entrypoint")
                .and_then(norito::json::Value::as_object)
                .and_then(|entrypoint| entrypoint.get("name"))
                .and_then(norito::json::Value::as_str),
            Some("inspect")
        );
    }

    #[test]
    fn debug_view_uses_embedded_source_path_for_snippets() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let mut ctx = TestContext::new(authority);
        let dir = tempfile::tempdir().expect("tempdir");
        let source_path = dir.path().join("debug_view_with_path.ko");
        let source = r#"
            seiyaku Demo {
                view fn inspect() -> int {
                    return 7;
                }
            }
        "#;
        std::fs::write(&source_path, source).expect("write source");
        let program =
            compile_contract_program_with_source_path(source, &source_path.display().to_string());
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let args = DebugViewArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: Some("inspect".to_owned()),
            gas_limit: 0,
            source_file: None,
            accounts_json: None,
            accounts_file: None,
            durable_state_json: None,
            durable_state_file: None,
            payload: ContractPayloadArgs {
                payload_json: None,
                payload_file: None,
            },
        };
        args.run(&mut ctx).expect("debug view");
        let output = ctx.take_output().expect("output");
        assert_eq!(
            output.get("ok").and_then(norito::json::Value::as_bool),
            Some(false)
        );
        let snippet = output
            .get("source_snippet")
            .and_then(norito::json::Value::as_object)
            .expect("source snippet");
        assert_eq!(
            snippet.get("path").and_then(norito::json::Value::as_str),
            Some(source_path.to_string_lossy().as_ref())
        );
        let excerpt = snippet
            .get("excerpt")
            .and_then(norito::json::Value::as_str)
            .expect("excerpt");
        assert!(
            excerpt.contains("view fn inspect"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn debug_view_source_file_override_beats_embedded_path() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let mut ctx = TestContext::new(authority);
        let dir = tempfile::tempdir().expect("tempdir");
        let embedded_path = dir.path().join("embedded.ko");
        let override_path = dir.path().join("override.ko");
        let source = r#"
            seiyaku Demo {
                view fn inspect() -> int {
                    return 7;
                }
            }
        "#;
        std::fs::write(&embedded_path, source).expect("write embedded source");
        std::fs::write(
            &override_path,
            r#"
                seiyaku Demo {
                    view fn inspect() -> int {
                        return 99;
                    }
                }
            "#,
        )
        .expect("write override source");
        let program =
            compile_contract_program_with_source_path(source, &embedded_path.display().to_string());
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let args = DebugViewArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: Some("inspect".to_owned()),
            gas_limit: 0,
            source_file: Some(override_path.clone()),
            accounts_json: None,
            accounts_file: None,
            durable_state_json: None,
            durable_state_file: None,
            payload: ContractPayloadArgs {
                payload_json: None,
                payload_file: None,
            },
        };
        args.run(&mut ctx).expect("debug view");
        let output = ctx.take_output().expect("output");
        let snippet = output
            .get("source_snippet")
            .and_then(norito::json::Value::as_object)
            .expect("source snippet");
        assert_eq!(
            snippet.get("path").and_then(norito::json::Value::as_str),
            Some(override_path.to_string_lossy().as_ref())
        );
        let excerpt = snippet
            .get("excerpt")
            .and_then(norito::json::Value::as_str)
            .expect("excerpt");
        assert!(
            excerpt.contains("return 99"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn debug_call_executes_public_entrypoint_and_reports_side_effects() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let mut ctx = TestContext::new(authority);
        let source = r#"
            seiyaku Demo {
                state int counter;

                kotoage fn bump() -> int permission(Admin) {
                    counter = counter + 1;
                    register_domain(domain("debugcall"));
                    return counter;
                }
            }
        "#;
        let program = compile_contract_program(&source);
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let durable_state_json = format!(
            r#"{{"counter":"0x{}"}}"#,
            hex::encode(norito::to_bytes(&41_i64).expect("encode state"))
        );
        let args = DebugCallArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: Some("bump".to_owned()),
            gas_limit: 50_000,
            source_file: None,
            accounts_json: None,
            accounts_file: None,
            durable_state_json: Some(durable_state_json),
            durable_state_file: None,
            payload: ContractPayloadArgs {
                payload_json: None,
                payload_file: None,
            },
        };
        args.run(&mut ctx).expect("debug call");
        let output = ctx.take_output().expect("output");
        assert_eq!(
            output.get("ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            output.get("result").and_then(norito::json::Value::as_i64),
            Some(42)
        );
        assert_eq!(
            output
                .get("queued_instruction_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            output
                .get("durable_state_mutation_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            output
                .get("queued_instructions")
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        let overlay = output
            .get("durable_state_overlay")
            .and_then(norito::json::Value::as_object)
            .expect("durable overlay");
        assert!(
            overlay.contains_key("counter"),
            "expected durable overlay to contain counter: {overlay:?}"
        );
    }

    #[test]
    fn debug_call_rejects_view_entrypoints() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let mut ctx = TestContext::new(authority);
        let program = minimal_view_contract_program();
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let args = DebugCallArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: Some("inspect".to_owned()),
            gas_limit: 50_000,
            source_file: None,
            accounts_json: None,
            accounts_file: None,
            durable_state_json: None,
            durable_state_file: None,
            payload: ContractPayloadArgs {
                payload_json: None,
                payload_file: None,
            },
        };
        let err = args
            .run(&mut ctx)
            .expect_err("view entrypoints must be rejected");
        assert!(
            err.to_string().contains("is not a public entrypoint"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn debug_call_matches_overlay_for_public_by_call_execution() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let mut ctx = TestContext::new(authority.clone());
        let source = r#"
            seiyaku Demo {
                state int counter;

                kotoage fn bump(amount: int) -> int permission(Admin) {
                    register_domain(domain("debugparity"));
                    counter = amount;
                    return counter;
                }
            }
        "#;
        let program = compile_contract_program(source);
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let payload_json = r#"{"amount":7}"#.to_owned();
        let args = DebugCallArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: Some("bump".to_owned()),
            gas_limit: 50_000,
            source_file: None,
            accounts_json: None,
            accounts_file: None,
            durable_state_json: None,
            durable_state_file: None,
            payload: ContractPayloadArgs {
                payload_json: Some(payload_json.clone()),
                payload_file: None,
            },
        };
        args.run(&mut ctx).expect("debug call");
        let output = ctx.take_output().expect("debug call output");
        assert_eq!(
            output.get("ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("gas_limit").expect("static gas_limit key"),
            iroha_primitives::json::Json::from(50_000u64),
        );
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static contract_entrypoint key"),
            iroha_primitives::json::Json::from("bump"),
        );
        metadata.insert(
            Name::from_str("contract_payload").expect("static contract_payload key"),
            iroha_primitives::json::Json::from(
                norito::json::from_str::<norito::json::Value>(&payload_json).expect("payload json"),
            ),
        );
        let tx = TransactionBuilder::new(ctx.config().chain.clone(), authority.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program.clone())))
            .sign(ctx.config().key_pair.private_key());
        let overlay =
            build_overlay_for_transaction_with_accounts(&tx, std::slice::from_ref(&authority))
                .expect("overlay");

        assert_eq!(
            output
                .get("queued_instruction_count")
                .and_then(norito::json::Value::as_u64),
            Some(overlay.instruction_count() as u64)
        );
        let expected_queue_json =
            norito::json::to_value(&overlay.instruction_slice().to_vec()).expect("serialize queue");
        assert_eq!(
            output.get("queued_instructions"),
            Some(&expected_queue_json)
        );
        assert_eq!(
            output
                .get("durable_state_mutation_count")
                .and_then(norito::json::Value::as_u64),
            Some(overlay.durable_state_overlay().len() as u64)
        );
        let expected_durable_json = render_durable_state_overlay(overlay.durable_state_overlay())
            .expect("serialize durable overlay");
        assert_eq!(
            output.get("durable_state_overlay"),
            Some(&expected_durable_json)
        );
        assert_eq!(
            output.get("result").and_then(norito::json::Value::as_i64),
            Some(7)
        );
    }

    #[test]
    fn load_contract_payload_value_accepts_inline_json() {
        let payload = load_contract_payload_value(Some(r#"{"amount":7}"#), None).expect("payload");
        let object = payload
            .as_ref()
            .and_then(norito::json::Value::as_object)
            .expect("payload object");
        assert_eq!(
            object.get("amount").and_then(norito::json::Value::as_i64),
            Some(7)
        );
    }

    #[test]
    fn load_contract_payload_value_accepts_json_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("payload.json");
        std::fs::write(&path, r#"{"entrypoint":"mirror_state"}"#).expect("write payload");
        let payload = load_contract_payload_value(None, Some(&path)).expect("payload from file");
        let object = payload
            .as_ref()
            .and_then(norito::json::Value::as_object)
            .expect("payload object");
        assert_eq!(
            object
                .get("entrypoint")
                .and_then(norito::json::Value::as_str),
            Some("mirror_state")
        );
    }

    #[test]
    fn resolve_contract_target_accepts_contract_address() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            0,
            &authority,
            1,
            iroha::data_model::nexus::DataSpaceId::new(0),
        )
        .expect("contract address");
        let resolved = resolve_contract_target(ContractTargetArgs {
            contract_address: Some(contract_address.to_string()),
            contract_alias: None,
        })
        .expect("resolved target");
        assert_eq!(resolved.contract_address, Some(contract_address));
        assert!(resolved.contract_alias.is_none());
    }

    #[test]
    fn resolve_contract_target_accepts_contract_alias() {
        let resolved = resolve_contract_target(ContractTargetArgs {
            contract_address: None,
            contract_alias: Some("router::dex.universal".to_owned()),
        })
        .expect("resolved target");
        assert_eq!(
            resolved
                .contract_alias
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            Some("router::dex.universal")
        );
        assert!(resolved.contract_address.is_none());
    }

    #[test]
    fn resolve_contract_target_rejects_missing_target() {
        let err = resolve_contract_target(ContractTargetArgs {
            contract_address: None,
            contract_alias: None,
        })
        .expect_err("missing target should fail");
        assert!(
            err.to_string()
                .contains("provide exactly one contract target via --contract-address or --contract-alias")
        );
    }

    #[test]
    fn resolve_contract_dataspace_id_hint_accepts_default_aliases() {
        assert_eq!(
            resolve_contract_dataspace_id_hint("universal", None)
                .expect("universal")
                .as_u64(),
            0
        );
        assert_eq!(
            resolve_contract_dataspace_id_hint("governance", None)
                .expect("governance")
                .as_u64(),
            1
        );
        assert_eq!(
            resolve_contract_dataspace_id_hint("zk", None)
                .expect("zk")
                .as_u64(),
            2
        );
    }

    #[test]
    fn resolve_contract_dataspace_id_hint_requires_override_for_unknown_alias() {
        let err = resolve_contract_dataspace_id_hint("private-ds", None).expect_err("must fail");
        assert!(
            err.to_string()
                .contains("pass --dataspace-id for non-default dataspaces")
        );
    }

    #[test]
    fn resolve_contract_call_private_key_uses_context_key_for_default_authority() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let ctx = TestContext::new(authority.clone());
        let private_key =
            resolve_contract_call_private_key(&ctx, &authority, None, false).expect("key");
        assert_eq!(
            private_key,
            Some(ctx.config().key_pair.private_key().clone())
        );
    }

    #[test]
    fn resolve_contract_call_private_key_rejects_mismatched_authority_without_override() {
        let ctx = TestContext::new(AccountId::new(KeyPair::random().public_key().clone()));
        let other_authority = AccountId::new(KeyPair::random().public_key().clone());
        let err = resolve_contract_call_private_key(&ctx, &other_authority, None, false)
            .expect_err("missing override should fail");
        assert!(
            err.to_string()
                .contains("--private-key is required when --authority does not match")
        );
    }

    struct TestContext {
        cfg: iroha::config::Config,
        output: Option<norito::json::Value>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new(account: AccountId) -> Self {
            let key_pair = KeyPair::from_seed(vec![0u8; 32], Algorithm::Ed25519);
            let cfg = iroha::config::Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                soracloud_http_witness_file: None,
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                output: None,
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }

        fn take_output(&mut self) -> Option<norito::json::Value> {
            self.output.take()
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            self.output = Some(norito::json::to_value(data)?);
            Ok(())
        }

        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            Ok(())
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct SimulateArgs {
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key used to sign the simulated transaction
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Required `gas_limit` metadata to include in the simulated transaction
    #[arg(long)]
    pub gas_limit: u64,
    /// Optional canonical contract target metadata for call-time binding checks
    #[command(flatten)]
    pub target: ContractTargetArgs,
}

impl Run for SimulateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let authority = crate::resolve_account_id(context, &self.authority)
            .wrap_err("failed to resolve --authority")?;
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        let code = load_code_bytes(self.code_file.clone(), self.code_b64.clone())?;
        let summary = program_summary_from_bytes(&code)?;
        let contract_address = resolve_optional_contract_address(context, &self.target)?;

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("gas_limit")?,
            iroha_primitives::json::Json::from(self.gas_limit),
        );
        if let Some(contract_address) = contract_address.as_ref() {
            metadata.insert(
                Name::from_str("contract_address")?,
                iroha_primitives::json::Json::from(contract_address.as_ref()),
            );
        }

        let chain_id = context.config().chain.clone();
        let tx = TransactionBuilder::new(chain_id, authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(code.clone())))
            .sign(&private_key);

        let decoded = ivm::ivm_cache::IvmCache::decode_stream(&code[summary.code_offset..])
            .map_err(|err| eyre!("instruction decode failed: {err}"))?;
        let decoded_bytes: u64 = decoded.iter().map(|op| u64::from(op.len)).sum::<u64>();

        let overlay =
            build_overlay_for_transaction_with_accounts(&tx, std::slice::from_ref(&authority))
                .map_err(|err| eyre!("simulation overlay failed: {err}"))?;

        let instruction_ids: Vec<String> =
            overlay.instructions().map(|i| i.id().to_string()).collect();
        let metadata_keys: Vec<String> = metadata
            .iter()
            .map(|(name, _)| name.as_ref().to_string())
            .collect();
        let mut summary_json = norito::json::Map::new();
        summary_json.insert(
            "code_hash_hex".to_string(),
            norito::json::to_value(&hex::encode(summary.code_hash.as_ref()))?,
        );
        summary_json.insert(
            "abi_hash_hex".to_string(),
            norito::json::to_value(&hex::encode(summary.abi_hash.as_ref()))?,
        );
        summary_json.insert(
            "abi_version".to_string(),
            norito::json::to_value(&summary.metadata.abi_version)?,
        );
        summary_json.insert(
            "max_cycles".to_string(),
            norito::json::to_value(&summary.metadata.max_cycles)?,
        );
        summary_json.insert(
            "decoded_instructions".to_string(),
            norito::json::to_value(&decoded.len())?,
        );
        summary_json.insert(
            "decoded_code_bytes".to_string(),
            norito::json::to_value(&decoded_bytes)?,
        );
        summary_json.insert(
            "queued_instruction_count".to_string(),
            norito::json::to_value(&overlay.instruction_count())?,
        );
        summary_json.insert(
            "instruction_ids".to_string(),
            norito::json::to_value(&instruction_ids)?,
        );
        summary_json.insert(
            "metadata_keys".to_string(),
            norito::json::to_value(&metadata_keys)?,
        );
        let summary_json = norito::json::Value::Object(summary_json);
        context.print_data(&summary_json)?;
        Ok(())
    }
}

// Unified Manifest handling supersedes earlier subcommands

#[derive(clap::Args, Debug)]
pub struct ManifestArgs {
    /// Hex-encoded 32-byte code hash (0x optional)
    #[arg(long, value_name = "HEX64")]
    pub code_hash: String,
    /// Optional output path; if provided, writes JSON manifest to file, otherwise prints to stdout
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

impl Run for ManifestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let code_hash = self.code_hash.trim_start_matches("0x");
        let v = client.get_contract_manifest_json(code_hash)?;
        if let Some(p) = self.out {
            let s = norito::json::to_json_pretty(&v)?;
            std::fs::write(&p, s.as_bytes())?;
            context.println(format_args!("Wrote manifest to {}", p.display()))?;
        } else {
            context.print_data(&v)?;
        }
        Ok(())
    }
}
