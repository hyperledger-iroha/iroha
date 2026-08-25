//! Taira public testnet diagnostics and write canaries.
use crate::{CliOutputFormat, Run, RunContext, quote_and_sign_transaction};
use eyre::{Context, Result, eyre};
use iroha::{
    client::{Client as IrohaClient, TransactionWaitOptions, TransactionWaitTerminalStatus},
    config::Config,
    data_model::{
        NetworkId,
        account::{AccountId, address::ChainDiscriminantGuard},
        isi::{InstructionBox, Log},
        level::Level as LogLevel,
        metadata::Metadata,
        name::Name,
        prelude::{FindTransactions, QueryBuilderExt, TransactionEntrypoint},
        transaction::{Executable, FeePaymentIntent},
    },
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use iroha_primitives::json::Json as IrohaJson;
use norito::json::{self, Map, Value};
use reqwest::blocking::Client as HttpClient;
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;
use zeroize::Zeroizing;
const DEFAULT_PUBLIC_ROOT: &str = "https://taira.sora.org";
const DEFAULT_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 369;
const DEFAULT_GAS_ASSET_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const DEFAULT_ALIAS_PREFIX: &str = "tairarolloutcanary";
const DEFAULT_WRITE_TTL_MS: u64 = 120_000;
const DEFAULT_WRITE_STATUS_TIMEOUT_MS: u64 = 120_000;
const FAUCET_POW_ALGORITHM: &str = "scrypt-leading-zero-bits-v2";
const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v3";
const ACCOUNT_ONBOARDING_TOKEN_HEADER: &str = "x-iroha-onboarding-token";
const REQUIRED_MCP_TOOLS: &[&str] = &[
    "iroha.health",
    "iroha.musubi.queries.exact_package",
    "iroha.musubi.queries.exact_release",
    "iroha.musubi.instructions.release_yank_set",
    "iroha.transactions.submit",
    "iroha.transactions.submit_and_wait",
];
#[derive(Clone, Copy)]
enum RouteCheckMethod {
    Get,
    PostEmptyObject,
}
const ROUTE_CHECKS: &[(&str, RouteCheckMethod, &str, &[u16])] = &[
    ("status", RouteCheckMethod::Get, "/status", &[200]),
    ("time_now", RouteCheckMethod::Get, "/v1/time/now", &[200]),
    (
        "sumeragi_status",
        RouteCheckMethod::Get,
        "/v1/sumeragi/status",
        &[401],
    ),
    (
        "pipeline_transaction_status",
        RouteCheckMethod::Get,
        "/v1/pipeline/transactions/status",
        &[400],
    ),
    (
        "retired_transaction_status_alias",
        RouteCheckMethod::Get,
        "/v1/transactions/status",
        &[404],
    ),
    (
        "sccp_capabilities",
        RouteCheckMethod::Get,
        "/v1/sccp/capabilities",
        &[200],
    ),
    (
        "zk_proofs_count",
        RouteCheckMethod::Get,
        "/v1/zk/proofs/count",
        &[200],
    ),
    (
        "public_lane_validators",
        RouteCheckMethod::Get,
        "/v1/nexus/public-lanes/0/validators",
        &[200],
    ),
    // A missing selector should reach the mounted contract-state route and be
    // rejected as bad input. Treating that as mounted keeps the doctor aligned
    // with the rollout harness instead of requiring a real contract key.
    (
        "contracts_state",
        RouteCheckMethod::Get,
        "/v1/contracts/state",
        &[400],
    ),
    // The V1 directory is POST-only and authenticates before typed body decode.
    // An unsigned malformed object must therefore fail at the mounted canonical
    // account boundary, independently of registry data.
    (
        "musubi_ordered_prefix",
        RouteCheckMethod::PostEmptyObject,
        "/v1/musubi/queries/ordered-prefix",
        &[401],
    ),
    (
        "soracloud_status",
        RouteCheckMethod::Get,
        "/v1/soracloud/status",
        // SoraCloud topology is account-authenticated.  The public doctor
        // proves that the protected route is mounted without weakening that
        // boundary or requiring an operator signer for read-side diagnostics.
        &[401],
    ),
];
/// Taira public testnet helpers.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Check Taira read-side health and MCP route posture.
    Doctor(Doctor),
    /// Onboard, faucet, submit, wait, and verify a signed ping canary.
    WriteCanary(WriteCanary),
    /// Generate the canonical deploy-mode Inrou canary workspace from AArch64 guest assets.
    InrouWorkspace(InrouWorkspace),
    /// Build the canonical offline artifact stage that operators preseed into all validators.
    InrouStage(InrouStage),
    /// Register an exact preseeded stage, mutate explicitly, and verify the four-replica Inrou canary.
    InrouCanary(InrouCanary),
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Doctor(cmd) => cmd.run(context),
            Self::WriteCanary(cmd) => cmd.run(context),
            Self::InrouWorkspace(cmd) => cmd.run(context),
            Self::InrouStage(cmd) => cmd.run(context),
            Self::InrouCanary(cmd) => cmd.run(context),
        }
    }
}
/// Read-only Taira public endpoint diagnostics.
#[derive(clap::Args, Debug)]
pub struct Doctor {
    /// Public Torii root URL.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Emit a stable JSON report.
    #[arg(long)]
    pub json: bool,
}
impl Run for Doctor {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = run_doctor(&self.public_root)?;
        render_report(context, self.json, &report)?;
        if report_status(&report) == Some("fail") {
            eyre::bail!("Taira doctor found hard failures");
        }
        Ok(())
    }
}
/// Signed Taira write canary.
#[derive(clap::Args, Debug)]
pub struct WriteCanary {
    /// Public Torii root URL.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Prefix used for the generated account alias.
    #[arg(long, default_value = DEFAULT_ALIAS_PREFIX)]
    pub alias_prefix: String,
    /// Faucet asset definition expected in the onboarding funding response.
    #[arg(long, default_value = DEFAULT_GAS_ASSET_ID)]
    pub faucet_asset_id: String,
    /// Owner-only regular file containing the exact account-onboarding route token.
    #[arg(long, value_name = "PATH")]
    pub onboarding_token_file: PathBuf,
    /// Persist the runtime signer config to this explicit path.
    #[arg(long, value_name = "PATH")]
    pub write_config: Option<PathBuf>,
    /// Use the signer from the loaded client config instead of an ephemeral signer.
    #[arg(long)]
    pub use_config_signer: bool,
    /// Emit a stable JSON receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for WriteCanary {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let fee_payment = context.transaction_fee_payment()?;
        let receipt = run_write_canary(context.config(), &self, fee_payment)?;
        render_report(context, self.json, &receipt)?;
        ensure_write_canary_succeeded(&receipt)
    }
}
/// Canonical deploy-mode Taira Inrou canary workspace generator.
#[derive(clap::Args, Debug)]
pub struct InrouWorkspace {
    /// Direct regular AArch64 kernel image prepared for PortableVM.
    #[arg(long, value_name = "PATH")]
    pub kernel: PathBuf,
    /// Direct regular AArch64 ext4 root filesystem image prepared for PortableVM.
    #[arg(long, value_name = "PATH")]
    pub rootfs: PathBuf,
    /// Direct regular AArch64 initrd image prepared for PortableVM.
    #[arg(long, value_name = "PATH")]
    pub initrd: PathBuf,
    /// Fresh owner-only directory to create with the exact canonical workspace layout.
    #[arg(long, value_name = "PATH")]
    pub output_dir: PathBuf,
    /// Emit a stable JSON report.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouWorkspace {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let receipt = crate::soracloud::create_taira_inrou_canary_workspace(
            &self.kernel,
            &self.rootfs,
            &self.initrd,
            &self.output_dir,
        )?;
        let mut extra = Map::new();
        extra.insert(
            "output_dir".into(),
            Value::String(self.output_dir.display().to_string()),
        );
        extra.insert("workspace".into(), json::to_value(&receipt)?);
        let report = report_value(
            "taira_inrou_workspace",
            "ok",
            "offline",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            extra,
        )?;
        render_report(context, self.json, &report)
    }
}
/// Explicit mutation mode for the Taira Inrou canary.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InrouCanaryMode {
    /// Register a new canary service.
    Deploy,
    /// Replace an already-deployed canary revision.
    Upgrade,
}
/// Canonical offline Taira Inrou artifact staging.
#[derive(clap::Args, Debug)]
pub struct InrouStage {
    /// Build the exact deploy or upgrade revision; no mutation mode is inferred.
    #[arg(long, value_enum)]
    pub mode: InrouCanaryMode,
    /// Path to the canonical four-replica Inrou container manifest.
    #[arg(long, value_name = "PATH")]
    pub container: PathBuf,
    /// Path to the matching public HttpService manifest.
    #[arg(long, value_name = "PATH")]
    pub service: PathBuf,
    /// Canonical service bundle bytes to preseed.
    #[arg(long, value_name = "PATH")]
    pub bundle_file: PathBuf,
    /// Fresh owner-only directory that will contain exact manifests and payloads.
    #[arg(long, value_name = "PATH")]
    pub stage_dir: PathBuf,
    /// Emit a stable JSON receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouStage {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        ensure_canonical_taira_client_identity(context.config())?;
        let _chain_discriminant = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
        let receipt = crate::soracloud::stage_taira_inrou_canary_deployment(
            self.mode,
            &self.container,
            &self.service,
            &self.bundle_file,
            &self.stage_dir,
            &context.config().key_pair,
        )?;
        let mut extra = Map::new();
        extra.insert(
            "stage_dir".into(),
            Value::String(self.stage_dir.display().to_string()),
        );
        extra.insert("receipt".into(), json::to_value(&receipt)?);
        let report = report_value(
            "taira_inrou_stage",
            "ok",
            "offline",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            extra,
        )?;
        render_report(context, self.json, &report)
    }
}
/// Canonical Taira Inrou preseed registration and route canary.
#[derive(clap::Args, Debug)]
pub struct InrouCanary {
    /// Public Torii root URL used for mutation, status, and route probes.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Owner-only stage created by `iroha taira inrou-stage` and preseeded into all validators.
    #[arg(long, value_name = "PATH")]
    pub stage_dir: PathBuf,
    /// Submit an explicit deploy or upgrade mutation; conflicts are never retried as another mode.
    #[arg(long, value_enum)]
    pub mode: InrouCanaryMode,
    /// Maximum convergence time for adverts, placements, runtime health, and all four routes.
    #[arg(long, value_name = "SECS", default_value_t = 180)]
    pub timeout_secs: u64,
    /// Emit a stable redacted JSON receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouCanary {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_inrou_canary_timeout(self.timeout_secs)?;
        ensure_canonical_taira_client_identity(context.config())?;
        let _chain_discriminant = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
        let public_root = normalize_root_url(&self.public_root)?;
        preflight_taira_network_identity(&public_root, context.config())?;
        let mut status_config = context.config().clone();
        status_config.torii_api_url = Url::parse(&format!("{public_root}/"))
            .wrap_err("failed to bind the signed status client to the selected Taira root")?;
        let status_client = IrohaClient::new(status_config);
        let deployment = crate::soracloud::run_taira_inrou_canary_deployment(
            context.config(),
            context.transaction_fee_payment()?,
            self.stage_dir,
            public_root.clone(),
            None,
            self.timeout_secs,
            self.mode,
        )?;
        let receipt =
            verify_inrou_canary(&public_root, &status_client, &deployment, self.timeout_secs)?;
        render_report(context, self.json, &receipt)?;
        if report_status(&receipt) != Some("ok") {
            eyre::bail!("Taira Inrou canary found hard failures");
        }
        Ok(())
    }
}
fn ensure_canonical_taira_client_identity(config: &Config) -> Result<()> {
    if config.chain.to_string() != DEFAULT_CHAIN_ID {
        eyre::bail!(
            "Taira Inrou canary requires canonical chain `{DEFAULT_CHAIN_ID}`; configured `{}`",
            config.chain
        );
    }
    if config.account_chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT {
        eyre::bail!(
            "Taira Inrou canary requires chain discriminant {DEFAULT_CHAIN_DISCRIMINANT}; configured {}",
            config.account_chain_discriminant
        );
    }
    Ok(())
}
fn preflight_taira_network_identity(public_root: &str, config: &Config) -> Result<()> {
    let http = http_client()?;
    let puzzle_url = join_url(public_root, "/v1/accounts/faucet/puzzle")?;
    let puzzle = http_json(&http, reqwest::Method::GET, puzzle_url.as_str(), None)?;
    if puzzle.status != 200 {
        eyre::bail!(
            "Taira network-identity preflight returned HTTP {} from {}",
            puzzle.status,
            puzzle_url
        );
    }
    let body = puzzle
        .body
        .as_ref()
        .ok_or_else(|| eyre!("Taira network-identity preflight returned a non-JSON puzzle"))?;
    validate_taira_puzzle_identity(body, &config.network_id).map(|_| ())
}
#[derive(Debug)]
struct HttpJson {
    status: u16,
    body: Option<Value>,
    text: String,
}
#[derive(Debug)]
struct CanarySigner {
    key_pair: KeyPair,
    account_id: AccountId,
    generated: bool,
}
fn run_doctor(public_root: &str) -> Result<Value> {
    let public_root = normalize_root_url(public_root)?;
    let http = http_client()?;
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let empty_object = norito::json!({});
    for (name, method, path, expected_statuses) in ROUTE_CHECKS {
        let url = join_url(&public_root, path)?;
        let (method, body) = match method {
            RouteCheckMethod::Get => (reqwest::Method::GET, None),
            RouteCheckMethod::PostEmptyObject => (reqwest::Method::POST, Some(&empty_object)),
        };
        let result = http_json(&http, method, url.as_str(), body)?;
        let status_ok = expected_statuses.contains(&result.status);
        let semantic_error = if status_ok {
            match *name {
                "time_now" => validate_time_snapshot(result.body.as_ref()).err(),
                "sumeragi_status" => {
                    validate_operator_signature_authentication_challenge(result.body.as_ref()).err()
                }
                "musubi_ordered_prefix" | "soracloud_status" => {
                    validate_canonical_authentication_challenge(result.body.as_ref()).err()
                }
                _ => None,
            }
        } else {
            None
        };
        let ok = status_ok && semantic_error.is_none();
        push_check(
            &mut checks,
            name,
            result.status,
            ok,
            semantic_error
                .clone()
                .or_else(|| route_check_detail(expected_statuses)),
        );
        if !status_ok {
            failures.push(format!(
                "{name} returned HTTP {}; expected {}",
                result.status,
                expected_statuses
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        } else if let Some(error) = semantic_error {
            failures.push(error);
        }
        if *name == "status" && ok {
            collect_status_warnings(result.body.as_ref(), &mut warnings);
        }
    }
    let mcp_url = join_url(&public_root, "/v1/mcp")?;
    let mcp_get = http_json(&http, reqwest::Method::GET, mcp_url.as_str(), None)?;
    let mcp_get_ok = (200..300).contains(&mcp_get.status);
    push_check(&mut checks, "mcp_get", mcp_get.status, mcp_get_ok, None);
    if !mcp_get_ok {
        failures.push(format!("mcp_get returned HTTP {}", mcp_get.status));
    }
    let initialize = norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "iroha-taira-doctor", "version": "1" }
        }
    });
    let mcp_init = http_json(
        &http,
        reqwest::Method::POST,
        mcp_url.as_str(),
        Some(&initialize),
    )?;
    let mcp_init_ok = (200..300).contains(&mcp_init.status);
    push_check(
        &mut checks,
        "mcp_initialize",
        mcp_init.status,
        mcp_init_ok,
        None,
    );
    if !mcp_init_ok {
        failures.push(format!("mcp_initialize returned HTTP {}", mcp_init.status));
    }
    let tools_payload = norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });
    let tools = http_json(
        &http,
        reqwest::Method::POST,
        mcp_url.as_str(),
        Some(&tools_payload),
    )?;
    let tools_ok = (200..300).contains(&tools.status);
    push_check(&mut checks, "mcp_tools_list", tools.status, tools_ok, None);
    if !tools_ok {
        failures.push(format!("mcp_tools_list returned HTTP {}", tools.status));
    } else {
        let tool_names = mcp_tool_names(tools.body.as_ref());
        let missing: Vec<String> = REQUIRED_MCP_TOOLS
            .iter()
            .copied()
            .filter(|name| !tool_names.iter().any(|present| present == name))
            .map(str::to_owned)
            .collect();
        let raw: Vec<String> = tool_names
            .iter()
            .filter(|name| name.starts_with("torii."))
            .cloned()
            .collect();
        if missing.is_empty() && raw.is_empty() {
            push_check(
                &mut checks,
                "mcp_required_tools",
                200,
                true,
                Some("all required curated tools are present".to_owned()),
            );
        } else {
            if !missing.is_empty() {
                failures.push(format!("MCP tools/list missing: {}", missing.join(", ")));
            }
            if !raw.is_empty() {
                failures.push(format!(
                    "MCP tools/list exposed raw torii.* names: {}",
                    raw.join(", ")
                ));
            }
            push_check(
                &mut checks,
                "mcp_required_tools",
                200,
                false,
                Some(format!(
                    "missing=[{}], raw=[{}]",
                    missing.join(", "),
                    raw.join(", ")
                )),
            );
        }
    }
    let status = if failures.is_empty() { "ok" } else { "fail" };
    report_value(
        "taira_doctor",
        status,
        &public_root,
        checks,
        warnings,
        failures,
        Map::new(),
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactInrouCanaryStatus {
    active_adverts: u64,
    hosted_replicas: u64,
    process_generation: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ExactInrouCanaryRouteEvidence {
    replica_slot: u64,
    identity: String,
    evidence_sha256: String,
    process_generation: u64,
}
#[derive(Debug, Default)]
struct InrouCanaryConvergence {
    exact_status: Option<ExactInrouCanaryStatus>,
    identities: BTreeMap<u64, (String, String)>,
}
impl InrouCanaryConvergence {
    fn observe_status(
        &mut self,
        observed: Result<ExactInrouCanaryStatus, String>,
    ) -> Result<(), String> {
        match observed {
            Ok(status) => {
                if self
                    .exact_status
                    .is_none_or(|current| current.process_generation != status.process_generation)
                {
                    self.identities.clear();
                }
                self.exact_status = Some(status);
                Ok(())
            }
            Err(error) => {
                self.exact_status = None;
                self.identities.clear();
                Err(error)
            }
        }
    }
    fn record_route(&mut self, evidence: ExactInrouCanaryRouteEvidence) -> Result<(), String> {
        let status = self.exact_status.ok_or_else(|| {
            "route evidence arrived without exact authoritative status".to_owned()
        })?;
        if status.process_generation != evidence.process_generation {
            return Err("route evidence belongs to a different process generation".to_owned());
        }
        if !(1..=4).contains(&evidence.replica_slot) || evidence.identity.is_empty() {
            return Err("route evidence has a non-canonical replica identity".to_owned());
        }
        self.identities.insert(
            evidence.replica_slot,
            (evidence.identity, evidence.evidence_sha256),
        );
        Ok(())
    }
    fn is_complete(&self) -> bool {
        self.exact_status.is_some() && self.identities.len() == 4
    }
}
fn validate_exact_inrou_canary_status(
    status: &Value,
    deployment: &crate::soracloud::TairaInrouCanaryDeployment,
) -> Result<ExactInrouCanaryStatus, String> {
    validate_soracloud_status(Some(status))?;
    let root = status
        .as_object()
        .ok_or_else(|| "Soracloud status response is not an object".to_owned())?;
    if root.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("Soracloud status is not canonical schema version 1".to_owned());
    }
    if root
        .get("runtime_manager")
        .and_then(Value::as_object)
        .and_then(|runtime| runtime.get("available"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("runtime manager is unavailable".to_owned());
    }
    let topology = root
        .get("hosted_http_topology")
        .and_then(Value::as_object)
        .ok_or_else(|| "Soracloud status is missing hosted HTTP topology".to_owned())?;
    let active_adverts = topology
        .get("active_capability_adverts")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Soracloud status is missing exact active_capability_adverts".to_owned())?;
    let hosted_replicas = topology
        .get("hosted_replica_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Soracloud status is missing exact hosted_replica_count".to_owned())?;
    if active_adverts != 4 || hosted_replicas != 4 {
        return Err(format!(
            "requires exactly four Inrou hosts and placements (adverts={active_adverts}, replicas={hosted_replicas})"
        ));
    }
    let services = root
        .get("control_plane")
        .and_then(Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(Value::as_array)
        .ok_or_else(|| "Soracloud status is missing control-plane services".to_owned())?;
    let mut matching_services = services.iter().filter(|service| {
        service.get("service_name").and_then(Value::as_str)
            == Some(deployment.service_name.as_str())
    });
    let service = matching_services.next().ok_or_else(|| {
        format!(
            "canary service `{}` is not present in authoritative status",
            deployment.service_name
        )
    })?;
    if matching_services.next().is_some() {
        return Err(format!(
            "authoritative status contains duplicate snapshots for canary service `{}`",
            deployment.service_name
        ));
    }
    if service.get("current_version").and_then(Value::as_str)
        != Some(deployment.service_version.as_str())
    {
        return Err(
            "authoritative canary version does not match the submitted revision".to_owned(),
        );
    }
    let (expected_action, revision_count_is_valid): (&str, fn(u64) -> bool) =
        match deployment.mutation_mode.as_str() {
            "deploy" => ("Deploy", |count| count == 1),
            "upgrade" => ("Upgrade", |count| count >= 2),
            other => return Err(format!("unsupported Inrou mutation mode `{other}`")),
        };
    let revision_count = service
        .get("revision_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "authoritative canary status is missing revision_count".to_owned())?;
    if !revision_count_is_valid(revision_count) {
        return Err(format!(
            "authoritative canary revision count {revision_count} is invalid after {}",
            deployment.mutation_mode
        ));
    }
    if !service.get("active_rollout").is_some_and(Value::is_null) {
        return Err("Taira Inrou canary must report an explicit null active_rollout".to_owned());
    }
    match deployment.mutation_mode.as_str() {
        "deploy" => {
            if !service.get("last_rollout").is_some_and(Value::is_null) {
                return Err(
                    "initial Taira Inrou deploy must report an explicit null last_rollout"
                        .to_owned(),
                );
            }
        }
        "upgrade" => {
            let rollout = service
                .get("last_rollout")
                .and_then(Value::as_object)
                .ok_or_else(|| "Taira Inrou upgrade is missing its promoted rollout".to_owned())?;
            let baseline_version = rollout
                .get("baseline_version")
                .and_then(Value::as_str)
                .filter(|version| {
                    !version.is_empty() && *version != deployment.service_version.as_str()
                });
            let promoted = baseline_version.is_some()
                && rollout.get("candidate_version").and_then(Value::as_str)
                    == Some(deployment.service_version.as_str())
                && rollout.get("canary_percent").and_then(Value::as_u64) == Some(100)
                && rollout.get("traffic_percent").and_then(Value::as_u64) == Some(100)
                && rollout
                    .get("stage")
                    .and_then(|stage| tagged_enum_name(stage, "stage"))
                    == Some("Promoted");
            if !promoted {
                return Err(
                    "Taira Inrou upgrade rollout is not an exact promoted immutable revision transition"
                        .to_owned(),
                );
            }
        }
        _ => unreachable!("mutation mode was validated above"),
    }
    let revision = service
        .get("latest_revision")
        .and_then(Value::as_object)
        .ok_or_else(|| "canary service is missing its latest revision".to_owned())?;
    if revision.get("service_version").and_then(Value::as_str)
        != Some(deployment.service_version.as_str())
    {
        return Err(
            "authoritative latest revision version does not match the submitted revision"
                .to_owned(),
        );
    }
    if revision
        .get("service_manifest_hash")
        .and_then(Value::as_str)
        != Some(deployment.service_manifest_hash.as_str())
    {
        return Err(
            "authoritative latest revision service manifest hash does not match the staged artifact"
                .to_owned(),
        );
    }
    if revision
        .get("container_manifest_hash")
        .and_then(Value::as_str)
        != Some(deployment.container_manifest_hash.as_str())
    {
        return Err(
            "authoritative latest revision container manifest hash does not match the staged artifact"
                .to_owned(),
        );
    }
    let canonical = revision.get("replicas").and_then(Value::as_u64) == Some(4)
        && revision.get("service_version").and_then(Value::as_str)
            == Some(deployment.service_version.as_str())
        && revision
            .get("action")
            .and_then(|action| tagged_enum_name(action, "action"))
            == Some(expected_action)
        && revision
            .get("runtime")
            .and_then(|runtime| tagged_enum_name(runtime, "runtime"))
            == Some("Inrou")
        && revision
            .get("execution_plane")
            .and_then(|plane| tagged_enum_name(plane, "execution_plane"))
            == Some("HttpService")
        && revision.get("route_host").and_then(Value::as_str)
            == Some(deployment.route_host.as_str())
        && revision.get("route_path_prefix").and_then(Value::as_str)
            == Some(deployment.route_path_prefix.as_str())
        && revision.get("healthcheck_path").and_then(Value::as_str)
            == Some(deployment.healthcheck_path.as_str());
    if !canonical {
        return Err(
            "authoritative canary revision differs from the canonical four-replica Inrou route"
                .to_owned(),
        );
    }
    let process_generation = revision
        .get("process_generation")
        .and_then(Value::as_u64)
        .filter(|generation| *generation > 0)
        .ok_or_else(|| {
            "authoritative canary revision has no positive process generation".to_owned()
        })?;
    Ok(ExactInrouCanaryStatus {
        active_adverts,
        hosted_replicas,
        process_generation,
    })
}
fn exact_inrou_canary_header<'a>(
    headers: &'a reqwest::header::HeaderMap,
    name: &str,
) -> Result<&'a str, String> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .ok_or_else(|| format!("hosted route response is missing `{name}`"))?;
    if values.next().is_some() {
        return Err(format!(
            "hosted route response contains duplicate `{name}` values"
        ));
    }
    value
        .to_str()
        .map_err(|_| format!("hosted route response contains non-text `{name}`"))
}
fn validate_exact_inrou_canary_route(
    headers: &reqwest::header::HeaderMap,
    body: &Value,
    deployment: &crate::soracloud::TairaInrouCanaryDeployment,
    expected_process_generation: u64,
) -> Result<ExactInrouCanaryRouteEvidence, String> {
    let served_service_name = exact_inrou_canary_header(
        headers,
        iroha_torii_shared::SORACLOUD_SERVED_SERVICE_NAME_HEADER,
    )?;
    if served_service_name != deployment.service_name.as_str() {
        return Err("hosted route served a different service identity".to_owned());
    }
    let served_service_version = exact_inrou_canary_header(
        headers,
        iroha_torii_shared::SORACLOUD_SERVED_SERVICE_VERSION_HEADER,
    )?;
    if served_service_version != deployment.service_version.as_str() {
        return Err("hosted route served a different immutable revision".to_owned());
    }
    let served_bundle_hash = exact_inrou_canary_header(
        headers,
        iroha_torii_shared::SORACLOUD_SERVED_MATERIALIZED_BUNDLE_HASH_HEADER,
    )?;
    if served_bundle_hash != deployment.bundle_hash.as_str() {
        return Err("hosted route served a different materialized bundle".to_owned());
    }
    let replica_slot_literal = exact_inrou_canary_header(
        headers,
        iroha_torii_shared::SORACLOUD_SERVED_REPLICA_SLOT_HEADER,
    )?;
    let replica_slot = replica_slot_literal
        .parse::<u64>()
        .map_err(|_| "hosted route returned a non-numeric replica slot".to_owned())?;
    if !(1..=4).contains(&replica_slot) || replica_slot_literal != replica_slot.to_string() {
        return Err("hosted route returned a non-canonical replica slot".to_owned());
    }
    let process_generation_literal = exact_inrou_canary_header(
        headers,
        iroha_torii_shared::SORACLOUD_SERVED_PROCESS_GENERATION_HEADER,
    )?;
    let process_generation = process_generation_literal
        .parse::<u64>()
        .map_err(|_| "hosted route returned a non-numeric process generation".to_owned())?;
    if process_generation == 0 || process_generation_literal != process_generation.to_string() {
        return Err("hosted route returned a non-canonical process generation".to_owned());
    }
    if process_generation != expected_process_generation {
        return Err("hosted route served a different authoritative process generation".to_owned());
    }
    let body = body
        .as_object()
        .ok_or_else(|| "health response is not an object".to_owned())?;
    if body.len() != 4 {
        return Err("health response has a non-canonical field set".to_owned());
    }
    let body_service = body.get("service").and_then(Value::as_str);
    let runtime = body.get("runtime").and_then(Value::as_str);
    let body_replica_slot = body.get("replica_slot").and_then(Value::as_u64);
    let identity = body.get("identity").and_then(Value::as_str);
    let expected_identity = format!("{}:replica:{replica_slot}", deployment.service_name);
    if body_service != Some(deployment.service_name.as_str())
        || runtime != Some("Inrou")
        || body_replica_slot != Some(replica_slot)
        || identity != Some(expected_identity.as_str())
    {
        return Err("health response violated the exact canary identity contract".to_owned());
    }
    let evidence = norito::json!({
        "body": (Value::Object(body.clone())),
        "served_service_name": served_service_name,
        "served_service_version": served_service_version,
        "served_replica_slot": replica_slot,
        "served_materialized_bundle_hash": served_bundle_hash,
        "served_process_generation": process_generation,
    });
    let encoded =
        json::to_vec(&evidence).map_err(|error| format!("encode exact route evidence: {error}"))?;
    Ok(ExactInrouCanaryRouteEvidence {
        replica_slot,
        identity: expected_identity,
        evidence_sha256: hex::encode(Sha256::digest(encoded)),
        process_generation,
    })
}
fn inrou_canary_health_path(route_prefix: &str, healthcheck_path: &str) -> String {
    format!(
        "{}/{}",
        route_prefix.trim_end_matches('/'),
        healthcheck_path.trim_start_matches('/')
    )
}
fn verify_inrou_canary(
    public_root: &str,
    status_client: &IrohaClient,
    deployment: &crate::soracloud::TairaInrouCanaryDeployment,
    timeout_secs: u64,
) -> Result<Value> {
    validate_inrou_canary_timeout(timeout_secs)?;
    let http = HttpClient::builder()
        .timeout(Duration::from_secs(timeout_secs.min(5)))
        .user_agent("iroha-taira-inrou-canary/1")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .wrap_err("failed to build Taira Inrou canary HTTP client")?;
    let health_path =
        inrou_canary_health_path(&deployment.route_path_prefix, &deployment.healthcheck_path);
    let health_base = join_url(public_root, &health_path)?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut nonce = 0_u64;
    let mut last_status_code = 0_u16;
    let mut last_status_error = "status not observed".to_owned();
    let mut last_route_code = 0_u16;
    let mut last_route_error = "route not observed".to_owned();
    let mut convergence = InrouCanaryConvergence::default();
    let mut completion_confirmed = false;
    while Instant::now() < deadline {
        let observed_status = match account_signed_soracloud_status(status_client) {
            Ok(response) => {
                last_status_code = response.status;
                if response.status != 200 {
                    Err(format!("Soracloud status returned HTTP {}", response.status))
                } else {
                    response
                        .body
                        .as_ref()
                        .ok_or_else(|| "Soracloud status returned non-JSON".to_owned())
                        .and_then(|status| validate_exact_inrou_canary_status(status, deployment))
                }
            }
            Err(error) => {
                last_status_code = 0;
                Err(format!("{error:#}"))
            }
        };
        match convergence.observe_status(observed_status) {
            Ok(()) => last_status_error.clear(),
            Err(error) => {
                last_status_error = error;
                last_route_code = 0;
                last_route_error =
                    "route probe skipped until exact authoritative status is current".to_owned();
                if !convergence.is_complete() {
                    std::thread::sleep(Duration::from_millis(200));
                }
                continue;
            }
        }
        // A full route set only becomes final after a later exact status poll
        // confirms that its authoritative process generation stayed current.
        if convergence.is_complete() {
            completion_confirmed = true;
            break;
        }
        let process_generation = convergence
            .exact_status
            .expect("successful status observation installs exact status")
            .process_generation;
        let mut health_url = health_base.clone();
        health_url
            .query_pairs_mut()
            .append_pair("taira_inrou_probe", &nonce.to_string());
        nonce = nonce.saturating_add(1);
        let route_response = http
            .get(health_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::HOST, deployment.route_host.as_str())
            .send();
        match route_response {
            Ok(response) => {
                last_route_code = response.status().as_u16();
                let headers = response.headers().clone();
                let response = match decode_http_json_response(response) {
                    Ok(response) => response,
                    Err(error) => {
                        last_route_error = format!("{error:#}");
                        if !convergence.is_complete() {
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        continue;
                    }
                };
                if response.status != 200 {
                    last_route_error = format!("HTTP {}", response.status);
                    if !convergence.is_complete() {
                        std::thread::sleep(Duration::from_millis(200));
                    }
                    continue;
                }
                let evidence = response
                    .body
                    .as_ref()
                    .ok_or_else(|| "health response returned non-JSON".to_owned())
                    .and_then(|body| {
                        validate_exact_inrou_canary_route(
                            &headers,
                            body,
                            deployment,
                            process_generation,
                        )
                    });
                match evidence.and_then(|evidence| convergence.record_route(evidence)) {
                    Ok(()) => last_route_error.clear(),
                    Err(error) => last_route_error = error,
                }
            }
            Err(error) => last_route_error = format!("{error:#}"),
        }
        if !convergence.is_complete() {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let exact_status = convergence.exact_status;
    let identities = convergence.identities;
    let status_ready = exact_status.is_some();
    let routes_ready = identities.len() == 4 && completion_confirmed;
    if identities.len() == 4 && !completion_confirmed {
        last_route_error =
            "full route evidence lacked a later exact authoritative status confirmation".to_owned();
    }
    let active_adverts = exact_status.map_or(0, |status| status.active_adverts);
    let hosted_replicas = exact_status.map_or(0, |status| status.hosted_replicas);
    let process_generation = exact_status.map_or(0, |status| status.process_generation);
    let mut checks = Vec::new();
    push_check(
        &mut checks,
        "inrou_authoritative_status",
        last_status_code,
        status_ready,
        Some(if status_ready {
            format!(
                "active_adverts={active_adverts}, hosted_replicas={hosted_replicas}, process_generation={process_generation}"
            )
        } else {
            last_status_error.clone()
        }),
    );
    push_check(
        &mut checks,
        "inrou_public_routes",
        last_route_code,
        routes_ready,
        Some(if routes_ready {
            "observed four exact replica identities and confirmed their process generation"
                .to_owned()
        } else {
            format!(
                "observed {}/4 replica identities; {last_route_error}",
                identities.len()
            )
        }),
    );
    let mut failures = Vec::new();
    if !status_ready {
        failures.push(format!(
            "authoritative Inrou status did not converge: {last_status_error}"
        ));
    }
    if !routes_ready {
        failures.push(format!(
            "public Inrou route convergence failed: {last_route_error}"
        ));
    }
    let mut extra = Map::new();
    extra.insert(
        "service_name".to_owned(),
        Value::from(deployment.service_name.clone()),
    );
    extra.insert(
        "service_version".to_owned(),
        Value::from(deployment.service_version.clone()),
    );
    extra.insert(
        "service_manifest_hash".to_owned(),
        Value::from(deployment.service_manifest_hash.clone()),
    );
    extra.insert(
        "container_manifest_hash".to_owned(),
        Value::from(deployment.container_manifest_hash.clone()),
    );
    extra.insert(
        "mutation_mode".to_owned(),
        Value::from(deployment.mutation_mode.clone()),
    );
    extra.insert(
        "route_host".to_owned(),
        Value::from(deployment.route_host.clone()),
    );
    extra.insert("route_path".to_owned(), Value::from(health_path));
    extra.insert(
        "active_host_adverts".to_owned(),
        Value::from(active_adverts),
    );
    extra.insert(
        "hosted_replica_count".to_owned(),
        Value::from(hosted_replicas),
    );
    extra.insert(
        "process_generation".to_owned(),
        Value::from(process_generation),
    );
    extra.insert(
        "post_route_status_confirmed".to_owned(),
        Value::from(completion_confirmed),
    );
    extra.insert(
        "bundle_hash".to_owned(),
        Value::from(deployment.bundle_hash.clone()),
    );
    extra.insert(
        "bundle_content_cid".to_owned(),
        Value::from(deployment.bundle_content_cid.clone()),
    );
    extra.insert(
        "bundle_manifest_digest_hex".to_owned(),
        Value::from(deployment.bundle_manifest_digest_hex.clone()),
    );
    extra.insert(
        "guest_content_cid".to_owned(),
        Value::from(deployment.guest_content_cid.clone()),
    );
    extra.insert(
        "guest_manifest_digest_hex".to_owned(),
        Value::from(deployment.guest_manifest_digest_hex.clone()),
    );
    extra.insert(
        "submitted_tx_hash".to_owned(),
        Value::from(deployment.submitted_tx_hash.clone()),
    );
    extra.insert(
        "mutation_response_digest".to_owned(),
        Value::from(deployment.mutation_response_digest.clone()),
    );
    extra.insert(
        "replica_identities".to_owned(),
        Value::Array(
            identities
                .into_iter()
                .map(|(slot, (identity, evidence_sha256))| {
                    norito::json!({
                        "replica_slot": slot,
                        "identity": identity,
                        "evidence_sha256": evidence_sha256
                    })
                })
                .collect(),
        ),
    );
    report_value(
        "taira_inrou_canary",
        if failures.is_empty() { "ok" } else { "fail" },
        public_root,
        checks,
        Vec::new(),
        failures,
        extra,
    )
}
fn validate_inrou_canary_timeout(timeout_secs: u64) -> Result<()> {
    if timeout_secs == 0 {
        eyre::bail!("--timeout-secs must be greater than zero");
    }
    Ok(())
}
fn run_write_canary(
    config: &Config,
    args: &WriteCanary,
    fee_payment: FeePaymentIntent,
) -> Result<Value> {
    let public_root = normalize_root_url(&args.public_root)?;
    let onboarding_token = read_onboarding_token_file(&args.onboarding_token_file)?;
    // Account literals in both onboarding and faucet payloads must use the
    // same I105 network discriminant as the fresh Taira genesis.  Install the
    // guard before the first AccountId formatting operation, not only before
    // the later signed transaction.
    let _guard = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
    let http = http_client()?;
    let signer = resolve_canary_signer(config, args.use_config_signer)?;
    let alias = build_alias(
        &args.alias_prefix,
        signer.key_pair.public_key(),
        "wonderland.universal",
    )?;
    let mut warnings = Vec::new();
    let mut checks = Vec::new();
    let mut failures = Vec::new();
    let onboarding_plan = plan_canary_onboarding(
        &http,
        &public_root,
        &alias,
        &signer.account_id,
        &onboarding_token,
    )?;
    let onboarding_receipt =
        validate_onboarding_plan_receipt(onboarding_plan.body.as_ref(), &signer.account_id, &alias);
    push_check(
        &mut checks,
        "accounts_onboard_plan",
        onboarding_plan.status,
        onboarding_plan.status == 200 && onboarding_receipt.is_ok(),
        onboarding_plan.body.as_ref().map(compact_json),
    );
    if onboarding_plan.status != 200 {
        failures.push(format!(
            "account onboarding planning failed with HTTP {}; onboarding apply was not attempted; faucet funding was not attempted",
            onboarding_plan.status
        ));
        let mut extra = Map::new();
        insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
        return report_value(
            "taira_write_canary",
            "fail",
            &public_root,
            checks,
            warnings,
            failures,
            extra,
        );
    }
    let onboarding_receipt = match onboarding_receipt {
        Ok(receipt) => receipt,
        Err(error) => {
            failures.push(format!(
                "account onboarding plan receipt was invalid: {error:#}"
            ));
            let mut extra = Map::new();
            insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
            return report_value(
                "taira_write_canary",
                "fail",
                &public_root,
                checks,
                warnings,
                failures,
                extra,
            );
        }
    };
    let onboarding_apply =
        apply_canary_onboarding(&http, &public_root, &onboarding_receipt, &onboarding_token)?;
    let onboarding_contract = validate_onboarding_apply_response(
        onboarding_apply.body.as_ref(),
        &signer.account_id,
        &alias,
    );
    let onboarding_apply_ok = onboarding_contract.as_ref().is_ok_and(|result| {
        (result.unchanged && onboarding_apply.status == 200)
            || (!result.unchanged && onboarding_apply.status == 202)
    });
    push_check(
        &mut checks,
        "accounts_onboard",
        onboarding_apply.status,
        onboarding_apply_ok,
        onboarding_apply.body.as_ref().map(compact_json),
    );
    let onboarding_contract = match onboarding_contract {
        Ok(result) if onboarding_apply_ok => result,
        Ok(_) => {
            failures.push(format!(
                "account onboarding apply returned incompatible HTTP {}",
                onboarding_apply.status
            ));
            let mut extra = Map::new();
            insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
            return report_value(
                "taira_write_canary",
                "fail",
                &public_root,
                checks,
                warnings,
                failures,
                extra,
            );
        }
        Err(error) => {
            failures.push(format!(
                "account onboarding apply response was invalid: {error:#}"
            ));
            let mut extra = Map::new();
            insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
            return report_value(
                "taira_write_canary",
                "fail",
                &public_root,
                checks,
                warnings,
                failures,
                extra,
            );
        }
    };
    if let Some(onboarding_tx_hash) = onboarding_contract.tx_hash_hex.as_deref() {
        let onboarding_final = wait_for_pipeline_terminal_status(
            &http,
            &public_root,
            onboarding_tx_hash,
            Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS),
        )?;
        let onboarding_terminal = pipeline_status_kind(onboarding_final.body.as_ref());
        let onboarding_applied =
            onboarding_final.status == 200 && onboarding_terminal.as_deref() == Some("Applied");
        push_check(
            &mut checks,
            "accounts_onboard_finality",
            onboarding_final.status,
            onboarding_applied,
            onboarding_final.body.as_ref().map(compact_json),
        );
        if onboarding_applied {
            // Continue to faucet funding.
        } else {
            failures.push(format!(
                "account onboarding transaction {onboarding_tx_hash} did not reach Applied finality; last status was {}",
                onboarding_terminal.as_deref().unwrap_or("not_observed")
            ));
        }
    } else {
        push_check(
            &mut checks,
            "accounts_onboard_finality",
            200,
            true,
            Some("Unchanged: no transaction submitted".to_owned()),
        );
    }
    if !failures.is_empty() {
        let mut extra = Map::new();
        insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
        return report_value(
            "taira_write_canary",
            "fail",
            &public_root,
            checks,
            warnings,
            failures,
            extra,
        );
    }
    let faucet = claim_faucet(&http, &public_root, &signer.account_id, &config.network_id)?;
    let faucet_contract = validate_faucet_response(
        faucet.body.as_ref(),
        &signer.account_id,
        &args.faucet_asset_id,
    );
    push_check(
        &mut checks,
        "accounts_faucet",
        faucet.status,
        faucet.status == 202 && faucet_contract.is_ok(),
        faucet.body.as_ref().map(compact_json),
    );
    if faucet.status != 202 {
        failures.push(faucet_failure_hint(&faucet));
    }
    let faucet_tx_hash = match faucet_contract {
        Ok(hash) if faucet.status == 202 => hash,
        Ok(_) => String::new(),
        Err(error) => {
            failures.push(format!("faucet response was invalid: {error:#}"));
            String::new()
        }
    };
    if !failures.is_empty() {
        let mut extra = Map::new();
        insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
        return report_value(
            "taira_write_canary",
            "fail",
            &public_root,
            checks,
            warnings,
            failures,
            extra,
        );
    }
    let faucet_final = wait_for_pipeline_terminal_status(
        &http,
        &public_root,
        &faucet_tx_hash,
        Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS),
    )?;
    let faucet_terminal = pipeline_status_kind(faucet_final.body.as_ref());
    let faucet_applied =
        faucet_final.status == 200 && faucet_terminal.as_deref() == Some("Applied");
    push_check(
        &mut checks,
        "accounts_faucet_finality",
        faucet_final.status,
        faucet_applied,
        faucet_final.body.as_ref().map(compact_json),
    );
    if !faucet_applied {
        failures.push(format!(
            "faucet transaction {faucet_tx_hash} did not reach Applied finality; last status was {}",
            faucet_terminal.as_deref().unwrap_or("not_observed")
        ));
        let mut extra = Map::new();
        insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
        return report_value(
            "taira_write_canary",
            "fail",
            &public_root,
            checks,
            warnings,
            failures,
            extra,
        );
    }
    let mut canary_config = config.clone();
    canary_config.chain = DEFAULT_CHAIN_ID.into();
    canary_config.torii_api_url = Url::parse(&format!("{public_root}/"))
        .wrap_err_with(|| format!("invalid public root `{public_root}`"))?;
    canary_config.account = signer.account_id.clone();
    canary_config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
    canary_config.key_pair = signer.key_pair.clone();
    canary_config.transaction_ttl = Duration::from_millis(DEFAULT_WRITE_TTL_MS);
    canary_config.transaction_status_timeout =
        Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS);
    canary_config.transaction_add_nonce = false;
    if let Some(path) = &args.write_config {
        write_runtime_config(path, &canary_config)?;
    }
    let client = IrohaClient::new(canary_config.clone());
    let mut metadata = Metadata::default();
    insert_string_metadata(&mut metadata, "taira_canary", "write-canary")?;
    let message = canary_message()?;
    let instruction = Log::new(LogLevel::INFO, message.clone());
    let executable = Executable::Instructions(vec![InstructionBox::from(instruction)].into());
    let (transaction, fee_quote) =
        quote_and_sign_transaction(&client, executable, fee_payment, metadata)
            .wrap_err("failed to quote and sign Taira canary transaction")?;
    let signed_hash = transaction.hash();
    let entrypoint_hash = TransactionEntrypoint::External(transaction.clone()).hash();
    client
        .submit_transaction(&transaction)
        .map_err(hint_submit_error)?;
    let wait = client
        .wait_for_transaction_terminal_status(
            signed_hash,
            TransactionWaitOptions {
                timeout: Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS),
                poll_interval: Duration::from_millis(500),
                terminal_statuses: vec![TransactionWaitTerminalStatus::Applied],
            },
        )
        .map_err(hint_wait_error)?;
    if wait.terminal_kind != "Applied" {
        failures.push(format!(
            "write canary stopped at {}; inspect /v1/pipeline/transactions/status for {}",
            wait.terminal_kind, wait.hash
        ));
    }
    let tx_query_verified = match client.query(FindTransactions).execute_all() {
        Ok(transactions) => transactions
            .iter()
            .any(|tx| tx.entrypoint_hash() == &entrypoint_hash),
        Err(err) => {
            warnings.push(format!("transaction query verification failed: {err}"));
            false
        }
    };
    if !tx_query_verified {
        warnings.push("write canary reached pipeline terminal status but transaction query did not return the entry yet".to_owned());
    }
    let status = if failures.is_empty() { "ok" } else { "fail" };
    let mut extra = Map::new();
    insert_write_receipt_identity(&mut extra, &signer, &alias, &args.faucet_asset_id);
    extra.insert(
        "fee_payment".into(),
        norito::json::to_value(&fee_quote.intent).wrap_err("serialize fee payment receipt")?,
    );
    extra.insert(
        "fee_quote".into(),
        norito::json::to_value(&fee_quote).wrap_err("serialize fee quote receipt")?,
    );
    extra.insert("message".into(), Value::String(message));
    extra.insert("faucet_tx_hash".into(), Value::String(faucet_tx_hash));
    extra.insert("ping_tx_hash".into(), Value::String(wait.hash.clone()));
    extra.insert(
        "applied_block_height".into(),
        wait.r#final
            .status
            .block_height
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    extra.insert("terminal_kind".into(), Value::String(wait.terminal_kind));
    extra.insert("tx_query_verified".into(), Value::from(tx_query_verified));
    if let Some(path) = &args.write_config {
        extra.insert(
            "config_path".into(),
            Value::String(path.display().to_string()),
        );
    }
    report_value(
        "taira_write_canary",
        status,
        &public_root,
        checks,
        warnings,
        failures,
        extra,
    )
}
fn render_report<C: RunContext>(context: &mut C, json: bool, report: &Value) -> Result<()> {
    if json || context.output_format() == CliOutputFormat::Json {
        context.print_data(report)
    } else {
        let object = report
            .as_object()
            .ok_or_else(|| eyre!("report must be an object"))?;
        let command = object
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("taira");
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let public_root = object
            .get("public_root")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_PUBLIC_ROOT);
        context.println_data(format!("{command}: {status} ({public_root})"))?;
        if let Some(checks) = object.get("checks").and_then(Value::as_array) {
            for check in checks {
                let name = check.get("name").and_then(Value::as_str).unwrap_or("check");
                let ok = check.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let status_code = check
                    .get("http_status")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "-".to_owned(), |value| value.to_string());
                let detail = check
                    .get("detail")
                    .and_then(Value::as_str)
                    .map(|detail| format!(" ({detail})"))
                    .unwrap_or_default();
                let marker = if ok { "ok" } else { "fail" };
                context.println_data(format!("  {marker} {name} HTTP {status_code}{detail}"))?;
            }
        }
        if let Some(warnings) = object.get("warnings").and_then(Value::as_array) {
            print_receipt_fields(context, object)?;
            for warning in warnings {
                if let Some(warning) = warning.as_str() {
                    context.println_data(format!("  warn {warning}"))?;
                }
            }
        }
        if let Some(failures) = object.get("failures").and_then(Value::as_array) {
            for failure in failures {
                if let Some(failure) = failure.as_str() {
                    context.println_data(format!("  fail {failure}"))?;
                }
            }
        }
        Ok(())
    }
}
fn report_status(report: &Value) -> Option<&str> {
    report
        .as_object()
        .and_then(|object| object.get("status"))
        .and_then(Value::as_str)
}
fn ensure_write_canary_succeeded(report: &Value) -> Result<()> {
    if report_status(report) == Some("ok") {
        return Ok(());
    }
    eyre::bail!("Taira write canary found hard failures")
}
fn print_receipt_fields<C: RunContext>(context: &mut C, object: &Map) -> Result<()> {
    const RECEIPT_FIELDS: &[&str] = &[
        "chain",
        "chain_discriminant",
        "account_id",
        "alias",
        "faucet_asset_id",
        "fee_payment",
        "fee_quote",
        "faucet_tx_hash",
        "ping_tx_hash",
        "applied_block_height",
        "terminal_kind",
        "tx_query_verified",
        "config_path",
    ];
    for field in RECEIPT_FIELDS {
        let Some(value) = object
            .get(*field)
            .filter(|value| !matches!(value, Value::Null))
        else {
            continue;
        };
        context.println_data(format!("  {field}: {}", display_json_scalar(value)))?;
    }
    Ok(())
}
fn display_json_scalar(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        _ => compact_json(value),
    }
}
fn report_value(
    command: &str,
    status: &str,
    public_root: &str,
    checks: Vec<Value>,
    warnings: Vec<String>,
    failures: Vec<String>,
    extra: Map,
) -> Result<Value> {
    let mut root = Map::new();
    root.insert("command".into(), Value::String(command.to_owned()));
    root.insert("status".into(), Value::String(status.to_owned()));
    root.insert("public_root".into(), Value::String(public_root.to_owned()));
    root.insert("checks".into(), Value::Array(checks));
    root.insert(
        "warnings".into(),
        Value::Array(warnings.into_iter().map(Value::String).collect()),
    );
    root.insert(
        "failures".into(),
        Value::Array(failures.into_iter().map(Value::String).collect()),
    );
    for (key, value) in extra {
        root.insert(key, value);
    }
    Ok(Value::Object(root))
}
fn push_check(
    checks: &mut Vec<Value>,
    name: &str,
    http_status: u16,
    ok: bool,
    detail: Option<String>,
) {
    let mut object = Map::new();
    object.insert("name".into(), Value::String(name.to_owned()));
    object.insert("http_status".into(), Value::from(u64::from(http_status)));
    object.insert("ok".into(), Value::from(ok));
    if let Some(detail) = detail {
        object.insert("detail".into(), Value::String(detail));
    }
    checks.push(Value::Object(object));
}
fn route_check_detail(expected_statuses: &[u16]) -> Option<String> {
    (expected_statuses.len() == 1 && expected_statuses[0] != 200).then(|| {
        format!(
            "mounted route is expected to return HTTP {} for this preflight shape",
            expected_statuses[0]
        )
    })
}
fn normalize_root_url(raw: &str) -> Result<String> {
    if raw.is_empty() {
        eyre::bail!("public root URL must not be empty");
    }
    if raw != raw.trim() {
        eyre::bail!("public root URL must not contain surrounding whitespace");
    }
    let parsed = Url::parse(raw).wrap_err_with(|| format!("invalid URL `{raw}`"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => eyre::bail!("unsupported URL scheme `{other}`"),
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        eyre::bail!("public root URL must not contain credentials");
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        eyre::bail!("public root URL must be an origin without a path, query, or fragment");
    }
    let canonical = parsed.origin().ascii_serialization();
    if raw != canonical {
        eyre::bail!("public root URL must use the canonical origin spelling `{canonical}`");
    }
    Ok(canonical)
}
fn join_url(root: &str, path: &str) -> Result<Url> {
    let root = format!("{}/", root.trim_end_matches('/'));
    let suffix = path.trim_start_matches('/');
    Url::parse(&root)
        .and_then(|url| url.join(suffix))
        .wrap_err_with(|| format!("failed to build URL from `{root}` and `{path}`"))
}
fn validate_onboarding_token(token: &str) -> Result<&str> {
    let bytes = token.as_bytes();
    if !(32..=256).contains(&bytes.len()) || !bytes.iter().all(|byte| (0x21..=0x7e).contains(byte))
    {
        eyre::bail!(
            "account onboarding token must contain 32..256 printable ASCII bytes without spaces or normalization"
        );
    }
    Ok(token)
}
#[cfg(unix)]
fn validate_onboarding_token_file_metadata(metadata: &fs::Metadata) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if metadata.uid() != rustix::process::geteuid().as_raw() {
        eyre::bail!("account onboarding token file must be owned by the current user");
    }
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        eyre::bail!("account onboarding token file must not grant group or other access");
    }
    if mode & 0o400 == 0 {
        eyre::bail!("account onboarding token file must be owner-readable");
    }
    Ok(())
}
#[cfg(not(unix))]
fn validate_onboarding_token_file_metadata(_metadata: &fs::Metadata) -> Result<()> {
    Ok(())
}
fn read_onboarding_token_file(path: &Path) -> Result<Zeroizing<String>> {
    let before =
        fs::symlink_metadata(path).wrap_err("failed to inspect account onboarding token file")?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        eyre::bail!("account onboarding token file must be a regular non-symlink file");
    }
    validate_onboarding_token_file_metadata(&before)?;
    #[cfg(unix)]
    let mut file = {
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
        .wrap_err("failed to securely open account onboarding token file")?;
        fs::File::from(descriptor)
    };
    #[cfg(not(unix))]
    let mut file = fs::OpenOptions::new()
        .read(true)
        .open(path)
        .wrap_err("failed to open account onboarding token file")?;
    let opened = file
        .metadata()
        .wrap_err("failed to inspect opened account onboarding token file")?;
    if !opened.file_type().is_file() {
        eyre::bail!("account onboarding token file must remain a regular file");
    }
    validate_onboarding_token_file_metadata(&opened)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if opened.dev() != before.dev() || opened.ino() != before.ino() {
            eyre::bail!("account onboarding token file changed during secure open");
        }
    }
    let mut raw = Zeroizing::new(Vec::with_capacity(257));
    std::io::Read::by_ref(&mut file)
        .take(257)
        .read_to_end(&mut raw)
        .wrap_err("failed to read account onboarding token file")?;
    if raw.len() > 256 {
        eyre::bail!("account onboarding token file exceeds the maximum credential length");
    }
    let token = std::str::from_utf8(&raw)
        .map_err(|_| eyre!("account onboarding token file must contain printable ASCII"))?;
    validate_onboarding_token(token)?;
    Ok(Zeroizing::new(token.to_owned()))
}
fn http_client() -> Result<HttpClient> {
    HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("iroha-taira-devex/1")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .wrap_err("failed to build HTTP client")
}
fn http_json(
    http: &HttpClient,
    method: reqwest::Method,
    url: &str,
    body: Option<&Value>,
) -> Result<HttpJson> {
    let mut request = http
        .request(method, url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(body) = body {
        let bytes = json::to_vec(body).map_err(|err| eyre!("encode JSON request body: {err}"))?;
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes);
    }
    let response = request
        .send()
        .wrap_err_with(|| format!("request failed for {url}"))?;
    decode_http_json_response(response)
}
fn account_signed_soracloud_status(client: &IrohaClient) -> Result<HttpJson> {
    let response = client
        .get_soracloud_status_response()
        .wrap_err("canonical account-signed Soracloud status request failed")?;
    let status = response.status().as_u16();
    let text = String::from_utf8(response.body().to_vec())
        .wrap_err("canonical account-signed Soracloud status response is not UTF-8")?;
    let body = if text.trim().is_empty() {
        None
    } else {
        json::from_str::<Value>(&text).ok()
    };
    Ok(HttpJson { status, body, text })
}
fn decode_http_json_response(response: reqwest::blocking::Response) -> Result<HttpJson> {
    let status = response.status().as_u16();
    let text = response
        .text()
        .wrap_err("failed to read Taira HTTP response body")?;
    let parsed = if text.trim().is_empty() {
        None
    } else {
        json::from_str::<Value>(&text).ok()
    };
    Ok(HttpJson {
        status,
        body: parsed,
        text,
    })
}
fn redact_sensitive_json(value: &mut Value, sensitive_value: &str) {
    match value {
        Value::String(text) => {
            if text.contains(sensitive_value) {
                *text = text.replace(sensitive_value, "<redacted>");
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_sensitive_json(item, sensitive_value);
            }
        }
        Value::Object(object) => {
            let original = std::mem::take(object);
            for (key, mut item) in original {
                redact_sensitive_json(&mut item, sensitive_value);
                object.insert(key.replace(sensitive_value, "<redacted>"), item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
fn redact_http_json(response: &mut HttpJson, sensitive_value: &str) {
    if response.text.contains(sensitive_value) {
        response.text = response.text.replace(sensitive_value, "<redacted>");
    }
    if let Some(body) = &mut response.body {
        redact_sensitive_json(body, sensitive_value);
        response.text =
            json::to_json(body).unwrap_or_else(|_| "\"<redacted JSON response>\"".to_owned());
    } else if !response.text.trim().is_empty() {
        response.text = "<invalid JSON response>".to_owned();
    }
}
fn collect_status_warnings(status: Option<&Value>, warnings: &mut Vec<String>) {
    let Some(status) = status else {
        warnings.push("/status returned a non-JSON body".to_owned());
        return;
    };
    if value_path_bool(status, &["sumeragi", "tx_queue_saturated"]).unwrap_or(false)
        || value_path_bool(status, &["tx_queue_saturated"]).unwrap_or(false)
    {
        warnings.push("public transaction queue reports saturation".to_owned());
    }
    if let Some(rejected) = value_path_u64(status, &["txs_rejected_recent_5m"])
        && rejected > 0
    {
        warnings.push(format!(
            "recent rejected transactions in /status: {rejected}"
        ));
    }
    if let Some(queue) = value_path_u64(status, &["queue_size"])
        && queue > 0
    {
        warnings.push(format!("public queue has {queue} pending transaction(s)"));
    }
}
fn value_path<'a>(mut value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    for key in path {
        value = value.as_object()?.get(*key)?;
    }
    Some(value)
}
fn value_path_u64(value: &Value, path: &[&str]) -> Option<u64> {
    value_path(value, path).and_then(Value::as_u64)
}
fn value_path_bool(value: &Value, path: &[&str]) -> Option<bool> {
    value_path(value, path).and_then(Value::as_bool)
}
fn validate_time_snapshot(snapshot: Option<&Value>) -> Result<(), String> {
    let snapshot = snapshot
        .and_then(Value::as_object)
        .ok_or_else(|| "/v1/time/now returned a non-object JSON body".to_owned())?;
    let positive_u64 = |field: &str| {
        snapshot
            .get(field)
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("/v1/time/now field `{field}` must be a positive integer"))
    };
    positive_u64("now")?;
    positive_u64("sample_count")?;
    positive_u64("peer_count")?;
    snapshot
        .get("confidence_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "/v1/time/now field `confidence_ms` must be a nonnegative integer".to_owned()
        })?;
    snapshot
        .get("offset_ms")
        .and_then(Value::as_i64)
        .ok_or_else(|| "/v1/time/now field `offset_ms` must be an integer".to_owned())?;
    if snapshot.get("enforcement_mode").and_then(Value::as_str) != Some("reject") {
        return Err("/v1/time/now is not using fail-closed time enforcement".to_owned());
    }
    if snapshot.get("fallback").and_then(Value::as_bool) != Some(false) {
        return Err("/v1/time/now is using the local-clock fallback".to_owned());
    }
    for field in ["healthy", "min_samples_ok", "offset_ok", "confidence_ok"] {
        if snapshot
            .get("health")
            .and_then(Value::as_object)
            .and_then(|health| health.get(field))
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(format!("/v1/time/now health field `{field}` is not true"));
        }
    }
    Ok(())
}
fn validate_canonical_authentication_challenge(body: Option<&Value>) -> Result<(), String> {
    let body = body.and_then(Value::as_object).ok_or_else(|| {
        "protected route returned a non-object authentication challenge".to_owned()
    })?;
    if body.len() != 2 {
        return Err("protected route returned a non-canonical authentication envelope".to_owned());
    }
    if body.get("code").and_then(Value::as_str) != Some("canonical_authentication_required") {
        return Err("protected route returned a non-canonical authentication code".to_owned());
    }
    if body.get("message").and_then(Value::as_str)
        != Some("canonical account request authentication is required")
    {
        return Err("protected route returned a non-canonical authentication message".to_owned());
    }
    Ok(())
}
fn validate_soracloud_status(status: Option<&Value>) -> Result<(), String> {
    let status = status
        .and_then(Value::as_object)
        .ok_or_else(|| "/v1/soracloud/status returned a non-object JSON body".to_owned())?;
    if status.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("/v1/soracloud/status is not canonical schema version 1".to_owned());
    }
    if status
        .get("runtime_manager")
        .and_then(Value::as_object)
        .and_then(|runtime| runtime.get("available"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("/v1/soracloud/status reports no runtime manager".to_owned());
    }
    let topology = status
        .get("hosted_http_topology")
        .and_then(Value::as_object)
        .ok_or_else(|| "/v1/soracloud/status is missing `hosted_http_topology`".to_owned())?;
    let active_adverts = topology
        .get("active_capability_adverts")
        .and_then(Value::as_u64)
        .ok_or_else(|| "/v1/soracloud/status is missing `active_capability_adverts`".to_owned())?;
    if active_adverts < 4 {
        return Err(format!(
            "/v1/soracloud/status reports {active_adverts} active Inrou host advert(s); expected at least 4"
        ));
    }
    let hosted_replicas = topology
        .get("hosted_replica_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "/v1/soracloud/status is missing `hosted_replica_count`".to_owned())?;
    if hosted_replicas < 4 {
        return Err(format!(
            "/v1/soracloud/status reports {hosted_replicas} hosted Inrou replica placement(s); expected at least 4"
        ));
    }
    let services = status
        .get("control_plane")
        .and_then(Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(Value::as_array)
        .ok_or_else(|| "/v1/soracloud/status is missing control-plane services".to_owned())?;
    let has_four_replica_public_inrou_route = services.iter().any(|service| {
        let Some(revision) = service
            .as_object()
            .and_then(|service| service.get("latest_revision"))
            .and_then(Value::as_object)
        else {
            return false;
        };
        revision.get("replicas").and_then(Value::as_u64) == Some(4)
            && revision
                .get("runtime")
                .and_then(|runtime| tagged_enum_name(runtime, "runtime"))
                == Some("Inrou")
            && revision
                .get("execution_plane")
                .and_then(|plane| tagged_enum_name(plane, "execution_plane"))
                == Some("HttpService")
            && revision
                .get("route_host")
                .and_then(Value::as_str)
                .is_some_and(|host| !host.trim().is_empty())
            && revision
                .get("route_path_prefix")
                .and_then(Value::as_str)
                .is_some_and(|path| path.starts_with('/'))
    });
    if !has_four_replica_public_inrou_route {
        return Err(
            "/v1/soracloud/status has no canonical four-replica public HttpService/Inrou route"
                .to_owned(),
        );
    }
    Ok(())
}
fn validate_operator_signature_authentication_challenge(
    body: Option<&Value>,
) -> Result<(), String> {
    let body = body
        .and_then(Value::as_object)
        .ok_or_else(|| "Sumeragi status returned a non-object operator challenge".to_owned())?;
    if body.len() != 2 {
        return Err("Sumeragi status returned a non-canonical operator envelope".to_owned());
    }
    if body.get("code").and_then(Value::as_str) != Some("operator_signature_missing") {
        return Err("Sumeragi status returned a non-canonical operator code".to_owned());
    }
    if body.get("message").and_then(Value::as_str)
        != Some("missing required operator signature header `x-iroha-operator-public-key`")
    {
        return Err("Sumeragi status returned a non-canonical operator message".to_owned());
    }
    Ok(())
}
fn tagged_enum_name<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    let object = value.as_object()?;
    if object.len() != 2 || !object.get("value").is_some_and(Value::is_null) {
        return None;
    }
    object.get(field)?.as_str()
}
fn mcp_tool_names(payload: Option<&Value>) -> Vec<String> {
    payload
        .and_then(|value| value_path(value, &["result", "tools"]))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            tool.as_object()
                .and_then(|obj| obj.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}
fn resolve_canary_signer(config: &Config, use_config_signer: bool) -> Result<CanarySigner> {
    let key_pair = if use_config_signer {
        config.key_pair.clone()
    } else {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .wrap_err("failed to generate Taira canary Ed25519 signer")?
    };
    let (algorithm, _) = key_pair
        .public_key()
        .try_to_bytes()
        .wrap_err("Taira canary signer public key is malformed")?;
    if algorithm != Algorithm::Ed25519 {
        eyre::bail!("Taira canary signer must use Ed25519");
    }
    let account_id = AccountId::new(key_pair.public_key().clone());
    Ok(CanarySigner {
        account_id,
        key_pair,
        generated: !use_config_signer,
    })
}
fn insert_write_receipt_identity(
    extra: &mut Map,
    signer: &CanarySigner,
    alias: &str,
    faucet_asset_id: &str,
) {
    extra.insert("chain".into(), Value::String(DEFAULT_CHAIN_ID.to_owned()));
    extra.insert(
        "chain_discriminant".into(),
        Value::from(u64::from(DEFAULT_CHAIN_DISCRIMINANT)),
    );
    extra.insert(
        "account_id".into(),
        Value::String(signer.account_id.to_string()),
    );
    extra.insert("alias".into(), Value::String(alias.to_owned()));
    extra.insert("generated_signer".into(), Value::from(signer.generated));
    extra.insert(
        "faucet_asset_id".into(),
        Value::String(faucet_asset_id.to_owned()),
    );
}
fn build_alias(prefix: &str, public_key: &iroha_crypto::PublicKey, domain: &str) -> Result<String> {
    if !(1..=32).contains(&prefix.len())
        || !prefix
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_lowercase)
        || !prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        eyre::bail!(
            "alias prefix must contain 1..32 canonical lowercase ASCII alphanumeric bytes and start with a letter"
        );
    }
    let dataspace = domain
        .rsplit('.')
        .next()
        .filter(|value| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
        .ok_or_else(|| eyre!("alias domain must end in a canonical dataspace label"))?;
    let digest = Sha256::digest(public_key.to_string().as_bytes());
    let suffix = hex::encode(&digest[..8]);
    Ok(format!("{prefix}{suffix}@{dataspace}"))
}
fn post_sponsored_onboarding_json(
    http: &HttpClient,
    public_root: &str,
    path: &str,
    body: &Value,
    onboarding_token: &str,
) -> Result<HttpJson> {
    let url = join_url(public_root, path)?;
    let bytes = json::to_vec(body).map_err(|err| eyre!("encode JSON request body: {err}"))?;
    let mut header_value =
        reqwest::header::HeaderValue::from_str(validate_onboarding_token(onboarding_token)?)
            .map_err(|_| {
                eyre!("validated account onboarding token was not a valid HTTP header value")
            })?;
    header_value.set_sensitive(true);
    let response = http
        .post(url.clone())
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(ACCOUNT_ONBOARDING_TOKEN_HEADER, header_value)
        .body(bytes)
        .send()
        .wrap_err_with(|| format!("request failed for {url}"))?;
    let mut response = decode_http_json_response(response)?;
    redact_http_json(&mut response, onboarding_token);
    Ok(response)
}
fn plan_canary_onboarding(
    http: &HttpClient,
    public_root: &str,
    alias: &str,
    account_id: &AccountId,
    onboarding_token: &str,
) -> Result<HttpJson> {
    post_sponsored_onboarding_json(
        http,
        public_root,
        "/v1/accounts/onboard/plan",
        &norito::json!({
            "version": 1,
            "alias": alias,
            "account_id": (account_id.to_string()),
            "permissions": []
        }),
        onboarding_token,
    )
}
fn apply_canary_onboarding(
    http: &HttpClient,
    public_root: &str,
    receipt: &Value,
    onboarding_token: &str,
) -> Result<HttpJson> {
    post_sponsored_onboarding_json(
        http,
        public_root,
        "/v1/accounts/onboard",
        &norito::json!({ "receipt": (receipt.clone()) }),
        onboarding_token,
    )
}
fn validate_onboarding_plan_receipt(
    response: Option<&Value>,
    expected_account: &AccountId,
    expected_alias: &str,
) -> Result<Value> {
    let object = response
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("onboarding plan receipt must be a JSON object"))?;
    let body = object
        .get("body")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("onboarding plan receipt is missing `body`"))?;
    if body.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(eyre!("onboarding plan receipt has an unsupported version"));
    }
    let request = body
        .get("request")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("onboarding plan receipt is missing its canonical request"))?;
    let expected_account = expected_account.to_string();
    if request.get("version").and_then(Value::as_u64) != Some(1)
        || request.get("alias").and_then(Value::as_str) != Some(expected_alias)
        || request.get("account_id").and_then(Value::as_str) != Some(expected_account.as_str())
        || !request
            .get("permissions")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err(eyre!(
            "onboarding plan receipt canonical request differs from the canary intent"
        ));
    }
    for key in [
        "authority",
        "chain_id",
        "anchor",
        "resource",
        "acquisition",
        "quote_guard",
        "instructions",
        "valid_until_ms",
    ] {
        if !body.contains_key(key) {
            return Err(eyre!("onboarding plan receipt body is missing `{key}`"));
        }
    }
    for key in ["plan_hash", "signature"] {
        if !object.contains_key(key) {
            return Err(eyre!("onboarding plan receipt is missing `{key}`"));
        }
    }
    Ok(response.expect("validated receipt response").clone())
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct OnboardingApplyResult {
    tx_hash_hex: Option<String>,
    unchanged: bool,
}
fn validate_onboarding_apply_response(
    response: Option<&Value>,
    expected_account: &AccountId,
    expected_alias: &str,
) -> Result<OnboardingApplyResult> {
    let object = response
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("onboarding apply response must be a JSON object"))?;
    let expected_account = expected_account.to_string();
    if object.get("account_id").and_then(Value::as_str) != Some(expected_account.as_str())
        || object.get("alias").and_then(Value::as_str) != Some(expected_alias)
    {
        return Err(eyre!(
            "onboarding apply response account or alias differs from the canary intent"
        ));
    }
    if !object.contains_key("disposition") {
        return Err(eyre!("onboarding apply response is missing `disposition`"));
    }
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("onboarding apply response is missing `status`"))?;
    let tx_hash_hex = object
        .get("tx_hash_hex")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match status {
        "Unchanged" if tx_hash_hex.is_none() => Ok(OnboardingApplyResult {
            tx_hash_hex: None,
            unchanged: true,
        }),
        "Queued" | "Repaired" => {
            let tx_hash = tx_hash_hex
                .as_deref()
                .ok_or_else(|| eyre!("queued onboarding apply response is missing tx_hash_hex"))?;
            let decoded = hex::decode(tx_hash).wrap_err("onboarding tx_hash_hex is not hex")?;
            if decoded.len() != 32 || tx_hash != tx_hash.to_ascii_lowercase() {
                return Err(eyre!(
                    "onboarding tx_hash_hex must be canonical lowercase 32-byte hex"
                ));
            }
            Ok(OnboardingApplyResult {
                tx_hash_hex,
                unchanged: false,
            })
        }
        _ => Err(eyre!(
            "unexpected onboarding apply status `{status}` or transaction hash shape"
        )),
    }
}
fn validate_faucet_response(
    response: Option<&Value>,
    expected_account: &AccountId,
    expected_asset_definition_id: &str,
) -> Result<String> {
    let object = response
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("response must be a JSON object"))?;
    let required_string = |key: &str| -> Result<&str> {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| eyre!("missing non-empty `{key}`"))
    };
    let account_id = required_string("account_id")?;
    if account_id != expected_account.to_string() {
        return Err(eyre!(
            "unexpected account_id; expected {expected_account}, actual {account_id}"
        ));
    }
    let asset_definition_id = required_string("asset_definition_id")?;
    if !expected_asset_definition_id.is_empty()
        && asset_definition_id != expected_asset_definition_id
    {
        return Err(eyre!(
            "unexpected asset_definition_id; expected {expected_asset_definition_id}, actual {asset_definition_id}"
        ));
    }
    for key in ["asset_id", "amount"] {
        required_string(key)?;
    }
    let status = required_string("status")?;
    if status != "QUEUED" {
        return Err(eyre!(
            "unexpected faucet status `{status}`; expected QUEUED"
        ));
    }
    let tx_hash = required_string("tx_hash_hex")?;
    let decoded = hex::decode(tx_hash).wrap_err("faucet tx_hash_hex is not hex")?;
    if decoded.len() != 32 {
        return Err(eyre!(
            "faucet tx_hash_hex must encode 32 bytes, got {}",
            decoded.len()
        ));
    }
    Ok(tx_hash.to_owned())
}
fn pipeline_status_kind(response: Option<&Value>) -> Option<String> {
    let status = response
        .and_then(Value::as_object)
        .and_then(|object| object.get("status"))?;
    status
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}
fn pipeline_status_is_terminal(response: Option<&Value>) -> bool {
    matches!(
        pipeline_status_kind(response).as_deref(),
        Some("Applied" | "Rejected" | "Expired")
    )
}
fn wait_for_pipeline_terminal_status(
    http: &HttpClient,
    public_root: &str,
    tx_hash_hex: &str,
    timeout: Duration,
) -> Result<HttpJson> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut url = join_url(public_root, "/v1/pipeline/transactions/status")?;
        url.query_pairs_mut()
            .append_pair("hash", tx_hash_hex)
            .append_pair("scope", "global");
        let mut response = http_json(http, reqwest::Method::GET, url.as_str(), None)?;
        if response.status != 200 && response.status != 404 {
            return Ok(response);
        }
        if pipeline_status_is_terminal(response.body.as_ref()) {
            return Ok(response);
        }
        if Instant::now() >= deadline {
            response.status = 504;
            return Ok(response);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
fn claim_faucet(
    http: &HttpClient,
    public_root: &str,
    account_id: &AccountId,
    expected_network_id: &NetworkId,
) -> Result<HttpJson> {
    let puzzle_url = join_url(public_root, "/v1/accounts/faucet/puzzle")?;
    let puzzle = http_json(http, reqwest::Method::GET, puzzle_url.as_str(), None)?;
    if puzzle.status != 200 {
        return Ok(puzzle);
    }
    let Some(puzzle_body) = puzzle.body.as_ref() else {
        return Ok(HttpJson {
            status: 502,
            body: Some(error_value(
                "invalid_faucet_puzzle",
                "faucet puzzle response was not JSON",
            )),
            text: puzzle.text,
        });
    };
    let claim_body =
        solve_faucet_puzzle(&account_id.to_string(), expected_network_id, puzzle_body)?;
    let claim_url = join_url(public_root, "/v1/accounts/faucet")?;
    http_json(
        http,
        reqwest::Method::POST,
        claim_url.as_str(),
        Some(&claim_body),
    )
}
fn solve_faucet_puzzle(
    account_id: &str,
    expected_network_id: &NetworkId,
    puzzle: &Value,
) -> Result<Value> {
    let algorithm = required_str(puzzle, "algorithm")?;
    if algorithm != FAUCET_POW_ALGORITHM {
        eyre::bail!(
            "unsupported faucet puzzle algorithm `{algorithm}`; expected `{FAUCET_POW_ALGORITHM}`"
        );
    }
    let network_id = validate_taira_puzzle_identity(puzzle, expected_network_id)?;
    let difficulty_bits = required_u64(puzzle, "difficulty_bits")?;
    if difficulty_bits == 0 {
        eyre::bail!("faucet puzzle difficulty_bits must be positive");
    }
    let mut body = Map::new();
    body.insert("account_id".into(), Value::String(account_id.to_owned()));
    let anchor_height = required_u64(puzzle, "anchor_height")?;
    if anchor_height == 0 {
        eyre::bail!("faucet puzzle anchor_height must be positive");
    }
    let anchor_hash_hex = required_str(puzzle, "anchor_block_hash_hex")?;
    let challenge_salt_hex = optional_str(puzzle, "challenge_salt_hex");
    let log_n = u8::try_from(required_u64(puzzle, "scrypt_log_n")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_log_n is too large"))?;
    let r = u32::try_from(required_u64(puzzle, "scrypt_r")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_r is too large"))?;
    let p = u32::try_from(required_u64(puzzle, "scrypt_p")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_p is too large"))?;
    let challenge = build_faucet_challenge(
        account_id,
        &network_id,
        anchor_height,
        anchor_hash_hex,
        challenge_salt_hex,
    )?;
    let params = ScryptParams::new(log_n, r, p, 32)
        .map_err(|err| eyre!("invalid faucet scrypt parameters: {err}"))?;
    let difficulty_bits =
        u32::try_from(difficulty_bits).map_err(|_| eyre!("faucet difficulty is too large"))?;
    let nonce = solve_faucet_pow(&challenge, &params, difficulty_bits)?;
    body.insert("pow_anchor_height".into(), Value::from(anchor_height));
    body.insert("pow_nonce_hex".into(), Value::String(hex::encode(nonce)));
    Ok(Value::Object(body))
}
fn validate_taira_puzzle_identity(
    puzzle: &Value,
    expected_network_id: &NetworkId,
) -> Result<NetworkId> {
    let network_id_literal = required_str(puzzle, "network_id")?;
    let network_id = network_id_literal
        .parse::<NetworkId>()
        .wrap_err("faucet puzzle network_id is not a canonical NetworkId")?;
    if network_id.to_string() != network_id_literal {
        eyre::bail!("faucet puzzle network_id is not canonically encoded");
    }
    if &network_id != expected_network_id {
        eyre::bail!(
            "faucet puzzle network_id `{network_id}` does not match configured network `{expected_network_id}`"
        );
    }
    let chain_discriminant = u16::try_from(required_u64(puzzle, "chain_discriminant")?)
        .map_err(|_| eyre!("faucet puzzle chain_discriminant is too large"))?;
    if chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT {
        eyre::bail!(
            "faucet puzzle chain_discriminant `{chain_discriminant}` does not match Taira `{DEFAULT_CHAIN_DISCRIMINANT}`"
        );
    }
    Ok(network_id)
}
fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("faucet puzzle missing numeric `{key}`"))
}
fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("faucet puzzle missing string `{key}`"))
}
fn optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
}
fn build_faucet_challenge(
    account_id: &str,
    network_id: &NetworkId,
    anchor_height: u64,
    anchor_hash_hex: &str,
    challenge_salt_hex: Option<&str>,
) -> Result<[u8; 32]> {
    let anchor_hash = decode_exact_lower_hex(anchor_hash_hex, "anchor_block_hash_hex", 32)?;
    let mut hasher = Sha256::new();
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
    hasher.update(network_id.as_bytes());
    hasher.update(account_id.as_bytes());
    hasher.update(anchor_height.to_be_bytes());
    hasher.update(anchor_hash);
    if let Some(salt) = challenge_salt_hex {
        let salt = decode_exact_lower_hex(salt, "challenge_salt_hex", 32)?;
        hasher.update(salt);
    }
    Ok(hasher.finalize().into())
}
fn decode_exact_lower_hex(value: &str, field: &str, byte_length: usize) -> Result<Vec<u8>> {
    if value.len() != byte_length.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        eyre::bail!(
            "faucet puzzle {field} must be an exact lowercase {byte_length}-byte hex string"
        );
    }
    hex::decode(value).wrap_err_with(|| format!("invalid faucet puzzle {field}"))
}
fn solve_faucet_pow(
    challenge: &[u8; 32],
    params: &ScryptParams,
    difficulty_bits: u32,
) -> Result<[u8; 8]> {
    for nonce in 0_u64..(1_u64 << 63) {
        let nonce_bytes = nonce.to_be_bytes();
        let mut digest = [0_u8; 32];
        derive_scrypt(&nonce_bytes, challenge, params, &mut digest)
            .map_err(|err| eyre!("failed faucet scrypt derivation: {err}"))?;
        if leading_zero_bits(&digest) >= difficulty_bits {
            return Ok(nonce_bytes);
        }
    }
    eyre::bail!("faucet PoW nonce space exhausted")
}
fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut total = 0_u32;
    for byte in bytes {
        if *byte == 0 {
            total += 8;
            continue;
        }
        total += byte.leading_zeros();
        break;
    }
    total
}
fn insert_string_metadata(metadata: &mut Metadata, key: &str, value: &str) -> Result<()> {
    metadata.insert(Name::from_str(key)?, IrohaJson::new(value.to_owned()));
    Ok(())
}
fn canary_message() -> Result<String> {
    canary_message_at(SystemTime::now())
}
fn canary_message_at(now: SystemTime) -> Result<String> {
    let unix_ms = now
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock predates the Unix epoch; refusing a defaulted canary message")?
        .as_millis();
    Ok(format!("taira-write-canary-{unix_ms}"))
}
fn write_runtime_config(path: &Path, config: &Config) -> Result<()> {
    let rendered = Zeroizing::new(render_runtime_config(config)?);
    write_private_runtime_config(path, rendered.as_bytes())
}
#[cfg(unix)]
fn write_private_runtime_config(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{ffi::OsString, os::unix::fs::MetadataExt as _, path::Component};

    if !path.is_absolute() {
        eyre::bail!("runtime config path must be absolute");
    }
    let parent_path = path
        .parent()
        .ok_or_else(|| eyre!("config path `{}` has no parent", path.display()))?;
    let target_name: OsString = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| eyre!("runtime config path must have one exact file name"))?
        .to_owned();
    if parent_path.join(&target_name) != path
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        eyre::bail!("runtime config path must be absolute and lexically canonical");
    }
    let canonical_parent = fs::canonicalize(parent_path)
        .wrap_err_with(|| format!("failed to resolve `{}`", parent_path.display()))?;
    if canonical_parent != parent_path {
        eyre::bail!("runtime config parent must be canonical and symlink-free");
    }
    if canonical_parent
        .ancestors()
        .any(|ancestor| fs::symlink_metadata(ancestor.join(".git")).is_ok())
    {
        eyre::bail!("runtime config must not be persisted inside a Git working tree");
    }

    let effective_uid = rustix::process::geteuid().as_raw();
    for ancestor in canonical_parent.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)
            .wrap_err_with(|| format!("failed to inspect `{}`", ancestor.display()))?;
        if !metadata.file_type().is_dir()
            || (metadata.uid() != 0 && metadata.uid() != effective_uid)
            || metadata.mode() & 0o022 != 0
        {
            eyre::bail!(
                "runtime config ancestry has unsafe custody at `{}`",
                ancestor.display()
            );
        }
    }
    let parent_metadata = fs::symlink_metadata(&canonical_parent)?;
    if parent_metadata.uid() != effective_uid || parent_metadata.mode() & 0o7777 != 0o700 {
        eyre::bail!("runtime config parent must be owned by the current user with mode 0700");
    }
    let parent = File::from(
        rustix::fs::open(
            &canonical_parent,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .wrap_err("failed to securely open runtime config parent")?,
    );
    let opened_parent = parent.metadata()?;
    if opened_parent.dev() != parent_metadata.dev()
        || opened_parent.ino() != parent_metadata.ino()
        || opened_parent.uid() != effective_uid
        || opened_parent.mode() & 0o7777 != 0o700
    {
        eyre::bail!("runtime config parent custody changed during secure open");
    }
    match rustix::fs::statat(&parent, &target_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Err(error) if error == rustix::io::Errno::NOENT => {}
        Ok(_) => eyre::bail!("runtime config destination already exists and will not be replaced"),
        Err(error) => return Err(error).wrap_err("failed to inspect runtime config destination"),
    }

    let mut output = File::from(
        rustix::fs::openat(
            &parent,
            &target_name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .wrap_err("failed to create private runtime config")?,
    );
    rustix::fs::fchmod(&output, rustix::fs::Mode::from_raw_mode(0o600))?;
    let created = output.metadata()?;
    let identity = (created.dev(), created.ino());
    let publication = (|| -> Result<()> {
        if !created.is_file()
            || created.uid() != effective_uid
            || created.nlink() != 1
            || created.mode() & 0o7777 != 0o600
            || created.len() != 0
        {
            eyre::bail!("new runtime config has unsafe initial custody");
        }
        output.write_all(bytes)?;
        output.sync_all()?;
        let complete = output.metadata()?;
        if (complete.dev(), complete.ino()) != identity
            || complete.uid() != effective_uid
            || complete.nlink() != 1
            || complete.mode() & 0o7777 != 0o600
            || complete.len() != u64::try_from(bytes.len())?
        {
            eyre::bail!("runtime config custody changed during publication");
        }
        parent.sync_all()?;
        Ok(())
    })();
    if let Err(error) = publication {
        drop(output);
        let named =
            rustix::fs::statat(&parent, &target_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW);
        if let Ok(named) = named
            && (u64::try_from(named.st_dev).ok(), named.st_ino) == (Some(identity.0), identity.1)
        {
            rustix::fs::unlinkat(&parent, &target_name, rustix::fs::AtFlags::empty())?;
            parent.sync_all()?;
        }
        return Err(error).wrap_err("private runtime config publication failed");
    }
    Ok(())
}
#[cfg(not(unix))]
fn write_private_runtime_config(_path: &Path, _bytes: &[u8]) -> Result<()> {
    eyre::bail!("private runtime config persistence requires Unix descriptor APIs")
}
fn render_runtime_config(config: &Config) -> Result<String> {
    let private_key = ExposedPrivateKey(config.key_pair.private_key().clone()).to_string();
    let public_key = config.key_pair.public_key().to_string();
    Ok(format!(
        "chain = \"{}\"\ntorii_url = \"{}\"\n\n[account]\ndomain = \"wonderland.universal\"\npublic_key = \"{}\"\nprivate_key = \"{}\"\nchain_discriminant = {}\n\n[transaction]\ntime_to_live_ms = {}\nstatus_timeout_ms = {}\nnonce = false\n",
        escape_toml(&config.chain.to_string()),
        escape_toml(config.torii_api_url.as_str()),
        escape_toml(&public_key),
        escape_toml(&private_key),
        config.account_chain_discriminant,
        DEFAULT_WRITE_TTL_MS,
        DEFAULT_WRITE_STATUS_TIMEOUT_MS,
    ))
}
fn escape_toml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn hint_submit_error(err: eyre::Report) -> eyre::Report {
    let text = format!("{err:#}");
    if text.contains("fee intent") || text.contains("fee_payment") {
        eyre!(
            "{text}\nTaira requires the exact signature-bound `FeePaymentIntent` returned by `/v1/fees/quote`; re-quote after any payload or sponsor-program revision change."
        )
    } else if text.contains("route_unavailable") || text.contains("ROUTE_UNRESOLVED") {
        eyre!(
            "{text}\nTaira accepted the request at ingress but no authoritative peer accepted the route; treat this as ingress or lane routing first."
        )
    } else if text.contains("Failed to find asset") {
        eyre!(
            "{text}\nThe canary signer appears unfunded for the fee asset; re-run the command so the faucet path can claim starter funds."
        )
    } else {
        err
    }
}
fn hint_wait_error(err: eyre::Report) -> eyre::Report {
    let text = format!("{err:#}");
    if text.contains("Expired") || text.contains("expired") {
        eyre!(
            "{text}\nThe canary transaction expired before application; inspect /status queue depth and validator health."
        )
    } else {
        err
    }
}
fn faucet_failure_hint(response: &HttpJson) -> String {
    let body = response
        .body
        .as_ref()
        .map(compact_json)
        .unwrap_or_else(|| response.text.clone());
    if body.contains("Failed to find asset") {
        format!(
            "faucet claim failed with HTTP {}: faucet asset is missing or not bootstrapped",
            response.status
        )
    } else {
        format!("faucet claim failed with HTTP {}: {body}", response.status)
    }
}
fn compact_json(value: &Value) -> String {
    json::to_json(value).unwrap_or_else(|_| format!("{value:?}"))
}
fn error_value(code: &str, message: &str) -> Value {
    let mut error = Map::new();
    error.insert("error_code".into(), Value::String(code.to_owned()));
    error.insert("message".into(), Value::String(message.to_owned()));
    Value::Object(error)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::{DataSpaceId, nexus::FeeDebitSource};
    use iroha_i18n::{Bundle, Language, Localizer};
    use iroha_torii_shared::{
        FeeQuoteDecision, FeeQuoteObservation, FeeQuoteRequest, FeeQuoteResponse, uri as torii_uri,
    };
    use std::{
        net::{TcpListener, TcpStream},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    use tempfile::NamedTempFile;
    const TEST_ONBOARDING_TOKEN: &str = "0123456789abcdef0123456789ABCDEF";
    fn test_onboarding_token_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("create onboarding token file");
        file.write_all(TEST_ONBOARDING_TOKEN.as_bytes())
            .expect("write onboarding token file");
        file.flush().expect("flush onboarding token file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))
                .expect("set onboarding token file mode");
        }
        file
    }
    #[derive(Clone)]
    struct MockRequest {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    }
    impl std::fmt::Debug for MockRequest {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("MockRequest")
                .field("method", &self.method)
                .field("path", &self.path)
                .field(
                    "header_names",
                    &self
                        .headers
                        .iter()
                        .map(|(name, _)| name)
                        .collect::<Vec<_>>(),
                )
                .field("body", &self.body)
                .finish()
        }
    }
    impl MockRequest {
        fn header_values(&self, name: &str) -> Vec<&str> {
            self.headers
                .iter()
                .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
                .collect()
        }
    }
    struct MockResponse {
        status: u16,
        content_type: &'static str,
        headers: Vec<(&'static str, String)>,
        body: String,
    }
    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    struct TextContext {
        cfg: Config,
        i18n: Localizer,
        lines: Vec<String>,
    }
    impl TextContext {
        fn new() -> Self {
            Self {
                cfg: crate::fallback_config(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
                lines: Vec::new(),
            }
        }
    }
    impl RunContext for TextContext {
        fn config(&self) -> &Config {
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
        fn output_format(&self) -> CliOutputFormat {
            CliOutputFormat::Text
        }
        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let _value = norito::json::to_value(data)?;
            Ok(())
        }
        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            self.lines.push(data.to_string());
            Ok(())
        }
    }
    impl MockResponse {
        fn json(status: u16, value: Value) -> Self {
            Self {
                status,
                content_type: "application/json",
                headers: Vec::new(),
                body: json::to_json(&value).expect("mock JSON response"),
            }
        }
        fn text(status: u16, body: impl Into<String>) -> Self {
            Self {
                status,
                content_type: "text/plain",
                headers: Vec::new(),
                body: body.into(),
            }
        }
    }
    struct MockHttpServer {
        base_url: String,
        requests: Arc<Mutex<Vec<MockRequest>>>,
        stop: Arc<AtomicBool>,
        handle: thread::JoinHandle<()>,
    }
    fn spawn_mock_http<F>(expected_requests: usize, responder: F) -> MockHttpServer
    where
        F: Fn(&MockRequest) -> MockResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("mock server address");
        listener
            .set_nonblocking(true)
            .expect("set mock server nonblocking");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let responder = Arc::new(responder);
        let handle = thread::spawn(move || {
            let mut accepted = 0_usize;
            while accepted < expected_requests && !server_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_nonblocking(false)
                            .expect("set accepted mock stream blocking");
                        let request = read_mock_request(&mut stream);
                        let response = responder(&request);
                        server_requests
                            .lock()
                            .expect("requests")
                            .push(request.clone());
                        write_mock_response(&mut stream, response);
                        accepted += 1;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("mock server accept failed: {error}"),
                }
            }
        });
        MockHttpServer {
            base_url: format!("http://{addr}"),
            requests,
            stop,
            handle,
        }
    }
    fn read_mock_request(stream: &mut TcpStream) -> MockRequest {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("mock stream timeout");
        let mut raw = Vec::new();
        let mut buf = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buf).expect("read mock request");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..read]);
            if let Some(header_end) = find_header_end(&raw) {
                let headers = String::from_utf8_lossy(&raw[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if raw.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        let header_end = find_header_end(&raw).expect("mock request header end");
        let headers = String::from_utf8_lossy(&raw[..header_end]);
        let request_line = headers.lines().next().expect("request line");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().expect("method").to_owned();
        let path = parts.next().expect("path").to_owned();
        let parsed_headers = headers
            .lines()
            .skip(1)
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_owned(), value.trim().to_owned()))
            })
            .collect();
        let body = String::from_utf8_lossy(&raw[header_end + 4..]).to_string();
        MockRequest {
            method,
            path,
            headers: parsed_headers,
            body,
        }
    }
    fn find_header_end(raw: &[u8]) -> Option<usize> {
        raw.windows(4).position(|window| window == b"\r\n\r\n")
    }
    fn write_mock_response(stream: &mut TcpStream, response: MockResponse) {
        let reason = match response.status {
            200 => "OK",
            202 => "Accepted",
            307 => "Temporary Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            404 => "Not Found",
            503 => "Service Unavailable",
            _ => "OK",
        };
        let body = response.body.as_bytes();
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            reason,
            response.content_type,
            body.len()
        )
        .expect("write mock response headers");
        for (name, value) in response.headers {
            write!(stream, "{name}: {value}\r\n").expect("write mock response header");
        }
        write!(stream, "\r\n").expect("finish mock response headers");
        stream.write_all(body).expect("write mock response body");
    }
    fn finish_mock(server: MockHttpServer) -> Vec<MockRequest> {
        server.stop.store(true, Ordering::Release);
        server.handle.join().expect("mock server thread");
        Arc::try_unwrap(server.requests)
            .expect("request references")
            .into_inner()
            .expect("requests")
    }
    fn path_only(path: &str) -> &str {
        path.split_once('?').map_or(path, |(path, _)| path)
    }
    fn doctor_mock_response(request: &MockRequest, omit_tool: Option<&str>) -> MockResponse {
        match (request.method.as_str(), path_only(&request.path)) {
            ("GET", "/status") => MockResponse::json(
                200,
                norito::json!({
                    "txs_rejected_recent_5m": 0,
                    "queue_size": 0
                }),
            ),
            ("GET", "/v1/time/now") => MockResponse::json(
                200,
                norito::json!({
                    "now": 1_785_168_000_000_u64,
                    "offset_ms": 0,
                    "confidence_ms": 1,
                    "sample_count": 3,
                    "peer_count": 3,
                    "enforcement_mode": "reject",
                    "fallback": false,
                    "health": {
                        "healthy": true,
                        "min_samples_ok": true,
                        "offset_ok": true,
                        "confidence_ok": true
                    }
                }),
            ),
            ("GET", "/v1/sumeragi/status") => MockResponse::json(
                401,
                norito::json!({
                    "code": "operator_signature_missing",
                    "message": "missing required operator signature header `x-iroha-operator-public-key`"
                }),
            ),
            ("GET", "/v1/contracts/state") => {
                MockResponse::json(400, norito::json!({"error": "missing selector"}))
            }
            ("GET", "/v1/pipeline/transactions/status") => {
                MockResponse::json(400, norito::json!({"error": "missing transaction hash"}))
            }
            ("GET", "/v1/transactions/status") => MockResponse::text(404, "not found"),
            ("POST", "/v1/musubi/queries/ordered-prefix") => MockResponse::json(
                401,
                norito::json!({
                    "code": "canonical_authentication_required",
                    "message": "canonical account request authentication is required"
                }),
            ),
            ("GET", "/v1/soracloud/status") => MockResponse::json(
                401,
                norito::json!({
                    "code": "canonical_authentication_required",
                    "message": "canonical account request authentication is required"
                }),
            ),
            ("GET", "/v1/mcp") => MockResponse::json(200, norito::json!({"ok": true})),
            ("POST", "/v1/mcp") if request.body.contains("tools/list") => {
                let tools: Vec<Value> = REQUIRED_MCP_TOOLS
                    .iter()
                    .copied()
                    .filter(|name| Some(*name) != omit_tool)
                    .map(|name| norito::json!({ "name": name, "description": "mock" }))
                    .collect();
                MockResponse::json(
                    200,
                    norito::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": { "tools": tools }
                    }),
                )
            }
            ("POST", "/v1/mcp") => MockResponse::json(
                200,
                norito::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {}
                    }
                }),
            ),
            ("GET", _) => MockResponse::json(200, norito::json!({"ok": true})),
            _ => MockResponse::text(404, "not found"),
        }
    }
    fn write_canary_mock_response(
        request: &MockRequest,
        onboarding_status: u16,
        capabilities_status: u16,
    ) -> MockResponse {
        // Account formatting is guarded per thread.  The mock responder runs
        // on its own thread, so mirror the Taira discriminant used by the
        // canary client before deriving the response account identifier.
        let _guard = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
        match (request.method.as_str(), path_only(&request.path)) {
            ("POST", "/v1/accounts/onboard/plan") => {
                if onboarding_status == 400 {
                    MockResponse::json(
                        400,
                        norito::json!({
                            "error_code": "account_already_exists",
                            "message": "account already exists",
                            "hint": "retry onboarding with a fresh runtime signer"
                        }),
                    )
                } else {
                    let request_body =
                        json::from_str::<Value>(&request.body).expect("decode onboarding request");
                    let request_object = request_body.as_object().expect("onboarding object");
                    let alias = request_object
                        .get("alias")
                        .and_then(Value::as_str)
                        .expect("onboarding alias")
                        .to_owned();
                    let account_id = request_object
                        .get("account_id")
                        .and_then(Value::as_str)
                        .expect("onboarding account id")
                        .to_owned();
                    MockResponse::json(
                        200,
                        norito::json!({
                            "body": {
                                "version": 1,
                                "request": request_body,
                                "authority": (account_id),
                                "chain_id": (DEFAULT_CHAIN_ID),
                                "anchor": { "block_height": 1, "block_hash": ("11".repeat(32)) },
                                "resource": { "intent": { "alias": (alias) }, "disposition": { "kind": "create" } },
                                "acquisition": { "term_years": 1, "pricing_class_hint": null },
                                "quote_guard": {
                                    "expected_policy_version": 1,
                                    "expected_payment_asset": (DEFAULT_GAS_ASSET_ID),
                                    "max_amount": "1",
                                    "valid_until_ms": (u64::MAX)
                                },
                                "instructions": [],
                                "owner_auto_renew_instruction": null,
                                "valid_until_ms": (u64::MAX)
                            },
                            "plan_hash": ("22".repeat(32)),
                            "signature": ("33".repeat(64))
                        }),
                    )
                }
            }
            ("POST", "/v1/accounts/onboard") => {
                let apply_body =
                    json::from_str::<Value>(&request.body).expect("decode onboarding apply");
                let canonical_request = apply_body
                    .as_object()
                    .and_then(|object| object.get("receipt"))
                    .and_then(Value::as_object)
                    .and_then(|receipt| receipt.get("body"))
                    .and_then(Value::as_object)
                    .and_then(|body| body.get("request"))
                    .and_then(Value::as_object)
                    .expect("receipt canonical request");
                let alias = canonical_request
                    .get("alias")
                    .and_then(Value::as_str)
                    .expect("receipt alias");
                let account_id = canonical_request
                    .get("account_id")
                    .and_then(Value::as_str)
                    .expect("receipt account id");
                MockResponse::json(
                    202,
                    norito::json!({
                        "account_id": (account_id),
                        "alias": (alias),
                        "tx_hash_hex": ("ab".repeat(32)),
                        "status": "Queued",
                        "disposition": { "kind": "create" }
                    }),
                )
            }
            ("GET", "/v1/accounts/faucet/puzzle") => MockResponse::json(
                200,
                norito::json!({
                    "algorithm": FAUCET_POW_ALGORITHM,
                    "network_id": (crate::fallback_config().network_id.to_string()),
                    "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
                    "difficulty_bits": 1,
                    "anchor_height": 1,
                    "anchor_block_hash_hex": ("11".repeat(32)),
                    "challenge_salt_hex": null,
                    "scrypt_log_n": 1,
                    "scrypt_r": 1,
                    "scrypt_p": 1
                }),
            ),
            ("POST", "/v1/accounts/faucet") => {
                let request_body =
                    json::from_str::<Value>(&request.body).expect("decode faucet request");
                let account_id = request_body
                    .as_object()
                    .and_then(|object| object.get("account_id"))
                    .and_then(Value::as_str)
                    .expect("faucet account_id");
                MockResponse::json(
                    202,
                    norito::json!({
                        "account_id": (account_id),
                        "asset_definition_id": (DEFAULT_GAS_ASSET_ID),
                        "asset_id": (format!("{DEFAULT_GAS_ASSET_ID}#{account_id}")),
                        "amount": "1000000000000000000",
                        "tx_hash_hex": ("cd".repeat(32)),
                        "status": "QUEUED"
                    }),
                )
            }
            ("GET", "/v1/node/capabilities") if capabilities_status == 200 => MockResponse::json(
                200,
                norito::json!({
                    "data_model_version": (iroha::data_model::DATA_MODEL_VERSION),
                    "signed_transaction_schema_hash_hex": (hex::encode(
                        <iroha::data_model::transaction::SignedTransaction as norito::core::NoritoSerialize>::schema_hash()
                    ))
                }),
            ),
            ("GET", "/v1/node/capabilities") => {
                MockResponse::text(capabilities_status, "capabilities unavailable")
            }
            ("POST", path) if path == torii_uri::FEES_QUOTE => {
                let request = json::from_str::<FeeQuoteRequest>(&request.body)
                    .expect("decode fee quote request");
                let response = FeeQuoteResponse {
                    intent: request.payload.fee_payment,
                    observation: FeeQuoteObservation {
                        ledger_time_ms: 1,
                        next_block_height: 42,
                        route_dataspace_id: DataSpaceId::UNIVERSAL,
                    },
                    components: Vec::new(),
                    capacities: Vec::new(),
                    decision: FeeQuoteDecision::Accepted {
                        debit_source: FeeDebitSource::Account(request.payload.authority),
                        program_revision: None,
                    },
                };
                MockResponse::json(
                    200,
                    json::to_value(&response).expect("encode fee quote response"),
                )
            }
            ("POST", path) if path == torii_uri::TRANSACTION => MockResponse::text(200, ""),
            ("GET", "/v1/pipeline/transactions/status") => {
                let hash = Url::parse(&format!("http://localhost{}", request.path))
                    .ok()
                    .and_then(|url| {
                        url.query_pairs()
                            .find(|(key, _)| key == "hash")
                            .map(|(_, value)| value.to_string())
                    })
                    .unwrap_or_else(|| "mockhash".to_owned());
                MockResponse::json(
                    200,
                    norito::json!({
                        "hash": (hash),
                        "status": { "kind": "Applied", "block_height": 42 },
                        "scope": "local",
                        "resolved_from": "state"
                    }),
                )
            }
            ("POST", path) if path == torii_uri::QUERY => {
                MockResponse::text(404, "query unavailable in mock")
            }
            _ => MockResponse::text(404, "not found"),
        }
    }
    fn inrou_canary_deployment(
        mode: &str,
        version: &str,
    ) -> crate::soracloud::TairaInrouCanaryDeployment {
        crate::soracloud::TairaInrouCanaryDeployment {
            service_name: "taira_inrou_canary".to_owned(),
            service_version: version.to_owned(),
            service_manifest_hash: "service-manifest-hash".to_owned(),
            container_manifest_hash: "container-manifest-hash".to_owned(),
            route_host: "taira-inrou-canary.sora".to_owned(),
            route_path_prefix: "/api/v1".to_owned(),
            healthcheck_path: "/health".to_owned(),
            mutation_mode: mode.to_owned(),
            bundle_hash: "bundle-hash".to_owned(),
            bundle_content_cid: "bundle-cid".to_owned(),
            bundle_manifest_digest_hex: "11".repeat(32),
            guest_content_cid: "guest-cid".to_owned(),
            guest_manifest_digest_hex: "22".repeat(32),
            submitted_tx_hash: json::to_value(&iroha_crypto::Hash::new(
                b"taira-inrou-test-submitted-transaction",
            ))
            .expect("encode submitted transaction hash")
            .as_str()
            .expect("transaction hash encodes as string")
            .to_owned(),
            mutation_response_digest: "response-hash".to_owned(),
        }
    }
    fn exact_inrou_status(version: &str, action: &str, revision_count: u64) -> Value {
        norito::json!({
            "schema_version": 1,
            "service_health": { "status": "healthy" },
            "runtime_manager": { "available": true },
            "hosted_http_topology": {
                "active_capability_adverts": 4,
                "hosted_replica_count": 4
            },
            "control_plane": {
                "services": [{
                    "service_name": "taira_inrou_canary",
                    "current_version": version,
                    "revision_count": revision_count,
                    "active_rollout": null,
                    "last_rollout": null,
                    "latest_revision": {
                        "action": { "action": action, "value": null },
                        "service_version": version,
                        "service_manifest_hash": "service-manifest-hash",
                        "container_manifest_hash": "container-manifest-hash",
                        "replicas": 4,
                        "runtime": { "runtime": "Inrou", "value": null },
                        "execution_plane": {
                            "execution_plane": "HttpService",
                            "value": null
                        },
                        "route_host": "taira-inrou-canary.sora",
                        "route_path_prefix": "/api/v1",
                        "healthcheck_path": "/health",
                        "process_generation": 1
                    }
                }]
            }
        })
    }
    #[test]
    fn exact_inrou_status_requires_distinct_promoted_upgrade() {
        let deployed_version = format!("artifact-{}", "11".repeat(32));
        let upgraded_version = format!("artifact-{}", "22".repeat(32));
        let deploy = inrou_canary_deployment("deploy", &deployed_version);
        let deploy_status = exact_inrou_status(&deployed_version, "Deploy", 1);
        assert!(validate_exact_inrou_canary_status(&deploy_status, &deploy).is_ok());
        let mut missing_schema = deploy_status.clone();
        missing_schema
            .as_object_mut()
            .expect("status fixture is an object")
            .remove("schema_version");
        assert!(validate_exact_inrou_canary_status(&missing_schema, &deploy).is_err());
        for missing_field in ["active_capability_adverts", "hosted_replica_count"] {
            let mut missing = deploy_status.clone();
            missing
                .pointer_mut("/hosted_http_topology")
                .and_then(Value::as_object_mut)
                .expect("status fixture has hosted topology")
                .remove(missing_field);
            assert!(
                validate_exact_inrou_canary_status(&missing, &deploy).is_err(),
                "missing {missing_field} must fail closed"
            );
        }
        let mut duplicate_service = deploy_status.clone();
        let services = duplicate_service
            .pointer_mut("/control_plane/services")
            .and_then(Value::as_array_mut)
            .expect("status fixture has services");
        let duplicate = services[0].clone();
        services.push(duplicate);
        assert!(
            validate_exact_inrou_canary_status(&duplicate_service, &deploy).is_err(),
            "duplicate authoritative service rows must fail closed"
        );
        for missing_field in ["active_rollout", "last_rollout"] {
            let mut missing = deploy_status.clone();
            missing
                .pointer_mut("/control_plane/services/0")
                .and_then(Value::as_object_mut)
                .expect("status fixture has one service")
                .remove(missing_field);
            assert!(
                validate_exact_inrou_canary_status(&missing, &deploy).is_err(),
                "missing {missing_field} must fail closed"
            );
        }
        let mut missing_revision_version = deploy_status.clone();
        missing_revision_version
            .pointer_mut("/control_plane/services/0/latest_revision")
            .and_then(Value::as_object_mut)
            .expect("status fixture has a latest revision")
            .remove("service_version");
        assert!(
            validate_exact_inrou_canary_status(&missing_revision_version, &deploy).is_err(),
            "latest revision must carry its exact service version"
        );
        for (path, retired) in [
            ("/control_plane/services/0/latest_revision/action", "Deploy"),
            ("/control_plane/services/0/latest_revision/runtime", "Inrou"),
            (
                "/control_plane/services/0/latest_revision/execution_plane",
                "HttpService",
            ),
        ] {
            let mut bare = deploy_status.clone();
            *bare
                .pointer_mut(path)
                .expect("status fixture has a tagged enum") = Value::from(retired);
            assert!(
                validate_exact_inrou_canary_status(&bare, &deploy).is_err(),
                "bare-string enum at {path} must fail closed"
            );
        }
        let mismatched_deploy = inrou_canary_deployment("deploy", &upgraded_version);
        assert!(
            validate_exact_inrou_canary_status(&deploy_status, &mismatched_deploy)
                .is_err()
        );

        let upgrade = inrou_canary_deployment("upgrade", &upgraded_version);
        let mut upgrade_status = exact_inrou_status(&upgraded_version, "Upgrade", 2);
        upgrade_status
            .pointer_mut("/control_plane/services/0")
            .and_then(Value::as_object_mut)
            .expect("status fixture has one service")
            .insert(
                "last_rollout".to_owned(),
                norito::json!({
                    "baseline_version": (deployed_version.clone()),
                    "candidate_version": (upgraded_version.clone()),
                    "canary_percent": 100,
                    "traffic_percent": 100,
                    "stage": { "stage": "Promoted", "value": null }
                }),
            );
        assert!(validate_exact_inrou_canary_status(&upgrade_status, &upgrade).is_ok());

        let mut bare_stage = upgrade_status.clone();
        *bare_stage
            .pointer_mut("/control_plane/services/0/last_rollout/stage")
            .expect("status fixture has a rollout stage") = Value::from("Promoted");
        assert!(
            validate_exact_inrou_canary_status(&bare_stage, &upgrade).is_err(),
            "bare-string rollout stage must fail closed"
        );

        let mut stale = upgrade_status.clone();
        *stale
            .pointer_mut("/control_plane/services/0/last_rollout/candidate_version")
            .expect("status fixture has a rollout candidate") =
            Value::from(deployed_version);
        assert!(validate_exact_inrou_canary_status(&stale, &upgrade).is_err());
        let mut extra_host = upgrade_status;
        *extra_host
            .pointer_mut("/hosted_http_topology/active_capability_adverts")
            .expect("status fixture has an advert count") = Value::from(5_u64);
        assert!(validate_exact_inrou_canary_status(&extra_host, &upgrade).is_err());
    }
    #[test]
    fn tagged_enum_name_requires_exact_tagged_unit_envelope() {
        let canonical = norito::json!({"runtime": "Inrou", "value": null});
        assert_eq!(tagged_enum_name(&canonical, "runtime"), Some("Inrou"));
        for retired in [
            Value::from("Inrou"),
            norito::json!({"runtime": "Inrou"}),
            norito::json!({"runtime": "Inrou", "value": {}}),
            norito::json!({"runtime": "Inrou", "value": null, "legacy": true}),
        ] {
            assert_eq!(
                tagged_enum_name(&retired, "runtime"),
                None,
                "noncanonical tagged enum must fail: {retired:?}"
            );
        }
    }
    #[test]
    fn doctor_soracloud_status_rejects_bare_string_enum_aliases() {
        let response = doctor_mock_response(
            &MockRequest {
                method: "GET".to_owned(),
                path: "/v1/soracloud/status".to_owned(),
                headers: Vec::new(),
                body: String::new(),
            },
            None,
        );
        let canonical: Value = json::from_str(&response.body).expect("decode status fixture");
        validate_soracloud_status(Some(&canonical)).expect("canonical tagged status");
        for (path, retired) in [
            ("/control_plane/services/0/latest_revision/runtime", "Inrou"),
            (
                "/control_plane/services/0/latest_revision/execution_plane",
                "HttpService",
            ),
        ] {
            let mut bare = canonical.clone();
            *bare
                .pointer_mut(path)
                .expect("status fixture has a tagged enum") = Value::from(retired);
            assert!(
                validate_soracloud_status(Some(&bare)).is_err(),
                "doctor must reject bare-string enum alias at {path}"
            );
        }
        for field in ["active_capability_adverts", "hosted_replica_count"] {
            let mut missing = canonical.clone();
            missing
                .pointer_mut("/hosted_http_topology")
                .and_then(Value::as_object_mut)
                .expect("status fixture has hosted HTTP topology")
                .remove(field);
            assert_eq!(
                validate_soracloud_status(Some(&missing)),
                Err(format!("/v1/soracloud/status is missing `{field}`")),
                "doctor must not infer a missing authoritative topology count"
            );
        }
    }
    #[test]
    fn write_canary_message_never_defaults_a_pre_epoch_clock() {
        assert_eq!(
            canary_message_at(UNIX_EPOCH + Duration::from_millis(42))
                .expect("post-epoch timestamp must be accepted"),
            "taira-write-canary-42"
        );
        let pre_epoch = UNIX_EPOCH
            .checked_sub(Duration::from_millis(1))
            .expect("one millisecond before the Unix epoch is representable");
        assert!(
            canary_message_at(pre_epoch).is_err(),
            "pre-epoch clocks must fail instead of producing timestamp zero"
        );
    }
    #[test]
    fn inrou_canary_rejects_zero_timeout_before_external_work() {
        assert!(validate_inrou_canary_timeout(1).is_ok());
        let error = validate_inrou_canary_timeout(0)
            .expect_err("zero timeout must fail before canary mutation");
        assert!(error.to_string().contains("must be greater than zero"));
    }
    #[test]
    fn doctor_mock_healthy_flow_reports_ok() {
        let server = spawn_mock_http(15, |request| doctor_mock_response(request, None));
        let report = run_doctor(&server.base_url).expect("doctor report");
        let requests = finish_mock(server);
        let rendered = compact_json(&report);
        assert_eq!(report_status(&report), Some("ok"), "{rendered}");
        assert!(
            requests
                .iter()
                .any(|request| request.method == "POST" && request.body.contains("initialize"))
        );
        assert!(
            requests
                .iter()
                .any(|request| request.method == "POST" && request.body.contains("tools/list"))
        );
        assert!(requests.iter().any(|request| {
            request.method == "GET"
                && path_only(&request.path) == "/v1/pipeline/transactions/status"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "GET" && path_only(&request.path) == "/v1/transactions/status"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "GET" && path_only(&request.path) == "/v1/time/now"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "POST"
                && path_only(&request.path) == "/v1/musubi/queries/ordered-prefix"
                && request.body == "{}"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "GET" && path_only(&request.path) == "/v1/soracloud/status"
        }));
    }
    #[test]
    fn doctor_mock_rejects_noncanonical_soracloud_401() {
        let server = spawn_mock_http(15, |request| {
            if request.method == "GET" && path_only(&request.path) == "/v1/soracloud/status" {
                MockResponse::json(
                    401,
                    norito::json!({
                        "code": "gateway_authentication_required",
                        "message": "canonical account request authentication is required"
                    }),
                )
            } else {
                doctor_mock_response(request, None)
            }
        });
        let report = run_doctor(&server.base_url).expect("doctor report");
        let _requests = finish_mock(server);
        let failures = report
            .as_object()
            .and_then(|object| object.get("failures"))
            .and_then(Value::as_array)
            .expect("failures");
        assert_eq!(report_status(&report), Some("fail"));
        assert!(
            failures
                .iter()
                .filter_map(Value::as_str)
                .any(|failure| { failure.contains("non-canonical authentication") })
        );
    }
    #[test]
    fn doctor_mock_rejects_noncanonical_sumeragi_401() {
        let server = spawn_mock_http(15, |request| {
            if request.method == "GET" && path_only(&request.path) == "/v1/sumeragi/status" {
                MockResponse::json(
                    401,
                    norito::json!({
                        "code": "gateway_authentication_required",
                        "message": "missing required operator signature header `x-iroha-operator-public-key`"
                    }),
                )
            } else {
                doctor_mock_response(request, None)
            }
        });
        let report = run_doctor(&server.base_url).expect("doctor report");
        let _requests = finish_mock(server);
        let failures = report
            .as_object()
            .and_then(|object| object.get("failures"))
            .and_then(Value::as_array)
            .expect("failures");
        assert_eq!(report_status(&report), Some("fail"));
        assert!(
            failures
                .iter()
                .filter_map(Value::as_str)
                .any(|failure| { failure.contains("non-canonical operator") })
        );
    }
    #[test]
    fn canonical_authentication_challenge_rejects_near_misses() {
        let canonical = norito::json!({
            "code": "canonical_authentication_required",
            "message": "canonical account request authentication is required"
        });
        validate_canonical_authentication_challenge(Some(&canonical))
            .expect("exact canonical challenge");

        for challenge in [
            norito::json!({
                "code": "canonical_authentication_required",
                "message": "authentication is required"
            }),
            norito::json!({
                "code": "canonical_authentication_required",
                "message": "canonical account request authentication is required",
                "source": "gateway"
            }),
            norito::json!("canonical_authentication_required"),
        ] {
            validate_canonical_authentication_challenge(Some(&challenge))
                .expect_err("near-miss challenge must fail closed");
        }
    }
    #[test]
    fn operator_signature_authentication_challenge_rejects_near_misses() {
        let canonical = norito::json!({
            "code": "operator_signature_missing",
            "message": "missing required operator signature header `x-iroha-operator-public-key`"
        });
        validate_operator_signature_authentication_challenge(Some(&canonical))
            .expect("exact operator-signature challenge");

        for challenge in [
            norito::json!({
                "code": "operator_signature_missing",
                "message": "operator signature required"
            }),
            norito::json!({
                "code": "operator_signature_missing",
                "message": "missing required operator signature header `x-iroha-operator-public-key`",
                "source": "gateway"
            }),
            norito::json!("operator_signature_missing"),
        ] {
            validate_operator_signature_authentication_challenge(Some(&challenge))
                .expect_err("near-miss operator-signature challenge must fail closed");
        }
    }
    #[test]
    fn inrou_status_probe_uses_canonical_account_authentication() {
        let server = spawn_mock_http(1, |request| {
            assert_eq!(request.method, "GET");
            assert_eq!(path_only(&request.path), "/v1/soracloud/status");
            MockResponse::json(200, norito::json!({ "schema_version": 1 }))
        });
        let key_pair = fixture_key_pair(0x41);
        let mut config = crate::fallback_config();
        config.account = AccountId::new(key_pair.public_key().clone());
        config.key_pair = key_pair;
        config.torii_api_url =
            Url::parse(&format!("{}/", server.base_url)).expect("mock Torii URL");
        let client = IrohaClient::new(config);
        let status = account_signed_soracloud_status(&client).expect("signed status response");
        assert_eq!(status.status, 200);
        assert_eq!(status.body, Some(norito::json!({ "schema_version": 1 })));
        let requests = finish_mock(server);
        let request = requests.first().expect("status request");
        for header in [
            "x-iroha-account",
            "x-iroha-signature",
            "x-iroha-timestamp-ms",
            "x-iroha-nonce",
        ] {
            assert_eq!(
                request.header_values(header).len(),
                1,
                "canonical header {header} must appear exactly once"
            );
        }
        assert_eq!(request.header_values("accept"), vec!["application/json"]);
    }
    #[test]
    fn render_report_text_includes_route_check_detail() {
        let mut checks = Vec::new();
        push_check(
            &mut checks,
            "contracts_state",
            400,
            true,
            route_check_detail(&[400]),
        );
        let report = report_value(
            "taira_doctor",
            "ok",
            DEFAULT_PUBLIC_ROOT,
            checks,
            Vec::new(),
            Vec::new(),
            Map::new(),
        )
        .expect("report");
        let mut context = TextContext::new();
        render_report(&mut context, false, &report).expect("render report");
        let output = context.lines.join("\n");
        assert!(
            output
                .contains("contracts_state HTTP 400 (mounted route is expected to return HTTP 400")
        );
    }
    #[test]
    fn time_snapshot_requires_network_time_and_every_health_axis() {
        let healthy = norito::json!({
            "now": 1_u64,
            "offset_ms": 0,
            "confidence_ms": 0_u64,
            "sample_count": 3_u64,
            "peer_count": 3_u64,
            "enforcement_mode": "reject",
            "fallback": false,
            "health": {
                "healthy": true,
                "min_samples_ok": true,
                "offset_ok": true,
                "confidence_ok": true
            }
        });
        validate_time_snapshot(Some(&healthy)).expect("healthy network time");
        for (label, mutation) in [
            ("fallback", "/v1/time/now is using the local-clock fallback"),
            (
                "samples",
                "/v1/time/now field `sample_count` must be a positive integer",
            ),
            ("health", "/v1/time/now health field `healthy` is not true"),
            (
                "enforcement",
                "/v1/time/now is not using fail-closed time enforcement",
            ),
        ] {
            let mut hostile = healthy.clone();
            let object = hostile.as_object_mut().expect("object");
            match label {
                "fallback" => {
                    object.insert("fallback".into(), Value::Bool(true));
                }
                "samples" => {
                    object.insert("sample_count".into(), Value::from(0_u64));
                }
                "health" => {
                    object
                        .get_mut("health")
                        .and_then(Value::as_object_mut)
                        .expect("health")
                        .insert("healthy".into(), Value::Bool(false));
                }
                "enforcement" => {
                    object.insert("enforcement_mode".into(), Value::from("warn"));
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_time_snapshot(Some(&hostile)),
                Err(mutation.to_owned())
            );
        }
        assert!(validate_time_snapshot(None).is_err());
    }
    #[test]
    fn write_canary_exit_gate_fails_closed() {
        ensure_write_canary_succeeded(&norito::json!({"status": "ok"}))
            .expect("an explicitly successful canary may exit zero");
        for report in [
            norito::json!({"status": "fail"}),
            norito::json!({"status": "unknown"}),
            norito::json!({}),
        ] {
            let _ = ensure_write_canary_succeeded(&report)
                .expect_err("non-success canary reports must produce a failing process exit");
        }
    }
    #[test]
    fn onboarding_token_validation_is_byte_exact_and_redacted() {
        assert_eq!(
            validate_onboarding_token(TEST_ONBOARDING_TOKEN).expect("valid token"),
            TEST_ONBOARDING_TOKEN
        );
        for malformed in [
            String::new(),
            "T".repeat(31),
            "T".repeat(257),
            format!("{} ", "T".repeat(31)),
            format!("{}é", "T".repeat(31)),
        ] {
            let error = validate_onboarding_token(&malformed)
                .expect_err("malformed onboarding token must fail closed");
            if !malformed.is_empty() {
                assert!(!format!("{error:#}").contains(&malformed));
            }
        }
    }
    #[test]
    fn onboarding_response_redaction_covers_text_keys_and_values() {
        let escaped_token = "\"".repeat(32);
        let mut body = Map::new();
        body.insert(
            escaped_token.clone(),
            Value::Array(vec![Value::String(escaped_token.clone())]),
        );
        let encoded = json::to_json(&Value::Object(body.clone())).expect("encode echoed token");
        assert!(!encoded.contains(&escaped_token));
        let mut response = HttpJson {
            status: 400,
            text: encoded,
            body: Some(Value::Object(body)),
        };
        redact_http_json(&mut response, &escaped_token);
        let rendered = compact_json(response.body.as_ref().expect("redacted body"));
        assert!(!response.text.contains(&escaped_token));
        assert!(!rendered.contains(&escaped_token));
        assert!(!response.text.contains(&"\\\"".repeat(32)));
        assert!(rendered.contains("<redacted>"));
    }
    #[test]
    fn onboarding_token_file_is_exact_owner_only_regular_and_not_cached() {
        let file = test_onboarding_token_file();
        assert_eq!(
            read_onboarding_token_file(file.path())
                .expect("read token")
                .as_str(),
            TEST_ONBOARDING_TOKEN
        );
        let replacement = "Z".repeat(32);
        fs::write(file.path(), &replacement).expect("replace token bytes");
        assert_eq!(
            read_onboarding_token_file(file.path())
                .expect("read replacement token")
                .as_str(),
            replacement
        );
        fs::write(file.path(), format!("{TEST_ONBOARDING_TOKEN}\n")).expect("write newline token");
        let error = read_onboarding_token_file(file.path())
            .expect_err("newline must not be trimmed from token file");
        assert!(format!("{error:#}").contains("printable ASCII"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, symlink};
            fs::write(file.path(), TEST_ONBOARDING_TOKEN).expect("restore token");
            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o640))
                .expect("set unsafe permissions");
            let error = read_onboarding_token_file(file.path())
                .expect_err("group-readable token file must fail closed");
            assert!(format!("{error:#}").contains("group or other"));
            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))
                .expect("restore safe permissions");
            let directory = tempfile::tempdir().expect("token symlink directory");
            let link = directory.path().join("onboarding.token");
            symlink(file.path(), &link).expect("create token symlink");
            let error =
                read_onboarding_token_file(&link).expect_err("token symlink must fail closed");
            assert!(format!("{error:#}").contains("non-symlink"));
        }
        let directory = tempfile::tempdir().expect("token directory");
        let error = read_onboarding_token_file(directory.path())
            .expect_err("directory token path must fail closed");
        assert!(format!("{error:#}").contains("regular non-symlink"));
    }
    #[test]
    fn onboarding_plan_refuses_redirect_without_forwarding_token() {
        let destination = spawn_mock_http(1, |_request| {
            MockResponse::json(200, norito::json!({"unexpected": "redirect followed"}))
        });
        let destination_url = format!("{}/v1/accounts/onboard/plan", destination.base_url);
        let redirect = spawn_mock_http(1, move |_request| MockResponse {
            status: 307,
            content_type: "text/plain",
            headers: vec![("Location", destination_url.clone())],
            body: format!("server echoed {TEST_ONBOARDING_TOKEN}"),
        });
        let http = http_client().expect("HTTP client");
        let key_pair = fixture_key_pair(12);
        let account_id = AccountId::new(key_pair.public_key().clone());
        let response = plan_canary_onboarding(
            &http,
            &redirect.base_url,
            "canary@universal",
            &account_id,
            TEST_ONBOARDING_TOKEN,
        )
        .expect("redirect remains an HTTP response");
        let redirect_requests = finish_mock(redirect);
        let destination_requests = finish_mock(destination);
        assert_eq!(response.status, 307);
        assert_eq!(response.text, "<invalid JSON response>");
        assert!(!response.text.contains(TEST_ONBOARDING_TOKEN));
        assert_eq!(redirect_requests.len(), 1);
        assert_eq!(
            redirect_requests[0]
                .header_values(ACCOUNT_ONBOARDING_TOKEN_HEADER)
                .len(),
            1
        );
        assert!(
            redirect_requests[0]
                .header_values(ACCOUNT_ONBOARDING_TOKEN_HEADER)
                .first()
                .is_some_and(|value| *value == TEST_ONBOARDING_TOKEN)
        );
        assert!(destination_requests.is_empty());
    }
    #[test]
    fn doctor_mock_required_tool_missing_reports_failure() {
        let missing_tool = REQUIRED_MCP_TOOLS[0];
        let server = spawn_mock_http(15, move |request| {
            doctor_mock_response(request, Some(missing_tool))
        });
        let report = run_doctor(&server.base_url).expect("doctor report");
        let _requests = finish_mock(server);
        let failures = report
            .as_object()
            .and_then(|object| object.get("failures"))
            .and_then(Value::as_array)
            .expect("failures");
        assert_eq!(report_status(&report), Some("fail"));
        assert!(
            failures
                .iter()
                .filter_map(Value::as_str)
                .any(|failure| failure.contains(missing_tool))
        );
    }
    #[test]
    fn write_canary_mock_success_returns_redacted_receipt() {
        let onboarding_token_file = test_onboarding_token_file();
        let server = spawn_mock_http(11, |request| write_canary_mock_response(request, 202, 200));
        let args = WriteCanary {
            public_root: server.base_url.clone(),
            alias_prefix: "mock-canary".to_owned(),
            faucet_asset_id: DEFAULT_GAS_ASSET_ID.to_owned(),
            onboarding_token_file: onboarding_token_file.path().to_path_buf(),
            write_config: None,
            use_config_signer: false,
            json: true,
        };
        let report = run_write_canary(
            &crate::fallback_config(),
            &args,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("write canary");
        let requests = finish_mock(server);
        let rendered = compact_json(&report);
        assert_eq!(report_status(&report), Some("ok"), "{rendered}");
        assert!(rendered.contains(&"cd".repeat(32)));
        assert!(rendered.contains(DEFAULT_CHAIN_ID));
        assert!(rendered.contains(DEFAULT_GAS_ASSET_ID));
        assert!(!rendered.contains("private_key"));
        assert!(requests.iter().any(|request| request.method == "POST"
            && path_only(&request.path) == torii_uri::TRANSACTION));
        assert!(
            requests.iter().any(|request| request.method == "POST"
                && path_only(&request.path) == torii_uri::FEES_QUOTE)
        );
        let onboarding_plan = requests
            .iter()
            .find(|request| {
                request.method == "POST" && path_only(&request.path) == "/v1/accounts/onboard/plan"
            })
            .expect("onboarding plan request");
        let onboarding_apply = requests
            .iter()
            .find(|request| {
                request.method == "POST" && path_only(&request.path) == "/v1/accounts/onboard"
            })
            .expect("onboarding apply request");
        for onboarding in [onboarding_plan, onboarding_apply] {
            assert_eq!(
                onboarding
                    .header_values(ACCOUNT_ONBOARDING_TOKEN_HEADER)
                    .len(),
                1
            );
            assert!(
                onboarding
                    .header_values(ACCOUNT_ONBOARDING_TOKEN_HEADER)
                    .first()
                    .is_some_and(|value| *value == TEST_ONBOARDING_TOKEN),
                "onboarding credential header did not match the exact token-file bytes"
            );
            assert_eq!(
                onboarding.header_values("content-type"),
                vec!["application/json"]
            );
            assert!(!onboarding.body.contains(TEST_ONBOARDING_TOKEN));
            assert!(!onboarding.body.contains("private_key"));
            assert!(!onboarding.body.contains("public_key_hex"));
            assert!(!onboarding.body.contains("uaid"));
        }
        let plan_body =
            json::from_str::<Value>(&onboarding_plan.body).expect("decode onboarding plan request");
        let plan_object = plan_body.as_object().expect("onboarding plan object");
        assert_eq!(
            plan_object
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["account_id", "alias", "permissions", "version"]
                .into_iter()
                .collect()
        );
        assert_eq!(plan_object.get("version").and_then(Value::as_u64), Some(1));
        assert!(
            plan_object
                .get("permissions")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
        );
        let apply_body = json::from_str::<Value>(&onboarding_apply.body)
            .expect("decode onboarding apply request");
        let apply_object = apply_body.as_object().expect("onboarding apply object");
        assert_eq!(
            apply_object.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["receipt"]
        );
        let receipt_request = apply_object
            .get("receipt")
            .and_then(Value::as_object)
            .and_then(|receipt| receipt.get("body"))
            .and_then(Value::as_object)
            .and_then(|body| body.get("request"));
        assert_eq!(receipt_request, Some(&plan_body));
        assert!(requests.iter().any(|request| {
            request.method == "GET"
                && path_only(&request.path) == "/v1/pipeline/transactions/status"
                && request.path.contains(&"ab".repeat(32))
        }));
        assert!(requests.iter().any(|request| {
            request.method == "GET"
                && path_only(&request.path) == "/v1/pipeline/transactions/status"
                && request.path.contains(&"cd".repeat(32))
        }));
    }
    #[test]
    fn write_canary_rejects_unavailable_capabilities_without_transaction_post() {
        for capabilities_status in [404, 429, 503] {
            let onboarding_token_file = test_onboarding_token_file();
            let server = spawn_mock_http(8, move |request| {
                write_canary_mock_response(request, 202, capabilities_status)
            });
            let args = WriteCanary {
                public_root: server.base_url.clone(),
                alias_prefix: "mock-canary".to_owned(),
                faucet_asset_id: DEFAULT_GAS_ASSET_ID.to_owned(),
                onboarding_token_file: onboarding_token_file.path().to_path_buf(),
                write_config: None,
                use_config_signer: false,
                json: true,
            };
            let error = run_write_canary(
                &crate::fallback_config(),
                &args,
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect_err("unavailable capabilities must reject the write canary");
            let requests = finish_mock(server);
            let rendered = format!("{error:#}");
            assert!(
                rendered.contains("Failed to get node capabilities"),
                "unexpected error for HTTP {capabilities_status}: {rendered}"
            );
            assert!(
                rendered.contains(&capabilities_status.to_string()),
                "capability error did not include HTTP {capabilities_status}: {rendered}"
            );
            assert!(requests.iter().any(|request| {
                request.method == "GET" && path_only(&request.path) == "/v1/node/capabilities"
            }));
            assert!(!requests.iter().any(|request| {
                request.method == "POST" && path_only(&request.path) == torii_uri::TRANSACTION
            }));
        }
    }
    #[test]
    fn faucet_response_rejects_retired_synchronous_shape() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account_id = AccountId::new(key_pair.public_key().clone());
        let response = norito::json!({
            "account_id": (account_id.to_string()),
            "asset_definition_id": (DEFAULT_GAS_ASSET_ID),
            "asset_id": (format!("{DEFAULT_GAS_ASSET_ID}#{account_id}")),
            "amount": "1000000000000000000",
            "tx_hash_hex": "faucetabc",
            "status": "Applied"
        });
        let error = validate_faucet_response(Some(&response), &account_id, DEFAULT_GAS_ASSET_ID)
            .expect_err("retired synchronous faucet response must fail closed");
        assert!(format!("{error:#}").contains("expected QUEUED"));
    }
    #[test]
    fn faucet_response_rejects_wrong_asset_and_short_hash() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account_id = AccountId::new(key_pair.public_key().clone());
        let response = |asset_definition_id: &str, tx_hash_hex: &str| {
            norito::json!({
                "account_id": (account_id.to_string()),
                "asset_definition_id": (asset_definition_id),
                "asset_id": (format!("{asset_definition_id}#{account_id}")),
                "amount": "1000000000000000000",
                "tx_hash_hex": (tx_hash_hex),
                "status": "QUEUED"
            })
        };
        let wrong_asset = response("wrong-asset", &"cd".repeat(32));
        let error = validate_faucet_response(Some(&wrong_asset), &account_id, DEFAULT_GAS_ASSET_ID)
            .expect_err("a different funded asset cannot satisfy the canary");
        assert!(format!("{error:#}").contains("unexpected asset_definition_id"));
        let short_hash = response(DEFAULT_GAS_ASSET_ID, "cd");
        let error = validate_faucet_response(Some(&short_hash), &account_id, DEFAULT_GAS_ASSET_ID)
            .expect_err("a short transaction hash must fail closed");
        assert!(format!("{error:#}").contains("must encode 32 bytes"));
    }
    #[test]
    fn pipeline_status_requires_current_nested_shape() {
        let current = norito::json!({"status": {"kind": "Applied"}});
        assert_eq!(
            pipeline_status_kind(Some(&current)).as_deref(),
            Some("Applied")
        );
        let retired = norito::json!({"status": "Applied"});
        assert_eq!(pipeline_status_kind(Some(&retired)), None);
    }
    #[test]
    fn pipeline_status_requires_applied_before_success() {
        for pending in ["Queued", "Approved", "Committed"] {
            let response = norito::json!({"status": {"kind": pending}});
            assert!(
                !pipeline_status_is_terminal(Some(&response)),
                "{pending} must not finish the Applied wait"
            );
        }
        for terminal in ["Applied", "Rejected", "Expired"] {
            let response = norito::json!({"status": {"kind": terminal}});
            assert!(pipeline_status_is_terminal(Some(&response)));
        }
    }
    #[test]
    fn pipeline_status_poll_fails_fast_on_noncanonical_http_error() {
        let server = spawn_mock_http(1, |_request| {
            MockResponse::json(503, norito::json!({"error": "route unavailable"}))
        });
        let http = http_client().expect("HTTP client");
        let response = wait_for_pipeline_terminal_status(
            &http,
            &server.base_url,
            &"ab".repeat(32),
            Duration::from_secs(30),
        )
        .expect("HTTP error remains a typed canary result");
        let requests = finish_mock(server);
        assert_eq!(response.status, 503);
        assert_eq!(requests.len(), 1);
    }
    #[test]
    fn write_canary_mock_onboarding_400_does_not_attempt_faucet() {
        let onboarding_token_file = test_onboarding_token_file();
        let server = spawn_mock_http(1, |request| write_canary_mock_response(request, 400, 200));
        let args = WriteCanary {
            public_root: server.base_url.clone(),
            alias_prefix: "mock-canary".to_owned(),
            faucet_asset_id: DEFAULT_GAS_ASSET_ID.to_owned(),
            onboarding_token_file: onboarding_token_file.path().to_path_buf(),
            write_config: None,
            use_config_signer: false,
            json: true,
        };
        let report = run_write_canary(
            &crate::fallback_config(),
            &args,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("write canary");
        let requests = finish_mock(server);
        let failures = report
            .as_object()
            .and_then(|object| object.get("failures"))
            .and_then(Value::as_array)
            .expect("failures");
        assert_eq!(report_status(&report), Some("fail"));
        assert!(
            failures
                .iter()
                .filter_map(Value::as_str)
                .any(|failure| failure.contains("faucet funding was not attempted"))
        );
        assert!(
            !requests.iter().any(|request| request.method == "POST"
                && path_only(&request.path) == "/v1/accounts/faucet")
        );
    }
    #[test]
    fn submit_failure_hints_cover_invalid_fee_intent_and_route_unavailable() {
        let invalid_fee = hint_submit_error(eyre!("invalid fee_payment intent"));
        assert!(format!("{invalid_fee:#}").contains("/v1/fees/quote"));
        let route = hint_submit_error(eyre!("route_unavailable"));
        assert!(format!("{route:#}").contains("ingress or lane routing"));
    }
    #[test]
    fn faucet_asset_failure_points_at_bootstrap() {
        let response = HttpJson {
            status: 400,
            body: Some(error_value("invalid", "Failed to find asset")),
            text: String::new(),
        };
        assert!(faucet_failure_hint(&response).contains("not bootstrapped"));
    }
    #[test]
    fn leading_zero_bits_counts_prefix() {
        assert_eq!(leading_zero_bits(&[0x00, 0x0f]), 12);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
        assert_eq!(leading_zero_bits(&[0x40]), 1);
    }
    #[test]
    fn faucet_challenge_matches_python_fixture_shape() {
        let network_id = crate::fallback_config().network_id;
        let challenge = build_faucet_challenge(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("challenge");
        assert_eq!(challenge.len(), 32);
        assert_ne!(challenge, [0_u8; 32]);
    }
    #[test]
    fn faucet_challenge_rejects_same_label_different_genesis_replay() {
        let first_network = crate::fallback_config().network_id;
        let second_network =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"foreign-faucet-genesis"),
            ));
        let account_id = "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        let first = build_faucet_challenge(
            account_id,
            &first_network,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("first challenge");
        let second = build_faucet_challenge(
            account_id,
            &second_network,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("second challenge");
        assert_ne!(first, second);
    }
    #[test]
    fn solve_faucet_puzzle_rejects_zero_difficulty() {
        let network_id = crate::fallback_config().network_id;
        let puzzle = norito::json!({
            "algorithm": FAUCET_POW_ALGORITHM,
            "network_id": (network_id.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
            "difficulty_bits": 0,
        });
        let error = solve_faucet_puzzle(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            &puzzle,
        )
        .expect_err("zero-difficulty faucet puzzle must fail closed");
        assert!(format!("{error:#}").contains("difficulty_bits must be positive"));
    }
    #[test]
    fn taira_puzzle_identity_requires_the_exact_network_and_discriminant() {
        let network_id = crate::fallback_config().network_id;
        let canonical = norito::json!({
            "network_id": (network_id.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
        });
        assert_eq!(
            validate_taira_puzzle_identity(&canonical, &network_id)
                .expect("canonical Taira puzzle identity"),
            network_id
        );

        let foreign_network =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"foreign-taira-genesis"),
            ));
        let foreign = norito::json!({
            "network_id": (foreign_network.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
        });
        let network_error = validate_taira_puzzle_identity(&foreign, &network_id)
            .expect_err("a foreign network identity must fail before publication");
        assert!(format!("{network_error:#}").contains("does not match configured network"));

        let wrong_discriminant = norito::json!({
            "network_id": (network_id.to_string()),
            "chain_discriminant": (DEFAULT_CHAIN_DISCRIMINANT + 1),
        });
        let discriminant_error = validate_taira_puzzle_identity(&wrong_discriminant, &network_id)
            .expect_err("a foreign chain discriminant must fail before publication");
        assert!(format!("{discriminant_error:#}").contains("does not match Taira"));
    }
    #[test]
    fn resolve_canary_signer_derives_account() {
        let key_pair = fixture_key_pair(3);
        let mut config = crate::fallback_config();
        config.key_pair = key_pair.clone();
        let signer = resolve_canary_signer(&config, true).expect("config signer");
        assert!(!signer.generated);
        assert_eq!(
            signer.account_id,
            AccountId::new(key_pair.public_key().clone())
        );
    }
    #[test]
    fn resolve_canary_signer_generates_checked_ed25519_signer() {
        let config = crate::fallback_config();
        let signer = resolve_canary_signer(&config, false).expect("generated signer");
        assert!(signer.generated);
        assert_eq!(signer.key_pair.algorithm(), Algorithm::Ed25519);
        assert_eq!(
            signer.account_id,
            AccountId::new(signer.key_pair.public_key().clone())
        );
    }
    #[test]
    fn onboarding_apply_rejects_the_retired_synchronous_shape() {
        let key_pair = fixture_key_pair(11);
        let account_id = AccountId::new(key_pair.public_key().clone());
        let stale = norito::json!({
            "account_id": (account_id.to_string()),
            "alias": "canary@universal",
            "tx_hash_hex": ("ab".repeat(32)),
            "status": "Applied",
            "disposition": { "kind": "create" }
        });
        let error =
            validate_onboarding_apply_response(Some(&stale), &account_id, "canary@universal")
                .expect_err("retired apply response must fail closed");
        assert!(
            error
                .to_string()
                .contains("unexpected onboarding apply status")
        );
    }
    #[test]
    fn write_canary_receipt_identity_is_redacted() {
        let key_pair = fixture_key_pair(5);
        let signer = CanarySigner {
            account_id: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            generated: true,
        };
        let mut extra = Map::new();
        insert_write_receipt_identity(
            &mut extra,
            &signer,
            "tairacanary123@universal",
            DEFAULT_GAS_ASSET_ID,
        );
        let report = report_value(
            "taira_write_canary",
            "ok",
            DEFAULT_PUBLIC_ROOT,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            extra,
        )
        .expect("report");
        let rendered = compact_json(&report);
        assert!(rendered.contains(DEFAULT_CHAIN_ID));
        assert!(rendered.contains("tairacanary123@universal"));
        assert!(rendered.contains(DEFAULT_GAS_ASSET_ID));
        assert!(!rendered.contains("private_key"));
        assert!(!rendered.contains("public_key_raw_hex"));
    }
    #[test]
    fn build_alias_requires_a_canonical_prefix() {
        let key_pair = fixture_key_pair(7);
        let alias = build_alias(
            DEFAULT_ALIAS_PREFIX,
            key_pair.public_key(),
            "wonderland.universal",
        )
        .expect("canonical alias inputs");
        assert!(alias.starts_with("tairarolloutcanary"));
        assert!(alias.ends_with("@universal"));
        assert!(
            alias
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '@')
        );
        for retired in ["Taira Rollout Canary!", "taira-rollout-canary", ""] {
            let error = build_alias(retired, key_pair.public_key(), "wonderland.universal")
                .expect_err("alias prefixes are never sanitized or defaulted");
            assert!(
                error
                    .to_string()
                    .contains("alias prefix must contain 1..32 canonical lowercase ASCII"),
                "{error}"
            );
        }
        let error = build_alias(DEFAULT_ALIAS_PREFIX, key_pair.public_key(), "wonderland.")
            .expect_err("dataspace labels are never defaulted");
        assert!(
            error
                .to_string()
                .contains("alias domain must end in a canonical dataspace label"),
            "{error}"
        );
    }
    #[test]
    fn public_root_requires_an_exact_origin() {
        assert_eq!(
            normalize_root_url(DEFAULT_PUBLIC_ROOT).expect("canonical public root"),
            DEFAULT_PUBLIC_ROOT
        );
        assert_eq!(
            normalize_root_url("http://127.0.0.1:18080").expect("canonical local origin"),
            "http://127.0.0.1:18080"
        );
        for invalid in [
            " https://taira.sora.org",
            "https://user@taira.sora.org",
            "https://taira.sora.org/",
            "https://taira.sora.org/path",
            "https://taira.sora.org?debug=1",
            "https://taira.sora.org#fragment",
            "HTTPS://TAIRA.SORA.ORG",
        ] {
            let error = normalize_root_url(invalid)
                .expect_err("noncanonical public roots must fail closed");
            assert!(error.to_string().contains("public root URL"), "{error}");
        }
    }
    #[test]
    fn render_runtime_config_redacts_nothing_only_when_explicitly_called() {
        let key_pair = fixture_key_pair(9);
        let mut config = crate::fallback_config();
        config.key_pair = key_pair;
        config.account = AccountId::new(config.key_pair.public_key().clone());
        config.chain = DEFAULT_CHAIN_ID.into();
        config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
        config.torii_api_url = Url::parse("https://taira.sora.org/").expect("url");
        let rendered = render_runtime_config(&config).expect("config");
        assert!(rendered.contains("private_key = "));
        assert!(rendered.contains("chain_discriminant = 369"));
        assert!(rendered.contains("nonce = false"));
    }
    fn inrou_canary_deployment_fixture() -> crate::soracloud::TairaInrouCanaryDeployment {
        crate::soracloud::TairaInrouCanaryDeployment {
            service_name: "taira_inrou_canary".to_owned(),
            service_version: format!("artifact-{}", "11".repeat(32)),
            service_manifest_hash: "service-manifest-hash".to_owned(),
            container_manifest_hash: "container-manifest-hash".to_owned(),
            route_host: "taira-inrou-canary.sora".to_owned(),
            route_path_prefix: "/api/v1".to_owned(),
            healthcheck_path: "/health".to_owned(),
            mutation_mode: "deploy".to_owned(),
            bundle_hash: "materialized-bundle-hash".to_owned(),
            bundle_content_cid: "bundle-cid".to_owned(),
            bundle_manifest_digest_hex: "22".repeat(32),
            guest_content_cid: "guest-cid".to_owned(),
            guest_manifest_digest_hex: "33".repeat(32),
            submitted_tx_hash: Hash::new(b"submitted-canary-mutation").to_string(),
            mutation_response_digest: "mutation-response-hash".to_owned(),
        }
    }
    fn exact_inrou_status_fixture(
        deployment: &crate::soracloud::TairaInrouCanaryDeployment,
        current_version: &str,
        latest_version: &str,
        service_manifest_hash: &str,
        container_manifest_hash: &str,
        process_generation: u64,
    ) -> Value {
        let (action, revision_count, last_rollout) = match deployment.mutation_mode.as_str() {
            "deploy" => ("Deploy", 1_u64, Value::Null),
            "upgrade" => (
                "Upgrade",
                2,
                norito::json!({
                    "baseline_version": "artifact-baseline",
                    "candidate_version": current_version,
                    "canary_percent": 100,
                    "traffic_percent": 100,
                    "stage": { "stage": "Promoted", "value": null }
                }),
            ),
            other => panic!("unsupported test mutation mode {other}"),
        };
        norito::json!({
            "schema_version": 1,
            "runtime_manager": { "available": true },
            "hosted_http_topology": {
                "active_capability_adverts": 4,
                "hosted_replica_count": 4
            },
            "control_plane": {
                "services": [{
                    "service_name": (deployment.service_name.clone()),
                    "current_version": current_version,
                    "revision_count": revision_count,
                    "active_rollout": null,
                    "last_rollout": last_rollout,
                    "latest_revision": {
                        "action": { "action": action, "value": null },
                        "service_version": latest_version,
                        "service_manifest_hash": service_manifest_hash,
                        "container_manifest_hash": container_manifest_hash,
                        "replicas": 4,
                        "runtime": {"runtime": "Inrou", "value": null},
                        "execution_plane": {
                            "execution_plane": "HttpService",
                            "value": null
                        },
                        "route_host": (deployment.route_host.clone()),
                        "route_path_prefix": (deployment.route_path_prefix.clone()),
                        "healthcheck_path": (deployment.healthcheck_path.clone()),
                        "process_generation": process_generation
                    }
                }]
            }
        })
    }
    fn canonical_inrou_status_fixture(
        deployment: &crate::soracloud::TairaInrouCanaryDeployment,
        process_generation: u64,
    ) -> Value {
        exact_inrou_status_fixture(
            deployment,
            &deployment.service_version,
            &deployment.service_version,
            &deployment.service_manifest_hash,
            &deployment.container_manifest_hash,
            process_generation,
        )
    }
    fn exact_inrou_route_headers(
        deployment: &crate::soracloud::TairaInrouCanaryDeployment,
        replica_slot: u64,
        process_generation: u64,
    ) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            iroha_torii_shared::SORACLOUD_SERVED_SERVICE_NAME_HEADER,
            reqwest::header::HeaderValue::from_str(&deployment.service_name)
                .expect("service header"),
        );
        headers.insert(
            iroha_torii_shared::SORACLOUD_SERVED_SERVICE_VERSION_HEADER,
            reqwest::header::HeaderValue::from_str(&deployment.service_version)
                .expect("version header"),
        );
        headers.insert(
            iroha_torii_shared::SORACLOUD_SERVED_REPLICA_SLOT_HEADER,
            reqwest::header::HeaderValue::from_str(&replica_slot.to_string()).expect("slot header"),
        );
        headers.insert(
            iroha_torii_shared::SORACLOUD_SERVED_PROCESS_GENERATION_HEADER,
            reqwest::header::HeaderValue::from_str(&process_generation.to_string())
                .expect("generation header"),
        );
        headers.insert(
            iroha_torii_shared::SORACLOUD_SERVED_MATERIALIZED_BUNDLE_HASH_HEADER,
            reqwest::header::HeaderValue::from_str(&deployment.bundle_hash).expect("bundle header"),
        );
        headers
    }
    fn exact_inrou_health_body(
        deployment: &crate::soracloud::TairaInrouCanaryDeployment,
        replica_slot: u64,
    ) -> Value {
        norito::json!({
            "service": (deployment.service_name.clone()),
            "runtime": "Inrou",
            "replica_slot": replica_slot,
            "identity": (format!("{}:replica:{replica_slot}", deployment.service_name))
        })
    }
    #[test]
    fn inrou_canary_status_binds_the_exact_admitted_revision() {
        let deployment = inrou_canary_deployment_fixture();
        assert_eq!(tagged_enum_name(&norito::json!("Inrou"), "runtime"), None);
        assert_eq!(
            tagged_enum_name(&norito::json!({"runtime": "Inrou"}), "runtime"),
            None
        );
        let exact = canonical_inrou_status_fixture(&deployment, 7);
        assert_eq!(
            validate_exact_inrou_canary_status(&exact, &deployment)
                .expect("exact admitted revision"),
            ExactInrouCanaryStatus {
                active_adverts: 4,
                hosted_replicas: 4,
                process_generation: 7,
            }
        );

        let wrong_latest = exact_inrou_status_fixture(
            &deployment,
            &deployment.service_version,
            "artifact-wrong",
            &deployment.service_manifest_hash,
            &deployment.container_manifest_hash,
            7,
        );
        assert!(
            validate_exact_inrou_canary_status(&wrong_latest, &deployment)
                .expect_err("latest revision version drift must fail")
                .contains("latest revision version")
        );
        let wrong_service_hash = exact_inrou_status_fixture(
            &deployment,
            &deployment.service_version,
            &deployment.service_version,
            "wrong-service-hash",
            &deployment.container_manifest_hash,
            7,
        );
        assert!(
            validate_exact_inrou_canary_status(&wrong_service_hash, &deployment)
                .expect_err("service manifest drift must fail")
                .contains("service manifest hash")
        );
        let wrong_container_hash = exact_inrou_status_fixture(
            &deployment,
            &deployment.service_version,
            &deployment.service_version,
            &deployment.service_manifest_hash,
            "wrong-container-hash",
            7,
        );
        assert!(
            validate_exact_inrou_canary_status(&wrong_container_hash, &deployment)
                .expect_err("container manifest drift must fail")
                .contains("container manifest hash")
        );
        let zero_generation = canonical_inrou_status_fixture(&deployment, 0);
        assert!(
            validate_exact_inrou_canary_status(&zero_generation, &deployment)
                .expect_err("zero generation must fail")
                .contains("positive process generation")
        );
        let coarse = norito::json!({
            "runtime_manager": { "available": true },
            "hosted_http_topology": {
                "active_capability_adverts": 4,
                "hosted_replica_count": 4
            },
            "control_plane": {
                "services": [{
                    "service_name": (deployment.service_name.clone()),
                    "current_version": (deployment.service_version.clone()),
                    "latest_revision": {
                        "replicas": 4,
                        "runtime": "Inrou",
                        "execution_plane": "HttpService",
                        "route_host": (deployment.route_host.clone()),
                        "route_path_prefix": (deployment.route_path_prefix.clone())
                    }
                }]
            }
        });
        assert!(
            validate_exact_inrou_canary_status(&coarse, &deployment)
                .expect_err("coarse shape must fail closed")
                .contains("latest revision version")
        );
        let duplicate_revision = canonical_inrou_status_fixture(&deployment, 7)
            .get("control_plane")
            .and_then(Value::as_object)
            .and_then(|control_plane| control_plane.get("services"))
            .and_then(Value::as_array)
            .and_then(|services| services.first())
            .cloned()
            .expect("canonical service snapshot");
        let mut duplicate = canonical_inrou_status_fixture(&deployment, 7);
        duplicate
            .get_mut("control_plane")
            .and_then(Value::as_object_mut)
            .and_then(|control_plane| control_plane.get_mut("services"))
            .and_then(Value::as_array_mut)
            .expect("services")
            .push(duplicate_revision);
        assert!(
            validate_exact_inrou_canary_status(&duplicate, &deployment)
                .expect_err("duplicate service snapshots must fail closed")
                .contains("duplicate snapshots")
        );
    }
    #[test]
    fn inrou_canary_route_binds_torii_served_revision_headers() {
        let deployment = inrou_canary_deployment_fixture();
        let body = exact_inrou_health_body(&deployment, 2);
        let exact_headers = exact_inrou_route_headers(&deployment, 2, 7);
        let evidence = validate_exact_inrou_canary_route(&exact_headers, &body, &deployment, 7)
            .expect("exact Torii-served revision evidence");
        assert_eq!(evidence.replica_slot, 2);
        assert_eq!(evidence.process_generation, 7);
        assert!(
            validate_exact_inrou_canary_route(&exact_headers, &body, &deployment, 8)
                .expect_err("a stale served generation must not be relabeled by the caller")
                .contains("different authoritative process generation")
        );
        let next_headers = exact_inrou_route_headers(&deployment, 2, 8);
        let next_generation =
            validate_exact_inrou_canary_route(&next_headers, &body, &deployment, 8)
                .expect("Torii must serve the new exact generation");
        assert_ne!(evidence.evidence_sha256, next_generation.evidence_sha256);

        let mut wrong_version = exact_headers.clone();
        wrong_version.insert(
            iroha_torii_shared::SORACLOUD_SERVED_SERVICE_VERSION_HEADER,
            reqwest::header::HeaderValue::from_static("artifact-wrong"),
        );
        assert!(
            validate_exact_inrou_canary_route(&wrong_version, &body, &deployment, 7)
                .expect_err("wrong served version must fail")
                .contains("immutable revision")
        );
        let mut wrong_bundle = exact_headers.clone();
        wrong_bundle.insert(
            iroha_torii_shared::SORACLOUD_SERVED_MATERIALIZED_BUNDLE_HASH_HEADER,
            reqwest::header::HeaderValue::from_static("wrong-bundle"),
        );
        assert!(
            validate_exact_inrou_canary_route(&wrong_bundle, &body, &deployment, 7)
                .expect_err("wrong materialized bundle must fail")
                .contains("materialized bundle")
        );
        let wrong_slot_body = exact_inrou_health_body(&deployment, 3);
        assert!(
            validate_exact_inrou_canary_route(&exact_headers, &wrong_slot_body, &deployment, 7,)
                .expect_err("Torii/body slot mismatch must fail")
                .contains("identity contract")
        );
        let mut missing_name = exact_headers;
        missing_name.remove(iroha_torii_shared::SORACLOUD_SERVED_SERVICE_NAME_HEADER);
        assert!(
            validate_exact_inrou_canary_route(&missing_name, &body, &deployment, 7)
                .expect_err("missing served identity must fail")
                .contains("is missing")
        );
        let mut missing_generation = next_headers.clone();
        missing_generation.remove(iroha_torii_shared::SORACLOUD_SERVED_PROCESS_GENERATION_HEADER);
        assert!(
            validate_exact_inrou_canary_route(&missing_generation, &body, &deployment, 8)
                .expect_err("missing served generation must fail")
                .contains("is missing")
        );
        let mut duplicate_generation = next_headers;
        duplicate_generation.append(
            iroha_torii_shared::SORACLOUD_SERVED_PROCESS_GENERATION_HEADER,
            reqwest::header::HeaderValue::from_static("8"),
        );
        assert!(
            validate_exact_inrou_canary_route(&duplicate_generation, &body, &deployment, 8)
                .expect_err("duplicate served generation must fail")
                .contains("duplicate")
        );
    }
    #[test]
    fn inrou_canary_convergence_discards_stale_route_evidence() {
        let first = ExactInrouCanaryStatus {
            active_adverts: 4,
            hosted_replicas: 4,
            process_generation: 7,
        };
        let mut convergence = InrouCanaryConvergence::default();
        convergence
            .observe_status(Ok(first))
            .expect("first exact status");
        convergence
            .record_route(ExactInrouCanaryRouteEvidence {
                replica_slot: 1,
                identity: "taira_inrou_canary:replica:1".to_owned(),
                evidence_sha256: "first".to_owned(),
                process_generation: 7,
            })
            .expect("route under exact status");
        convergence
            .observe_status(Ok(first))
            .expect("same generation remains exact");
        assert_eq!(convergence.identities.len(), 1);

        assert!(
            convergence
                .observe_status(Err("status failed".to_owned()))
                .is_err()
        );
        assert!(convergence.exact_status.is_none());
        assert!(convergence.identities.is_empty());
        assert!(
            convergence
                .record_route(ExactInrouCanaryRouteEvidence {
                    replica_slot: 1,
                    identity: "taira_inrou_canary:replica:1".to_owned(),
                    evidence_sha256: "stale".to_owned(),
                    process_generation: 7,
                })
                .expect_err("route evidence before exact status must fail")
                .contains("without exact authoritative status")
        );

        convergence
            .observe_status(Ok(first))
            .expect("status recovers");
        convergence
            .record_route(ExactInrouCanaryRouteEvidence {
                replica_slot: 2,
                identity: "taira_inrou_canary:replica:2".to_owned(),
                evidence_sha256: "second".to_owned(),
                process_generation: 7,
            })
            .expect("fresh route evidence");
        convergence
            .observe_status(Ok(ExactInrouCanaryStatus {
                process_generation: 8,
                ..first
            }))
            .expect("new generation is exact");
        assert!(convergence.identities.is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn runtime_config_is_created_once_with_private_custody() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let directory = tempfile::tempdir().expect("runtime config directory");
        let parent = fs::canonicalize(directory.path()).expect("canonical temp directory");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("private runtime config directory");
        let path = parent.join("taira-canary.toml");
        let key_pair = fixture_key_pair(10);
        let mut config = crate::fallback_config();
        config.key_pair = key_pair;
        config.account = AccountId::new(config.key_pair.public_key().clone());
        config.chain = DEFAULT_CHAIN_ID.into();
        config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
        config.torii_api_url = Url::parse("https://taira.sora.org/").expect("url");

        write_runtime_config(&path, &config).expect("secure runtime config publication");
        let metadata = fs::symlink_metadata(&path).expect("published runtime config");
        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert!(
            fs::read_to_string(&path)
                .expect("read config")
                .contains("private_key = ")
        );
        let error = write_runtime_config(&path, &config)
            .expect_err("an existing runtime config must never be replaced");
        assert!(
            error
                .to_string()
                .contains("destination already exists and will not be replaced"),
            "{error}"
        );
    }
    #[test]
    fn inrou_canary_requires_exact_taira_client_identity_before_publication() {
        let mut config = crate::fallback_config();
        config.chain = DEFAULT_CHAIN_ID.into();
        config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
        ensure_canonical_taira_client_identity(&config).expect("canonical Taira identity");

        config.chain = "iroha3-taira".into();
        let chain_error = ensure_canonical_taira_client_identity(&config)
            .expect_err("retired chain alias must fail before publication");
        assert!(
            chain_error.to_string().contains("requires canonical chain"),
            "unexpected chain identity error: {chain_error:#}"
        );

        config.chain = DEFAULT_CHAIN_ID.into();
        config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT + 1;
        let discriminant_error = ensure_canonical_taira_client_identity(&config)
            .expect_err("wrong Taira discriminant must fail before publication");
        assert!(
            discriminant_error
                .to_string()
                .contains("requires chain discriminant"),
            "unexpected chain discriminant error: {discriminant_error:#}"
        );
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(11).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
}
