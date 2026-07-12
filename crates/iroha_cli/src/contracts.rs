//! Contracts helpers.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use base64::Engine as _;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{Config, LoadPath},
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
use iroha_crypto::{KeyPair, PrivateKey};
use ivm::host::IVMHost;
use ivm::kotodama::driver::{
    BuildDriver as KotodamaBuildDriver, BuildStatus as KotodamaBuildStatus,
    PublishLayout as KotodamaPublishLayout, PublishMode as KotodamaPublishMode,
    SourceBuildRequest as KotodamaSourceBuildRequest, read_source_file as read_kotodama_source,
};
use reqwest::StatusCode;

use crate::{Run, RunContext, TransactionWaitArgs, wait_for_transaction_status};

// Canonical argument preparation reserves the bounded 1 MiB HEAP before
// decoding; keep the default above that floor with room for a small call.
const DEFAULT_CONTRACT_GAS_LIMIT: u64 = 1_500_000;

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Contract app bundle helpers
    #[command(subcommand)]
    App(AppCommand),
    /// First-release contract developer workflow
    #[command(subcommand)]
    Dev(DevCommand),
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
            Command::App(cmd) => cmd.run(context),
            Command::Dev(cmd) => cmd.run(context),
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

impl Command {
    /// Return whether this contract command is entirely local and may use the
    /// deterministic offline fallback configuration.
    pub(crate) fn allows_fallback_config(&self) -> bool {
        match self {
            Self::App(AppCommand::Build(_))
            | Self::Dev(
                DevCommand::Check(_)
                | DevCommand::Build(_)
                | DevCommand::Test(_)
                | DevCommand::Schema(_),
            )
            | Self::DeriveAddress(_)
            | Self::DebugView(_)
            | Self::DebugCall(_)
            | Self::Manifest(ManifestCommand::Build(_))
            | Self::Simulate(_) => true,
            Self::App(AppCommand::Plan(_) | AppCommand::Deploy(_) | AppCommand::Resume(_))
            | Self::Dev(
                DevCommand::Doctor(_)
                | DevCommand::Deploy(_)
                | DevCommand::Resume(_)
                | DevCommand::Call(_)
                | DevCommand::View(_)
                | DevCommand::Smoke(_),
            )
            | Self::Code(_)
            | Self::Alias(_)
            | Self::Deploy(_)
            | Self::Call(_)
            | Self::View(_)
            | Self::Manifest(ManifestCommand::Get(_)) => false,
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum AppCommand {
    /// Build an `iroha.contracts.toml` manifest into a compiled deployable bundle
    Build(AppBuildArgs),
    /// Compile a manifest and ask Torii for a dry-run deployment plan
    Plan(AppPlanArgs),
    /// Compile a manifest and deploy the bundle through Torii
    Deploy(AppDeployArgs),
    /// Resume an interrupted bundle deployment using the same manifest payload
    Resume(AppResumeArgs),
}

impl Run for AppCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            AppCommand::Build(args) => args.run(context),
            AppCommand::Plan(args) => args.run(context),
            AppCommand::Deploy(args) => args.run(context),
            AppCommand::Resume(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum DevCommand {
    /// Lint, build interfaces, and run Kotodama tests from a contract manifest
    Check(DevCheckArgs),
    /// Build all contract artifacts and generated interface files
    Build(DevBuildArgs),
    /// Run Kotodama test suites declared or discovered for the manifest
    Test(DevTestArgs),
    /// Validate local developer prerequisites for a contract manifest
    Doctor(DevDoctorArgs),
    /// Generate Markdown schema docs and sample payloads from interfaces
    Schema(DevSchemaArgs),
    /// Build and deploy all contracts from the manifest
    Deploy(DevDeployArgs),
    /// Build and resume deployment for all contracts from the manifest
    Resume(DevDeployArgs),
    /// Call a named manifest contract with typed payload validation
    Call(DevCallArgs),
    /// View a named manifest contract with typed payload validation
    View(DevViewArgs),
    /// Run smoke assertions declared by the manifest
    Smoke(DevSmokeArgs),
}

impl Run for DevCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            DevCommand::Check(args) => args.run(context),
            DevCommand::Build(args) => args.run(context),
            DevCommand::Test(args) => args.run(context),
            DevCommand::Doctor(args) => args.run(context),
            DevCommand::Schema(args) => args.run(context),
            DevCommand::Deploy(args) => args.run_with_action(context, DevDeployAction::Deploy),
            DevCommand::Resume(args) => args.run_with_action(context, DevDeployAction::Resume),
            DevCommand::Call(args) => args.run(context),
            DevCommand::View(args) => args.run(context),
            DevCommand::Smoke(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct DevManifestArgs {
    /// Path to the Iroha-first contract manifest.
    #[arg(long, default_value = "iroha.contracts.toml")]
    pub manifest: PathBuf,
    /// Named profile inside the manifest.
    #[arg(long, default_value = "local")]
    pub profile: String,
}

#[derive(clap::Args, Debug)]
pub struct DevBuildArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Fail if generated interface files differ from checked-in files.
    #[arg(long)]
    pub locked: bool,
}

#[derive(clap::Args, Debug)]
pub struct DevCheckArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Fail if generated interface files differ from checked-in files.
    #[arg(long)]
    pub locked: bool,
}

#[derive(clap::Args, Debug)]
pub struct DevTestArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Only run test source paths containing this text.
    #[arg(long)]
    pub path_filter: Option<String>,
    /// Only run test function names containing this text.
    #[arg(long)]
    pub filter: Option<String>,
    /// Match the complete test function name supplied by `--filter`.
    #[arg(long, requires = "filter")]
    pub exact: bool,
    /// Emit text or JSON from the native test runner when supported.
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Run coverage mode instead of normal test mode.
    #[arg(long)]
    pub coverage: bool,
    /// Run profile mode instead of normal test mode.
    #[arg(long)]
    pub profile_mode: bool,
}

#[derive(clap::Args, Debug)]
pub struct DevDoctorArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
}

#[derive(clap::Args, Debug)]
pub struct DevSchemaArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Output Markdown path. Omit to print Markdown to stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct DevDeployArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
}

#[derive(clap::Args, Debug)]
pub struct DevCallArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Manifest contract name, for example `dlmm.dlmm_pool`.
    #[arg(long)]
    pub contract: String,
    /// Authority account identifier. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Hex-encoded private key override used to sign and submit the call directly.
    #[arg(long, value_name = "HEX", conflicts_with = "scaffold_only")]
    pub private_key: Option<String>,
    /// Request an unsigned transaction scaffold instead of direct submission.
    #[arg(long)]
    pub scaffold_only: bool,
    /// Contract entrypoint selector.
    #[arg(long)]
    pub entrypoint: String,
    /// Optional gas asset id forwarded to transaction metadata.
    #[arg(long)]
    pub gas_asset_id: Option<String>,
    /// Optional fee sponsor account charged for gas/fees when supported.
    #[arg(long)]
    pub fee_sponsor: Option<String>,
    /// Gas limit metadata forwarded to the contract call. Defaults to the manifest profile value.
    #[arg(long)]
    pub gas_limit: Option<u64>,
    #[command(flatten)]
    pub payload: ContractPayloadArgs,
    #[command(flatten)]
    pub wait: TransactionWaitArgs,
}

#[derive(clap::Args, Debug)]
pub struct DevViewArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Manifest contract name, for example `n3x.n3x_hub`.
    #[arg(long)]
    pub contract: String,
    /// Authority account identifier used as the read context. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Contract view entrypoint selector.
    #[arg(long)]
    pub entrypoint: String,
    /// Gas limit applied to the view execution. Defaults to the manifest profile value.
    #[arg(long)]
    pub gas_limit: Option<u64>,
    #[command(flatten)]
    pub payload: ContractPayloadArgs,
}

#[derive(clap::Args, Debug)]
pub struct DevSmokeArgs {
    #[command(flatten)]
    pub manifest: DevManifestArgs,
    /// Authority account identifier used for smoke views/calls. Defaults to the profile client config.
    #[arg(long)]
    pub authority: Option<String>,
    /// Hex-encoded private key override used for smoke call scenarios.
    #[arg(long, value_name = "HEX")]
    pub private_key: Option<String>,
    #[command(flatten)]
    pub wait: TransactionWaitArgs,
}

enum DevDeployAction {
    Deploy,
    Resume,
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

#[derive(Debug)]
struct ContractAppManifest {
    bundle_name: String,
    default_dataspace: Option<String>,
    contracts: Vec<ContractAppManifestContract>,
    hajimari: Vec<ContractAppManifestHajimariCall>,
    assertions: Vec<ContractAppManifestAssertion>,
    profiles: BTreeMap<String, ContractDevManifestProfile>,
    tests: Vec<ContractDevManifestTest>,
    smoke: Vec<ContractDevManifestSmoke>,
}

#[derive(Debug)]
struct ContractAppManifestContract {
    name: String,
    alias: String,
    source: Option<PathBuf>,
    artifact: Option<PathBuf>,
    depends_on: Vec<String>,
    lease_expiry_ms: Option<u64>,
}

#[derive(Debug)]
struct ContractAppManifestHajimariCall {
    id: String,
    contract: String,
    entrypoint: String,
    payload: Option<toml::Value>,
    gas_limit: u64,
    gas_asset_id: Option<String>,
    fee_sponsor: Option<String>,
}

#[derive(Debug)]
struct ContractAppManifestAssertion {
    id: String,
    contract: String,
    entrypoint: String,
    payload: Option<toml::Value>,
    gas_limit: u64,
    expected_result: Option<toml::Value>,
}

#[derive(Debug, Default)]
struct ContractDevManifestProfile {
    client_config: Option<PathBuf>,
    default_gas_limit: Option<u64>,
    fee_asset_id: Option<String>,
}

#[derive(Debug, Default)]
struct ContractDevManifestTest {
    path: PathBuf,
}

#[derive(Debug, Default)]
struct ContractDevManifestSmoke {
    id: String,
    contract: String,
    mode: Option<String>,
    entrypoint: String,
    payload: Option<toml::Value>,
    expected_result: Option<toml::Value>,
    gas_limit: Option<u64>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ContractAppManifestArgs {
    /// Path to the contract app manifest (`iroha.contracts.toml`)
    #[arg(long, default_value = "iroha.contracts.toml")]
    pub manifest: PathBuf,
}

#[derive(clap::Args, Debug)]
pub struct AppBuildArgs {
    #[command(flatten)]
    pub manifest: ContractAppManifestArgs,
    /// Optional output path for the compiled bundle JSON
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct AppPlanArgs {
    #[command(flatten)]
    pub manifest: ContractAppManifestArgs,
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Optional transaction time-to-live in milliseconds for bundle deploy and hajimari transactions.
    #[arg(long)]
    pub transaction_ttl_ms: Option<u64>,
}

#[derive(clap::Args, Debug)]
pub struct AppDeployArgs {
    #[command(flatten)]
    pub manifest: ContractAppManifestArgs,
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Optional transaction time-to-live in milliseconds for bundle deploy and hajimari transactions.
    #[arg(long)]
    pub transaction_ttl_ms: Option<u64>,
}

#[derive(clap::Args, Debug)]
pub struct AppResumeArgs {
    #[command(flatten)]
    pub manifest: ContractAppManifestArgs,
    /// Authority account identifier (canonical I105 account literal)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Optional transaction time-to-live in milliseconds for bundle deploy and hajimari transactions.
    #[arg(long)]
    pub transaction_ttl_ms: Option<u64>,
}

fn default_contract_artifact_path(manifest_path: &Path, seiyaku_name: &str) -> Result<PathBuf> {
    let base = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artifacts");
    fs::create_dir_all(&base)?;
    Ok(base.join(format!("{seiyaku_name}.to")))
}

fn resolve_manifest_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn resolve_contract_manifest_alias(
    alias_literal: &str,
    default_dataspace: Option<&str>,
) -> Result<iroha::data_model::smart_contract::ContractAlias> {
    let alias_literal = alias_literal.trim();
    if alias_literal.contains("::") {
        return alias_literal
            .parse()
            .wrap_err_with(|| format!("invalid contract alias `{alias_literal}`"));
    }
    let dataspace = default_dataspace.ok_or_else(|| {
        eyre!(
            "contract alias `{alias_literal}` is missing a dataspace and manifest has no default_dataspace"
        )
    })?;
    format!("{alias_literal}::{dataspace}")
        .parse()
        .wrap_err_with(|| format!("invalid contract alias `{alias_literal}`"))
}

fn toml_to_json_value(value: toml::Value) -> Result<norito::json::Value> {
    match value {
        toml::Value::String(value) => Ok(norito::json::Value::from(value)),
        toml::Value::Integer(value) => Ok(norito::json::Value::from(value)),
        toml::Value::Float(value) => Ok(norito::json::Value::from(value)),
        toml::Value::Boolean(value) => Ok(norito::json::Value::from(value)),
        toml::Value::Datetime(value) => Ok(norito::json::Value::from(value.to_string())),
        toml::Value::Array(values) => Ok(norito::json::Value::Array(
            values
                .into_iter()
                .map(toml_to_json_value)
                .collect::<Result<Vec<_>>>()?,
        )),
        toml::Value::Table(values) => Ok(norito::json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, toml_to_json_value(value)?)))
                .collect::<Result<norito::json::Map>>()?,
        )),
    }
}

fn toml_required_string(table: &toml::Table, key: &str, context: &str) -> Result<String> {
    match table.get(key).and_then(toml::Value::as_str) {
        Some(value) => Ok(value.to_owned()),
        None => Err(eyre!("`{context}.{key}` must be a string")),
    }
}

fn toml_optional_string(table: &toml::Table, key: &str, context: &str) -> Result<Option<String>> {
    match table.get(key) {
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| eyre!("`{context}.{key}` must be a string")),
        None => Ok(None),
    }
}

fn toml_optional_path(table: &toml::Table, key: &str, context: &str) -> Result<Option<PathBuf>> {
    Ok(toml_optional_string(table, key, context)?.map(PathBuf::from))
}

fn toml_required_u64(table: &toml::Table, key: &str, context: &str) -> Result<u64> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| eyre!("`{context}.{key}` must be a non-negative integer"))?;
    u64::try_from(value).map_err(|_| eyre!("`{context}.{key}` must be a non-negative integer"))
}

fn toml_optional_u64(table: &toml::Table, key: &str, context: &str) -> Result<Option<u64>> {
    match table.get(key) {
        Some(_) => toml_required_u64(table, key, context).map(Some),
        None => Ok(None),
    }
}

fn toml_optional_string_array(
    table: &toml::Table,
    key: &str,
    context: &str,
) -> Result<Vec<String>> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| eyre!("`{context}.{key}` must be an array of strings"))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| eyre!("`{context}.{key}[{index}]` must be a string"))
        })
        .collect()
}

fn toml_table_array<'a>(
    table: &'a toml::Table,
    key: &str,
    context: &str,
) -> Result<Vec<&'a toml::Table>> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| eyre!("`{context}.{key}` must be an array of tables"))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_table()
                .ok_or_else(|| eyre!("`{context}.{key}[{index}]` must be a table"))
        })
        .collect()
}

fn parse_contract_manifest_contract(
    table: &toml::Table,
    index: usize,
) -> Result<ContractAppManifestContract> {
    let context = format!("contracts[{index}]");
    Ok(ContractAppManifestContract {
        name: toml_required_string(table, "name", &context)?,
        alias: toml_required_string(table, "alias", &context)?,
        source: toml_optional_path(table, "source", &context)?,
        artifact: toml_optional_path(table, "artifact", &context)?,
        depends_on: toml_optional_string_array(table, "depends_on", &context)?,
        lease_expiry_ms: toml_optional_u64(table, "lease_expiry_ms", &context)?,
    })
}

fn parse_contract_manifest_hajimari_call(
    table: &toml::Table,
    index: usize,
) -> Result<ContractAppManifestHajimariCall> {
    let context = format!("hajimari[{index}]");
    Ok(ContractAppManifestHajimariCall {
        id: toml_required_string(table, "id", &context)?,
        contract: toml_required_string(table, "contract", &context)?,
        entrypoint: toml_required_string(table, "entrypoint", &context)?,
        payload: table.get("payload").cloned(),
        gas_limit: toml_required_u64(table, "gas_limit", &context)?,
        gas_asset_id: toml_optional_string(table, "gas_asset_id", &context)?,
        fee_sponsor: toml_optional_string(table, "fee_sponsor", &context)?,
    })
}

fn parse_contract_manifest_assertion(
    table: &toml::Table,
    index: usize,
) -> Result<ContractAppManifestAssertion> {
    let context = format!("assertions[{index}]");
    Ok(ContractAppManifestAssertion {
        id: toml_required_string(table, "id", &context)?,
        contract: toml_required_string(table, "contract", &context)?,
        entrypoint: toml_required_string(table, "entrypoint", &context)?,
        payload: table.get("payload").cloned(),
        gas_limit: toml_required_u64(table, "gas_limit", &context)?,
        expected_result: table.get("expected_result").cloned(),
    })
}

fn parse_contract_manifest_profile(
    table: &toml::Table,
    name: &str,
) -> Result<ContractDevManifestProfile> {
    let context = format!("profiles.{name}");
    Ok(ContractDevManifestProfile {
        client_config: toml_optional_path(table, "client_config", &context)?,
        default_gas_limit: toml_optional_u64(table, "default_gas_limit", &context)?,
        fee_asset_id: toml_optional_string(table, "fee_asset_id", &context)?,
    })
}

fn parse_contract_manifest_test(
    table: &toml::Table,
    index: usize,
) -> Result<ContractDevManifestTest> {
    let context = format!("tests[{index}]");
    Ok(ContractDevManifestTest {
        path: PathBuf::from(toml_required_string(table, "path", &context)?),
    })
}

fn parse_contract_manifest_smoke(
    table: &toml::Table,
    index: usize,
) -> Result<ContractDevManifestSmoke> {
    let context = format!("smoke[{index}]");
    Ok(ContractDevManifestSmoke {
        id: toml_required_string(table, "id", &context)?,
        contract: toml_required_string(table, "contract", &context)?,
        mode: toml_optional_string(table, "mode", &context)?,
        entrypoint: toml_required_string(table, "entrypoint", &context)?,
        payload: table.get("payload").cloned(),
        expected_result: table.get("expected_result").cloned(),
        gas_limit: toml_optional_u64(table, "gas_limit", &context)?,
    })
}

fn parse_contract_app_manifest(value: toml::Value) -> Result<ContractAppManifest> {
    let table = value
        .as_table()
        .ok_or_else(|| eyre!("contract app manifest root must be a TOML table"))?;
    const ALLOWED_TOP_LEVEL_KEYS: &[&str] = &[
        "bundle_name",
        "default_dataspace",
        "contracts",
        "hajimari",
        "assertions",
        "profiles",
        "tests",
        "smoke",
    ];
    if let Some(key) = table
        .keys()
        .find(|key| !ALLOWED_TOP_LEVEL_KEYS.contains(&key.as_str()))
    {
        return Err(eyre!(
            "unknown contract manifest field `{key}`; expected one of {}",
            ALLOWED_TOP_LEVEL_KEYS.join(", ")
        ));
    }
    let contracts = toml_table_array(table, "contracts", "manifest")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_contract_manifest_contract(table, index))
        .collect::<Result<Vec<_>>>()?;
    if contracts.is_empty() {
        return Err(eyre!(
            "`manifest.contracts` must contain at least one table"
        ));
    }
    let hajimari = toml_table_array(table, "hajimari", "manifest")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_contract_manifest_hajimari_call(table, index))
        .collect::<Result<Vec<_>>>()?;
    let assertions = toml_table_array(table, "assertions", "manifest")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_contract_manifest_assertion(table, index))
        .collect::<Result<Vec<_>>>()?;
    let profiles = match table.get("profiles") {
        Some(value) => value
            .as_table()
            .ok_or_else(|| eyre!("`manifest.profiles` must be a table"))?
            .iter()
            .map(|(name, value)| {
                let profile = value
                    .as_table()
                    .ok_or_else(|| eyre!("`profiles.{name}` must be a table"))?;
                Ok((
                    name.clone(),
                    parse_contract_manifest_profile(profile, name)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?,
        None => BTreeMap::new(),
    };
    let tests = toml_table_array(table, "tests", "manifest")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_contract_manifest_test(table, index))
        .collect::<Result<Vec<_>>>()?;
    let smoke = toml_table_array(table, "smoke", "manifest")?
        .into_iter()
        .enumerate()
        .map(|(index, table)| parse_contract_manifest_smoke(table, index))
        .collect::<Result<Vec<_>>>()?;

    Ok(ContractAppManifest {
        bundle_name: toml_required_string(table, "bundle_name", "manifest")?,
        default_dataspace: toml_optional_string(table, "default_dataspace", "manifest")?,
        contracts,
        hajimari,
        assertions,
        profiles,
        tests,
        smoke,
    })
}

fn compile_or_load_contract_code(
    manifest_path: &Path,
    contract: &ContractAppManifestContract,
) -> Result<Vec<u8>> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    match (&contract.source, &contract.artifact) {
        (Some(source), artifact) => {
            let source_path = resolve_manifest_path(manifest_dir, source);
            let source_text = read_kotodama_source(&source_path).map_err(|error| eyre!(error))?;
            let source_name = source_path.display().to_string();
            let compiler = ivm::kotodama::session::CompilerSession::default();
            let output = compiler
                .build(ivm::kotodama::session::CompileRequest {
                    source: &source_text,
                    source_name: Some(&source_name),
                })
                .map_err(|err| eyre!("failed to compile `{}`: {err}", source_path.display()))?;
            let program = output.artifact;
            let artifact_path = artifact
                .as_ref()
                .map(|path| resolve_manifest_path(manifest_dir, path))
                .unwrap_or(default_contract_artifact_path(
                    manifest_path,
                    &contract.name,
                )?);
            if let Some(parent) = artifact_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&artifact_path, &program).wrap_err_with(|| {
                format!(
                    "failed to write compiled artifact `{}`",
                    artifact_path.display()
                )
            })?;
            Ok(program)
        }
        (None, Some(artifact)) => {
            let artifact_path = resolve_manifest_path(manifest_dir, artifact);
            fs::read(&artifact_path)
                .wrap_err_with(|| format!("failed to read `{}`", artifact_path.display()))
        }
        (None, None) => Err(eyre!(
            "contract `{}` must declare either `source` or `artifact`",
            contract.name
        )),
    }
}

fn load_contract_app_manifest(path: &Path) -> Result<ContractAppManifest> {
    let body = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
    let value = toml::from_str::<toml::Value>(&body)
        .wrap_err_with(|| format!("failed to parse `{}`", path.display()))?;
    parse_contract_app_manifest(value)
        .wrap_err_with(|| format!("failed to decode `{}`", path.display()))
}

fn build_contract_app_bundle(manifest_path: &Path) -> Result<norito::json::Value> {
    let manifest = load_contract_app_manifest(manifest_path)?;
    let default_dataspace = manifest.default_dataspace.as_deref();
    let mut contracts = Vec::with_capacity(manifest.contracts.len());
    let mut alias_by_name = BTreeMap::new();
    let mut interface_by_name = BTreeMap::new();

    for contract in &manifest.contracts {
        let contract_alias = resolve_contract_manifest_alias(&contract.alias, default_dataspace)?;
        alias_by_name.insert(contract.name.clone(), contract_alias.clone());
        let code = compile_or_load_contract_code(manifest_path, contract)?;
        let verified = ivm::verify_contract_artifact(&code).map_err(|error| {
            eyre!(
                "contract `{}` is not a valid deployable artifact: {error}",
                contract.name
            )
        })?;
        interface_by_name.insert(contract.name.clone(), verified.manifest);
        contracts.push(norito::json!({
            "name": (contract.name.clone()),
            "contract_alias": (contract_alias),
            "code_b64": (base64::engine::general_purpose::STANDARD.encode(code)),
            "lease_expiry_ms": (contract.lease_expiry_ms),
            "depends_on": (contract.depends_on.clone()),
        }));
    }

    let resolve_alias_ref =
        |value: &str| -> Result<iroha::data_model::smart_contract::ContractAlias> {
            if let Some(alias) = alias_by_name.get(value) {
                return Ok(alias.clone());
            }
            resolve_contract_manifest_alias(value, default_dataspace)
        };

    let hajimari_calls = manifest
        .hajimari
        .into_iter()
        .map(|call| {
            if let Some(interface) = interface_by_name.get(&call.contract) {
                let descriptor = interface
                    .entrypoints
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .find(|entrypoint| entrypoint.name == call.entrypoint)
                    .ok_or_else(|| {
                        eyre!(
                            "hajimari call `{}` targets unknown entrypoint `{}` on contract `{}`",
                            call.id,
                            call.entrypoint,
                            call.contract
                        )
                    })?;
                if descriptor.kind
                    != iroha::data_model::smart_contract::manifest::EntryPointKind::Hajimari
                {
                    return Err(eyre!(
                        "hajimari call `{}` must target a hajimari/始まり entrypoint",
                        call.id
                    ));
                }
            }
            Ok(norito::json!({
                "id": (call.id),
                "contract_alias": (resolve_alias_ref(&call.contract)?),
                "entrypoint": (call.entrypoint),
                "payload": (call.payload.map(toml_to_json_value).transpose()?),
                "gas_limit": (call.gas_limit),
                "gas_asset_id": (call.gas_asset_id),
                "fee_sponsor": (call.fee_sponsor),
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    let assertions = manifest
        .assertions
        .into_iter()
        .map(|assertion| {
            Ok(norito::json!({
                "id": (assertion.id),
                "contract_alias": (resolve_alias_ref(&assertion.contract)?),
                "entrypoint": (assertion.entrypoint),
                "payload": (assertion.payload.map(toml_to_json_value).transpose()?),
                "gas_limit": (assertion.gas_limit),
                "expected_result": (assertion.expected_result.map(toml_to_json_value).transpose()?),
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(norito::json!({
        "bundle_name": (manifest.bundle_name),
        "default_dataspace": (manifest.default_dataspace),
        "contracts": (contracts),
        "hajimari_calls": (hajimari_calls),
        "assertions": (assertions),
    }))
}

fn wrap_contract_bundle_request(
    bundle: norito::json::Value,
    authority: &AccountId,
    private_key: &PrivateKey,
    transaction_ttl_ms: Option<u64>,
) -> Result<norito::json::Value> {
    let mut object = bundle
        .as_object()
        .cloned()
        .ok_or_else(|| eyre!("compiled bundle must be a JSON object"))?;
    object.insert("authority".to_owned(), authority.to_string().into());
    object.insert(
        "private_key".to_owned(),
        norito::json::to_value(&iroha_data_model::prelude::ExposedPrivateKey(
            private_key.clone(),
        ))?,
    );
    if let Some(transaction_ttl_ms) = transaction_ttl_ms {
        object.insert("transaction_ttl_ms".to_owned(), transaction_ttl_ms.into());
    }
    Ok(norito::json::Value::Object(object))
}

fn resolve_contract_app_authority<C: RunContext>(
    context: &C,
    authority: &str,
    private_key: &str,
) -> Result<(AccountId, PrivateKey)> {
    let authority =
        crate::resolve_account_id(context, authority).wrap_err("failed to resolve --authority")?;
    let private_key = private_key.parse().wrap_err("invalid --private-key")?;
    Ok((authority, private_key))
}

impl Run for AppBuildArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bundle = build_contract_app_bundle(&self.manifest.manifest)?;
        if let Some(out) = self.out {
            let body = norito::json::to_json_pretty(&bundle)?;
            fs::write(&out, body)
                .wrap_err_with(|| format!("failed to write `{}`", out.display()))?;
            context.println(format_args!("Wrote bundle to {}", out.display()))?;
        } else {
            context.print_data(&bundle)?;
        }
        Ok(())
    }
}

impl Run for AppPlanArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bundle = build_contract_app_bundle(&self.manifest.manifest)?;
        let (authority, private_key) =
            resolve_contract_app_authority(context, &self.authority, &self.private_key)?;
        let request = wrap_contract_bundle_request(
            bundle,
            &authority,
            &private_key,
            self.transaction_ttl_ms,
        )?;
        let client: Client = context.client_from_config();
        let response = client.post_contract_deploy_bundle_json(&request, true)?;
        context.print_data(&response)?;
        Ok(())
    }
}

impl Run for AppDeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bundle = build_contract_app_bundle(&self.manifest.manifest)?;
        let (authority, private_key) =
            resolve_contract_app_authority(context, &self.authority, &self.private_key)?;
        let request = wrap_contract_bundle_request(
            bundle,
            &authority,
            &private_key,
            self.transaction_ttl_ms,
        )?;
        let client: Client = context.client_from_config();
        let response = client.post_contract_deploy_bundle_json(&request, false)?;
        context.print_data(&response)?;
        Ok(())
    }
}

impl Run for AppResumeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bundle = build_contract_app_bundle(&self.manifest.manifest)?;
        let (authority, private_key) =
            resolve_contract_app_authority(context, &self.authority, &self.private_key)?;
        let request = wrap_contract_bundle_request(
            bundle,
            &authority,
            &private_key,
            self.transaction_ttl_ms,
        )?;
        let client: Client = context.client_from_config();
        let response = client.post_contract_deploy_bundle_json(&request, false)?;
        context.print_data(&response)?;
        Ok(())
    }
}

impl DevBuildArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report =
            dev_build_manifest(&self.manifest.manifest, &self.manifest.profile, self.locked)?;
        context.print_data(&report)
    }
}

impl DevCheckArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let lint = dev_run_lints(&self.manifest.manifest)?;
        let lint_ok = lint
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if !lint_ok {
            context.print_data(&norito::json!({
                "ok": false,
                "profile": (self.manifest.profile),
                "lint": (lint),
            }))?;
            return Err(eyre!(
                "contract lint failed; detailed diagnostics were emitted in the lint report"
            ));
        }
        let build =
            dev_build_manifest(&self.manifest.manifest, &self.manifest.profile, self.locked)?;
        let test = dev_run_tests(
            &self.manifest.manifest,
            None,
            None,
            false,
            false,
            false,
            "text",
        )?;
        context.print_data(&norito::json!({
            "ok": true,
            "profile": (self.manifest.profile),
            "lint": (lint),
            "build": (build),
            "test": (test),
        }))
    }
}

impl DevTestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = dev_run_tests(
            &self.manifest.manifest,
            self.path_filter.as_deref(),
            self.filter.as_deref(),
            self.exact,
            self.coverage,
            self.profile_mode,
            &self.format,
        )?;
        context.print_data(&report)
    }
}

impl DevDoctorArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest = load_contract_app_manifest(&self.manifest.manifest)?;
        let manifest_dir = self
            .manifest
            .manifest
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let mut source_count = 0_u64;
        for contract in &manifest.contracts {
            if let Some(source) = &contract.source {
                let path = resolve_manifest_path(manifest_dir, source);
                if !path.is_file() {
                    return Err(eyre!(
                        "contract `{}` source missing: {}",
                        contract.name,
                        path.display()
                    ));
                }
                source_count += 1;
            }
        }
        let profile = manifest
            .profiles
            .get(&self.manifest.profile)
            .ok_or_else(|| {
                eyre!(
                    "manifest `{}` does not define profile `{}`",
                    self.manifest.manifest.display(),
                    self.manifest.profile
                )
            })?;
        let client_config_path = profile
            .client_config
            .as_ref()
            .map(|path| resolve_manifest_path(manifest_dir, path));
        let profile_config = load_dev_profile_config(manifest_dir, Some(profile))?;
        let effective_config = profile_config.as_ref().unwrap_or_else(|| context.config());
        let client = dev_client_from_profile(context, profile_config.as_ref());
        let default_gas_limit = profile.default_gas_limit;
        let server_version = client.get_server_version().wrap_err_with(|| {
            format!(
                "failed to contact Torii for profile `{}` at {}",
                self.manifest.profile, effective_config.torii_api_url
            )
        })?;
        let status = client.get_status().wrap_err_with(|| {
            format!(
                "failed to fetch Torii status for profile `{}` at {}",
                self.manifest.profile, effective_config.torii_api_url
            )
        })?;
        context.print_data(&norito::json!({
            "ok": true,
            "manifest": (self.manifest.manifest.display().to_string()),
            "profile": (self.manifest.profile),
            "contract_count": (manifest.contracts.len() as u64),
            "source_count": (source_count),
            "profile_known": true,
            "client_config": (client_config_path.map(|path| path.display().to_string())),
            "torii_url": (effective_config.torii_api_url.to_string()),
            "default_gas_limit": (default_gas_limit),
            "fee_asset_id": (profile.fee_asset_id.as_deref()),
            "signer_account": (effective_config.account.to_string()),
            "signer_public_key": (effective_config.key_pair.public_key().to_string()),
            "server_version": (server_version),
            "block_height": (status.blocks),
            "block_height_sysvar": (status.blocks >= 1),
            "signature_syscall": true,
            "manifest_admission": true,
        }))
    }
}

impl DevSchemaArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = dev_build_manifest(&self.manifest.manifest, &self.manifest.profile, false)?;
        let markdown = render_dev_schema_markdown(&self.manifest.manifest, &report)?;
        if let Some(path) = self.out {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, markdown)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
            context
                .print_data(&norito::json!({ "ok": true, "schema": (path.display().to_string()) }))
        } else {
            context.println(markdown)
        }
    }
}

impl DevDeployArgs {
    fn run_with_action<C: RunContext>(
        self,
        context: &mut C,
        action: DevDeployAction,
    ) -> Result<()> {
        let _ = dev_build_manifest(&self.manifest.manifest, &self.manifest.profile, true)?;
        match action {
            DevDeployAction::Deploy => AppDeployArgs {
                manifest: ContractAppManifestArgs {
                    manifest: self.manifest.manifest,
                },
                authority: self.authority,
                private_key: self.private_key,
                transaction_ttl_ms: None,
            }
            .run(context),
            DevDeployAction::Resume => AppResumeArgs {
                manifest: ContractAppManifestArgs {
                    manifest: self.manifest.manifest,
                },
                authority: self.authority,
                private_key: self.private_key,
                transaction_ttl_ms: None,
            }
            .run(context),
        }
    }
}

impl DevCallArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest = load_contract_app_manifest(&self.manifest.manifest)?;
        let contract = dev_contract_by_name(&manifest, &self.contract)?;
        let manifest_dir = self
            .manifest
            .manifest
            .parent()
            .unwrap_or_else(|| Path::new("."));
        validate_dev_payload_for_contract(
            &self.manifest.manifest,
            &self.manifest.profile,
            contract,
            &self.entrypoint,
            self.payload.payload_json.as_deref(),
            self.payload.payload_file.as_deref(),
        )?;
        let profile_config =
            load_dev_profile_config(manifest_dir, manifest.profiles.get(&self.manifest.profile))?;
        let client = dev_client_from_profile(context, profile_config.as_ref());
        let authority = resolve_dev_contract_authority(
            context,
            profile_config.as_ref(),
            self.authority.as_deref(),
        )?;
        let private_key = resolve_dev_contract_private_key(
            context,
            profile_config.as_ref(),
            &authority,
            self.private_key.as_deref(),
            self.scaffold_only,
        )?;
        let fee_sponsor = self
            .fee_sponsor
            .as_deref()
            .map(|value| crate::resolve_account_id(context, value))
            .transpose()
            .wrap_err("failed to resolve --fee-sponsor")?;
        let contract_alias = resolve_contract_manifest_alias(
            &contract.alias,
            manifest.default_dataspace.as_deref(),
        )?;
        let payload = load_contract_payload_value(
            self.payload.payload_json.as_deref(),
            self.payload.payload_file.as_deref(),
        )?;
        let gas_limit = self.gas_limit.unwrap_or_else(|| {
            dev_profile_default_gas_limit(manifest.profiles.get(&self.manifest.profile))
        });
        let value = client.post_contract_call_json(
            &authority,
            private_key.as_ref(),
            None,
            Some(&contract_alias),
            &self.entrypoint,
            payload.as_ref(),
            None,
            self.gas_asset_id.as_deref(),
            fee_sponsor.as_ref(),
            gas_limit,
        )?;
        if self.wait.is_enabled() {
            let tx_hash = extract_submitted_transaction_hash(&value)
                .wrap_err("contract call response missing canonical `tx_hash_hex`")?;
            let status = wait_for_transaction_status(&client, tx_hash, &self.wait)?;
            context.print_data(&ContractSubmissionWaitResponse {
                submit: value,
                trace: None,
                terminal_kind: status.terminal_kind,
                attempts: status.attempts,
                elapsed_ms: status.elapsed_ms,
                block_height: status.block_height,
                rejection_reason: status.rejection_reason,
                scope: status.scope,
                resolved_from: status.resolved_from,
                summary: status.summary,
                diagnostics: status.diagnostics,
                trigger_completions: status.trigger_completions,
                r#final: status.r#final,
            })
        } else {
            context.print_data(&contract_submit_only_response(value, None))
        }
    }
}

impl DevViewArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest = load_contract_app_manifest(&self.manifest.manifest)?;
        let contract = dev_contract_by_name(&manifest, &self.contract)?;
        let manifest_dir = self
            .manifest
            .manifest
            .parent()
            .unwrap_or_else(|| Path::new("."));
        validate_dev_payload_for_contract(
            &self.manifest.manifest,
            &self.manifest.profile,
            contract,
            &self.entrypoint,
            self.payload.payload_json.as_deref(),
            self.payload.payload_file.as_deref(),
        )?;
        let profile_config =
            load_dev_profile_config(manifest_dir, manifest.profiles.get(&self.manifest.profile))?;
        let client = dev_client_from_profile(context, profile_config.as_ref());
        let authority = resolve_dev_contract_authority(
            context,
            profile_config.as_ref(),
            self.authority.as_deref(),
        )?;
        let contract_alias = resolve_contract_manifest_alias(
            &contract.alias,
            manifest.default_dataspace.as_deref(),
        )?;
        let payload = load_contract_payload_value(
            self.payload.payload_json.as_deref(),
            self.payload.payload_file.as_deref(),
        )?;
        let gas_limit = self.gas_limit.unwrap_or_else(|| {
            dev_profile_default_gas_limit(manifest.profiles.get(&self.manifest.profile))
        });
        let value = client.post_contract_view_json(
            &authority,
            None,
            Some(&contract_alias),
            &self.entrypoint,
            payload.as_ref(),
            gas_limit,
        )?;
        context.print_data(&value)
    }
}

impl DevSmokeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let _ = dev_build_manifest(&self.manifest.manifest, &self.manifest.profile, true)?;
        let manifest = load_contract_app_manifest(&self.manifest.manifest)?;
        let manifest_dir = self
            .manifest
            .manifest
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let profile = manifest.profiles.get(&self.manifest.profile);
        let profile_config = load_dev_profile_config(manifest_dir, profile)?;
        let client = dev_client_from_profile(context, profile_config.as_ref());
        let authority = resolve_dev_contract_authority(
            context,
            profile_config.as_ref(),
            self.authority.as_deref(),
        )?;
        let private_key = resolve_dev_contract_private_key(
            context,
            profile_config.as_ref(),
            &authority,
            self.private_key.as_deref(),
            false,
        )
        .ok()
        .flatten();
        let cases = prepare_dev_smoke_cases(&self.manifest.manifest, &self.manifest.profile)?;
        let mut smoke_results = Vec::with_capacity(cases.len());
        for case in cases {
            let response = match case.mode {
                DevSmokeMode::View => client.post_contract_view_json(
                    &authority,
                    None,
                    Some(&case.contract_alias),
                    &case.entrypoint,
                    case.payload.as_ref(),
                    case.gas_limit,
                )?,
                DevSmokeMode::Call => {
                    let private_key = private_key.as_ref().ok_or_else(|| {
                        eyre!(
                            "smoke `{}` is a call scenario; provide --private-key or use a matching profile client config",
                            case.id
                        )
                    })?;
                    let submit = client.post_contract_call_json(
                        &authority,
                        Some(private_key),
                        None,
                        Some(&case.contract_alias),
                        &case.entrypoint,
                        case.payload.as_ref(),
                        None,
                        None,
                        None,
                        case.gas_limit,
                    )?;
                    if self.wait.is_enabled() {
                        let tx_hash = extract_submitted_transaction_hash(&submit)
                            .wrap_err("contract call response missing canonical `tx_hash_hex`")?;
                        let status = wait_for_transaction_status(&client, tx_hash, &self.wait)?;
                        norito::json!({
                            "submit": (submit),
                            "terminal_kind": (status.terminal_kind),
                            "attempts": (status.attempts),
                            "elapsed_ms": (status.elapsed_ms),
                            "summary": (status.summary),
                            "diagnostics": (status.diagnostics),
                            "trigger_completions": (status.trigger_completions),
                            "final": (status.r#final),
                        })
                    } else {
                        contract_submit_only_response(submit, None)
                    }
                }
            };
            let actual_result = response
                .get("result")
                .cloned()
                .unwrap_or(norito::json::Value::Null);
            if let Some(expected) = &case.expected_result {
                if &actual_result != expected {
                    return Err(eyre!(
                        "smoke `{}` result mismatch: expected {}, got {}",
                        case.id,
                        norito::json::to_json(expected)?,
                        norito::json::to_json(&actual_result)?
                    ));
                }
            }
            smoke_results.push(norito::json!({
                "id": (case.id),
                "mode": (case.mode.as_str()),
                "contract": (case.contract),
                "contract_alias": (case.contract_alias),
                "entrypoint": (case.entrypoint),
                "gas_limit": (case.gas_limit),
                "result": (actual_result),
                "response": (response),
            }));
        }
        context.print_data(&norito::json!({
            "ok": true,
            "profile": (self.manifest.profile),
            "smoke_count": (smoke_results.len() as u64),
            "smoke": (smoke_results),
        }))
    }
}

fn dev_contract_by_name<'a>(
    manifest: &'a ContractAppManifest,
    name: &str,
) -> Result<&'a ContractAppManifestContract> {
    manifest
        .contracts
        .iter()
        .find(|contract| contract.name == name)
        .ok_or_else(|| eyre!("contract `{name}` is not declared in manifest"))
}

fn validate_dev_payload_for_contract(
    manifest_path: &Path,
    profile: &str,
    contract: &ContractAppManifestContract,
    entrypoint: &str,
    payload_json: Option<&str>,
    payload_file: Option<&Path>,
) -> Result<()> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let artifact_path = dev_contract_publish_layout(manifest_path, profile, contract)?.artifact;
    let payload = load_contract_payload_value(payload_json, payload_file)?;
    validate_dev_payload_value_for_contract(
        manifest_dir,
        contract,
        &artifact_path,
        entrypoint,
        payload.as_ref(),
    )
}

fn dev_contract_publish_layout(
    manifest_path: &Path,
    profile: &str,
    contract: &ContractAppManifestContract,
) -> Result<KotodamaPublishLayout> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(artifact) = &contract.artifact {
        let artifact = resolve_manifest_path(manifest_dir, artifact);
        let manifest = dev_sidecar_path(&artifact, ".manifest.json");
        let interface = dev_sidecar_path(&artifact, ".interface.json");
        return KotodamaPublishLayout::for_artifact(artifact, Some(manifest), Some(interface))
            .map_err(|error| eyre!(error));
    }
    KotodamaPublishLayout::standard(
        manifest_dir.join("target/kotodama"),
        profile,
        &contract.name,
        true,
    )
    .map_err(|error| eyre!(error))
}

fn validate_dev_payload_value_for_contract(
    manifest_dir: &Path,
    contract: &ContractAppManifestContract,
    artifact_path: &Path,
    entrypoint: &str,
    payload: Option<&norito::json::Value>,
) -> Result<()> {
    let _ = manifest_dir;
    if !artifact_path.is_file() {
        return Ok(());
    }
    let artifact = verify_contract_from_bytes(&fs::read(&artifact_path)?)?;
    let entrypoint_name = entrypoint;
    let descriptor = artifact
        .contract_interface
        .entrypoints
        .iter()
        .find(|entry| entry.name == entrypoint_name)
        .ok_or_else(|| {
            eyre!(
                "contract `{}` artifact does not declare entrypoint `{entrypoint_name}`",
                contract.name
            )
        })?;
    let _ = normalize_local_contract_payload(descriptor, payload)?;
    Ok(())
}

fn dev_profile_default_gas_limit(profile: Option<&ContractDevManifestProfile>) -> u64 {
    profile
        .and_then(|profile| profile.default_gas_limit)
        .unwrap_or(DEFAULT_CONTRACT_GAS_LIMIT)
}

fn load_dev_profile_config(
    manifest_dir: &Path,
    profile: Option<&ContractDevManifestProfile>,
) -> Result<Option<Config>> {
    let Some(client_config) = profile.and_then(|profile| profile.client_config.as_ref()) else {
        return Ok(None);
    };
    let path = resolve_manifest_path(manifest_dir, client_config);
    Config::load(LoadPath::Explicit(path.clone()))
        .map(Some)
        .map_err(|report| {
            eyre!(
                "failed to load profile client config `{}`: {report}",
                path.display()
            )
        })
}

fn dev_client_from_profile<C: RunContext>(context: &C, profile_config: Option<&Config>) -> Client {
    profile_config
        .cloned()
        .map(Client::new)
        .unwrap_or_else(|| context.client_from_config())
}

fn resolve_dev_contract_authority<C: RunContext>(
    context: &mut C,
    profile_config: Option<&Config>,
    authority: Option<&str>,
) -> Result<AccountId> {
    match authority {
        Some(authority) => {
            crate::resolve_account_id(context, authority).wrap_err("failed to resolve --authority")
        }
        None => profile_config
            .map(|config| config.account.clone())
            .or_else(|| Some(context.config().account.clone()))
            .ok_or_else(|| eyre!("failed to resolve default authority")),
    }
}

fn resolve_dev_contract_private_key<C: RunContext>(
    context: &C,
    profile_config: Option<&Config>,
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
    if let Some(config) = profile_config
        && authority == &config.account
    {
        return Ok(Some(config.key_pair.private_key().clone()));
    }
    resolve_contract_call_private_key(context, authority, None, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevSmokeMode {
    View,
    Call,
}

impl DevSmokeMode {
    fn parse(raw: Option<&str>, id: &str) -> Result<Self> {
        match raw.unwrap_or("view") {
            "view" => Ok(Self::View),
            "call" => Ok(Self::Call),
            other => Err(eyre!(
                "smoke `{id}` has unsupported mode `{other}`; expected `view` or `call`"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Call => "call",
        }
    }
}

#[derive(Debug)]
struct PreparedDevSmoke {
    id: String,
    contract: String,
    contract_alias: iroha::data_model::smart_contract::ContractAlias,
    mode: DevSmokeMode,
    entrypoint: String,
    payload: Option<norito::json::Value>,
    expected_result: Option<norito::json::Value>,
    gas_limit: u64,
}

fn prepare_dev_smoke_cases(
    manifest_path: &Path,
    profile_name: &str,
) -> Result<Vec<PreparedDevSmoke>> {
    let manifest = load_contract_app_manifest(manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let profile = manifest.profiles.get(profile_name);
    let profile_gas_limit = dev_profile_default_gas_limit(profile);
    let mut cases = Vec::with_capacity(manifest.smoke.len());
    for smoke in &manifest.smoke {
        let contract = dev_contract_by_name(&manifest, &smoke.contract)?;
        let artifact_path =
            dev_contract_publish_layout(manifest_path, profile_name, contract)?.artifact;
        let payload = smoke.payload.clone().map(toml_to_json_value).transpose()?;
        validate_dev_payload_value_for_contract(
            manifest_dir,
            contract,
            &artifact_path,
            &smoke.entrypoint,
            payload.as_ref(),
        )
        .wrap_err_with(|| format!("invalid payload for smoke `{}`", smoke.id))?;
        cases.push(PreparedDevSmoke {
            id: smoke.id.clone(),
            contract: smoke.contract.clone(),
            contract_alias: resolve_contract_manifest_alias(
                &contract.alias,
                manifest.default_dataspace.as_deref(),
            )?,
            mode: DevSmokeMode::parse(smoke.mode.as_deref(), &smoke.id)?,
            entrypoint: smoke.entrypoint.clone(),
            payload,
            expected_result: smoke
                .expected_result
                .clone()
                .map(toml_to_json_value)
                .transpose()?,
            gas_limit: smoke.gas_limit.unwrap_or(profile_gas_limit),
        });
    }
    Ok(cases)
}

fn dev_build_manifest(
    manifest_path: &Path,
    profile: &str,
    locked: bool,
) -> Result<norito::json::Value> {
    let manifest = load_contract_app_manifest(manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut requests = Vec::new();
    let mut sources = Vec::new();
    for contract in &manifest.contracts {
        let Some(source) = &contract.source else {
            continue;
        };
        let source_path = resolve_manifest_path(manifest_dir, source);
        let source_text = read_kotodama_source(&source_path).map_err(|error| eyre!(error))?;
        let source_name = source_path.display().to_string();
        let layout = dev_contract_publish_layout(manifest_path, profile, contract)?;
        requests.push(KotodamaSourceBuildRequest {
            source: source_text,
            source_name,
            profile: profile.to_owned(),
            layout,
            mode: if locked {
                KotodamaPublishMode::Verify
            } else {
                KotodamaPublishMode::Write
            },
        });
        sources.push((contract, source_path));
    }
    let driver = KotodamaBuildDriver::for_current_executable(
        ivm::kotodama::session::CompilerSession::default(),
    )
    .map_err(|error| eyre!(error))?;
    let outcomes = driver
        .build_source_batch(requests)
        .map_err(|error| eyre!(error))?;
    let mut contracts = Vec::with_capacity(outcomes.len());
    for ((contract, source_path), outcome) in sources.into_iter().zip(outcomes) {
        let contract_manifest = outcome.manifest;
        let interface_out = outcome
            .paths
            .interface
            .as_ref()
            .expect("developer builds always request an interface");
        contracts.push(norito::json!({
            "name": (contract.name.clone()),
            "source": (source_path.display().to_string()),
            "artifact": (outcome.paths.artifact.display().to_string()),
            "manifest": (outcome.paths.manifest.display().to_string()),
            "interface": (interface_out.display().to_string()),
            "source_map": (outcome.paths.source_map.display().to_string()),
            "budget": (outcome.paths.budget.display().to_string()),
            "record": (outcome.paths.record.display().to_string()),
            "status": (match outcome.status { KotodamaBuildStatus::Fresh => "fresh", KotodamaBuildStatus::Built => "built" }),
            "entrypoint_count": (contract_manifest.entrypoints.as_ref().map_or(0_u64, |entries| entries.len() as u64)),
            "state_count": (contract_manifest.states.as_ref().map_or(0_u64, |states| states.len() as u64)),
        }));
    }

    Ok(norito::json!({
        "ok": true,
        "manifest": (manifest_path.display().to_string()),
        "profile": (profile),
        "contract_count": (contracts.len() as u64),
        "contracts": (contracts),
    }))
}

fn dev_run_lints(manifest_path: &Path) -> Result<norito::json::Value> {
    let manifest = load_contract_app_manifest(manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let session = ivm::kotodama::session::CompilerSession::default();
    let mut checked = 0_u64;
    let mut diagnostic_count = 0_u64;
    let mut diagnostics = Vec::new();
    for contract in &manifest.contracts {
        if let Some(source) = &contract.source {
            let source_path = resolve_manifest_path(manifest_dir, source);
            let source_text = read_kotodama_source(&source_path).map_err(|error| eyre!(error))?;
            let warnings = match session.check_with_lints(ivm::kotodama::session::CompileRequest {
                source: &source_text,
                source_name: source_path.to_str(),
            }) {
                Ok(warnings) => warnings,
                Err(bundle) => {
                    diagnostic_count = diagnostic_count.saturating_add(
                        u64::try_from(bundle.diagnostics.len()).unwrap_or(u64::MAX),
                    );
                    diagnostics.push(norito::json!({
                        "source": (source_path.display().to_string()),
                        "kind": "compile",
                        "diagnostics": (norito::json::Value::Array(bundle.diagnostics.iter().map(ivm::kotodama::diagnostic::Diagnostic::to_json_value).collect())),
                    }));
                    checked += 1;
                    continue;
                }
            };
            if !warnings.is_empty() {
                diagnostic_count = diagnostic_count
                    .saturating_add(u64::try_from(warnings.len()).unwrap_or(u64::MAX));
                let language = ivm::kotodama::i18n::detect_language();
                let warnings = warnings
                    .into_iter()
                    .map(|warning| {
                        let (line, column) = warning
                            .source
                            .as_ref()
                            .map_or((1, 1), |span| (span.line.max(1), span.column.max(1)));
                        let position = ivm::kotodama::diagnostic::SourcePosition { line, column };
                        let mut diagnostic = ivm::kotodama::diagnostic::Diagnostic::warning(
                            warning.diagnostic_code(),
                            ivm::kotodama::diagnostic::DiagnosticPhase::Semantic,
                            warning.localized_message(language),
                            Some(ivm::kotodama::diagnostic::SourceSpan {
                                source: Some(source_path.display().to_string()),
                                start: position,
                                end: position,
                                byte_range: None,
                            }),
                        );
                        diagnostic.notes.push(format!(
                            "lint `{}` in category `{}`",
                            warning.code,
                            warning.category.as_str()
                        ));
                        diagnostic.to_json_value()
                    })
                    .collect::<Vec<_>>();
                diagnostics.push(norito::json!({
                    "source": (source_path.display().to_string()),
                    "kind": "lint",
                    "diagnostics": (warnings),
                }));
            }
            checked += 1;
        }
    }
    let ok = diagnostics.is_empty();
    Ok(norito::json!({
        "ok": (ok),
        "checked": (checked),
        "failed_source_count": (diagnostics.len() as u64),
        "diagnostic_count": (diagnostic_count),
        "diagnostics": (diagnostics),
    }))
}

fn dev_run_tests(
    manifest_path: &Path,
    path_filter: Option<&str>,
    name_filter: Option<&str>,
    exact: bool,
    coverage: bool,
    profile: bool,
    format: &str,
) -> Result<norito::json::Value> {
    let manifest = load_contract_app_manifest(manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mode = if profile {
        "profile"
    } else if coverage {
        "coverage"
    } else {
        "run"
    };
    let mut test_paths = manifest
        .tests
        .iter()
        .map(|test| resolve_manifest_path(manifest_dir, &test.path))
        .collect::<Vec<_>>();
    if test_paths.is_empty() {
        let tests_dir = manifest_dir.join("tests").join("kotodama");
        if tests_dir.is_dir() {
            for entry in fs::read_dir(tests_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "ko") {
                    test_paths.push(path);
                }
            }
        }
    }
    test_paths.sort();
    let invocations = prepare_dev_test_invocations(
        test_paths,
        path_filter,
        name_filter,
        exact,
        mode,
        format,
        |path| {
            ivm::koto_test_driver::discover_test_names(path)
                .map_err(|error| eyre!("failed to discover tests in `{}`: {error}", path.display()))
        },
    )?;
    let mut executed = Vec::new();
    for (rendered, args) in invocations {
        ivm::koto_test_driver::run_cli(args)
            .map_err(|error| eyre!("Kotodama tests in `{rendered}` failed: {error}"))?;
        executed.push(rendered);
    }
    Ok(norito::json!({
        "ok": true,
        "mode": (mode),
        "executed_count": (executed.len() as u64),
        "executed": (executed),
    }))
}

fn prepare_dev_test_invocations<F>(
    mut test_paths: Vec<PathBuf>,
    path_filter: Option<&str>,
    name_filter: Option<&str>,
    exact: bool,
    mode: &str,
    format: &str,
    mut discover_names: F,
) -> Result<Vec<(String, Vec<String>)>>
where
    F: FnMut(&Path) -> Result<Vec<String>>,
{
    if path_filter.is_some_and(str::is_empty) {
        return Err(eyre!("--path-filter must not be empty"));
    }
    if name_filter.is_some_and(str::is_empty) {
        return Err(eyre!("--filter must not be empty"));
    }
    if exact && name_filter.is_none() {
        return Err(eyre!("--exact requires --filter"));
    }
    match format {
        "text" | "human" => {}
        "json" | "junit" if mode == "run" => {}
        "json" | "junit" => {
            return Err(eyre!(
                "{format} test output is available only in normal run mode"
            ));
        }
        other => return Err(eyre!("unsupported contract test format `{other}`")),
    }

    test_paths.sort();
    if let Some(needle) = path_filter {
        test_paths.retain(|path| path.to_string_lossy().contains(needle));
        if test_paths.is_empty() {
            return Err(eyre!(
                "no Kotodama test source path matched --path-filter `{needle}`"
            ));
        }
    }

    if let Some(needle) = name_filter {
        let mut matched_paths = Vec::new();
        for path in test_paths {
            let names = discover_names(&path)?;
            let matches = names.iter().any(|name| {
                if exact {
                    name == needle
                } else {
                    name.contains(needle)
                }
            });
            if matches {
                matched_paths.push(path);
            }
        }
        test_paths = matched_paths;
        if test_paths.is_empty() {
            return Err(eyre!(
                "no Kotodama test function matched --filter `{needle}`{}",
                if exact { " with --exact" } else { "" }
            ));
        }
    }

    Ok(test_paths
        .into_iter()
        .map(|path| {
            let rendered = path.display().to_string();
            let mut args = vec![mode.to_owned()];
            if let Some(needle) = name_filter {
                args.push("--filter".to_owned());
                args.push(needle.to_owned());
            }
            if exact {
                args.push("--exact".to_owned());
            }
            if matches!(format, "json" | "junit") {
                args.push("--format".to_owned());
                args.push(format.to_owned());
            }
            args.push(rendered.clone());
            (rendered, args)
        })
        .collect())
}

fn dev_sidecar_path(artifact: &Path, suffix: &str) -> PathBuf {
    let stem = artifact
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("contract");
    artifact
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}{suffix}"))
}

fn render_dev_schema_markdown(
    manifest_path: &Path,
    report: &norito::json::Value,
) -> Result<String> {
    let mut out = String::new();
    out.push_str("# Contract Interface Schema\n\n");
    out.push_str(&format!("Manifest: `{}`\n\n", manifest_path.display()));
    let contracts = report
        .get("contracts")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("build report missing contracts"))?;
    for contract in contracts {
        let name = contract
            .get("name")
            .and_then(norito::json::Value::as_str)
            .unwrap_or("");
        let interface = contract
            .get("interface")
            .and_then(norito::json::Value::as_str)
            .unwrap_or("");
        out.push_str(&format!("## {name}\n\n"));
        out.push_str(&format!("- Interface: `{interface}`\n"));
        out.push_str(&format!(
            "- Entrypoints: `{}`\n",
            contract
                .get("entrypoint_count")
                .and_then(norito::json::Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "0".to_owned())
        ));
        out.push_str(&format!(
            "- State keys: `{}`\n\n",
            contract
                .get("state_count")
                .and_then(norito::json::Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "0".to_owned())
        ));
        if interface.is_empty() {
            continue;
        }
        let interface_path = Path::new(interface);
        let interface_text = fs::read_to_string(interface_path)
            .wrap_err_with(|| format!("failed to read {}", interface_path.display()))?;
        let interface_json: norito::json::Value = norito::json::from_str(&interface_text)
            .wrap_err_with(|| format!("failed to parse {}", interface_path.display()))?;
        let Some(entrypoints) = interface_json
            .get("entrypoints")
            .and_then(norito::json::Value::as_array)
        else {
            continue;
        };
        for entrypoint in entrypoints {
            let Some(entrypoint_name) =
                entrypoint.get("name").and_then(norito::json::Value::as_str)
            else {
                continue;
            };
            let raw_kind = entrypoint
                .get("kind")
                .and_then(|kind| kind.get("kind"))
                .and_then(norito::json::Value::as_str)
                .unwrap_or("Unknown");
            let kind = match raw_kind {
                "Kotoage" => "kotoage",
                "View" => "view",
                "Hajimari" => "hajimari",
                "Kaizen" => "kaizen",
                other => other,
            };
            let return_type = entrypoint
                .get("return_type")
                .and_then(norito::json::Value::as_str)
                .unwrap_or("null");
            let sample_payload = dev_sample_payload_json(entrypoint)?;
            out.push_str(&format!("### {entrypoint_name}\n\n"));
            out.push_str(&format!("- Kind: `{kind}`\n"));
            out.push_str(&format!("- Return: `{return_type}`\n"));
            out.push_str("- Sample payload:\n\n");
            out.push_str("```json\n");
            out.push_str(&sample_payload);
            out.push_str("\n```\n\n");
        }
    }
    Ok(out)
}

fn dev_sample_payload_json(entrypoint: &norito::json::Value) -> Result<String> {
    let mut payload = std::collections::BTreeMap::new();
    if let Some(params) = entrypoint
        .get("params")
        .and_then(norito::json::Value::as_array)
    {
        for param in params {
            let Some(name) = param.get("name").and_then(norito::json::Value::as_str) else {
                continue;
            };
            let type_name = param
                .get("type_name")
                .and_then(norito::json::Value::as_str)
                .unwrap_or("Json");
            payload.insert(name.to_owned(), dev_sample_value_for_type(type_name));
        }
    }
    norito::json::to_json_pretty(&norito::json::Value::Object(payload)).map_err(Into::into)
}

fn dev_sample_value_for_type(type_name: &str) -> norito::json::Value {
    match type_name {
        "int" | "decimal" | "quantity" => norito::json!("0"),
        "bool" => norito::json!(false),
        "string" => norito::json!(""),
        "bytes" => norito::json!("0x"),
        "Json" => norito::json!({}),
        "AccountId" => norito::json!("ed0120..."),
        "AssetDefinitionId" => norito::json!("xor#universal"),
        "AssetId" => norito::json!("xor#universal:ed0120..."),
        "DataSpaceId" => norito::json!(0_u64),
        "DomainId" => norito::json!("soraswap.universal"),
        "Name" => norito::json!("sample"),
        "NftId" => norito::json!("nft#soraswap.universal"),
        _ => norito::json!("sample"),
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
    pub contract_alias: Option<String>,
    /// Contract alias name segment. Prefer this with --dataspace over precomposed --contract-alias.
    #[arg(long, conflicts_with = "contract_alias")]
    pub alias: Option<String>,
    /// Contract alias domain segment.
    #[arg(long, conflicts_with = "contract_alias")]
    pub domain: Option<String>,
    /// Contract alias dataspace segment.
    #[arg(long, conflicts_with = "contract_alias")]
    pub dataspace: Option<String>,
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
        let contract_alias = resolve_deploy_contract_alias(
            self.contract_alias.as_deref(),
            self.alias.as_deref(),
            self.domain.as_deref(),
            self.dataspace.as_deref(),
        )?;
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
                trace: None,
                terminal_kind: status.terminal_kind,
                attempts: status.attempts,
                elapsed_ms: status.elapsed_ms,
                block_height: status.block_height,
                rejection_reason: status.rejection_reason,
                scope: status.scope,
                resolved_from: status.resolved_from,
                summary: status.summary,
                diagnostics: status.diagnostics,
                trigger_completions: status.trigger_completions,
                r#final: status.r#final,
            })?;
        } else {
            context.print_data(&contract_submit_only_response(v, None))?;
        }
        Ok(())
    }
}

fn resolve_deploy_contract_alias(
    contract_alias: Option<&str>,
    alias: Option<&str>,
    domain: Option<&str>,
    dataspace: Option<&str>,
) -> Result<iroha::data_model::smart_contract::ContractAlias> {
    if let Some(contract_alias) = contract_alias
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if alias.is_some() || domain.is_some() || dataspace.is_some() {
            return Err(eyre!(
                "use either --contract-alias or the explicit --alias/--domain/--dataspace fields"
            ));
        }
        return contract_alias.parse().wrap_err("invalid --contract-alias");
    }

    let alias = alias
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| eyre!("provide --alias and --dataspace, or pass --contract-alias"))?;
    let dataspace = dataspace
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| eyre!("provide --dataspace with --alias"))?;
    let domain = domain.map(str::trim).filter(|value| !value.is_empty());
    iroha::data_model::smart_contract::ContractAlias::from_components(alias, domain, dataspace)
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err("invalid contract alias fields")
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
    /// Public network profile used for Bech32m contract-address derivation
    #[arg(long)]
    pub profile: Option<String>,
    /// Explicit chain discriminant used for Bech32m contract-address derivation
    #[arg(long)]
    pub chain_discriminant: Option<u16>,
    /// Optional numeric dataspace id override for non-default dataspaces
    #[arg(long)]
    pub dataspace_id: Option<u64>,
}

impl Run for DeriveAddressArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (profile_name, chain_discriminant) =
            resolve_network_context(self.profile.as_deref(), self.chain_discriminant)?;
        let authority = parse_account_address(&self.authority, Some(chain_discriminant))
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to resolve --authority")?
            .address
            .to_account_id()
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to decode --authority")?;
        let dataspace_id = resolve_contract_dataspace_id_hint(&self.dataspace, self.dataspace_id)?;
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            chain_discriminant,
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
            "profile": (profile_name),
            "chain_discriminant": (chain_discriminant),
            "contract_address": (contract_address),
        }))?;
        Ok(())
    }
}

fn resolve_network_context(
    profile: Option<&str>,
    chain_discriminant: Option<u16>,
) -> Result<(Option<String>, u16)> {
    match (
        profile.map(str::trim).filter(|value| !value.is_empty()),
        chain_discriminant,
    ) {
        (Some(profile_name), Some(actual)) => {
            let expected = iroha_torii_shared::network_profile(profile_name).ok_or_else(|| {
                eyre!(
                    "unknown network profile `{profile_name}` (supported: {})",
                    iroha_torii_shared::network_profile_names()
                )
            })?;
            if expected.chain_discriminant != actual {
                eyre::bail!(
                    "network profile mismatch: profile `{}` expects chain_discriminant={}, actual chain_discriminant={}",
                    expected.name,
                    expected.chain_discriminant,
                    actual
                );
            }
            Ok((Some(expected.name.to_owned()), actual))
        }
        (Some(profile_name), None) => {
            let expected = iroha_torii_shared::network_profile(profile_name).ok_or_else(|| {
                eyre!(
                    "unknown network profile `{profile_name}` (supported: {})",
                    iroha_torii_shared::network_profile_names()
                )
            })?;
            Ok((Some(expected.name.to_owned()), expected.chain_discriminant))
        }
        (None, Some(chain_discriminant)) => Ok((None, chain_discriminant)),
        (None, None) => eyre::bail!("provide --profile or --chain-discriminant"),
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
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    trace: Option<norito::json::Value>,
    terminal_kind: String,
    attempts: u64,
    elapsed_ms: u64,
    block_height: Option<u64>,
    rejection_reason: Option<iroha::data_model::transaction::error::TransactionRejectionReason>,
    scope: String,
    resolved_from: String,
    summary: String,
    diagnostics: Vec<iroha_torii_shared::PipelineDiagnostic>,
    trigger_completions: Vec<iroha_torii_shared::TriggerCompletionSummary>,
    r#final: iroha_torii_shared::PipelineTransactionStatusResponse,
}

fn contract_submit_only_response(
    submit: norito::json::Value,
    trace: Option<norito::json::Value>,
) -> norito::json::Value {
    if let Some(trace) = trace {
        norito::json!({
            "submit": submit,
            "trace": trace,
            "finalized": false,
        })
    } else {
        norito::json!({
            "submit": submit,
            "finalized": false,
        })
    }
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
    /// Run Torii simulation first and include the server-side execution trace in the submit response.
    #[arg(long, conflicts_with = "simulate")]
    pub trace: bool,
    /// Contract entrypoint selector.
    #[arg(long)]
    pub entrypoint: String,
    /// Optional gas asset id forwarded to transaction metadata.
    #[arg(long)]
    pub gas_asset_id: Option<String>,
    /// Optional fee sponsor account charged for gas/fees when supported.
    #[arg(long)]
    pub fee_sponsor: Option<String>,
    /// Gas limit metadata forwarded to the contract call.
    #[arg(long, default_value_t = DEFAULT_CONTRACT_GAS_LIMIT)]
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
                &self.entrypoint,
                payload.as_ref(),
                self.gas_asset_id.as_deref(),
                fee_sponsor.as_ref(),
                self.gas_limit,
            )?;
            context.print_data(&value)?;
            return Ok(());
        }
        let trace = if self.trace {
            Some(client.post_contract_call_simulate_json(
                &authority,
                target.contract_address.as_ref(),
                target.contract_alias.as_ref(),
                &self.entrypoint,
                payload.as_ref(),
                self.gas_asset_id.as_deref(),
                fee_sponsor.as_ref(),
                self.gas_limit,
            )?)
        } else {
            None
        };
        let value = client.post_contract_call_json(
            &authority,
            private_key.as_ref(),
            target.contract_address.as_ref(),
            target.contract_alias.as_ref(),
            &self.entrypoint,
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
                trace,
                terminal_kind: status.terminal_kind,
                attempts: status.attempts,
                elapsed_ms: status.elapsed_ms,
                block_height: status.block_height,
                rejection_reason: status.rejection_reason,
                scope: status.scope,
                resolved_from: status.resolved_from,
                summary: status.summary,
                diagnostics: status.diagnostics,
                trigger_completions: status.trigger_completions,
                r#final: status.r#final,
            })?;
        } else {
            context.print_data(&contract_submit_only_response(value, trace))?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ViewArgs {
    /// Authority account identifier used as the read context. Defaults to the configured client authority.
    #[arg(long)]
    pub authority: Option<String>,
    /// Contract view entrypoint selector.
    #[arg(long)]
    pub entrypoint: String,
    /// Gas limit applied to the local view execution.
    #[arg(long, default_value_t = DEFAULT_CONTRACT_GAS_LIMIT)]
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
            &self.entrypoint,
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
    /// Contract view entrypoint selector.
    #[arg(long)]
    pub entrypoint: String,
    /// Gas limit applied to the local view execution.
    #[arg(long, default_value_t = DEFAULT_CONTRACT_GAS_LIMIT)]
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
    /// Contract entrypoint selector.
    #[arg(long)]
    pub entrypoint: String,
    /// Gas limit applied to the local call execution.
    #[arg(long, default_value_t = DEFAULT_CONTRACT_GAS_LIMIT)]
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
            manifest = manifest
                .try_signed(&kp)
                .wrap_err("sign contract manifest failed")?;
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

fn resolve_contract_target(args: ContractTargetArgs) -> Result<ResolvedContractTarget> {
    match (
        args.contract_address.as_deref(),
        args.contract_alias.as_deref(),
    ) {
        (Some(address), None) => Ok(ResolvedContractTarget {
            contract_address: Some(
                address
                    .parse()
                    .wrap_err("invalid --contract-address canonical literal")?,
            ),
            contract_alias: None,
        }),
        (None, Some(alias)) => Ok(ResolvedContractTarget {
            contract_address: None,
            contract_alias: Some(alias.parse().wrap_err("invalid --contract-alias")?),
        }),
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
    match (
        args.contract_address.as_deref(),
        args.contract_alias.as_deref(),
    ) {
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(eyre!(
            "provide exactly one contract target via --contract-address or --contract-alias"
        )),
        (Some(contract_address), None) => {
            Ok(Some(contract_address.parse().wrap_err(
                "invalid --contract-address canonical literal",
            )?))
        }
        (None, Some(contract_alias_raw)) => {
            let contract_alias: iroha::data_model::smart_contract::ContractAlias =
                contract_alias_raw
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
                    let value: norito::json::Value = norito::json::from_slice(&body)
                        .wrap_err("decode contract alias response")?;
                    let resolved = value
                        .get("contract_address")
                        .and_then(norito::json::Value::as_str)
                        .ok_or_else(|| {
                            eyre!("contract alias response missing `contract_address`")
                        })?;
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
    let summary = ProgramSummary::from_artifact(bytes)
        .map_err(|err| eyre!("failed to prepare IVM program summary: {err}"))?;
    match summary.metadata.abi_version {
        1 => {}
        v => {
            return Err(eyre!(
                "unsupported abi_version {v}; expected 1 for the first release"
            ));
        }
    }
    Ok(summary)
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
    fn prepare_syscall(&self, number: u32, vm: &ivm::IVM) -> Result<u64, ivm::VMError> {
        self.inner.prepare_syscall(number, vm)
    }

    fn syscall(&mut self, number: u32, vm: &mut ivm::IVM) -> Result<u64, ivm::VMError> {
        let record = LocalContractSyscallTrace {
            pc: vm.pc(),
            syscall: number,
            gas_remaining_at_call: vm.remaining_gas(),
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
    Decimal,
    Quantity,
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

fn prepare_local_contract_arguments(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    payload: Option<&Json>,
    gas_limit: u64,
) -> Result<Option<ivm::PreparedArgumentRecord>> {
    match (descriptor.argument_schema.as_ref(), payload) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(eyre!(
            "zero-parameter entrypoint `{}` must not receive a payload",
            descriptor.name
        )),
        (Some(_), None) => Err(eyre!(
            "parameterized entrypoint `{}` requires a payload",
            descriptor.name
        )),
        (Some(schema), Some(payload)) => {
            let canonical =
                ivm::encode_argument_record_from_json(schema, payload).map_err(|err| {
                    eyre!(
                        "payload for entrypoint `{}` does not match its argument schema: {err}",
                        descriptor.name
                    )
                })?;
            ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), gas_limit)
                .map(Some)
                .map_err(|err| {
                    eyre!(
                        "failed to prepare arguments for entrypoint `{}`: {err}",
                        descriptor.name
                    )
                })
        }
    }
}

fn execute_local_contract_debug_view<C: RunContext>(
    context: &C,
    args: DebugViewArgs,
    authority: AccountId,
) -> Result<LocalContractDebugViewResponse> {
    let code = load_code_bytes(args.code_file.clone(), args.code_b64.clone())?;
    let verified = verify_contract_from_bytes(&code)?;
    let summary = program_summary_from_bytes(&code)?;
    let selector = args.entrypoint;
    let descriptor = resolve_local_view_entrypoint(&verified, &selector)?;
    let entrypoint_pc = resolve_local_contract_entrypoint_pc(&code, descriptor)?;
    let payload = load_contract_payload_value(
        args.payload.payload_json.as_deref(),
        args.payload.payload_file.as_deref(),
    )?;
    let payload = normalize_local_contract_payload(descriptor, payload.as_ref())?;
    let arguments = prepare_local_contract_arguments(descriptor, payload.as_ref(), args.gas_limit)?;
    let prepared_arguments = arguments.clone();
    let accounts = load_debug_accounts_fixture(
        &authority,
        args.accounts_json.as_deref(),
        args.accounts_file.as_deref(),
    )?;
    let durable_state = load_debug_durable_state_fixture(
        args.durable_state_json.as_deref(),
        args.durable_state_file.as_deref(),
    )?;

    let mut host = if let Some(arguments) = arguments {
        CoreHost::with_accounts_and_argument_record(
            authority,
            Arc::clone(&accounts),
            Some(arguments),
        )
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
    if let Some(arguments) = prepared_arguments.as_ref() {
        arguments
            .precharge_vm(&mut vm)
            .map_err(|err| eyre!("failed to precharge contract debug arguments: {err}"))?;
    }
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

    let result = descriptor.return_schema.as_ref().map_or_else(
        || Ok(norito::json::Value::Null),
        |schema| {
            iroha_core::smartcontracts::ivm::return_value::decode_entrypoint_return(&vm, schema)
                .map_err(|err| eyre!("failed to decode contract debug view return value: {err}"))
        },
    )?;

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
    let selector = args.entrypoint;
    let descriptor = resolve_local_public_entrypoint(&verified, &selector)?;
    let entrypoint_pc = resolve_local_contract_entrypoint_pc(&code, descriptor)?;
    let payload = load_contract_payload_value(
        args.payload.payload_json.as_deref(),
        args.payload.payload_file.as_deref(),
    )?;
    let payload = normalize_local_contract_payload(descriptor, payload.as_ref())?;
    let arguments = prepare_local_contract_arguments(descriptor, payload.as_ref(), args.gas_limit)?;
    let prepared_arguments = arguments.clone();
    let accounts = load_debug_accounts_fixture(
        &authority,
        args.accounts_json.as_deref(),
        args.accounts_file.as_deref(),
    )?;
    let durable_state = load_debug_durable_state_fixture(
        args.durable_state_json.as_deref(),
        args.durable_state_file.as_deref(),
    )?;

    let mut host = if let Some(arguments) = arguments {
        CoreHost::with_accounts_and_argument_record(
            authority,
            Arc::clone(&accounts),
            Some(arguments),
        )
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
    if let Some(arguments) = prepared_arguments.as_ref() {
        arguments
            .precharge_vm(&mut vm)
            .map_err(|err| eyre!("failed to precharge contract debug arguments: {err}"))?;
    }
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

    let result = descriptor
        .return_schema
        .as_ref()
        .map(|schema| {
            iroha_core::smartcontracts::ivm::return_value::decode_entrypoint_return(&vm, schema)
                .map_err(|err| eyre!("failed to decode contract debug call return value: {err}"))
        })
        .transpose()?;

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
        iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
        "kotoage",
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
        "decimal" => Ok(LocalContractSchemaType::Decimal),
        "quantity" => Ok(LocalContractSchemaType::Quantity),
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

fn validate_local_exact_json_string<T>(value: &norito::json::Value) -> bool
where
    T: FromStr + ToString,
{
    let norito::json::Value::String(raw) = value else {
        return false;
    };
    raw.parse::<T>()
        .is_ok_and(|parsed| parsed.to_string() == *raw)
}

fn validate_local_contract_value(
    schema: &LocalContractSchemaType,
    value: &norito::json::Value,
    field_name: &str,
) -> Result<()> {
    let ok = match schema {
        LocalContractSchemaType::Unit => matches!(value, norito::json::Value::Null),
        LocalContractSchemaType::Int => {
            validate_local_exact_json_string::<iroha_primitives::bigint::BigInt>(value)
        }
        LocalContractSchemaType::Decimal => {
            validate_local_exact_json_string::<iroha_primitives::numeric::Numeric>(value)
        }
        LocalContractSchemaType::Quantity => {
            validate_local_exact_json_string::<iroha_primitives::numeric::Quantity>(value)
        }
        LocalContractSchemaType::Bool => matches!(value, norito::json::Value::Bool(_)),
        LocalContractSchemaType::String => matches!(value, norito::json::Value::String(_)),
        LocalContractSchemaType::Json => true,
        LocalContractSchemaType::Name => match value {
            norito::json::Value::String(raw) => Name::from_str(raw).is_ok(),
            _ => false,
        },
        LocalContractSchemaType::AccountId => match value {
            norito::json::Value::String(raw) => AccountId::parse_encoded(raw)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .or_else(|_| {
                    raw.parse::<iroha_data_model::smart_contract::ContractAddress>()
                        .map(|address| address.subject_id())
                })
                .is_ok(),
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
    match (descriptor.argument_schema.as_ref(), payload) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(eyre!(
            "contract payload must be omitted for zero-parameter entrypoints"
        )),
        (Some(_), None) => Err(eyre!(
            "contract payload is required for parameterized entrypoints"
        )),
        (Some(schema), Some(payload)) => {
            let payload = iroha_primitives::json::Json::from(payload.clone());
            ivm::encode_argument_record_from_json(schema, &payload).map_err(|error| {
                eyre!(
                    "contract payload for entrypoint `{}` does not match its exact argument schema: {error}",
                    descriptor.name
                )
            })?;
            Ok(Some(payload))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use iroha_crypto::{Algorithm, ExposedPrivateKey};
    use iroha_i18n::{Bundle, Language, Localizer};
    use ivm::kotodama::session::{CompileRequest, CompilerSession};
    use tempfile::tempdir;
    use url::Url;

    #[test]
    fn default_contract_gas_limit_covers_strict_argument_admission_floor() {
        assert_eq!(dev_profile_default_gas_limit(None), DEFAULT_CONTRACT_GAS_LIMIT);
        assert!(DEFAULT_CONTRACT_GAS_LIMIT > 1_048_752);
    }

    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }

    fn fixture_account(seed: u8) -> AccountId {
        AccountId::new(fixture_key_pair(seed).public_key().clone())
    }

    fn encode_int_state_value(value: i64) -> Vec<u8> {
        use ivm::state_value::{
            StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1,
            StateValueSchemaV1, state_value_schema_hash_v1,
        };

        let schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
        };
        let schema_bytes = norito::to_bytes(&schema).expect("encode state int schema");
        let envelope = ivm::numeric_tlv::encode_int(
            &iroha_primitives::bigint::BigInt::from_i128(i128::from(value)),
        )
        .expect("encode canonical state int pointer");
        norito::to_bytes(&StateValueRecordV1 {
            schema_hash: state_value_schema_hash_v1(&schema_bytes),
            atoms: vec![StateValueAtomV1::Pointer(envelope)],
        })
        .expect("encode state int record")
    }

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

    fn embedded_entrypoint(program: &[u8], name: &str) -> ivm::EmbeddedEntrypointDescriptor {
        let parsed = ivm::ProgramMetadata::parse(program).expect("parse contract metadata");
        parsed
            .contract_interface
            .expect("compiled contract interface")
            .entrypoints
            .into_iter()
            .find(|entrypoint| entrypoint.name == name)
            .unwrap_or_else(|| panic!("missing embedded entrypoint `{name}`"))
    }

    fn compile_contract_program_with_source_path(source: &str, source_path: &str) -> Vec<u8> {
        let output = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some(source_path),
            })
            .expect("compile contract with source path");
        output.artifact
    }

    #[test]
    fn resolve_contract_manifest_alias_uses_default_dataspace() {
        let alias = resolve_contract_manifest_alias("router", Some("universal"))
            .expect("resolve contract alias");
        assert_eq!(alias.to_string(), "router::universal");
    }

    #[test]
    fn contract_manifest_rejects_retired_init_table() {
        let value = toml::from_str::<toml::Value>(
            r#"
            bundle_name = "demo"

            [[contracts]]
            name = "demo.greeter"
            alias = "greeter::universal"
            source = "greeter.ko"

            [[init]]
            id = "seed"
            contract = "demo.greeter"
            entrypoint = "hajimari"
            gas_limit = 1000
            "#,
        )
        .expect("parse retired manifest spelling");
        let error = parse_contract_app_manifest(value)
            .expect_err("the English lifecycle table must not be accepted");
        assert!(
            error
                .to_string()
                .contains("unknown contract manifest field `init`"),
            "unexpected error: {error}",
        );
    }

    #[test]
    fn toml_to_json_value_preserves_nested_tables() {
        let value = toml::from_str::<toml::Value>(
            r#"
            retries = 2
            enabled = true
            [nested]
            label = "alpha"
            values = [1, 2]
            "#,
        )
        .expect("parse toml");
        let json = toml_to_json_value(value).expect("convert to json");
        assert_eq!(
            json.get("retries").and_then(norito::json::Value::as_i64),
            Some(2)
        );
        assert_eq!(
            json.get("enabled").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            json.get("nested")
                .and_then(|nested| nested.get("label"))
                .and_then(norito::json::Value::as_str),
            Some("alpha")
        );
        assert_eq!(
            json.get("nested")
                .and_then(|nested| nested.get("values"))
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn extract_submitted_transaction_hash_prefers_top_level_field() {
        let value = norito::json!({
            "tx_hash_hex": "1111111111111111111111111111111111111111111111111111111111111111",
            "contracts": [
                {
                    "tx_hash_hex": "2222222222222222222222222222222222222222222222222222222222222222"
                }
            ]
        });

        let hash = extract_submitted_transaction_hash(&value).expect("extract hash");

        assert_eq!(
            hash.to_string(),
            "1111111111111111111111111111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn extract_submitted_transaction_hash_rejects_nested_contract_receipt() {
        let value = norito::json!({
            "contracts": [
                {
                    "tx_hash_hex": "3333333333333333333333333333333333333333333333333333333333333333"
                }
            ]
        });

        let err = extract_submitted_transaction_hash(&value)
            .expect_err("nested contract receipt hash should not be accepted");

        assert!(err.to_string().contains("response missing `tx_hash_hex`"));
    }

    #[test]
    fn resolve_network_context_accepts_public_profile() {
        let (profile, chain_discriminant) =
            resolve_network_context(Some("taira"), None).expect("resolve profile");

        assert_eq!(profile.as_deref(), Some("taira"));
        assert_eq!(
            chain_discriminant,
            iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT
        );
    }

    #[test]
    fn resolve_network_context_rejects_profile_discriminant_mismatch() {
        let err = resolve_network_context(Some("taira"), Some(753))
            .expect_err("profile mismatch should fail");

        assert!(
            err.to_string()
                .contains("profile `taira` expects chain_discriminant=369")
        );
    }

    #[test]
    fn contract_submit_only_response_marks_unfinalized() {
        let response =
            contract_submit_only_response(norito::json!({ "tx_hash_hex": "deadbeef" }), None);

        assert_eq!(
            response
                .get("finalized")
                .and_then(norito::json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            response
                .get("submit")
                .and_then(|submit| submit.get("tx_hash_hex"))
                .and_then(norito::json::Value::as_str),
            Some("deadbeef")
        );
    }

    #[test]
    fn contract_submit_only_response_keeps_operation_receipt_under_submit() {
        let response = contract_submit_only_response(
            norito::json!({
                "tx_hash_hex": "deadbeef",
                "operation_receipt": {
                    "operation_kind": "contract_call",
                    "status": "submitted",
                    "transport": "torii",
                    "dataspace": "universal",
                    "payload_digest_hex": "payload-digest"
                }
            }),
            None,
        );

        assert!(response.get("operation_receipt").is_none());
        assert!(response.get("tx_hash_hex").is_none());
        assert_eq!(
            response
                .get("submit")
                .and_then(|submit| submit.get("operation_receipt"))
                .and_then(|receipt| receipt.get("operation_kind"))
                .and_then(norito::json::Value::as_str),
            Some("contract_call")
        );
        let submit = response
            .get("submit")
            .and_then(norito::json::Value::as_object)
            .expect("submit object");
        for forbidden_key in [
            "private_key",
            "payload",
            "raw_payload",
            "normalized_payload",
            "transaction_scaffold_b64",
            "signed_transaction_b64",
            "signing_message_b64",
        ] {
            assert!(
                submit.get(forbidden_key).is_none(),
                "CLI submit response must not expose `{forbidden_key}`"
            );
        }
    }

    #[test]
    fn resolve_deploy_contract_alias_composes_explicit_fields() {
        let alias = resolve_deploy_contract_alias(None, Some("router"), None, Some("is"))
            .expect("domainless alias");
        assert_eq!(alias.to_string(), "router::is");

        let domain_alias =
            resolve_deploy_contract_alias(None, Some("router"), Some("finance"), Some("alpha"))
                .expect("domain-scoped alias");
        assert_eq!(domain_alias.to_string(), "router::finance.alpha");
    }

    #[test]
    fn resolve_deploy_contract_alias_trims_explicit_fields_and_blank_domain() {
        let alias = resolve_deploy_contract_alias(None, Some(" router "), Some("  "), Some(" is "))
            .expect("trimmed explicit alias");

        assert_eq!(alias.to_string(), "router::is");
    }

    #[test]
    fn resolve_deploy_contract_alias_preserves_legacy_contract_alias() {
        let alias = resolve_deploy_contract_alias(Some("router::finance.alpha"), None, None, None)
            .expect("legacy alias");

        assert_eq!(alias.to_string(), "router::finance.alpha");
    }

    #[test]
    fn resolve_deploy_contract_alias_rejects_ambiguous_or_partial_inputs() {
        let ambiguous = resolve_deploy_contract_alias(
            Some("router::universal"),
            Some("router"),
            None,
            Some("is"),
        )
        .expect_err("legacy and explicit fields conflict");
        assert!(
            ambiguous
                .to_string()
                .contains("use either --contract-alias"),
            "unexpected error: {ambiguous}"
        );

        let missing_dataspace = resolve_deploy_contract_alias(None, Some("router"), None, None)
            .expect_err("dataspace is required with alias");
        assert!(
            missing_dataspace
                .to_string()
                .contains("provide --dataspace with --alias"),
            "unexpected error: {missing_dataspace}"
        );

        let missing_alias = resolve_deploy_contract_alias(None, None, None, Some("is"))
            .expect_err("alias is required with dataspace");
        assert!(
            missing_alias
                .to_string()
                .contains("provide --alias and --dataspace"),
            "unexpected error: {missing_alias}"
        );
    }

    #[test]
    fn resolve_deploy_contract_alias_rejects_adversarial_components() {
        for (alias, domain, dataspace) in [
            ("", None, "is"),
            ("router::evil", None, "is"),
            ("router", Some("bad domain"), "is"),
            ("router", None, ""),
            ("router", None, "bad dataspace"),
        ] {
            assert!(
                resolve_deploy_contract_alias(None, Some(alias), domain, Some(dataspace)).is_err(),
                "adversarial alias components should fail: alias={alias:?} domain={domain:?} dataspace={dataspace:?}"
            );
        }
    }

    #[test]
    fn build_contract_app_bundle_compiles_manifest_sources() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        let contract_path = contracts_dir.join("greeter.ko");
        fs::write(
            &contract_path,
            r#"
                seiyaku Greeter {
                    hajimari(int value) {}
                    view fn status() -> int { return 7; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.app.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[hajimari]]
                id = "seed"
                contract = "demo.greeter"
                entrypoint = "hajimari"
                gas_limit = 1000
                payload = { value = "7" }
            "#,
        )
        .expect("write manifest");

        let bundle = build_contract_app_bundle(&manifest_path).expect("build bundle");
        assert_eq!(
            bundle
                .get("bundle_name")
                .and_then(norito::json::Value::as_str),
            Some("demo")
        );
        assert_eq!(
            bundle
                .get("contracts")
                .and_then(norito::json::Value::as_array)
                .and_then(|contracts| contracts.first())
                .and_then(|contract| contract.get("contract_alias"))
                .and_then(norito::json::Value::as_str),
            Some("greeter::universal")
        );
        assert_eq!(
            bundle
                .get("hajimari_calls")
                .and_then(norito::json::Value::as_array)
                .and_then(|calls| calls.first())
                .and_then(|call| call.get("contract_alias"))
                .and_then(norito::json::Value::as_str),
            Some("greeter::universal")
        );
        assert!(dir.path().join("artifacts/greeter.to").exists());
    }

    #[test]
    fn build_contract_app_bundle_rejects_non_hajimari_lifecycle_calls() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    kotoage fn run() authorize("Run") {}
                }
            "#,
        )
        .expect("write contract");
        let manifest_path = dir.path().join("iroha.app.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter::universal"
                source = "contracts/greeter.ko"

                [[hajimari]]
                id = "seed"
                contract = "demo.greeter"
                entrypoint = "run"
                gas_limit = 1000
            "#,
        )
        .expect("write manifest");

        let error = build_contract_app_bundle(&manifest_path)
            .expect_err("hajimari tables must not dispatch ordinary kotoage entrypoints");
        assert!(
            error
                .to_string()
                .contains("must target a hajimari/始まり entrypoint"),
            "unexpected error: {error}",
        );
    }

    #[test]
    fn dev_build_manifest_emits_interface_source_map_and_budget_sidecars() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    state int Counter;
                    hajimari(int value) { Counter = value; }
                    view fn status() -> int { return Counter; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [profiles.local]
                client_config = "client.toml"
                default_gas_limit = 500000

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"
            "#,
        )
        .expect("write manifest");

        let report = dev_build_manifest(&manifest_path, "local", false).expect("dev build");
        assert_eq!(
            report
                .get("contract_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert!(artifacts_dir.join("greeter.to").exists());
        assert!(artifacts_dir.join("greeter.manifest.json").exists());
        assert!(artifacts_dir.join("greeter.interface.json").exists());
        let contract = report
            .get("contracts")
            .and_then(norito::json::Value::as_array)
            .and_then(|contracts| contracts.first())
            .expect("one contract report");
        let source_map = contract
            .get("source_map")
            .and_then(norito::json::Value::as_str)
            .expect("source-map report path");
        let budget = contract
            .get("budget")
            .and_then(norito::json::Value::as_str)
            .expect("budget report path");
        assert!(Path::new(source_map).is_file());
        assert!(Path::new(budget).is_file());
        assert!(source_map.contains("/.sidecars/"));
        assert!(budget.contains("/.sidecars/"));
        let generated_paths = [
            "artifact",
            "manifest",
            "interface",
            "source_map",
            "budget",
            "record",
        ]
        .map(|field| {
            PathBuf::from(
                contract
                    .get(field)
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or_else(|| panic!("build report is missing `{field}`")),
            )
        });
        assert!(generated_paths.iter().all(|path| path.is_file()));
        assert!(generated_paths[3].to_string_lossy().contains("/.sidecars/"));
        assert!(generated_paths[4].to_string_lossy().contains("/.sidecars/"));
        assert!(
            generated_paths[5]
                .to_string_lossy()
                .contains("/.fingerprints/")
        );
        let modified = generated_paths.map(|path| {
            let modified = fs::metadata(&path)
                .unwrap_or_else(|error| panic!("metadata for {}: {error}", path.display()))
                .modified()
                .ok();
            (path, modified)
        });
        let fresh = dev_build_manifest(&manifest_path, "local", false).expect("no-op dev build");
        assert_eq!(
            fresh
                .get("contracts")
                .and_then(norito::json::Value::as_array)
                .and_then(|contracts| contracts.first())
                .and_then(|contract| contract.get("status"))
                .and_then(norito::json::Value::as_str),
            Some("fresh")
        );
        for (path, expected_modified) in modified {
            assert_eq!(
                fs::metadata(&path)
                    .unwrap_or_else(|error| {
                        panic!("fresh metadata for {}: {error}", path.display())
                    })
                    .modified()
                    .ok(),
                expected_modified,
                "no-op dev build must not rewrite {}",
                path.display()
            );
        }

        let schema = render_dev_schema_markdown(&manifest_path, &report).expect("schema markdown");
        assert!(schema.contains("demo.greeter"));
        assert!(schema.contains("### hajimari"));
        assert!(schema.contains("- Kind: `hajimari`"));
        assert!(schema.contains("\"value\": \"0\""));
    }

    #[test]
    fn dev_lint_report_preserves_canonical_diagnostic_records() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    view fn status(int unused) -> int { return 7; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter::universal"
                source = "contracts/greeter.ko"
            "#,
        )
        .expect("write manifest");

        let report = dev_run_lints(&manifest_path).expect("lint report");
        assert_eq!(
            report.get("ok").and_then(norito::json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            report
                .get("diagnostic_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .get("failed_source_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        let diagnostic = report
            .get("diagnostics")
            .and_then(norito::json::Value::as_array)
            .and_then(|sources| sources.first())
            .and_then(|source| source.get("diagnostics"))
            .and_then(norito::json::Value::as_array)
            .and_then(|diagnostics| diagnostics.first())
            .expect("canonical lint diagnostic");
        assert_eq!(
            diagnostic.get("code").and_then(norito::json::Value::as_str),
            Some("K5003")
        );
        assert_eq!(
            diagnostic
                .get("severity")
                .and_then(norito::json::Value::as_str),
            Some("warning")
        );
        assert_eq!(
            diagnostic
                .get("phase")
                .and_then(norito::json::Value::as_str),
            Some("semantic")
        );
        assert!(diagnostic.get("primary_span").is_some());
        assert!(diagnostic.get("notes").is_some());
        assert!(diagnostic.get("help").is_some());
        assert!(diagnostic.get("fix").is_some());

        let mut context = TestContext::new(fixture_account(0x44));
        let error = DevCheckArgs {
            manifest: DevManifestArgs {
                manifest: manifest_path,
                profile: "local".to_owned(),
            },
            locked: false,
        }
        .run(&mut context)
        .expect_err("lint findings must keep a failing exit status");
        assert!(
            error
                .to_string()
                .contains("detailed diagnostics were emitted")
        );
        let output = context.take_output().expect("printed dev-check report");
        assert_eq!(
            output.get("ok").and_then(norito::json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            output
                .pointer("/lint/diagnostics/0/diagnostics/0/code")
                .and_then(norito::json::Value::as_str),
            Some("K5003")
        );
    }

    #[test]
    fn dev_test_path_filter_selects_files_without_becoming_a_name_filter() {
        let invocations = prepare_dev_test_invocations(
            vec![
                PathBuf::from("tests/kotodama/payments.test.ko"),
                PathBuf::from("tests/kotodama/assets.test.ko"),
            ],
            Some("payments"),
            None,
            false,
            "run",
            "text",
            |_| -> Result<Vec<String>> { panic!("path-only selection must not parse test names") },
        )
        .expect("select one test source by path");

        assert_eq!(
            invocations,
            vec![(
                "tests/kotodama/payments.test.ko".to_owned(),
                vec![
                    "run".to_owned(),
                    "tests/kotodama/payments.test.ko".to_owned()
                ]
            )]
        );
    }

    #[test]
    fn dev_test_name_filter_selects_matching_files_and_is_always_forwarded() {
        let invocations = prepare_dev_test_invocations(
            vec![
                PathBuf::from("tests/kotodama/assets.test.ko"),
                PathBuf::from("tests/kotodama/payments.test.ko"),
            ],
            None,
            Some("reject"),
            false,
            "run",
            "text",
            |path| {
                Ok(if path.ends_with("payments.test.ko") {
                    vec!["rejects_invalid_payment".to_owned()]
                } else {
                    vec!["mints_asset".to_owned()]
                })
            },
        )
        .expect("select one test source by function name");

        assert_eq!(
            invocations,
            vec![(
                "tests/kotodama/payments.test.ko".to_owned(),
                vec![
                    "run".to_owned(),
                    "--filter".to_owned(),
                    "reject".to_owned(),
                    "tests/kotodama/payments.test.ko".to_owned(),
                ]
            )]
        );
    }

    #[test]
    fn dev_test_filters_fail_closed_when_paths_or_names_do_not_match() {
        let paths = vec![PathBuf::from("tests/kotodama/payments.test.ko")];
        let path_error = prepare_dev_test_invocations(
            paths.clone(),
            Some("missing"),
            None,
            false,
            "run",
            "text",
            |_| -> Result<Vec<String>> { Ok(Vec::new()) },
        )
        .expect_err("an unmatched path filter must fail");
        assert!(
            path_error
                .to_string()
                .contains("no Kotodama test source path matched")
        );

        let name_error = prepare_dev_test_invocations(
            paths,
            None,
            Some("missing"),
            false,
            "run",
            "text",
            |_| Ok(vec!["smoke".to_owned()]),
        )
        .expect_err("an unmatched name filter must fail");
        assert!(
            name_error
                .to_string()
                .contains("no Kotodama test function matched")
        );
    }

    #[test]
    fn dev_test_exact_requires_and_forwards_a_complete_name_filter() {
        let path = PathBuf::from("tests/kotodama/payments.test.ko");
        let invocations = prepare_dev_test_invocations(
            vec![path.clone()],
            None,
            Some("smoke"),
            true,
            "run",
            "json",
            |_| Ok(vec!["smoke".to_owned(), "smoke_extended".to_owned()]),
        )
        .expect("select one exact test");
        assert_eq!(
            invocations,
            vec![(
                path.display().to_string(),
                vec![
                    "run".to_owned(),
                    "--filter".to_owned(),
                    "smoke".to_owned(),
                    "--exact".to_owned(),
                    "--format".to_owned(),
                    "json".to_owned(),
                    path.display().to_string(),
                ]
            )]
        );

        let error = prepare_dev_test_invocations(
            vec![path],
            None,
            None,
            true,
            "run",
            "text",
            |_| -> Result<Vec<String>> { Ok(Vec::new()) },
        )
        .expect_err("--exact without --filter must fail");
        assert!(error.to_string().contains("--exact requires --filter"));
    }

    #[test]
    fn dev_schema_samples_use_only_v1_canonical_scalar_types() {
        assert_eq!(dev_sample_value_for_type("int"), norito::json!("0"));
        assert_eq!(dev_sample_value_for_type("decimal"), norito::json!("0"));
        assert_eq!(dev_sample_value_for_type("quantity"), norito::json!("0"));
        assert_eq!(dev_sample_value_for_type("bool"), norito::json!(false));
        assert_eq!(dev_sample_value_for_type("string"), norito::json!(""));
        assert_eq!(dev_sample_value_for_type("bytes"), norito::json!("0x"));
        assert_eq!(
            dev_sample_value_for_type("DataSpaceId"),
            norito::json!(0_u64)
        );

        for retired in ["i64", "u128", "Amount", "Balance", "FixedU128", "Blob"] {
            assert_eq!(
                dev_sample_value_for_type(retired),
                norito::json!("sample"),
                "retired type `{retired}` must not receive canonical schema handling"
            );
        }
    }

    #[test]
    fn prepare_dev_smoke_cases_validates_payloads_and_profile_defaults() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    view fn status(int value) -> int { return value; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [profiles.local]
                default_gas_limit = 123456

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[smoke]]
                id = "status"
                contract = "demo.greeter"
                entrypoint = "status"
                payload = { value = "7" }
                expected_result = "7"
            "#,
        )
        .expect("write manifest");

        dev_build_manifest(&manifest_path, "local", false).expect("dev build");
        let cases = prepare_dev_smoke_cases(&manifest_path, "local").expect("prepare smoke");

        assert_eq!(cases.len(), 1);
        let case = &cases[0];
        assert_eq!(case.id, "status");
        assert_eq!(case.mode, DevSmokeMode::View);
        assert_eq!(case.gas_limit, 123456);
        assert_eq!(case.contract_alias.to_string(), "greeter::universal");
        assert_eq!(
            case.payload
                .as_ref()
                .and_then(norito::json::Value::as_object)
                .and_then(|object| object.get("value"))
                .and_then(norito::json::Value::as_str),
            Some("7")
        );
        assert_eq!(
            case.expected_result
                .as_ref()
                .and_then(norito::json::Value::as_str),
            Some("7")
        );
    }

    #[test]
    fn contract_payload_validation_rejects_adversarial_shapes() {
        let account = fixture_account(0x11).to_string();
        let program = compile_contract_program(
            r#"
            seiyaku Demo {
                kotoage fn submit(int amount, recipient: AccountId) -> int authorize("Submit") {
                    return amount;
                }

                kotoage fn upload(owner: AccountId, tag: Name, payload: bytes) -> int authorize("Upload") {
                    return codec::tlv_len(payload);
                }

                view fn ping() -> int {
                    return 1;
                }
            }
            "#,
        );
        let submit = embedded_entrypoint(&program, "submit");
        let upload = embedded_entrypoint(&program, "upload");
        let ping = embedded_entrypoint(&program, "ping");

        let err =
            normalize_local_contract_payload(&submit, None).expect_err("missing payload fails");
        assert!(
            err.to_string()
                .contains("contract payload is required for parameterized entrypoints"),
            "unexpected error: {err}"
        );

        let err = normalize_local_contract_payload(&submit, Some(&norito::json!([1, 2])))
            .expect_err("array payload fails");
        assert!(
            err.to_string()
                .contains("contract payload must be a JSON object keyed by parameter name"),
            "unexpected error: {err}"
        );

        let missing = norito::json!({ "recipient": (account.clone()) });
        let err = normalize_local_contract_payload(&submit, Some(&missing))
            .expect_err("missing required field fails");
        assert!(
            err.to_string()
                .contains("missing contract payload field `amount`"),
            "unexpected error: {err}"
        );

        let wrong_type = norito::json!({ "amount": 7, "recipient": (account.clone()) });
        let err = normalize_local_contract_payload(&submit, Some(&wrong_type))
            .expect_err("wrong amount type fails");
        assert!(
            err.to_string()
                .contains("contract payload field `amount` does not match the declared schema"),
            "unexpected error: {err}"
        );

        let extra = norito::json!({ "amount": "7", "recipient": (account), "extra": 1 });
        let err = normalize_local_contract_payload(&submit, Some(&extra))
            .expect_err("unexpected field fails");
        assert!(
            err.to_string()
                .contains("unexpected contract payload field `extra`"),
            "unexpected error: {err}"
        );

        let err = normalize_local_contract_payload(&ping, Some(&norito::json!({ "extra": 1 })))
            .expect_err("zero-parameter entrypoint rejects non-empty payload");
        assert!(
            err.to_string().contains("contract payload must be omitted"),
            "unexpected error: {err}"
        );
        let err = normalize_local_contract_payload(&ping, Some(&norito::json!(null)))
            .expect_err("zero-parameter entrypoint rejects null payload");
        assert!(
            err.to_string().contains("contract payload must be omitted"),
            "unexpected error: {err}"
        );

        for (payload, expected_field) in [
            (
                norito::json!({ "owner": "not-an-account", "tag": "safe_tag", "payload": "0x00" }),
                "owner",
            ),
            (
                norito::json!({ "owner": (fixture_account(0x12).to_string()), "tag": "bad tag", "payload": "0x00" }),
                "tag",
            ),
            (
                norito::json!({ "owner": (fixture_account(0x13).to_string()), "tag": "safe_tag", "payload": "0x0" }),
                "payload",
            ),
        ] {
            let err = normalize_local_contract_payload(&upload, Some(&payload))
                .expect_err("invalid typed payload field fails");
            assert!(
                err.to_string().contains(&format!(
                    "contract payload field `{expected_field}` does not match the declared schema"
                )),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn prepare_dev_smoke_cases_rejects_adversarial_payload_drift() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    view fn status(int value) -> int { return value; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [profiles.local]
                default_gas_limit = 123456

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[smoke]]
                id = "status_with_extra_field"
                contract = "demo.greeter"
                entrypoint = "status"
                payload = { value = "7", unexpected = 9 }
            "#,
        )
        .expect("write manifest");

        dev_build_manifest(&manifest_path, "local", false).expect("dev build");
        let err = prepare_dev_smoke_cases(&manifest_path, "local")
            .expect_err("manifest smoke payload drift must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("invalid payload for smoke `status_with_extra_field`"),
            "unexpected error: {rendered}"
        );
        assert!(
            rendered.contains("unexpected contract payload field `unexpected`"),
            "unexpected error: {rendered}"
        );
    }

    #[test]
    fn prepare_dev_smoke_cases_rejects_non_object_parameter_payloads() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    view fn status(int value) -> int { return value; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[smoke]]
                id = "array_payload"
                contract = "demo.greeter"
                entrypoint = "status"
                payload = [7]
            "#,
        )
        .expect("write manifest");

        dev_build_manifest(&manifest_path, "local", false).expect("dev build");
        let err = prepare_dev_smoke_cases(&manifest_path, "local")
            .expect_err("array smoke payload must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("invalid payload for smoke `array_payload`")
                && rendered
                    .contains("contract payload must be a JSON object keyed by parameter name"),
            "unexpected error: {rendered}"
        );
    }

    #[test]
    fn prepare_dev_smoke_cases_rejects_unknown_entrypoints_and_modes() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    view fn status(int value) -> int { return value; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [profiles.local]
                default_gas_limit = 123456

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[smoke]]
                id = "missing_entrypoint"
                contract = "demo.greeter"
                entrypoint = "missing"
                payload = {}
            "#,
        )
        .expect("write manifest");

        dev_build_manifest(&manifest_path, "local", false).expect("dev build");
        let err = prepare_dev_smoke_cases(&manifest_path, "local")
            .expect_err("unknown smoke entrypoint must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("invalid payload for smoke `missing_entrypoint`"),
            "unexpected error: {rendered}"
        );
        assert!(
            rendered.contains("does not declare entrypoint `missing`"),
            "unexpected error: {rendered}"
        );

        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [profiles.local]
                default_gas_limit = 123456

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[smoke]]
                id = "bad_mode"
                contract = "demo.greeter"
                mode = "stream"
                entrypoint = "status"
                payload = { value = "7" }
            "#,
        )
        .expect("rewrite manifest");
        let err = prepare_dev_smoke_cases(&manifest_path, "local")
            .expect_err("unsupported smoke mode must fail");
        assert!(
            err.to_string()
                .contains("smoke `bad_mode` has unsupported mode `stream`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn prepare_dev_smoke_cases_rejects_unknown_contracts() {
        let dir = tempdir().expect("tempdir");
        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"

                [[smoke]]
                id = "unknown_contract"
                contract = "demo.missing"
                entrypoint = "status"
                payload = {}
            "#,
        )
        .expect("write manifest");

        let err = prepare_dev_smoke_cases(&manifest_path, "local")
            .expect_err("unknown smoke contract must fail");
        assert!(
            err.to_string()
                .contains("contract `demo.missing` is not declared in manifest"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dev_build_manifest_locked_rejects_missing_and_stale_generated_outputs() {
        let dir = tempdir().expect("tempdir");
        let contracts_dir = dir.path().join("contracts");
        let artifacts_dir = dir.path().join("artifacts");
        fs::create_dir_all(&contracts_dir).expect("create contracts dir");
        fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

        fs::write(
            contracts_dir.join("greeter.ko"),
            r#"
                seiyaku Greeter {
                    state int Counter;
                    hajimari(int value) { Counter = value; }
                    view fn status() -> int { return Counter; }
                }
            "#,
        )
        .expect("write contract");

        let manifest_path = dir.path().join("iroha.contracts.toml");
        fs::write(
            &manifest_path,
            r#"
                bundle_name = "demo"
                default_dataspace = "universal"

                [[contracts]]
                name = "demo.greeter"
                alias = "greeter"
                source = "contracts/greeter.ko"
                artifact = "artifacts/greeter.to"
            "#,
        )
        .expect("write manifest");

        dev_build_manifest(&manifest_path, "local", false).expect("initial dev build");
        let interface_path = artifacts_dir.join("greeter.interface.json");
        fs::remove_file(&interface_path).expect("remove generated interface");
        let err = dev_build_manifest(&manifest_path, "local", true)
            .expect_err("locked build must reject missing interface");
        assert!(
            err.to_string().contains("generated artifact is missing")
                && err.to_string().contains("greeter.interface.json"),
            "unexpected error: {err}"
        );

        dev_build_manifest(&manifest_path, "local", false).expect("regenerate outputs");
        fs::write(&interface_path, "{}\n").expect("poison generated interface");
        let err = dev_build_manifest(&manifest_path, "local", true)
            .expect_err("locked build must reject stale interface");
        assert!(
            err.to_string().contains("generated artifact is stale")
                && err.to_string().contains("greeter.interface.json"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn local_contract_schema_validation_rejects_malformed_nested_shapes() {
        for raw in ["(int", "int)", "(int,,bool)", "(bytes,(int,)"] {
            let err =
                parse_local_contract_schema_type(raw).expect_err("malformed schema type must fail");
            assert!(
                err.to_string().contains("contract schema type")
                    || err.to_string().contains("unsupported contract schema type"),
                "unexpected error for {raw}: {err}"
            );
        }

        let tuple_schema =
            parse_local_contract_schema_type("(int,(bool,bytes))").expect("nested tuple schema");
        validate_local_contract_value(
            &tuple_schema,
            &norito::json!([7, [true, "0x00"]]),
            "tuple_payload",
        )
        .expect("valid nested tuple payload");

        for invalid in [
            norito::json!([7, true]),
            norito::json!([7, [true, "0x0"]]),
            norito::json!([7, [true, "0x00"], 9]),
        ] {
            let err = validate_local_contract_value(&tuple_schema, &invalid, "tuple_payload")
                .expect_err("invalid nested tuple payload must fail");
            assert!(
                err.to_string().contains(
                    "contract payload field `tuple_payload` does not match the declared schema"
                ),
                "unexpected error: {err}"
            );
        }

        let dataspace_schema =
            parse_local_contract_schema_type("DataSpaceId").expect("dataspace schema");
        validate_local_contract_value(&dataspace_schema, &norito::json!(0_i64), "dataspace")
            .expect("zero dataspace id is valid");
        let err =
            validate_local_contract_value(&dataspace_schema, &norito::json!(-1_i64), "dataspace")
                .expect_err("negative dataspace id must fail");
        assert!(
            err.to_string()
                .contains("contract payload field `dataspace` does not match the declared schema"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn local_contract_schema_validation_rejects_scalar_boundary_values() {
        for (schema_raw, payload, field_name) in [
            ("()", norito::json!({}), "unit"),
            ("fixed_u128", norito::json!("not-a-number"), "amount"),
            ("bool", norito::json!("true"), "flag"),
            ("Name", norito::json!("bad name"), "name"),
            ("AssetDefinitionId", norito::json!("xor"), "asset_def"),
            ("DomainId", norito::json!("bad domain"), "domain"),
            ("NftId", norito::json!("nft"), "nft"),
            ("DataSpaceId", norito::json!("-1"), "dataspace"),
            ("bytes", norito::json!("0xabc"), "payload"),
        ] {
            let schema = parse_local_contract_schema_type(schema_raw).expect("schema");
            let err = validate_local_contract_value(&schema, &payload, field_name)
                .expect_err("invalid scalar boundary must fail");
            assert!(
                err.to_string().contains(&format!(
                    "contract payload field `{field_name}` does not match the declared schema"
                )),
                "unexpected error for {schema_raw}: {err}"
            );
        }

        let numeric_schema = parse_local_contract_schema_type("fixed_u128").expect("numeric");
        validate_local_contract_value(&numeric_schema, &norito::json!("1.25"), "amount")
            .expect("decimal numeric string is valid");
        validate_local_contract_value(&numeric_schema, &norito::json!(7_i64), "amount")
            .expect("integer numeric value is valid");
    }

    #[test]
    fn render_dev_schema_markdown_rejects_malformed_interface_json() {
        let dir = tempdir().expect("tempdir");
        let interface_path = dir.path().join("bad.interface.json");
        fs::write(&interface_path, "{").expect("write malformed interface");
        let report = norito::json!({
            "contracts": [
                {
                    "name": "demo.bad",
                    "interface": (interface_path.display().to_string()),
                    "entrypoint_count": 1,
                    "state_count": 0
                }
            ]
        });

        let manifest_path = dir.path().join("iroha.contracts.toml");
        let err = render_dev_schema_markdown(&manifest_path, &report)
            .expect_err("malformed interface JSON must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("failed to parse") && rendered.contains("bad.interface.json"),
            "unexpected error: {rendered}"
        );
    }

    #[test]
    fn render_dev_schema_markdown_rejects_missing_interface_json() {
        let dir = tempdir().expect("tempdir");
        let interface_path = dir.path().join("missing.interface.json");
        let report = norito::json!({
            "contracts": [
                {
                    "name": "demo.missing",
                    "interface": (interface_path.display().to_string()),
                    "entrypoint_count": 1,
                    "state_count": 0
                }
            ]
        });

        let manifest_path = dir.path().join("iroha.contracts.toml");
        let err = render_dev_schema_markdown(&manifest_path, &report)
            .expect_err("missing interface JSON must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("failed to read") && rendered.contains("missing.interface.json"),
            "unexpected error: {rendered}"
        );
    }

    #[test]
    fn program_summary_reports_hashes() {
        let program = minimal_program();
        let expected_code_hash = ivm::contract_code_hash(&program);
        let summary = program_summary_from_bytes(&program).expect("summary");
        assert_eq!(summary.code_hash, expected_code_hash);
        assert_eq!(
            summary.abi_hash,
            iroha_crypto::Hash::prehashed(ivm::syscalls::compute_abi_hash(
                ivm::SyscallPolicy::AbiV1,
            ))
        );
    }

    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(1).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }

    #[test]
    fn simulate_emits_gas_limit_metadata_key() {
        let key_pair = fixture_key_pair(1);
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
        let authority = fixture_account(0x21);
        let mut ctx = TestContext::new(authority);
        let program = minimal_view_contract_program();
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let args = DebugViewArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: "inspect".to_owned(),
            gas_limit: DEFAULT_CONTRACT_GAS_LIMIT,
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
            output.get("result").and_then(norito::json::Value::as_str),
            Some("7")
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
        let authority = fixture_account(0x22);
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
            entrypoint: "inspect".to_owned(),
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
        let authority = fixture_account(0x23);
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
            entrypoint: "inspect".to_owned(),
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
        let authority = fixture_account(0x31);
        let mut ctx = TestContext::new(authority);
        let source = r#"
            seiyaku Demo {
                state int counter;
                hajimari() { counter = 0; }

                kotoage fn bump() -> int authorize("Admin") {
                    counter = counter + 1;
                    ledger::domain::register(DomainId::parse("debugcall.universal"));
                    return counter;
                }
            }
        "#;
        let program = compile_contract_program(&source);
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let durable_state_json = format!(
            r#"{{"counter":"0x{}"}}"#,
            hex::encode(encode_int_state_value(41))
        );
        let args = DebugCallArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: "bump".to_owned(),
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
            output.get("result").and_then(norito::json::Value::as_str),
            Some("42")
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
        let authority = fixture_account(0x32);
        let mut ctx = TestContext::new(authority);
        let program = minimal_view_contract_program();
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let args = DebugCallArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: "inspect".to_owned(),
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
            err.to_string().contains("is not a kotoage entrypoint"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn debug_call_matches_overlay_for_public_by_call_execution() {
        let authority = fixture_account(0x33);
        let mut ctx = TestContext::new(authority.clone());
        let source = r#"
            seiyaku Demo {
                state int counter;
                hajimari() { counter = 0; }

                kotoage fn bump(int amount) -> int authorize("Admin") {
                    ledger::domain::register(DomainId::parse("debugparity.universal"));
                    counter = amount;
                    return counter;
                }
            }
        "#;
        let program = compile_contract_program(source);
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let payload_json = r#"{"amount":"7"}"#.to_owned();
        let args = DebugCallArgs {
            authority: None,
            code_file: None,
            code_b64: Some(code_b64),
            entrypoint: "bump".to_owned(),
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
            iroha_primitives::json::Json::from(DEFAULT_CONTRACT_GAS_LIMIT),
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
            output.get("result").and_then(norito::json::Value::as_str),
            Some("7")
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
    fn load_contract_payload_value_rejects_invalid_json_and_conflicting_sources() {
        let err = load_contract_payload_value(Some("{"), None)
            .expect_err("malformed inline payload must fail");
        assert!(
            format!("{err:?}").contains("invalid --payload-json"),
            "unexpected error: {err:?}"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("payload.json");
        std::fs::write(&path, "{").expect("write malformed payload");
        let err = load_contract_payload_value(None, Some(&path))
            .expect_err("malformed payload file must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("invalid JSON in") && rendered.contains("payload.json"),
            "unexpected error: {rendered}"
        );

        let missing_path = dir.path().join("missing.json");
        let err = load_contract_payload_value(None, Some(&missing_path))
            .expect_err("missing payload file must fail");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("read") && rendered.contains("missing.json"),
            "unexpected error: {rendered}"
        );

        let err = load_contract_payload_value(Some("{}"), Some(&path))
            .expect_err("dual payload sources must fail");
        assert!(
            err.to_string()
                .contains("--payload-json and --payload-file are mutually exclusive"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_contract_target_accepts_contract_address() {
        let authority = fixture_account(0x41);
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
        assert!(err.to_string().contains(
            "provide exactly one contract target via --contract-address or --contract-alias"
        ));
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
        let authority = fixture_account(0x51);
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
        let ctx = TestContext::new(fixture_account(0x52));
        let other_authority = fixture_account(0x53);
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
            let key_pair = fixture_key_pair(0xA5);
            let cfg = iroha::config::Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
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
            .try_sign(&private_key)
            .wrap_err("sign simulated contract transaction failed")?;

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
