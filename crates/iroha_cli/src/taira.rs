//! Taira public testnet diagnostics and write canaries.

use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result, eyre};
use iroha::{
    client::{Client as IrohaClient, TransactionWaitOptions, TransactionWaitTerminalStatus},
    config::Config,
    data_model::{
        account::{AccountId, address::ChainDiscriminantGuard},
        isi::{InstructionBox, Log},
        level::Level as LogLevel,
        metadata::Metadata,
        name::Name,
        prelude::{FindTransactions, HashOf, QueryBuilderExt, TransactionEntrypoint},
        transaction::{Executable, FeePaymentIntent},
    },
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use iroha_primitives::json::Json as IrohaJson;
use norito::json::{self, Map, Value};
use reqwest::blocking::Client as HttpClient;
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroizing;

use crate::{CliOutputFormat, Run, RunContext, quote_and_sign_transaction};

const DEFAULT_PUBLIC_ROOT: &str = "https://taira.sora.org";
const DEFAULT_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 369;
const DEFAULT_GAS_ASSET_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const DEFAULT_ALIAS_PREFIX: &str = "taira-rollout-canary";
const DEFAULT_WRITE_TTL_MS: u64 = 120_000;
const DEFAULT_WRITE_STATUS_TIMEOUT_MS: u64 = 120_000;
const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v2";
const ACCOUNT_ONBOARDING_TOKEN_HEADER: &str = "x-iroha-onboarding-token";

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

const ROUTE_CHECKS: &[(&str, &str, &[u16])] = &[
    ("status", "/status", &[200]),
    ("sumeragi_status", "/v1/sumeragi/status", &[200]),
    (
        "pipeline_transaction_status",
        "/v1/pipeline/transactions/status",
        &[400],
    ),
    (
        "retired_transaction_status_alias",
        "/v1/transactions/status",
        &[404],
    ),
    ("sccp_capabilities", "/v1/sccp/capabilities", &[200]),
    ("zk_proofs_count", "/v1/zk/proofs/count", &[200]),
    ("validator_sets", "/v1/sumeragi/validator-sets", &[200]),
    (
        "public_lane_validators",
        "/v1/nexus/public-lanes/0/validators",
        &[200],
    ),
    // A missing selector should reach the mounted contract-state route and be
    // rejected as bad input. Treating that as mounted keeps the doctor aligned
    // with the rollout harness instead of requiring a real contract key.
    ("contracts_state", "/v1/contracts/state", &[400]),
    (
        "musubi_search",
        "/v1/musubi/packages?query=&limit=1",
        &[200],
    ),
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

    for (name, path, expected_statuses) in ROUTE_CHECKS {
        let url = join_url(&public_root, path)?;
        let result = http_json(&http, reqwest::Method::GET, url.as_str(), None)?;
        let ok = expected_statuses.contains(&result.status);
        push_check(
            &mut checks,
            name,
            result.status,
            ok,
            route_check_detail(expected_statuses),
        );
        if !ok {
            failures.push(format!(
                "{name} returned HTTP {}; expected {}",
                result.status,
                expected_statuses
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
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
    );
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
            "account onboarding planning failed with HTTP {}; apply and faucet funding were not attempted",
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
        let onboarding_applied = onboarding_final.status == 200
            && matches!(
                onboarding_terminal.as_deref(),
                Some("Applied" | "Committed")
            );
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

    let faucet = claim_faucet(&http, &public_root, &signer.account_id)?;
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
    let faucet_applied = faucet_final.status == 200
        && matches!(faucet_terminal.as_deref(), Some("Applied" | "Committed"));
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
    let message = canary_message();
    let instruction = Log::new(LogLevel::INFO, message.clone());
    let executable = Executable::Instructions(vec![InstructionBox::from(instruction)].into());
    let (transaction, fee_quote) =
        quote_and_sign_transaction(&client, executable, fee_payment, metadata)
            .wrap_err("failed to quote and sign Taira canary transaction")?;
    let signed_hash = transaction.hash();
    let entrypoint_hash = HashOf::new(&TransactionEntrypoint::External(transaction.clone()));

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
    file.by_ref()
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

fn decode_http_json_response(response: reqwest::blocking::Response) -> Result<HttpJson> {
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
        let terminal = matches!(
            pipeline_status_kind(response.body.as_ref()).as_deref(),
            Some("Applied" | "Committed" | "Rejected" | "Expired")
        );
        if terminal {
            return Ok(response);
        }
        if Instant::now() >= deadline {
            response.status = 504;
            return Ok(response);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
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
    let difficulty_bits =
        u32::try_from(difficulty_bits).map_err(|_| eyre!("faucet difficulty is too large"))?;
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
        io::Write as _,
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
            ("GET", "/v1/contracts/state") => {
                MockResponse::json(400, norito::json!({"error": "missing selector"}))
            }
            ("GET", "/v1/pipeline/transactions/status") => {
                MockResponse::json(400, norito::json!({"error": "missing transaction hash"}))
            }
            ("GET", "/v1/transactions/status") => MockResponse::text(404, "not found"),
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
            ("GET", "/v1/accounts/faucet/puzzle") => {
                MockResponse::json(200, norito::json!({ "difficulty_bits": 0 }))
            }
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
            ("GET", "/v1/node/capabilities") => MockResponse::text(404, "not advertised"),
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

    #[test]
    fn doctor_mock_healthy_flow_reports_ok() {
        let server = spawn_mock_http(13, |request| doctor_mock_response(request, None));
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
        assert!(requests.iter().any(|request| {
            request.method == "GET"
                && path_only(&request.path) == "/v1/pipeline/transactions/status"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "GET" && path_only(&request.path) == "/v1/transactions/status"
        }));
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
        let server = spawn_mock_http(13, move |request| {
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
        let server = spawn_mock_http(11, |request| write_canary_mock_response(request, 202));
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
        let server = spawn_mock_http(1, |request| write_canary_mock_response(request, 400));
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
        let challenge =
            build_faucet_challenge("testu1example", 7, &"11".repeat(32), Some(&"22".repeat(32)))
                .expect("challenge");
        assert_eq!(challenge.len(), 32);
        assert_ne!(challenge, [0_u8; 32]);
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
    fn build_alias_is_stable_and_sanitized() {
        let key_pair = fixture_key_pair(7);
        let alias = build_alias(
            "Taira Rollout Canary!",
            key_pair.public_key(),
            "wonderland.universal",
        );
        assert!(alias.starts_with("tairarolloutcanary"));
        assert!(alias.ends_with("@universal"));
        assert!(
            alias
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '@')
        );
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

    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(11).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
}
