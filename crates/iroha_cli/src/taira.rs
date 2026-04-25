//! Taira public testnet diagnostics and write canaries.

use std::{
    fs,
    path::PathBuf,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result, eyre};
use iroha::{
    client::{Client as IrohaClient, TransactionWaitOptions, TransactionWaitTerminalStatus},
    config::Config,
    data_model::{
        account::{AccountId, address::ChainDiscriminantGuard},
        isi::Log,
        level::Level as LogLevel,
        metadata::Metadata,
        name::Name,
        prelude::{FindTransactions, HashOf, QueryBuilderExt, TransactionEntrypoint},
    },
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use iroha_primitives::json::Json as IrohaJson;
use norito::json::{self, Map, Value};
use reqwest::blocking::Client as HttpClient;
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{CliOutputFormat, Run, RunContext};

const DEFAULT_PUBLIC_ROOT: &str = "https://taira.sora.org";
const DEFAULT_CHAIN_ID: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 369;
const DEFAULT_GAS_ASSET_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const DEFAULT_ALIAS_PREFIX: &str = "taira-rollout-canary";
const DEFAULT_WRITE_TTL_MS: u64 = 120_000;
const DEFAULT_WRITE_STATUS_TIMEOUT_MS: u64 = 120_000;
const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v2";

const REQUIRED_MCP_TOOLS: &[&str] = &[
    "iroha.status",
    "iroha.sumeragi.status",
    "iroha.time.now",
    "iroha.musubi.search",
    "iroha.musubi.release.get",
    "iroha.musubi.instructions.yank_release",
    "iroha.transactions.submit",
    "iroha.transactions.submit_and_wait",
];

const ROUTE_CHECKS: &[(&str, &str)] = &[
    ("status", "/status"),
    ("sumeragi_status", "/v1/sumeragi/status"),
    ("sccp_capabilities", "/v1/sccp/capabilities"),
    ("zk_proofs_count", "/v1/zk/proofs/count"),
    ("validator_sets", "/v1/sumeragi/validator-sets"),
    ("public_lane_validators", "/v1/nexus/public_lanes/0/validators"),
    ("contracts_state", "/v1/contracts/state"),
    ("musubi_search", "/v1/musubi/packages?query=&limit=1"),
];

/// Taira public testnet helpers.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Check Taira read-side health and MCP route posture.
    Doctor(Doctor),
    /// Onboard, faucet, submit, wait, and verify a signed ping canary.
    WriteCanary(WriteCanary),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Doctor(cmd) => cmd.run(context),
            Self::WriteCanary(cmd) => cmd.run(context),
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
    /// Gas asset definition id inserted into transaction metadata.
    #[arg(long, default_value = DEFAULT_GAS_ASSET_ID)]
    pub gas_asset_id: String,
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
        let receipt = run_write_canary(context.config(), &self)?;
        render_report(context, self.json, &receipt)?;
        Ok(())
    }
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
    public_key_raw_hex: String,
    generated: bool,
}

fn run_doctor(public_root: &str) -> Result<Value> {
    let public_root = normalize_root_url(public_root)?;
    let http = http_client()?;
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let mut failures = Vec::new();

    for (name, path) in ROUTE_CHECKS {
        let url = join_url(&public_root, path)?;
        let result = http_json(&http, reqwest::Method::GET, url.as_str(), None)?;
        let ok = (200..300).contains(&result.status);
        push_check(&mut checks, name, result.status, ok, None);
        if !ok {
            failures.push(format!("{name} returned HTTP {}", result.status));
        }
        if *name == "status" && ok {
            collect_status_warnings(result.body.as_ref(), &mut warnings);
        }
    }

    let mcp_url = join_url(&public_root, "/v1/mcp")?;
    let mcp_get = http_json(&http, reqwest::Method::GET, mcp_url.as_str(), None)?;
    let mcp_get_ok = (200..300).contains(&mcp_get.status);
    push_check(
        &mut checks,
        "mcp_get",
        mcp_get.status,
        mcp_get_ok,
        None,
    );
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
    push_check(
        &mut checks,
        "mcp_tools_list",
        tools.status,
        tools_ok,
        None,
    );
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

fn run_write_canary(config: &Config, args: &WriteCanary) -> Result<Value> {
    let public_root = normalize_root_url(&args.public_root)?;
    let http = http_client()?;
    let signer = resolve_canary_signer(config, args.use_config_signer)?;
    let alias = build_alias(&args.alias_prefix, signer.key_pair.public_key(), "wonderland.universal");
    let mut warnings = Vec::new();
    let mut checks = Vec::new();
    let mut failures = Vec::new();

    let onboarding = onboard_canary(
        &http,
        &public_root,
        &alias,
        &signer.public_key_raw_hex,
        &mut warnings,
    )?;
    push_check(
        &mut checks,
        "accounts_onboard",
        onboarding.status,
        (200..300).contains(&onboarding.status) || onboarding.status == 400,
        onboarding.body.as_ref().map(compact_json),
    );

    let faucet = claim_faucet(&http, &public_root, &signer.account_id)?;
    push_check(
        &mut checks,
        "accounts_faucet",
        faucet.status,
        (200..300).contains(&faucet.status),
        faucet.body.as_ref().map(compact_json),
    );
    if !(200..300).contains(&faucet.status) {
        failures.push(faucet_failure_hint(&faucet));
    }

    let mut canary_config = config.clone();
    canary_config.chain = DEFAULT_CHAIN_ID.into();
    canary_config.torii_api_url = Url::parse(&format!("{public_root}/"))
        .wrap_err_with(|| format!("invalid public root `{public_root}`"))?;
    canary_config.account = signer.account_id.clone();
    canary_config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
    canary_config.key_pair = signer.key_pair.clone();
    canary_config.transaction_ttl = Duration::from_millis(DEFAULT_WRITE_TTL_MS);
    canary_config.transaction_status_timeout = Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS);
    canary_config.transaction_add_nonce = false;

    let _guard = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
    let client = IrohaClient::new(canary_config.clone());
    let mut metadata = metadata_with_gas_asset(&args.gas_asset_id)?;
    insert_string_metadata(&mut metadata, "taira_canary", "write-canary")?;
    let message = canary_message();
    let instruction = Log::new(LogLevel::INFO, message.clone());
    let transaction = client.build_transaction([instruction], metadata);
    let signed_hash = transaction.hash();
    let entrypoint_hash = HashOf::new(&TransactionEntrypoint::External(transaction.clone()));

    client
        .submit_transaction(&transaction)
        .map_err(|err| hint_submit_error(err, &args.gas_asset_id))?;
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

    if let Some(path) = &args.write_config {
        write_runtime_config(path, &canary_config)?;
    }

    let status = if failures.is_empty() { "ok" } else { "fail" };
    let mut extra = Map::new();
    insert_write_receipt_identity(&mut extra, &signer, &alias, &args.gas_asset_id);
    extra.insert("message".into(), Value::String(message));
    extra.insert("faucet_tx_hash".into(), extract_response_string(faucet.body.as_ref(), "tx_hash_hex"));
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
        let object = report.as_object().ok_or_else(|| eyre!("report must be an object"))?;
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
                let marker = if ok { "ok" } else { "fail" };
                context.println_data(format!("  {marker} {name} HTTP {status_code}"))?;
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

fn print_receipt_fields<C: RunContext>(context: &mut C, object: &Map) -> Result<()> {
    const RECEIPT_FIELDS: &[&str] = &[
        "chain",
        "chain_discriminant",
        "account_id",
        "alias",
        "gas_asset_id",
        "faucet_tx_hash",
        "ping_tx_hash",
        "applied_block_height",
        "terminal_kind",
        "tx_query_verified",
        "config_path",
    ];
    for field in RECEIPT_FIELDS {
        let Some(value) = object.get(*field).filter(|value| !matches!(value, Value::Null)) else {
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

fn normalize_root_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        eyre::bail!("public root URL must not be empty");
    }
    let parsed = Url::parse(trimmed).wrap_err_with(|| format!("invalid URL `{trimmed}`"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(trimmed.to_owned()),
        other => eyre::bail!("unsupported URL scheme `{other}`"),
    }
}

fn join_url(root: &str, path: &str) -> Result<Url> {
    let root = format!("{}/", root.trim_end_matches('/'));
    let suffix = path.trim_start_matches('/');
    Url::parse(&root)
        .and_then(|url| url.join(suffix))
        .wrap_err_with(|| format!("failed to build URL from `{root}` and `{path}`"))
}

fn http_client() -> Result<HttpClient> {
    HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("iroha-taira-devex/1")
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
    let status = response.status().as_u16();
    let text = response.text().unwrap_or_default();
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
        warnings.push(format!("recent rejected transactions in /status: {rejected}"));
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
        KeyPair::random_with_algorithm(Algorithm::Ed25519)
    };
    let (algorithm, public_key_bytes) = key_pair.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 {
        eyre::bail!("Taira canary signer must use Ed25519");
    }
    let account_id = AccountId::new(key_pair.public_key().clone());
    Ok(CanarySigner {
        public_key_raw_hex: hex::encode(public_key_bytes),
        account_id,
        key_pair,
        generated: !use_config_signer,
    })
}

fn insert_write_receipt_identity(
    extra: &mut Map,
    signer: &CanarySigner,
    alias: &str,
    gas_asset_id: &str,
) {
    extra.insert("chain".into(), Value::String(DEFAULT_CHAIN_ID.to_owned()));
    extra.insert(
        "chain_discriminant".into(),
        Value::from(u64::from(DEFAULT_CHAIN_DISCRIMINANT)),
    );
    extra.insert("account_id".into(), Value::String(signer.account_id.to_string()));
    extra.insert("alias".into(), Value::String(alias.to_owned()));
    extra.insert("generated_signer".into(), Value::from(signer.generated));
    extra.insert("gas_asset_id".into(), Value::String(gas_asset_id.to_owned()));
}

fn build_alias(prefix: &str, public_key: &iroha_crypto::PublicKey, domain: &str) -> String {
    let label_prefix = sanitize_alias_part(prefix).unwrap_or_else(|| "tairacanary".to_owned());
    let public_key = public_key.to_string();
    let suffix_source = public_key
        .get(public_key.len().saturating_sub(16)..)
        .unwrap_or(public_key.as_str());
    let suffix = sanitize_alias_part(suffix_source).unwrap_or_else(|| "signer".to_owned());
    let dataspace = domain
        .rsplit('.')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("universal")
        .to_ascii_lowercase();
    format!("{label_prefix}{suffix}@{dataspace}")
}

fn sanitize_alias_part(raw: &str) -> Option<String> {
    let sanitized: String = raw
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

fn onboard_canary(
    http: &HttpClient,
    public_root: &str,
    alias: &str,
    public_key_raw_hex: &str,
    warnings: &mut Vec<String>,
) -> Result<HttpJson> {
    let url = join_url(public_root, "/v1/accounts/onboard")?;
    let mut body = Map::new();
    body.insert("alias".into(), Value::String(alias.to_owned()));
    body.insert(
        "public_key_hex".into(),
        Value::String(public_key_raw_hex.to_owned()),
    );
    body.insert(
        "identity".into(),
        norito::json!({
            "source": "iroha taira write-canary"
        }),
    );
    let result = http_json(http, reqwest::Method::POST, url.as_str(), Some(&Value::Object(body)))?;
    if !(200..300).contains(&result.status) {
        warnings.push(format!(
            "account onboarding returned HTTP {}; continuing with faucet registration fallback",
            result.status
        ));
    }
    Ok(result)
}

fn claim_faucet(http: &HttpClient, public_root: &str, account_id: &AccountId) -> Result<HttpJson> {
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
    let claim_body = solve_faucet_puzzle(&account_id.to_string(), puzzle_body)?;
    let claim_url = join_url(public_root, "/v1/accounts/faucet")?;
    http_json(
        http,
        reqwest::Method::POST,
        claim_url.as_str(),
        Some(&claim_body),
    )
}

fn solve_faucet_puzzle(account_id: &str, puzzle: &Value) -> Result<Value> {
    let difficulty_bits = required_u64(puzzle, "difficulty_bits")?;
    let mut body = Map::new();
    body.insert("account_id".into(), Value::String(account_id.to_owned()));
    if difficulty_bits == 0 {
        return Ok(Value::Object(body));
    }
    let anchor_height = required_u64(puzzle, "anchor_height")?;
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
        anchor_height,
        anchor_hash_hex,
        challenge_salt_hex,
    )?;
    let params = ScryptParams::new(log_n, r, p, 32)
        .map_err(|err| eyre!("invalid faucet scrypt parameters: {err}"))?;
    let difficulty_bits = u32::try_from(difficulty_bits)
        .map_err(|_| eyre!("faucet difficulty is too large"))?;
    let nonce = solve_faucet_pow(&challenge, &params, difficulty_bits)?;
    body.insert("pow_anchor_height".into(), Value::from(anchor_height));
    body.insert("pow_nonce_hex".into(), Value::String(hex::encode(nonce)));
    Ok(Value::Object(body))
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
    anchor_height: u64,
    anchor_hash_hex: &str,
    challenge_salt_hex: Option<&str>,
) -> Result<[u8; 32]> {
    let anchor_hash = hex::decode(anchor_hash_hex)
        .wrap_err_with(|| format!("invalid faucet anchor hash `{anchor_hash_hex}`"))?;
    let mut hasher = Sha256::new();
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
    hasher.update(account_id.as_bytes());
    hasher.update(anchor_height.to_be_bytes());
    hasher.update(anchor_hash);
    if let Some(salt) = challenge_salt_hex.filter(|value| !value.is_empty()) {
        let salt = hex::decode(salt).wrap_err("invalid faucet challenge salt")?;
        hasher.update(salt);
    }
    Ok(hasher.finalize().into())
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

fn metadata_with_gas_asset(gas_asset_id: &str) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    if !gas_asset_id.trim().is_empty() {
        insert_string_metadata(&mut metadata, "gas_asset_id", gas_asset_id.trim())?;
    }
    Ok(metadata)
}

fn insert_string_metadata(metadata: &mut Metadata, key: &str, value: &str) -> Result<()> {
    metadata.insert(Name::from_str(key)?, IrohaJson::new(value.to_owned()));
    Ok(())
}

fn canary_message() -> String {
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("taira-write-canary-{unix_ms}")
}

fn write_runtime_config(path: &PathBuf, config: &Config) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("config path `{}` has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .wrap_err_with(|| format!("failed to create `{}`", parent.display()))?;
    let rendered = render_runtime_config(config)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, rendered).wrap_err_with(|| format!("failed to write `{}`", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .wrap_err_with(|| format!("failed to chmod `{}`", tmp.display()))?;
    }
    fs::rename(&tmp, path).wrap_err_with(|| {
        format!(
            "failed to replace runtime config `{}` with `{}`",
            path.display(),
            tmp.display()
        )
    })
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

fn hint_submit_error(err: eyre::Report, gas_asset_id: &str) -> eyre::Report {
    let text = format!("{err:#}");
    if text.contains("gas_asset_id") || text.contains("GasAsset") {
        eyre!(
            "{text}\nTaira requires transaction metadata `gas_asset_id`; this command used `{gas_asset_id}`. Re-check that the public endpoint accepts this asset definition id."
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
        eyre!("{text}\nThe canary transaction expired before application; inspect /status queue depth and validator health.")
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

fn extract_response_string(response: Option<&Value>, key: &str) -> Value {
    response
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .map(|value| Value::String(value.to_owned()))
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read as _, Write as _},
        net::{TcpListener, TcpStream},
        sync::{Arc, Mutex},
        thread,
    };

    #[derive(Clone, Debug)]
    struct MockRequest {
        method: String,
        path: String,
        body: String,
    }

    struct MockResponse {
        status: u16,
        content_type: &'static str,
        body: String,
    }

    impl MockResponse {
        fn json(status: u16, value: Value) -> Self {
            Self {
                status,
                content_type: "application/json",
                body: json::to_json(&value).expect("mock JSON response"),
            }
        }

        fn text(status: u16, body: impl Into<String>) -> Self {
            Self {
                status,
                content_type: "text/plain",
                body: body.into(),
            }
        }
    }

    struct MockHttpServer {
        base_url: String,
        requests: Arc<Mutex<Vec<MockRequest>>>,
        handle: thread::JoinHandle<()>,
    }

    fn spawn_mock_http<F>(expected_requests: usize, responder: F) -> MockHttpServer
    where
        F: Fn(&MockRequest) -> MockResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("mock server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let responder = Arc::new(responder);
        let handle = thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().expect("mock server accept");
                let request = read_mock_request(&mut stream);
                let response = responder(&request);
                server_requests
                    .lock()
                    .expect("requests")
                    .push(request.clone());
                write_mock_response(&mut stream, response);
            }
        });
        MockHttpServer {
            base_url: format!("http://{addr}"),
            requests,
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
        let body = String::from_utf8_lossy(&raw[header_end + 4..]).to_string();
        MockRequest { method, path, body }
    }

    fn find_header_end(raw: &[u8]) -> Option<usize> {
        raw.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn write_mock_response(stream: &mut TcpStream, response: MockResponse) {
        let reason = match response.status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            503 => "Service Unavailable",
            _ => "OK",
        };
        let body = response.body.as_bytes();
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.status,
            reason,
            response.content_type,
            body.len()
        )
        .expect("write mock response headers");
        stream.write_all(body).expect("write mock response body");
    }

    fn finish_mock(server: MockHttpServer) -> Vec<MockRequest> {
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

    fn write_canary_mock_response(request: &MockRequest, onboarding_status: u16) -> MockResponse {
        match (request.method.as_str(), path_only(&request.path)) {
            ("POST", "/v1/accounts/onboard") => {
                if onboarding_status == 400 {
                    MockResponse::json(
                        400,
                        norito::json!({
                            "error_code": "account_already_exists",
                            "message": "account already exists",
                            "hint": "continue with faucet registration fallback"
                        }),
                    )
                } else {
                    MockResponse::json(
                        200,
                        norito::json!({
                            "account_id": "mock",
                            "uaid": "uaid:mock",
                            "tx_hash_hex": "onboardabc",
                            "status": "Applied",
                            "lease": {
                                "alias": "mock@universal",
                                "account_id": "mock",
                                "dataspace": "universal",
                                "domain": "wonderland.universal",
                                "expires_at_ms": null,
                                "auto_renew": false
                            }
                        }),
                    )
                }
            }
            ("GET", "/v1/accounts/faucet/puzzle") => {
                MockResponse::json(200, norito::json!({ "difficulty_bits": 0 }))
            }
            ("POST", "/v1/accounts/faucet") => MockResponse::json(
                200,
                norito::json!({
                    "account_id": "mock",
                    "asset_definition_id": DEFAULT_GAS_ASSET_ID,
                    "asset_id": "mock",
                    "amount": "1000000000000000000",
                    "tx_hash_hex": "faucetabc",
                    "status": "Applied"
                }),
            ),
            ("GET", "/v1/node/capabilities") => MockResponse::text(404, "not advertised"),
            ("POST", "/transaction") => MockResponse::text(200, ""),
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
                        "hash": hash,
                        "status": { "kind": "Applied", "block_height": 42 },
                        "scope": "auto",
                        "resolved_from": "state"
                    }),
                )
            }
            ("POST", "/query") => MockResponse::text(404, "query unavailable in mock"),
            _ => MockResponse::text(404, "not found"),
        }
    }

    #[test]
    fn doctor_mock_healthy_flow_reports_ok() {
        let server = spawn_mock_http(11, |request| doctor_mock_response(request, None));
        let report = run_doctor(&server.base_url).expect("doctor report");
        let requests = finish_mock(server);

        assert_eq!(report_status(&report), Some("ok"));
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
    }

    #[test]
    fn doctor_mock_required_tool_missing_reports_failure() {
        let missing_tool = REQUIRED_MCP_TOOLS[0];
        let server = spawn_mock_http(11, move |request| {
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
        let server = spawn_mock_http(7, |request| write_canary_mock_response(request, 200));
        let args = WriteCanary {
            public_root: server.base_url.clone(),
            alias_prefix: "mock-canary".to_owned(),
            gas_asset_id: DEFAULT_GAS_ASSET_ID.to_owned(),
            write_config: None,
            use_config_signer: false,
            json: true,
        };
        let report = run_write_canary(&crate::fallback_config(), &args).expect("write canary");
        let requests = finish_mock(server);
        let rendered = compact_json(&report);

        assert_eq!(report_status(&report), Some("ok"));
        assert!(rendered.contains("faucetabc"));
        assert!(rendered.contains(DEFAULT_CHAIN_ID));
        assert!(rendered.contains(DEFAULT_GAS_ASSET_ID));
        assert!(!rendered.contains("private_key"));
        assert!(
            requests
                .iter()
                .any(|request| request.method == "POST" && path_only(&request.path) == "/transaction")
        );
    }

    #[test]
    fn write_canary_mock_onboarding_400_continues_to_faucet_fallback() {
        let server = spawn_mock_http(7, |request| write_canary_mock_response(request, 400));
        let args = WriteCanary {
            public_root: server.base_url.clone(),
            alias_prefix: "mock-canary".to_owned(),
            gas_asset_id: DEFAULT_GAS_ASSET_ID.to_owned(),
            write_config: None,
            use_config_signer: false,
            json: true,
        };
        let report = run_write_canary(&crate::fallback_config(), &args).expect("write canary");
        let requests = finish_mock(server);
        let warnings = report
            .as_object()
            .and_then(|object| object.get("warnings"))
            .and_then(Value::as_array)
            .expect("warnings");

        assert_eq!(report_status(&report), Some("ok"));
        assert!(
            warnings
                .iter()
                .filter_map(Value::as_str)
                .any(|warning| warning.contains("faucet registration fallback"))
        );
        assert!(
            requests
                .iter()
                .any(|request| request.method == "POST"
                    && path_only(&request.path) == "/v1/accounts/faucet")
        );
    }

    #[test]
    fn submit_failure_hints_cover_missing_gas_and_route_unavailable() {
        let missing_gas = hint_submit_error(eyre!("missing gas_asset_id"), DEFAULT_GAS_ASSET_ID);
        assert!(format!("{missing_gas:#}").contains("Taira requires transaction metadata"));

        let route = hint_submit_error(eyre!("route_unavailable"), DEFAULT_GAS_ASSET_ID);
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
        let challenge = build_faucet_challenge(
            "testu1example",
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("challenge");
        assert_eq!(challenge.len(), 32);
        assert_ne!(challenge, [0_u8; 32]);
    }

    #[test]
    fn metadata_with_gas_asset_inserts_string_value() {
        let metadata = metadata_with_gas_asset(DEFAULT_GAS_ASSET_ID).expect("metadata");
        let key = Name::from_str("gas_asset_id").expect("key");
        let rendered = metadata.get(&key).expect("gas asset").to_string();
        assert!(rendered.contains(DEFAULT_GAS_ASSET_ID));
    }

    #[test]
    fn solve_faucet_puzzle_handles_zero_difficulty_without_pow_fields() {
        let puzzle = norito::json!({
            "difficulty_bits": 0,
        });
        let body = solve_faucet_puzzle("testu1example", &puzzle).expect("claim body");
        let body = body.as_object().expect("object");
        assert_eq!(
            body.get("account_id").and_then(Value::as_str),
            Some("testu1example")
        );
        assert!(!body.contains_key("pow_nonce_hex"));
    }

    #[test]
    fn resolve_canary_signer_derives_account_and_raw_public_key_hex() {
        let key_pair = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        let mut config = crate::fallback_config();
        config.key_pair = key_pair.clone();
        let signer = resolve_canary_signer(&config, true).expect("config signer");
        let (_, public_key_bytes) = key_pair.public_key().to_bytes();

        assert!(!signer.generated);
        assert_eq!(signer.account_id, AccountId::new(key_pair.public_key().clone()));
        assert_eq!(signer.public_key_raw_hex, hex::encode(public_key_bytes));
    }

    #[test]
    fn write_canary_receipt_identity_is_redacted() {
        let key_pair = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519);
        let signer = CanarySigner {
            account_id: AccountId::new(key_pair.public_key().clone()),
            public_key_raw_hex: "11".repeat(32),
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
    fn build_alias_is_stable_and_sanitized() {
        let key_pair = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
        let alias = build_alias("Taira Rollout Canary!", key_pair.public_key(), "wonderland.universal");
        assert!(alias.starts_with("tairarolloutcanary"));
        assert!(alias.ends_with("@universal"));
        assert!(alias.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '@'));
    }

    #[test]
    fn render_runtime_config_redacts_nothing_only_when_explicitly_called() {
        let key_pair = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
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
}
