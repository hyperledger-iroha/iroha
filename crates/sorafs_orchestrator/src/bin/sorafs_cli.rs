//! Aggregated CLI entry point for SoraFS packaging helpers.
#![allow(unexpected_cfgs)]

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    env,
    fmt::Write as FmtWrite,
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
    process,
    str::FromStr,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{
    Engine,
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, URL_SAFE as BASE64_URL_SAFE,
        URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD,
    },
};
use blake3::hash as blake3_hash;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hex::encode as hex_encode;
use iroha_config::parameters::defaults::streaming::soranet::PROVISION_SPOOL_DIR;
use iroha_crypto::{Algorithm, PrivateKey, PublicKey};
use iroha_data_model::{
    account::{AccountId, address::AccountAddress},
    da::types::{BlobDigest, StorageTicketId},
    name::Name,
    prelude::ExposedPrivateKey,
    sorafs::{
        moderation::{AdversarialCorpusManifestV1, ModerationReproManifestV1},
        pin_registry::{
            ChunkerProfileHandle, PinPolicy as RegistryPinPolicy,
            StorageClass as RegistryStorageClass,
        },
    },
    taikai::{
        TaikaiAudioLayout, TaikaiCodec, TaikaiEventId, TaikaiRenditionId, TaikaiResolution,
        TaikaiStreamId, TaikaiTrackMetadata,
    },
};
use iroha_version::codec::EncodeVersioned;
use ivm::kotodama::compiler::Compiler;
use norito::{
    core::DecodeFromSlice,
    decode_from_bytes,
    derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{Map, Number, Value, from_slice, to_string_pretty, to_value, to_vec},
    to_bytes,
};
use reqwest::{StatusCode, blocking::Client as HttpClient, header::CONTENT_TYPE};
use rust_decimal::Decimal;
use sha3::{Digest, Sha3_256};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarStreamingWriter, CarVerifier, CarWriteError, ChunkFetchSpec,
    FileEntry, FilePlan, StoredChunk,
    chunker_registry::{self, ChunkerProfileDescriptor},
    fetch_plan::{chunk_fetch_specs_from_json, chunk_fetch_specs_to_string},
    gateway::{GatewayFetchConfig, GatewayFetchContext, GatewayProviderInput},
    multi_fetch::{ProviderMetadata, RangeCapability, StreamBudget},
    policy::{PolicyEvidenceValidator, run_honey_probe},
    proof_stream::{
        ProofKind, ProofStreamItem, ProofStreamMetrics, ProofStreamRequest, ProofStreamSummary,
        ProofTier,
    },
    scoreboard::{Eligibility, TelemetrySnapshot},
    taikai::{BundleRequest, BundleSummary, bundle_segment, load_extra_metadata},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    ChunkingProfileV1, DagCodecId, GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1,
    GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
    GovernanceLogSignatureV1, GovernanceSignatureAlgorithm, MANIFEST_DAG_CODEC, ManifestBuildError,
    ManifestBuilder, ManifestV1, ManualPorChallengeV1, PinPolicy, PorChallengeOutcome,
    PorChallengeStatusV1, PorReportIsoWeek, PorWeeklyReportV1, ReputationMerkleProofV1,
    ReputationSnapshotV1, StorageClass, ValidationOutcomeV1,
    chunker_registry as manifest_chunker_registry, governance_dag_block_cid_v1,
    validate_governance_dag_head_against_chain_v1, validate_governance_log_node_bytes,
};
use sorafs_orchestrator::{
    AnonymityPolicy, FetchSession, OrchestratorConfig, RolloutPhase, TransportPolicy,
    WriteModeHint,
    appeals::{
        AppealClass, AppealClassConfig, AppealDecision, AppealDisbursementError,
        AppealDisbursementInput, AppealDisbursementPlan, AppealPricingConfig, AppealQuote,
        AppealQuoteInput, AppealSettlementBreakdown, AppealSettlementConfig, AppealSettlementError,
        AppealUrgency, AppealVerdict,
    },
    bindings::{
        config_from_json as orchestrator_config_from_json,
        config_to_json as orchestrator_config_to_json,
    },
    fetch_via_gateway,
    proxy::{ProxyKaigiBridgeConfig, ProxyMode, ProxyNoritoBridgeConfig},
    taikai_cache::{TaikaiCacheConfig, TaikaiCacheStatsSnapshot, TaikaiPullQueueStats},
};
use tokio::runtime::Runtime;

const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");
use url::{Url, form_urlencoded::Serializer};

const DEFAULT_CHUNKER_HANDLE: &str = "sorafs.sf1@1.0.0";
const DEFAULT_IDENTITY_TOKEN_ENV: &str = "SIGSTORE_ID_TOKEN";
const DEFAULT_MANIFEST_SUBMIT_CONFIRM_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MANIFEST_SUBMIT_CONFIRM_POLL_MS: u64 = 1_000;
const POR_TRIGGER_REQUEST_VERSION_V1: u8 = 1;
const CHALLENGE_AUTH_TOKEN_VERSION_V1: u8 = 1;
const CONTEXT_APPEAL_QUOTE: &str = "sorafs_cli appeal quote";
const CONTEXT_APPEAL_SETTLE: &str = "sorafs_cli appeal settle";
const CONTEXT_APPEAL_DISBURSE: &str = "sorafs_cli appeal disburse";

fn parse_u32_arg(flag: &str, raw: &str, context: &str) -> Result<u32, String> {
    raw.trim()
        .parse::<u32>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}

fn parse_u64_arg(flag: &str, raw: &str, context: &str) -> Result<u64, String> {
    raw.trim()
        .parse::<u64>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}

fn parse_u16_arg(flag: &str, raw: &str, context: &str) -> Result<u16, String> {
    raw.trim()
        .parse::<u16>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}

fn parse_decimal_arg(flag: &str, raw: &str, context: &str) -> Result<Decimal, String> {
    raw.trim()
        .parse::<Decimal>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}

fn infer_i105_network_prefix(raw: &str) -> Option<u16> {
    let trimmed = raw.trim();
    if trimmed.starts_with("sora") {
        return Some(753);
    }
    if trimmed.starts_with("test") {
        return Some(369);
    }
    if trimmed.starts_with("dev") {
        return Some(0);
    }
    trimmed
        .strip_prefix('n')
        .and_then(|digits| digits.parse::<u16>().ok())
}

fn parse_account_id_arg(flag: &str, raw: &str, context: &str) -> Result<AccountId, String> {
    let trimmed = raw.trim();
    if let Some(prefix) = infer_i105_network_prefix(trimmed)
        && let Ok(address) = AccountAddress::from_i105_for_discriminant(trimmed, Some(prefix))
    {
        return address.to_account_id().map_err(|err| {
            format!("failed to decode `{flag}` for `{context}` as account id: {err}")
        });
    }
    if let Ok(address) = AccountAddress::from_i105(trimmed) {
        return address.to_account_id().map_err(|err| {
            format!("failed to decode `{flag}` for `{context}` as account id: {err}")
        });
    }
    AccountId::parse_encoded(trimmed)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| format!("failed to parse `{flag}` for `{context}` as account id: {err}"))
}

fn parse_account_id_arg_with_prefix(
    flag: &str,
    raw: &str,
    context: &str,
    expected_prefix: Option<u16>,
) -> Result<AccountId, String> {
    let trimmed = raw.trim();
    let address = match expected_prefix.or_else(|| infer_i105_network_prefix(trimmed)) {
        Some(prefix) => AccountAddress::from_i105_for_discriminant(trimmed, Some(prefix)),
        None => AccountAddress::from_i105(trimmed),
    };
    if let Ok(address) = address {
        return address.to_account_id().map_err(|err| {
            format!("failed to decode `{flag}` for `{context}` as account id: {err}")
        });
    }

    if expected_prefix.is_none() {
        return parse_account_id_arg(flag, trimmed, context);
    }

    AccountId::parse_encoded(trimmed)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| format!("failed to parse `{flag}` for `{context}` as account id: {err}"))
}

fn authority_payload_literal(
    authority: &AccountId,
    network_prefix: Option<u16>,
) -> Result<String, String> {
    match network_prefix {
        Some(prefix) => authority
            .to_i105_for_discriminant(prefix)
            .map_err(|err| format!("failed to encode authority for payload: {err}")),
        None => authority
            .canonical_i105()
            .map_err(|err| format!("failed to encode authority for payload: {err}")),
    }
}

fn parse_appeal_verdict(value: &str) -> Result<AppealVerdict, String> {
    let normalized = value.trim().to_ascii_lowercase();
    let verdict = match normalized.as_str() {
        "uphold" => AppealVerdict::Decision(AppealDecision::Uphold),
        "overturn" => AppealVerdict::Decision(AppealDecision::Overturn),
        "modify" => AppealVerdict::Decision(AppealDecision::Modify),
        "withdrawn_before_panel" | "withdrawn-before-panel" | "withdrawn_pre" => {
            AppealVerdict::WithdrawnBeforePanel
        }
        "withdrawn_after_panel" | "withdrawn-after-panel" | "withdrawn_post" => {
            AppealVerdict::WithdrawnAfterPanel
        }
        "frivolous" => AppealVerdict::Frivolous,
        "escalated" | "pending" => AppealVerdict::Escalated,
        other => {
            return Err(format!(
                "unsupported `--outcome={other}` for `{CONTEXT_APPEAL_SETTLE}`; expected uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated"
            ));
        }
    };
    Ok(verdict)
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
struct ChallengeAuthScopeV1 {
    manifest_digest: [u8; 32],
    #[norito(default)]
    providers: Vec<[u8; 32]>,
    #[norito(default)]
    max_requests: u16,
}

impl<'a> DecodeFromSlice<'a> for ChallengeAuthScopeV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
struct ChallengeAuthTokenV1 {
    version: u8,
    token_id: String,
    operator_account: String,
    expires_at: u64,
    #[norito(default)]
    scopes: Vec<ChallengeAuthScopeV1>,
    #[norito(default)]
    justification: Option<String>,
}

impl<'a> DecodeFromSlice<'a> for ChallengeAuthTokenV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl ChallengeAuthTokenV1 {
    fn validate(&self, manifest: &[u8; 32], provider: &[u8; 32]) -> Result<(), String> {
        if self.version != CHALLENGE_AUTH_TOKEN_VERSION_V1 {
            return Err(format!(
                "unsupported challenge auth token version {}; expected {CHALLENGE_AUTH_TOKEN_VERSION_V1}",
                self.version
            ));
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_secs();
        if now >= self.expires_at {
            return Err(format!(
                "challenge auth token `{}` expired at {}",
                self.token_id, self.expires_at
            ));
        }
        if self
            .scopes
            .iter()
            .any(|scope| scope.allows(manifest, provider))
        {
            Ok(())
        } else {
            Err("challenge auth token does not permit this manifest/provider pair".to_string())
        }
    }
}

impl ChallengeAuthScopeV1 {
    fn allows(&self, manifest: &[u8; 32], provider: &[u8; 32]) -> bool {
        if &self.manifest_digest != manifest {
            return false;
        }
        if self.providers.is_empty() {
            return true;
        }
        self.providers.iter().any(|candidate| candidate == provider)
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
struct ManualPorTriggerRequestV1 {
    version: u8,
    challenge: ManualPorChallengeV1,
    auth_token: ChallengeAuthTokenV1,
}

#[derive(Clone, Copy, Debug)]
enum IdentityTokenProvider {
    GithubActions,
}

fn resolve_identity_token(
    inline: Option<String>,
    env_var: Option<String>,
    file_path: Option<PathBuf>,
    provider: Option<IdentityTokenProvider>,
    audience_override: Option<String>,
) -> Result<(String, String), String> {
    if let Some(provider) = provider {
        if inline.is_some() || env_var.is_some() || file_path.is_some() {
            return Err("identity token provider flags cannot be combined with explicit identity token options (`--identity-token*`)".to_string());
        }
        return fetch_identity_token_from_provider(provider, audience_override);
    }

    if audience_override.is_some() {
        return Err("`--identity-token-audience` requires `--identity-token-provider`".to_string());
    }

    match (inline, env_var, file_path) {
        (Some(token), None, None) => {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                return Err("`--identity-token` may not be empty".to_string());
            }
            Ok((trimmed.to_string(), "inline".to_string()))
        }
        (None, Some(var), None) => {
            let value = env::var(&var)
                .map_err(|err| format!("failed to read identity token from `{var}`: {err}"))?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(format!("environment variable `{var}` is empty"));
            }
            Ok((trimmed.to_string(), format!("env:{var}")))
        }
        (None, None, Some(path)) => {
            let contents = fs::read_to_string(&path).map_err(|err| {
                format!(
                    "failed to read identity token from `{}`: {err}",
                    path.display()
                )
            })?;
            let trimmed = contents.trim();
            if trimmed.is_empty() {
                return Err(format!("identity token file `{}` is empty", path.display()));
            }
            Ok((trimmed.to_string(), format!("file:{}", path.display())))
        }
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => Err(
            "identity token flags are mutually exclusive; provide exactly one of `--identity-token`, `--identity-token-env`, `--identity-token-file`, or `--identity-token-provider`"
                .to_string(),
        ),
        (None, None, None) => {
            let value = env::var(DEFAULT_IDENTITY_TOKEN_ENV).map_err(|_| {
                "missing identity token for `sorafs_cli manifest sign`; supply `--identity-token`, `--identity-token-env`, `--identity-token-file`, or set SIGSTORE_ID_TOKEN".to_string()
            })?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(format!(
                    "environment variable `{DEFAULT_IDENTITY_TOKEN_ENV}` is set but empty"
                ));
            }
            Ok((
                trimmed.to_string(),
                format!("env:{DEFAULT_IDENTITY_TOKEN_ENV}"),
            ))
        }
    }
}

fn fetch_identity_token_from_provider(
    provider: IdentityTokenProvider,
    audience_override: Option<String>,
) -> Result<(String, String), String> {
    match provider {
        IdentityTokenProvider::GithubActions => {
            let audience_raw = audience_override.ok_or_else(|| {
                "`--identity-token-provider` requires --identity-token-audience".to_string()
            })?;
            let trimmed = audience_raw.trim();
            if trimmed.is_empty() {
                return Err("`--identity-token-audience` may not be empty".to_string());
            }
            let audience = trimmed.to_string();
            let token = request_github_actions_token(&audience)?;
            Ok((token, provider.label(&audience)))
        }
    }
}

fn request_github_actions_token(audience: &str) -> Result<String, String> {
    let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {
        "ACTIONS_ID_TOKEN_REQUEST_URL is not set; GitHub Actions OIDC tokens require running inside GitHub Actions with the id-token permission enabled".to_string()
    })?;
    let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {
        "ACTIONS_ID_TOKEN_REQUEST_TOKEN is not set; GitHub Actions OIDC tokens require running inside GitHub Actions with the id-token permission enabled".to_string()
    })?;

    let mut url = Url::parse(&raw_url)
        .map_err(|err| format!("failed to parse ACTIONS_ID_TOKEN_REQUEST_URL: {err}"))?;
    let mut existing_pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let mut replaced = false;
    for (key, value) in &mut existing_pairs {
        if key == "audience" {
            *value = audience.to_string();
            replaced = true;
        }
    }
    if !replaced {
        existing_pairs.push(("audience".into(), audience.to_string()));
    }
    url.set_query(None);
    if !existing_pairs.is_empty() {
        let mut serializer = Serializer::new(String::new());
        for (key, value) in existing_pairs {
            serializer.append_pair(&key, &value);
        }
        let query = serializer.finish();
        url.set_query(Some(&query));
    }

    let client = HttpClient::new();
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {request_token}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|err| format!("failed to request GitHub Actions OIDC token: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read GitHub Actions OIDC response: {err}"))?;
    if !status.is_success() {
        let snippet = String::from_utf8_lossy(&body);
        return Err(format!(
            "GitHub Actions OIDC token request failed with status {status}: {snippet}"
        ));
    }
    let json: Value = from_slice(&body)
        .map_err(|err| format!("failed to parse GitHub Actions OIDC response JSON: {err}"))?;
    let token = json
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| "GitHub Actions OIDC response missing `value` field".to_string())?;
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err("GitHub Actions OIDC response returned an empty token".to_string());
    }
    Ok(trimmed.to_string())
}

impl IdentityTokenProvider {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "github-actions" | "gha" => Ok(Self::GithubActions),
            other => Err(format!(
                "unsupported identity token provider `{other}`; supported providers: github-actions"
            )),
        }
    }

    fn label(self, audience: &str) -> String {
        match self {
            Self::GithubActions => format!("oidc:github-actions({audience})"),
        }
    }
}
fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        return Err(usage());
    };

    match first.as_str() {
        "car" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "pack" => car_pack(args.collect()),
                _ => Err(usage()),
            }
        }
        "manifest" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "build" => manifest_build(args.collect()),
                "sign" => manifest_sign(args.collect()),
                "verify-signature" => manifest_verify_signature(args.collect()),
                "submit" => manifest_submit(args.collect()),
                "proposal" => manifest_proposal(args.collect()),
                _ => Err(usage()),
            }
        }
        "norito" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "build" => norito_build(args.collect()),
                _ => Err(usage()),
            }
        }
        "deploy" => deploy(args.collect()),
        "fetch" => fetch_gateway(args.collect()),
        "proof" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "stream" => proof_stream(args.collect()),
                "verify" => proof_verify(args.collect()),
                _ => Err(usage()),
            }
        }
        "reputation" => {
            let Some(sub) = args.next() else {
                return Err(reputation_usage());
            };
            match sub.as_str() {
                "fetch" => reputation_fetch(args.collect()),
                "publish" => reputation_publish(args.collect()),
                "snapshot" => reputation_snapshot(args.collect()),
                "watch" => reputation_watch(args.collect()),
                "verify" => reputation_verify(args.collect()),
                _ => Err(reputation_usage()),
            }
        }
        "storage" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "prepare" => storage_prepare(args.collect()),
                "pin" => storage_pin(args.collect()),
                _ => Err(usage()),
            }
        }
        "por" => {
            let Some(sub) = args.next() else {
                return Err(por_usage());
            };
            match sub.as_str() {
                "status" => por_status(args.collect()),
                "trigger" => por_trigger(args.collect()),
                "export" => por_export(args.collect()),
                "report" => por_report(args.collect()),
                _ => Err(por_usage()),
            }
        }
        "proxy" => {
            let Some(sub) = args.next() else {
                return Err(proxy_usage());
            };
            match sub.as_str() {
                "set-mode" => proxy_set_mode(args.collect()),
                _ => Err(proxy_usage()),
            }
        }
        "taikai" => {
            let Some(sub) = args.next() else {
                return Err(taikai_usage());
            };
            match sub.as_str() {
                "bundle" => taikai_bundle(args.collect()),
                _ => Err(taikai_usage()),
            }
        }
        "moderation" => {
            let Some(sub) = args.next() else {
                return Err(moderation_usage());
            };
            match sub.as_str() {
                "validate-repro" => moderation_validate_repro(args.collect()),
                "validate-corpus" => moderation_validate_corpus(args.collect()),
                "honey-audit" => moderation_honey_audit(args.collect()),
                _ => Err(moderation_usage()),
            }
        }
        "appeal" => {
            let Some(sub) = args.next() else {
                return Err(appeal_usage());
            };
            match sub.as_str() {
                "quote" => appeal_quote(args.collect()),
                "settle" => appeal_settle(args.collect()),
                "disburse" => appeal_disburse(args.collect()),
                _ => Err(appeal_usage()),
            }
        }
        "governance" => {
            let Some(sub) = args.next() else {
                return Err(governance_usage());
            };
            match sub.as_str() {
                "dag" => {
                    let Some(dag_sub) = args.next() else {
                        return Err(governance_usage());
                    };
                    match dag_sub.as_str() {
                        "list" => governance_dag_list(args.collect()),
                        "show" => governance_dag_show(args.collect()),
                        "verify" => governance_dag_verify(args.collect()),
                        "export" => governance_dag_export(args.collect()),
                        "build" => governance_dag_build(args.collect()),
                        "verify-build" => governance_dag_verify_build(args.collect()),
                        "rebuild-head" => governance_dag_rebuild_head(args.collect()),
                        "checkpoint" => governance_dag_checkpoint(args.collect()),
                        "checkpoint-verify" => governance_dag_checkpoint_verify(args.collect()),
                        "checkpoint-recover" => governance_dag_checkpoint_recover(args.collect()),
                        "mirror-build" => governance_dag_mirror_build(args.collect()),
                        "mirror-query" => governance_dag_mirror_query(args.collect()),
                        _ => Err(governance_usage()),
                    }
                }
                _ => Err(governance_usage()),
            }
        }
        "--help" | "-h" | "help" => Err(usage()),
        _ => Err(usage()),
    }
}

fn car_pack(raw_args: Vec<String>) -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut chunker_handle: Option<String> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut plan_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--input" => input = Some(PathBuf::from(value)),
            "--chunker-handle" => chunker_handle = Some(value.to_string()),
            "--car-out" => car_out = Some(PathBuf::from(value)),
            "--plan-out" => plan_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli car pack`"
                ));
            }
        }
    }

    let input_path = input
        .ok_or_else(|| "missing required `--input=PATH` for `sorafs_cli car pack`".to_string())?;
    let car_path = car_out
        .ok_or_else(|| "missing required `--car-out=PATH` for `sorafs_cli car pack`".to_string())?;
    let handle = chunker_handle
        .as_deref()
        .unwrap_or(DEFAULT_CHUNKER_HANDLE)
        .trim();

    let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
        format!("unknown chunker profile handle `{handle}`; see `sorafs_manifest_stub --list-chunker-profiles` for options")
    })?;

    let metadata =
        fs::metadata(&input_path).map_err(|err| format!("failed to stat input: {err}"))?;
    if metadata.is_dir() {
        build_from_directory(
            &input_path,
            descriptor,
            &car_path,
            plan_out.as_ref(),
            summary_out.as_ref(),
            handle,
        )
    } else if metadata.is_file() {
        build_from_file(
            &input_path,
            descriptor,
            &car_path,
            plan_out.as_ref(),
            summary_out.as_ref(),
            handle,
        )
    } else {
        Err(format!(
            "input `{}` is neither a regular file nor directory",
            input_path.display()
        ))
    }
}

struct DeployClientConfig {
    torii_url: Option<String>,
    public_key: PublicKey,
    private_key: PrivateKey,
    chain_discriminant: u16,
}

struct DeployPackArtifacts {
    manifest: ManifestV1,
    manifest_bytes: Vec<u8>,
    manifest_digest_hex: String,
    root_cid_hex: String,
    root_cid_base32: String,
    chunk_digest_sha3: [u8; 32],
    payload_bytes: Vec<u8>,
    storage_files: Option<Vec<StorageFileEntryOwned>>,
    gateway_expectations: Vec<GatewayExpectation>,
    payload_kind: &'static str,
    payload_digest_hex: String,
}

#[derive(Clone, Debug)]
struct GatewayExpectation {
    path: Option<Vec<String>>,
    bytes: u64,
    blake3_hex: String,
}

struct StoragePinHttpResponse {
    endpoint: String,
    status: StatusCode,
    response_bytes: Vec<u8>,
    response_value: Value,
    already_stored: bool,
}

impl StoragePinHttpResponse {
    fn success(&self) -> bool {
        self.status.is_success() || self.already_stored
    }
}

struct PublishPeerDiscovery {
    gateway_base_url: Option<String>,
    pin_torii_urls: Vec<String>,
    status: Option<u16>,
    error: Option<String>,
}

struct ManifestRegisterSubmission {
    endpoint_requested: String,
    endpoint_used: String,
    status: StatusCode,
    response_bytes: Vec<u8>,
    response_value: Value,
    submission_mode: &'static str,
    fallback_reason: Option<String>,
    chain_id_hint: Option<String>,
    failure_message: Option<String>,
}

fn deploy(raw_args: Vec<String>) -> Result<(), String> {
    let mut payload_path: Option<PathBuf> = None;
    let mut client_config_path: Option<PathBuf> = None;
    let mut torii_url_override: Option<String> = None;
    let mut name: Option<String> = None;
    let mut out_dir_override: Option<PathBuf> = None;
    let mut gateway_base_url_override: Option<String> = None;
    let mut explicit_pin_torii_urls: Vec<String> = Vec::new();
    let mut peer_discovery_enabled = true;
    let mut submitted_epoch_override: Option<u64> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--no-peer-discovery" {
            peer_discovery_enabled = false;
            continue;
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--client-config" => client_config_path = Some(PathBuf::from(value)),
            "--torii-url" => torii_url_override = Some(value.to_string()),
            "--name" => name = Some(value.to_string()),
            "--out-dir" => out_dir_override = Some(PathBuf::from(value)),
            "--gateway-base-url" => gateway_base_url_override = Some(value.to_string()),
            "--pin-torii-url" => explicit_pin_torii_urls.push(value.to_string()),
            "--submitted-epoch" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--submitted-epoch` value: {err}"))?;
                submitted_epoch_override = Some(parsed);
            }
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli deploy`"
                ));
            }
        }
    }

    let payload_path = payload_path
        .ok_or_else(|| "missing required `--payload=PATH` for `sorafs_cli deploy`".to_string())?;
    let client_config_path = client_config_path.ok_or_else(|| {
        "missing required `--client-config=PATH` for `sorafs_cli deploy`".to_string()
    })?;
    let client_config = load_deploy_client_config(&client_config_path)?;
    let torii_url = torii_url_override
        .or(client_config.torii_url.clone())
        .ok_or_else(|| {
            "`--torii-url=URL` is required when client config does not define top-level `torii_url`"
                .to_string()
        })?;
    let torii_base_url =
        Url::parse(&torii_url).map_err(|err| format!("invalid Torii URL `{torii_url}`: {err}"))?;
    let deploy_name = sanitize_deploy_name(
        name.as_deref()
            .or_else(|| payload_path.file_name().and_then(|name| name.to_str()))
            .unwrap_or("payload"),
    );
    let out_dir =
        out_dir_override.unwrap_or_else(|| PathBuf::from(".sorafs/deploy").join(&deploy_name));
    fs::create_dir_all(&out_dir).map_err(|err| {
        format!(
            "failed to create deploy directory `{}`: {err}",
            out_dir.display()
        )
    })?;
    let receipt_path = summary_out
        .clone()
        .unwrap_or_else(|| out_dir.join(format!("{deploy_name}.deploy.receipt.json")));

    let car_path = out_dir.join(format!("{deploy_name}.car"));
    let plan_path = out_dir.join(format!("{deploy_name}.plan.json"));
    let pack_summary_path = out_dir.join(format!("{deploy_name}.pack.json"));
    let manifest_path = out_dir.join(format!("{deploy_name}.manifest.to"));
    let manifest_json_path = out_dir.join(format!("{deploy_name}.manifest.json"));
    let register_response_path = out_dir.join(format!("{deploy_name}.pin-register.response.json"));

    let mut errors: Vec<String> = Vec::new();
    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let artifacts = build_deploy_artifacts(
        &payload_path,
        &car_path,
        &plan_path,
        &pack_summary_path,
        &manifest_path,
        &manifest_json_path,
    );

    let mut receipt = Map::new();
    receipt.insert("name".into(), Value::from(deploy_name.clone()));
    receipt.insert(
        "payload_path".into(),
        Value::from(payload_path.display().to_string()),
    );
    receipt.insert(
        "client_config_path".into(),
        Value::from(client_config_path.display().to_string()),
    );
    receipt.insert("torii_url".into(), Value::from(torii_url.clone()));
    receipt.insert("out_dir".into(), Value::from(out_dir.display().to_string()));
    receipt.insert(
        "receipt_path".into(),
        Value::from(receipt_path.display().to_string()),
    );

    let artifacts = match artifacts {
        Ok(artifacts) => artifacts,
        Err(err) => {
            errors.push(err);
            receipt.insert("success".into(), Value::from(false));
            receipt.insert(
                "errors".into(),
                Value::Array(errors.iter().cloned().map(Value::from).collect()),
            );
            write_deploy_receipt_and_stdout(&receipt_path, &Value::Object(receipt))?;
            return Err("deploy failed while building local artifacts".to_string());
        }
    };

    receipt.insert(
        "cid_hex".into(),
        Value::from(artifacts.root_cid_hex.clone()),
    );
    receipt.insert(
        "cid_base32".into(),
        Value::from(artifacts.root_cid_base32.clone()),
    );
    receipt.insert(
        "manifest_digest_hex".into(),
        Value::from(artifacts.manifest_digest_hex.clone()),
    );
    receipt.insert(
        "payload_digest_blake3_hex".into(),
        Value::from(artifacts.payload_digest_hex.clone()),
    );
    receipt.insert(
        "payload_bytes".into(),
        Value::from(artifacts.manifest.content_length),
    );
    receipt.insert("payload_kind".into(), Value::from(artifacts.payload_kind));

    let mut artifact_paths = Map::new();
    artifact_paths.insert("car".into(), Value::from(car_path.display().to_string()));
    artifact_paths.insert(
        "plan_json".into(),
        Value::from(plan_path.display().to_string()),
    );
    artifact_paths.insert(
        "pack_summary_json".into(),
        Value::from(pack_summary_path.display().to_string()),
    );
    artifact_paths.insert(
        "manifest_to".into(),
        Value::from(manifest_path.display().to_string()),
    );
    artifact_paths.insert(
        "manifest_json".into(),
        Value::from(manifest_json_path.display().to_string()),
    );
    artifact_paths.insert(
        "pin_register_response".into(),
        Value::from(register_response_path.display().to_string()),
    );
    receipt.insert("artifacts".into(), Value::Object(artifact_paths));

    let authority = AccountId::new(client_config.public_key.clone());
    let authority_literal =
        authority_payload_literal(&authority, Some(client_config.chain_discriminant))?;
    let submitted_epoch = match submitted_epoch_override {
        Some(epoch) => epoch,
        None => match resolve_submitted_epoch_from_status(&client, &torii_base_url) {
            Ok(epoch) => epoch,
            Err(err) => {
                errors.push(format!("failed to resolve submitted epoch: {err}"));
                0
            }
        },
    };

    let submit_request = ManifestSubmitRequest {
        client: &client,
        torii_base_url: &torii_base_url,
        authority: &authority,
        private_key: &client_config.private_key,
        gas_asset_id: None,
        alias_inputs: None,
        api_version_hint: None,
    };
    let registration = if errors.is_empty() {
        submit_pin_register_with_fallback(
            &submit_request,
            &authority_literal,
            &artifacts.manifest,
            artifacts.chunk_digest_sha3,
            submitted_epoch,
            None,
        )
    } else {
        Err(
            "skipped paid pin registration because submitted epoch could not be resolved"
                .to_string(),
        )
    };

    let mut registration_summary = Map::new();
    registration_summary.insert("submitted_epoch".into(), Value::from(submitted_epoch));
    registration_summary.insert("authority".into(), Value::from(authority_literal));
    registration_summary.insert(
        "response_path".into(),
        Value::from(register_response_path.display().to_string()),
    );
    let mut registration_ok = false;
    let mut paid_pin_fee = Value::Null;
    match registration {
        Ok(response) => {
            ensure_parent_dir(&register_response_path)?;
            fs::write(&register_response_path, &response.response_bytes).map_err(|err| {
                format!(
                    "failed to write `{}`: {err}",
                    register_response_path.display()
                )
            })?;
            let fallback_failure = response.failure_message.clone();
            registration_ok = response.status.is_success() && fallback_failure.is_none();
            registration_summary.insert(
                "status".into(),
                Value::from(response.status.as_u16() as u64),
            );
            registration_summary.insert("endpoint".into(), Value::from(response.endpoint_used));
            registration_summary.insert(
                "endpoint_requested".into(),
                Value::from(response.endpoint_requested),
            );
            registration_summary.insert(
                "submission_mode".into(),
                Value::from(response.submission_mode),
            );
            if let Some(reason) = response.fallback_reason {
                registration_summary.insert("fallback_reason".into(), Value::from(reason));
            }
            if let Some(chain_id) = response.chain_id_hint {
                registration_summary.insert("chain_id".into(), Value::from(chain_id));
            }
            registration_summary.insert("success".into(), Value::from(registration_ok));
            paid_pin_fee = paid_pin_fee_from_register_response(&response.response_value);
            if let Some(message) = fallback_failure {
                errors.push(message);
            }
            if !registration_ok {
                let body = String::from_utf8_lossy(&response.response_bytes);
                errors.push(format!(
                    "paid pin registration failed with status {}: {body}",
                    response.status
                ));
            }
        }
        Err(err) => {
            registration_summary.insert("success".into(), Value::from(false));
            registration_summary.insert("error".into(), Value::from(err.clone()));
            errors.push(err);
        }
    }
    receipt.insert("registration".into(), Value::Object(registration_summary));
    receipt.insert("paid_pin_fee".into(), paid_pin_fee);

    let discovery = if peer_discovery_enabled {
        discover_publish_peers(&client, &torii_base_url)
    } else {
        PublishPeerDiscovery {
            gateway_base_url: None,
            pin_torii_urls: Vec::new(),
            status: None,
            error: Some("peer discovery disabled by --no-peer-discovery".to_string()),
        }
    };
    let gateway_base_url = gateway_base_url_override
        .or(discovery.gateway_base_url.clone())
        .unwrap_or_else(|| torii_url.clone());
    let mut discovery_json = Map::new();
    discovery_json.insert("enabled".into(), Value::from(peer_discovery_enabled));
    discovery_json.insert(
        "gateway_base_url".into(),
        discovery
            .gateway_base_url
            .as_ref()
            .map_or(Value::Null, |url| Value::from(url.clone())),
    );
    discovery_json.insert(
        "pin_torii_urls".into(),
        Value::Array(
            discovery
                .pin_torii_urls
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
    if let Some(status) = discovery.status {
        discovery_json.insert("status".into(), Value::from(status as u64));
    }
    if let Some(err) = discovery.error.as_ref() {
        discovery_json.insert("warning".into(), Value::from(err.clone()));
    }
    receipt.insert("peer_discovery".into(), Value::Object(discovery_json));

    let pin_endpoints = resolve_pin_torii_urls(
        &torii_url,
        &discovery.pin_torii_urls,
        &explicit_pin_torii_urls,
    );
    receipt.insert(
        "pin_endpoints".into(),
        Value::Array(pin_endpoints.iter().cloned().map(Value::from).collect()),
    );

    let mut pin_results = Vec::new();
    if registration_ok {
        for (index, peer_url) in pin_endpoints.iter().enumerate() {
            let response_path =
                out_dir.join(format!("{deploy_name}.storage-pin.{index}.response.json"));
            let result = submit_storage_pin_request(
                &client,
                peer_url,
                &artifacts.manifest_bytes,
                &artifacts.payload_bytes,
                artifacts.storage_files.as_deref(),
            );
            let mut entry = Map::new();
            entry.insert("torii_url".into(), Value::from(peer_url.clone()));
            entry.insert(
                "response_path".into(),
                Value::from(response_path.display().to_string()),
            );
            match result {
                Ok(response) => {
                    ensure_parent_dir(&response_path)?;
                    fs::write(&response_path, &response.response_bytes).map_err(|err| {
                        format!("failed to write `{}`: {err}", response_path.display())
                    })?;
                    let ok = response.success();
                    entry.insert("endpoint".into(), Value::from(response.endpoint));
                    entry.insert(
                        "status".into(),
                        Value::from(response.status.as_u16() as u64),
                    );
                    entry.insert("success".into(), Value::from(ok));
                    entry.insert(
                        "already_stored".into(),
                        Value::from(response.already_stored),
                    );
                    if !ok {
                        let body = String::from_utf8_lossy(&response.response_bytes);
                        let message = storage_pin_failure_message(response.status, body.as_ref());
                        entry.insert("error".into(), Value::from(message.clone()));
                        errors.push(format!("{peer_url}: {message}"));
                    }
                }
                Err(err) => {
                    entry.insert("success".into(), Value::from(false));
                    entry.insert("error".into(), Value::from(err.clone()));
                    errors.push(format!("{peer_url}: {err}"));
                }
            }
            pin_results.push(Value::Object(entry));
        }
    } else {
        errors.push("skipped storage pinning because paid registration failed".to_string());
    }
    receipt.insert("pin_results".into(), Value::Array(pin_results));

    let gateway_verification = verify_gateway_deploy(
        &client,
        &gateway_base_url,
        &artifacts.root_cid_base32,
        &artifacts.gateway_expectations,
    );
    let gateway_success = gateway_verification
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let cid_url = gateway_url_for_cid(&gateway_base_url, &artifacts.root_cid_base32)?;
    receipt.insert("gateway_base_url".into(), Value::from(gateway_base_url));
    receipt.insert("cid_base32_url".into(), Value::from(cid_url));
    receipt.insert("gateway_verification".into(), gateway_verification);
    if !gateway_success {
        errors.push("gateway verification failed".to_string());
    }

    let success = errors.is_empty();
    receipt.insert("success".into(), Value::from(success));
    receipt.insert(
        "errors".into(),
        Value::Array(errors.iter().cloned().map(Value::from).collect()),
    );
    write_deploy_receipt_and_stdout(&receipt_path, &Value::Object(receipt))?;

    if success {
        Ok(())
    } else {
        Err("deploy failed; see receipt for details".to_string())
    }
}

fn load_deploy_client_config(path: &Path) -> Result<DeployClientConfig, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read client config `{}`: {err}", path.display()))?;
    let root: toml::Table = raw.parse().map_err(|err| {
        format!(
            "failed to parse client config TOML `{}`: {err}",
            path.display()
        )
    })?;
    let torii_url = root
        .get("torii_url")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let account = root
        .get("account")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "client config must contain an `[account]` table".to_string())?;
    let public_key_raw = account
        .get("public_key")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "client config `[account]` must define `public_key`".to_string())?;
    let private_key_raw = account
        .get("private_key")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "client config `[account]` must define `private_key`".to_string())?;
    let chain_discriminant = resolve_deploy_chain_discriminant(&root, account)?;
    let public_key = PublicKey::from_str(public_key_raw)
        .map_err(|err| format!("failed to parse client config public_key: {err}"))?;
    let private_key = parse_private_key_inline(private_key_raw)
        .map_err(|err| format!("failed to parse client config private_key: {err}"))?;
    Ok(DeployClientConfig {
        torii_url,
        public_key,
        private_key,
        chain_discriminant,
    })
}

fn resolve_deploy_chain_discriminant(
    root: &toml::Table,
    account: &toml::Table,
) -> Result<u16, String> {
    if let Some(value) = account.get("chain_discriminant") {
        let value = value.as_integer().ok_or_else(|| {
            "client config `[account].chain_discriminant` must be an integer".to_string()
        })?;
        return u16::try_from(value).map_err(|_| {
            "client config `[account].chain_discriminant` must fit in u16".to_string()
        });
    }

    let chain = root
        .get("chain")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            "client config must define integer `[account].chain_discriminant` or a known top-level `chain`"
                .to_string()
        })?;
    known_deploy_chain_discriminant(chain).ok_or_else(|| {
        format!(
            "client config top-level `chain` `{chain}` is not known; define integer `[account].chain_discriminant`"
        )
    })
}

fn known_deploy_chain_discriminant(chain: &str) -> Option<u16> {
    match chain.trim() {
        "iroha3-taira" | "809574f5-fee7-5e69-bfcf-52451e42d50f" => Some(369),
        "iroha3-nexus" | "00000000-0000-0000-0000-000000000753" => Some(753),
        "00000000-0000-0000-0000-000000000000" => {
            Some(iroha_config::parameters::defaults::common::chain_discriminant())
        }
        _ => None,
    }
}

fn build_deploy_artifacts(
    payload_path: &Path,
    car_path: &Path,
    plan_path: &Path,
    pack_summary_path: &Path,
    manifest_path: &Path,
    manifest_json_path: &Path,
) -> Result<DeployPackArtifacts, String> {
    let handle = DEFAULT_CHUNKER_HANDLE;
    let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
        format!("unknown chunker profile handle `{handle}`; refresh the SoraFS chunker registry")
    })?;
    let metadata = fs::metadata(payload_path)
        .map_err(|err| format!("failed to stat payload `{}`: {err}", payload_path.display()))?;
    let (input_summary, mut plan, payload_cursor): (InputSummary, CarBuildPlan, Cursor<Vec<u8>>) =
        if metadata.is_dir() {
            let (plan, payload) =
                CarBuildPlan::from_directory_with_profile(payload_path, descriptor.profile)
                    .map_err(|err| format!("failed to build directory payload plan: {err}"))?;
            (
                InputSummary::Directory {
                    path: payload_path.to_path_buf(),
                    file_count: plan.files.len() as u64,
                },
                plan,
                Cursor::new(payload),
            )
        } else if metadata.is_file() {
            let payload = fs::read(payload_path).map_err(|err| {
                format!("failed to read payload `{}`: {err}", payload_path.display())
            })?;
            let plan = CarBuildPlan::single_file_with_profile(&payload, descriptor.profile)
                .map_err(|err| format!("failed to chunk payload: {err}"))?;
            (
                InputSummary::File {
                    path: payload_path.to_path_buf(),
                    bytes: payload.len() as u64,
                },
                plan,
                Cursor::new(payload),
            )
        } else {
            return Err(format!(
                "payload `{}` is neither a regular file nor directory",
                payload_path.display()
            ));
        };

    ensure_parent_dir(car_path)?;
    let car_file = File::create(car_path)
        .map_err(|err| format!("failed to create `{}`: {err}", car_path.display()))?;
    let mut writer = BufWriter::new(car_file);
    let mut payload_reader = payload_cursor;
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload_reader, &mut writer)
        .map_err(format_car_error)?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush `{}`: {err}", car_path.display()))?;

    let plan_specs = plan.chunk_fetch_specs();
    let plan_json = chunk_fetch_specs_to_string(&plan_specs)
        .map_err(|err| format!("failed to render chunk plan JSON: {err}"))?;
    ensure_parent_dir(plan_path)?;
    write_text(plan_path, plan_json.as_bytes())?;

    let pack_summary = render_summary(&input_summary, descriptor, handle, &plan, &stats, car_path);
    let pack_rendered = to_string_pretty(&pack_summary)
        .map_err(|err| format!("failed to render pack summary JSON: {err}"))?;
    ensure_parent_dir(pack_summary_path)?;
    write_text(pack_summary_path, pack_rendered.as_bytes())?;

    let root_cid = stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| "CAR build did not emit a root CID".to_string())?;
    let car_digest: [u8; 32] = *stats.car_archive_digest.as_bytes();
    let manifest_descriptor =
        manifest_chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
            format!("unknown manifest chunker profile handle `{handle}`; refresh the registry")
        })?;
    let chunking_profile = ChunkingProfileV1::from_descriptor(manifest_descriptor);
    let manifest = ManifestBuilder::new()
        .root_cid(root_cid.clone())
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_profile(chunking_profile)
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .build()
        .map_err(format_manifest_error)?;
    let manifest_bytes = manifest
        .encode()
        .map_err(|err| format!("failed to encode manifest: {err}"))?;
    ensure_parent_dir(manifest_path)?;
    fs::write(manifest_path, &manifest_bytes)
        .map_err(|err| format!("failed to write `{}`: {err}", manifest_path.display()))?;
    let manifest_json = to_string_pretty(
        &to_value(&manifest).map_err(|err| format!("failed to serialise manifest JSON: {err}"))?,
    )
    .map_err(|err| format!("failed to render manifest JSON: {err}"))?;
    ensure_parent_dir(manifest_json_path)?;
    write_text(manifest_json_path, manifest_json.as_bytes())?;

    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let (payload_bytes, storage_files, payload_kind) =
        load_storage_pin_payload(payload_path, &manifest)?;
    let gateway_expectations =
        build_gateway_expectations(payload_path, storage_files.as_deref(), &payload_bytes)?;
    let payload_digest_hex = hex_encode(blake3_hash(&payload_bytes).as_bytes());
    let root_cid_hex = hex_encode(&root_cid);
    let root_cid_base32 = encode_content_cid_base32(&root_cid);
    let chunk_digest_sha3 = chunk_digest_sha3_from_specs(&plan_specs);
    plan.chunks.shrink_to_fit();

    Ok(DeployPackArtifacts {
        manifest,
        manifest_bytes,
        manifest_digest_hex: hex_encode(manifest_digest.as_bytes()),
        root_cid_hex,
        root_cid_base32,
        chunk_digest_sha3,
        payload_bytes,
        storage_files,
        gateway_expectations,
        payload_kind,
        payload_digest_hex,
    })
}

fn build_gateway_expectations(
    payload_path: &Path,
    files: Option<&[StorageFileEntryOwned]>,
    payload_bytes: &[u8],
) -> Result<Vec<GatewayExpectation>, String> {
    if let Some(entries) = files {
        let mut expectations = Vec::new();
        if let Some(index) = entries
            .iter()
            .find(|entry| entry.path.len() == 1 && entry.path[0] == "index.html")
        {
            expectations.push(read_gateway_expectation(payload_path, None, index)?);
        }

        let mut ordered = entries.to_vec();
        ordered.sort_by(|left, right| left.path.cmp(&right.path));
        for entry in ordered.iter().take(32) {
            expectations.push(read_gateway_expectation(
                payload_path,
                Some(entry.path.clone()),
                entry,
            )?);
        }
        return Ok(expectations);
    }

    Ok(vec![GatewayExpectation {
        path: None,
        bytes: payload_bytes.len() as u64,
        blake3_hex: hex_encode(blake3_hash(payload_bytes).as_bytes()),
    }])
}

fn read_gateway_expectation(
    root: &Path,
    gateway_path: Option<Vec<String>>,
    entry: &StorageFileEntryOwned,
) -> Result<GatewayExpectation, String> {
    let file_path = entry
        .path
        .iter()
        .fold(root.to_path_buf(), |acc, component| acc.join(component));
    let bytes = fs::read(&file_path).map_err(|err| {
        format!(
            "failed to read gateway verification file `{}`: {err}",
            file_path.display()
        )
    })?;
    Ok(GatewayExpectation {
        path: gateway_path,
        bytes: entry.size,
        blake3_hex: hex_encode(blake3_hash(&bytes).as_bytes()),
    })
}

fn submit_pin_register_with_fallback(
    request: &ManifestSubmitRequest<'_>,
    authority_literal: &str,
    manifest: &ManifestV1,
    chunk_digest_sha3: [u8; 32],
    submitted_epoch: u64,
    successor_digest: Option<[u8; 32]>,
) -> Result<ManifestRegisterSubmission, String> {
    let endpoint = request
        .torii_base_url
        .join("v1/sorafs/pin/register")
        .map_err(|err| format!("failed to build Torii pin-register endpoint URL: {err}"))?;
    let requested_endpoint = endpoint.as_str().to_string();
    if request
        .gas_asset_id
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        let fallback = submit_manifest_via_transaction_endpoint(
            request,
            manifest,
            chunk_digest_sha3,
            submitted_epoch,
            successor_digest,
        )
        .map_err(|err| format!("direct /transaction manifest submit failed: {err}"))?;
        return Ok(ManifestRegisterSubmission {
            endpoint_requested: requested_endpoint,
            endpoint_used: fallback.endpoint,
            status: fallback.status,
            response_bytes: fallback.response_bytes,
            response_value: fallback.response_value,
            submission_mode: "transaction_direct",
            fallback_reason: Some(
                "gas_asset_id requested; bypassed pin-register route".to_string(),
            ),
            chain_id_hint: Some(fallback.chain_id),
            failure_message: fallback.failure_message,
        });
    }

    let payload = build_pin_register_payload(
        authority_literal,
        request.private_key.clone(),
        manifest,
        chunk_digest_sha3,
        submitted_epoch,
        request.alias_inputs,
        successor_digest,
    )?;
    let body_bytes =
        to_vec(&payload).map_err(|err| format!("failed to encode Torii payload: {err}"))?;
    let response = request
        .client
        .post(endpoint.as_str())
        .header(CONTENT_TYPE, "application/json")
        .body(body_bytes)
        .send()
        .map_err(|err| format!("failed to submit manifest to Torii: {err}"))?;
    let status = response.status();
    let api_version_hint = response
        .headers()
        .get("x-iroha-api-version")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let response_bytes = response
        .bytes()
        .map_err(|err| format!("failed to read Torii response: {err}"))?
        .to_vec();

    if status.is_success() {
        return Ok(ManifestRegisterSubmission {
            endpoint_requested: requested_endpoint.clone(),
            endpoint_used: requested_endpoint,
            status,
            response_value: decode_response_value_or_text(&response_bytes),
            response_bytes,
            submission_mode: "pin_register_http",
            fallback_reason: None,
            chain_id_hint: None,
            failure_message: None,
        });
    }

    if should_fallback_manifest_submit_status(status) {
        let fallback = submit_manifest_via_transaction_endpoint(
            &ManifestSubmitRequest {
                client: request.client,
                torii_base_url: request.torii_base_url,
                authority: request.authority,
                private_key: request.private_key,
                gas_asset_id: request.gas_asset_id,
                alias_inputs: request.alias_inputs,
                api_version_hint: api_version_hint.as_deref(),
            },
            manifest,
            chunk_digest_sha3,
            submitted_epoch,
            successor_digest,
        )
        .map_err(|err| {
            format!(
                "Torii pin-register endpoint returned {status}; generic /transaction fallback failed: {err}"
            )
        })?;
        return Ok(ManifestRegisterSubmission {
            endpoint_requested: requested_endpoint,
            endpoint_used: fallback.endpoint,
            status: fallback.status,
            response_bytes: fallback.response_bytes,
            response_value: fallback.response_value,
            submission_mode: "transaction_fallback",
            fallback_reason: Some(format!("pin register route returned {status}")),
            chain_id_hint: Some(fallback.chain_id),
            failure_message: fallback.failure_message,
        });
    }

    let body_text = String::from_utf8_lossy(&response_bytes);
    Err(format!(
        "Torii returned {status} when submitting manifest: {body_text}"
    ))
}

fn submit_storage_pin_request(
    client: &HttpClient,
    torii_url: &str,
    manifest_bytes: &[u8],
    payload_bytes: &[u8],
    files: Option<&[StorageFileEntryOwned]>,
) -> Result<StoragePinHttpResponse, String> {
    let torii_base_url =
        Url::parse(torii_url).map_err(|err| format!("invalid Torii URL `{torii_url}`: {err}"))?;
    let torii_endpoint = torii_base_url
        .join("v1/sorafs/storage/pin")
        .map_err(|err| format!("failed to build Torii storage pin endpoint URL: {err}"))?;
    let request_body = norito::json!({
        "manifest_b64": (BASE64_STANDARD.encode(manifest_bytes)),
        "payload_b64": (BASE64_STANDARD.encode(payload_bytes)),
        "files": (storage_files_to_json_value(files)),
    });
    let body_bytes =
        to_vec(&request_body).map_err(|err| format!("failed to encode Torii payload: {err}"))?;
    let response = client
        .post(torii_endpoint.as_str())
        .header(CONTENT_TYPE, "application/json")
        .body(body_bytes)
        .send()
        .map_err(|err| format!("failed to submit storage pin request to Torii: {err}"))?;
    let status = response.status();
    let response_bytes = response
        .bytes()
        .map_err(|err| format!("failed to read Torii response: {err}"))?
        .to_vec();
    let response_value = decode_response_value_or_text(&response_bytes);
    let response_text = String::from_utf8_lossy(&response_bytes);
    let already_stored = status == StatusCode::CONFLICT && response_text.contains("already stored");
    Ok(StoragePinHttpResponse {
        endpoint: torii_endpoint.as_str().to_string(),
        status,
        response_bytes,
        response_value,
        already_stored,
    })
}

fn discover_publish_peers(client: &HttpClient, torii_base_url: &Url) -> PublishPeerDiscovery {
    let endpoint = match torii_base_url.join("v1/sorafs/storage/peers") {
        Ok(endpoint) => endpoint,
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                pin_torii_urls: Vec::new(),
                status: None,
                error: Some(format!(
                    "failed to build peer discovery endpoint URL: {err}"
                )),
            };
        }
    };
    let response = match client
        .get(endpoint.as_str())
        .header("Accept", "application/json")
        .send()
    {
        Ok(response) => response,
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                pin_torii_urls: Vec::new(),
                status: None,
                error: Some(format!("peer discovery unavailable: {err}")),
            };
        }
    };
    let status = response.status();
    let response_bytes = match response.bytes() {
        Ok(bytes) => bytes.to_vec(),
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                pin_torii_urls: Vec::new(),
                status: Some(status.as_u16()),
                error: Some(format!("failed to read peer discovery response: {err}")),
            };
        }
    };
    if !status.is_success() {
        return PublishPeerDiscovery {
            gateway_base_url: None,
            pin_torii_urls: Vec::new(),
            status: Some(status.as_u16()),
            error: Some(format!(
                "peer discovery returned {status}; falling back to primary Torii URL"
            )),
        };
    }
    let value: Value = match from_slice(&response_bytes) {
        Ok(value) => value,
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                pin_torii_urls: Vec::new(),
                status: Some(status.as_u16()),
                error: Some(format!("failed to parse peer discovery JSON: {err}")),
            };
        }
    };
    let gateway_base_url = value
        .get("gateway_base_url")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let pin_torii_urls = value
        .get("pin_torii_urls")
        .and_then(Value::as_array)
        .map(|urls| {
            urls.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    PublishPeerDiscovery {
        gateway_base_url,
        pin_torii_urls,
        status: Some(status.as_u16()),
        error: None,
    }
}

fn resolve_pin_torii_urls(
    primary: &str,
    discovered: &[String],
    explicit: &[String],
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut urls = Vec::new();
    for url in discovered.iter().chain(explicit.iter()) {
        let normalized = normalize_url_for_receipt(url);
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            urls.push(normalized);
        }
    }
    let normalized_primary = normalize_url_for_receipt(primary);
    if !normalized_primary.is_empty() && seen.insert(normalized_primary.clone()) {
        urls.push(normalized_primary);
    }
    if urls.is_empty() {
        urls.push(normalize_url_for_receipt(primary));
    }
    urls
}

fn verify_gateway_deploy(
    client: &HttpClient,
    gateway_base_url: &str,
    cid_base32: &str,
    expectations: &[GatewayExpectation],
) -> Value {
    let root_url = match gateway_url_for_cid(gateway_base_url, cid_base32) {
        Ok(url) => url,
        Err(err) => {
            return Value::Object(Map::from_iter([
                ("success".into(), Value::from(false)),
                (
                    "gateway_base_url".into(),
                    Value::from(gateway_base_url.to_string()),
                ),
                ("error".into(), Value::from(err)),
            ]));
        }
    };

    let mut checks = Vec::new();
    for expectation in expectations {
        let (url, label) = match expectation.path.as_ref() {
            Some(path) => match gateway_url_for_file(gateway_base_url, cid_base32, path) {
                Ok(url) => (url, path.join("/")),
                Err(err) => {
                    checks.push(Value::Object(Map::from_iter([
                        ("success".into(), Value::from(false)),
                        ("path".into(), Value::from(path.join("/"))),
                        ("error".into(), Value::from(err)),
                    ])));
                    continue;
                }
            },
            None => (root_url.clone(), "/".to_string()),
        };
        checks.push(fetch_gateway_check(client, &url, &label, expectation));
    }
    if checks.is_empty() {
        checks.push(Value::Object(Map::from_iter([
            ("success".into(), Value::from(false)),
            (
                "error".into(),
                Value::from("no gateway verification expectations were generated"),
            ),
        ])));
    }

    let success = checks
        .iter()
        .all(|check| check.get("success").and_then(Value::as_bool) == Some(true));
    let mut map = Map::new();
    map.insert("success".into(), Value::from(success));
    map.insert(
        "gateway_base_url".into(),
        Value::from(gateway_base_url.to_string()),
    );
    map.insert("root_url".into(), Value::from(root_url));
    map.insert("checks".into(), Value::Array(checks));
    Value::Object(map)
}

fn fetch_gateway_check(
    client: &HttpClient,
    url: &str,
    label: &str,
    expectation: &GatewayExpectation,
) -> Value {
    let mut map = Map::new();
    map.insert("path".into(), Value::from(label.to_string()));
    map.insert("url".into(), Value::from(url.to_string()));
    map.insert("expected_bytes".into(), Value::from(expectation.bytes));
    map.insert(
        "expected_blake3_hex".into(),
        Value::from(expectation.blake3_hex.clone()),
    );
    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            let bytes = match response.bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    map.insert("status".into(), Value::from(status.as_u16() as u64));
                    map.insert("success".into(), Value::from(false));
                    map.insert(
                        "error".into(),
                        Value::from(format!("failed to read gateway response: {err}")),
                    );
                    return Value::Object(map);
                }
            };
            let actual = bytes.len() as u64;
            let actual_hash = hex_encode(blake3_hash(&bytes).as_bytes());
            let length_ok = expectation.bytes == actual;
            let hash_ok = expectation.blake3_hex == actual_hash;
            map.insert("status".into(), Value::from(status.as_u16() as u64));
            map.insert("actual_bytes".into(), Value::from(actual));
            map.insert("actual_blake3_hex".into(), Value::from(actual_hash));
            map.insert("length_ok".into(), Value::from(length_ok));
            map.insert("hash_ok".into(), Value::from(hash_ok));
            map.insert(
                "success".into(),
                Value::from(status == StatusCode::OK && length_ok && hash_ok),
            );
        }
        Err(err) => {
            map.insert("success".into(), Value::from(false));
            map.insert("error".into(), Value::from(err.to_string()));
        }
    }
    Value::Object(map)
}

fn gateway_url_for_cid(gateway_base_url: &str, cid_base32: &str) -> Result<String, String> {
    let base = Url::parse(gateway_base_url)
        .map_err(|err| format!("invalid gateway base URL `{gateway_base_url}`: {err}"))?;
    Ok(base
        .join(&format!("sorafs/cid/{cid_base32}/"))
        .map_err(|err| format!("failed to build gateway CID URL: {err}"))?
        .to_string())
}

fn gateway_url_for_file(
    gateway_base_url: &str,
    cid_base32: &str,
    path: &[String],
) -> Result<String, String> {
    let root = gateway_url_for_cid(gateway_base_url, cid_base32)?;
    let mut url = Url::parse(&root)
        .map_err(|err| format!("failed to parse gateway CID root URL `{root}`: {err}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "gateway URL cannot be a base for path segments".to_string())?;
        for component in path {
            segments.push(component);
        }
    }
    Ok(url.to_string())
}

fn paid_pin_fee_from_register_response(response: &Value) -> Value {
    let mut map = Map::new();
    if let Some(value) = response.get("pin_fee_nano") {
        map.insert("pin_fee_nano".into(), value.clone());
    }
    if let Some(value) = response.get("pin_fee_asset_id") {
        map.insert("pin_fee_asset_id".into(), value.clone());
    }
    if let Some(value) = response.get("pin_fee_treasury_account_id") {
        map.insert("pin_fee_treasury_account_id".into(), value.clone());
    }
    if map.is_empty() {
        Value::Null
    } else {
        Value::Object(map)
    }
}

fn write_deploy_receipt_and_stdout(path: &Path, value: &Value) -> Result<(), String> {
    let rendered =
        to_string_pretty(value).map_err(|err| format!("failed to render deploy receipt: {err}"))?;
    println!("{rendered}");
    ensure_parent_dir(path)?;
    write_text(path, rendered.as_bytes())
}

fn storage_pin_failure_message(status: StatusCode, response_text: &str) -> String {
    let mut message =
        format!("Torii returned {status} when pinning bundle into storage: {response_text}");
    if status == StatusCode::PAYMENT_REQUIRED
        || response_text.contains("no paid pin registry record")
    {
        message.push_str(
            ". Register the paid pin first with `sorafs_cli deploy --payload=PATH --client-config=PATH` or `sorafs_cli manifest submit` before running `sorafs_cli storage pin`.",
        );
    }
    message
}

fn sanitize_deploy_name(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches(['.', '-']).trim();
    if trimmed.is_empty() {
        "payload".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_url_for_receipt(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

fn encode_content_cid_base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if bytes.is_empty() {
        return "b".to_string();
    }
    let mut acc = 0_u32;
    let mut bits = 0_u32;
    let mut out = Vec::with_capacity((bytes.len() * 8).div_ceil(5) + 1);
    out.push(b'b');
    for byte in bytes {
        acc = (acc << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1f) as usize;
            out.push(ALPHABET[index]);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[index]);
    }
    String::from_utf8(out).expect("CID base32 alphabet is valid UTF-8")
}

fn taikai_bundle(raw_args: Vec<String>) -> Result<(), String> {
    let mut payload: Option<PathBuf> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut envelope_out: Option<PathBuf> = None;
    let mut indexes_out: Option<PathBuf> = None;
    let mut ingest_metadata_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut event_id: Option<String> = None;
    let mut stream_id: Option<String> = None;
    let mut rendition_id: Option<String> = None;
    let mut track_kind: Option<String> = None;
    let mut codec: Option<String> = None;
    let mut bitrate_kbps: Option<u32> = None;
    let mut resolution: Option<String> = None;
    let mut audio_layout: Option<String> = None;
    let mut segment_sequence: Option<u64> = None;
    let mut segment_start_pts: Option<u64> = None;
    let mut segment_duration: Option<u32> = None;
    let mut wallclock_unix_ms: Option<u64> = None;
    let mut manifest_hash_hex: Option<String> = None;
    let mut storage_ticket_hex: Option<String> = None;
    let mut ingest_latency_ms: Option<u32> = None;
    let mut live_edge_drift_ms: Option<i32> = None;
    let mut ingest_node_id: Option<String> = None;
    let mut metadata_json: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(taikai_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--payload" => payload = Some(PathBuf::from(value)),
            "--car-out" => car_out = Some(PathBuf::from(value)),
            "--envelope-out" => envelope_out = Some(PathBuf::from(value)),
            "--indexes-out" => indexes_out = Some(PathBuf::from(value)),
            "--ingest-metadata-out" => ingest_metadata_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--event-id" => event_id = Some(value.to_string()),
            "--stream-id" => stream_id = Some(value.to_string()),
            "--rendition-id" => rendition_id = Some(value.to_string()),
            "--track-kind" => track_kind = Some(value.to_string()),
            "--codec" => codec = Some(value.to_string()),
            "--bitrate-kbps" => {
                bitrate_kbps = Some(parse_u32_arg(
                    "--bitrate-kbps",
                    value,
                    "sorafs_cli taikai bundle",
                )?)
            }
            "--resolution" => resolution = Some(value.to_string()),
            "--audio-layout" => audio_layout = Some(value.to_string()),
            "--segment-sequence" => {
                let parsed = value.trim().parse::<u64>().map_err(|err| {
                    format!(
                        "failed to parse `--segment-sequence` for `sorafs_cli taikai bundle`: {err}"
                    )
                })?;
                segment_sequence = Some(parsed);
            }
            "--segment-start-pts" => {
                let parsed = value.trim().parse::<u64>().map_err(|err| {
                    format!("failed to parse `--segment-start-pts` for `sorafs_cli taikai bundle`: {err}")
                })?;
                segment_start_pts = Some(parsed);
            }
            "--segment-duration" => {
                segment_duration = Some(parse_u32_arg(
                    "--segment-duration",
                    value,
                    "sorafs_cli taikai bundle",
                )?)
            }
            "--wallclock-unix-ms" => {
                let parsed = value.trim().parse::<u64>().map_err(|err| {
                    format!("failed to parse `--wallclock-unix-ms` for `sorafs_cli taikai bundle`: {err}")
                })?;
                wallclock_unix_ms = Some(parsed);
            }
            "--manifest-hash" => manifest_hash_hex = Some(value.to_string()),
            "--storage-ticket" => storage_ticket_hex = Some(value.to_string()),
            "--ingest-latency-ms" => {
                ingest_latency_ms = Some(parse_u32_arg(
                    "--ingest-latency-ms",
                    value,
                    "sorafs_cli taikai bundle",
                )?)
            }
            "--live-edge-drift-ms" => {
                let parsed = value.trim().parse::<i32>().map_err(|err| {
                    format!("failed to parse `--live-edge-drift-ms` for `sorafs_cli taikai bundle`: {err}")
                })?;
                live_edge_drift_ms = Some(parsed);
            }
            "--ingest-node-id" => ingest_node_id = Some(value.to_string()),
            "--metadata-json" => metadata_json = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli taikai bundle`"
                ));
            }
        }
    }

    let payload_path = payload.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let car_path = car_out.ok_or_else(|| {
        "missing required `--car-out=PATH` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let envelope_path = envelope_out.ok_or_else(|| {
        "missing required `--envelope-out=PATH` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let event_raw = event_id.ok_or_else(|| {
        "missing required `--event-id=NAME` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let stream_raw = stream_id.ok_or_else(|| {
        "missing required `--stream-id=NAME` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let rendition_raw = rendition_id.ok_or_else(|| {
        "missing required `--rendition-id=NAME` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let track_kind_label = track_kind.ok_or_else(|| {
        "missing required `--track-kind=video|audio|data` for `sorafs_cli taikai bundle`"
            .to_string()
    })?;
    let codec_label = codec.ok_or_else(|| {
        "missing required `--codec=<identifier>` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let bitrate = bitrate_kbps.ok_or_else(|| {
        "missing required `--bitrate-kbps=KBPS` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let sequence = segment_sequence.ok_or_else(|| {
        "missing required `--segment-sequence=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let pts = segment_start_pts.ok_or_else(|| {
        "missing required `--segment-start-pts=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let duration = segment_duration.ok_or_else(|| {
        "missing required `--segment-duration=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let wallclock = wallclock_unix_ms.ok_or_else(|| {
        "missing required `--wallclock-unix-ms=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let manifest_hex = manifest_hash_hex.ok_or_else(|| {
        "missing required `--manifest-hash=HEX` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let storage_ticket_hex = storage_ticket_hex.ok_or_else(|| {
        "missing required `--storage-ticket=HEX` for `sorafs_cli taikai bundle`".to_string()
    })?;

    let event_name = Name::from_str(&event_raw)
        .map_err(|err| format!("invalid `--event-id` value `{event_raw}`: {err}"))?;
    let stream_name = Name::from_str(&stream_raw)
        .map_err(|err| format!("invalid `--stream-id` value `{stream_raw}`: {err}"))?;
    let rendition_name = Name::from_str(&rendition_raw)
        .map_err(|err| format!("invalid `--rendition-id` value `{rendition_raw}`: {err}"))?;

    let manifest_digest = parse_blob_digest_field(&manifest_hex, "--manifest-hash")?;
    let storage_ticket = parse_storage_ticket_field(&storage_ticket_hex, "--storage-ticket")?;

    let parsed_kind = parse_taikai_track_kind(&track_kind_label)?;
    let metadata = build_taikai_track_metadata(
        parsed_kind,
        &codec_label,
        bitrate,
        resolution.as_deref(),
        audio_layout.as_deref(),
    )?;

    let extra_metadata =
        if let Some(path) = metadata_json.as_ref() {
            Some(load_extra_metadata(path).map_err(|err| {
                format!("failed to load metadata JSON `{}`: {err}", path.display())
            })?)
        } else {
            None
        };

    let bundle_inputs = TaikaiBundleInputs {
        event_id: event_raw,
        stream_id: stream_raw,
        rendition_id: rendition_raw,
        track_kind: parsed_kind.as_str().to_string(),
        codec: codec_label.clone(),
        bitrate_kbps: bitrate,
        resolution: resolution.clone(),
        audio_layout: audio_layout.clone(),
        segment_sequence: sequence,
        segment_start_pts: pts,
        segment_duration: duration,
        wallclock_unix_ms: wallclock,
        manifest_hash_hex: manifest_hex.clone(),
        storage_ticket_hex: storage_ticket_hex.clone(),
        ingest_latency_ms,
        live_edge_drift_ms,
        ingest_node_id: ingest_node_id.clone(),
    };

    let summary = bundle_segment(&BundleRequest {
        payload_path: &payload_path,
        payload_bytes: None,
        car_out: &car_path,
        envelope_out: &envelope_path,
        indexes_out: indexes_out.as_deref(),
        ingest_metadata_out: ingest_metadata_out.as_deref(),
        manifest_hash: manifest_digest,
        storage_ticket,
        event_id: TaikaiEventId::new(event_name),
        stream_id: TaikaiStreamId::new(stream_name),
        rendition_id: TaikaiRenditionId::new(rendition_name),
        track: metadata,
        segment_sequence: sequence,
        segment_start_pts: pts,
        segment_duration: duration,
        wallclock_unix_ms: wallclock,
        ingest_latency_ms,
        live_edge_drift_ms,
        ingest_node_id,
        extra_metadata,
    })
    .map_err(|err| format!("failed to bundle Taikai segment: {err}"))?;

    println!("Taikai segment bundle generated");
    println!("car_cid (multibase): {}", summary.car_pointer.cid_multibase);
    println!(
        "car_digest (blake3-256 hex): {}",
        hex::encode(summary.car_pointer.car_digest.as_bytes())
    );
    println!("car_size_bytes: {}", summary.car_pointer.car_size_bytes);
    println!(
        "chunk_root (blake3-256 hex): {}",
        hex::encode(summary.chunk_root.as_bytes())
    );
    println!("chunk_count: {}", summary.chunk_count);
    println!("car_out: {}", summary.car_out.display());
    println!("envelope_out: {}", summary.envelope_out.display());
    if let Some(path) = summary.indexes_out.as_ref() {
        println!("indexes_out: {}", path.display());
    }
    if let Some(path) = summary.ingest_metadata_out.as_ref() {
        println!("ingest_metadata_out: {}", path.display());
    }

    if let Some(path) = summary_out.as_ref() {
        let summary_value = render_taikai_summary_value(&bundle_inputs, &summary);
        write_summary_json(path, &summary_value)?;
        println!("summary_out: {}", path.display());
    }

    Ok(())
}

struct TaikaiBundleInputs {
    event_id: String,
    stream_id: String,
    rendition_id: String,
    track_kind: String,
    codec: String,
    bitrate_kbps: u32,
    resolution: Option<String>,
    audio_layout: Option<String>,
    segment_sequence: u64,
    segment_start_pts: u64,
    segment_duration: u32,
    wallclock_unix_ms: u64,
    manifest_hash_hex: String,
    storage_ticket_hex: String,
    ingest_latency_ms: Option<u32>,
    live_edge_drift_ms: Option<i32>,
    ingest_node_id: Option<String>,
}

#[derive(Clone, Copy)]
enum TaikaiCliTrackKind {
    Video,
    Audio,
    Data,
}

impl TaikaiCliTrackKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Data => "data",
        }
    }
}

fn parse_taikai_track_kind(value: &str) -> Result<TaikaiCliTrackKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "video" => Ok(TaikaiCliTrackKind::Video),
        "audio" => Ok(TaikaiCliTrackKind::Audio),
        "data" => Ok(TaikaiCliTrackKind::Data),
        other => Err(format!(
            "invalid `--track-kind` value `{other}`; expected video|audio|data"
        )),
    }
}

fn build_taikai_track_metadata(
    kind: TaikaiCliTrackKind,
    codec_label: &str,
    bitrate_kbps: u32,
    resolution: Option<&str>,
    audio_layout: Option<&str>,
) -> Result<TaikaiTrackMetadata, String> {
    let codec = TaikaiCodec::from_str(codec_label)
        .map_err(|err| format!("invalid `--codec` value `{codec_label}`: {err}"))?;
    match kind {
        TaikaiCliTrackKind::Video => {
            let value = resolution
                .ok_or_else(|| "`--resolution` is required for `--track-kind=video`".to_string())?;
            let parsed = TaikaiResolution::from_str(value).map_err(|err| {
                format!("invalid `--resolution` value `{value}` for video track: {err}")
            })?;
            Ok(TaikaiTrackMetadata::video(codec, bitrate_kbps, parsed))
        }
        TaikaiCliTrackKind::Audio => {
            let value = audio_layout.ok_or_else(|| {
                "`--audio-layout` is required for `--track-kind=audio`".to_string()
            })?;
            let parsed = TaikaiAudioLayout::from_str(value)
                .map_err(|err| format!("invalid `--audio-layout` value `{value}`: {err}"))?;
            Ok(TaikaiTrackMetadata::audio(codec, bitrate_kbps, parsed))
        }
        TaikaiCliTrackKind::Data => Ok(TaikaiTrackMetadata::data(codec, bitrate_kbps)),
    }
}

fn parse_blob_digest_field(value: &str, flag: &str) -> Result<BlobDigest, String> {
    let trimmed = value.trim_start_matches("0x");
    let digest = parse_digest_hex(trimmed)
        .map_err(|err| format!("invalid `{flag}` value `{value}`: {err}"))?;
    Ok(BlobDigest::new(digest))
}

fn parse_storage_ticket_field(value: &str, flag: &str) -> Result<StorageTicketId, String> {
    let trimmed = value.trim_start_matches("0x");
    let bytes = parse_digest_hex(trimmed)
        .map_err(|err| format!("invalid `{flag}` value `{value}`: {err}"))?;
    Ok(StorageTicketId::new(bytes))
}

fn render_taikai_summary_value(inputs: &TaikaiBundleInputs, summary: &BundleSummary) -> Value {
    let mut ingest = Map::new();
    ingest.insert("event_id".into(), Value::from(inputs.event_id.clone()));
    ingest.insert("stream_id".into(), Value::from(inputs.stream_id.clone()));
    ingest.insert(
        "rendition_id".into(),
        Value::from(inputs.rendition_id.clone()),
    );
    ingest.insert(
        "segment_sequence".into(),
        Value::from(inputs.segment_sequence),
    );
    ingest.insert(
        "segment_start_pts".into(),
        Value::from(inputs.segment_start_pts),
    );
    ingest.insert(
        "segment_duration".into(),
        Value::from(inputs.segment_duration),
    );
    ingest.insert(
        "wallclock_unix_ms".into(),
        Value::from(inputs.wallclock_unix_ms),
    );
    ingest.insert(
        "manifest_hash".into(),
        Value::from(inputs.manifest_hash_hex.clone()),
    );
    ingest.insert(
        "storage_ticket".into(),
        Value::from(inputs.storage_ticket_hex.clone()),
    );
    if let Some(latency) = inputs.ingest_latency_ms {
        ingest.insert("ingest_latency_ms".into(), Value::from(latency));
    }
    if let Some(drift) = inputs.live_edge_drift_ms {
        ingest.insert("live_edge_drift_ms".into(), Value::from(drift));
    }
    if let Some(node) = inputs.ingest_node_id.as_ref() {
        ingest.insert("ingest_node_id".into(), Value::from(node.clone()));
    }

    let mut track = Map::new();
    track.insert("kind".into(), Value::from(inputs.track_kind.clone()));
    track.insert("codec".into(), Value::from(inputs.codec.clone()));
    track.insert("bitrate_kbps".into(), Value::from(inputs.bitrate_kbps));
    if let Some(resolution) = inputs.resolution.as_ref() {
        track.insert("resolution".into(), Value::from(resolution.clone()));
    }
    if let Some(layout) = inputs.audio_layout.as_ref() {
        track.insert("audio_layout".into(), Value::from(layout.clone()));
    }

    let mut car = Map::new();
    car.insert(
        "cid_multibase".into(),
        Value::from(summary.car_pointer.cid_multibase.clone()),
    );
    car.insert(
        "digest_blake3_hex".into(),
        Value::from(hex::encode(summary.car_pointer.car_digest.as_bytes())),
    );
    car.insert(
        "size_bytes".into(),
        Value::from(summary.car_pointer.car_size_bytes),
    );

    let mut chunk = Map::new();
    chunk.insert(
        "root_blake3_hex".into(),
        Value::from(hex::encode(summary.chunk_root.as_bytes())),
    );
    chunk.insert("count".into(), Value::from(summary.chunk_count));

    let mut outputs = Map::new();
    outputs.insert(
        "car_out".into(),
        Value::from(summary.car_out.display().to_string()),
    );
    outputs.insert(
        "envelope_out".into(),
        Value::from(summary.envelope_out.display().to_string()),
    );
    if let Some(path) = summary.indexes_out.as_ref() {
        outputs.insert(
            "indexes_out".into(),
            Value::from(path.display().to_string()),
        );
    }
    if let Some(path) = summary.ingest_metadata_out.as_ref() {
        outputs.insert(
            "ingest_metadata_out".into(),
            Value::from(path.display().to_string()),
        );
    }

    let mut root = Map::new();
    root.insert("ingest".into(), Value::Object(ingest));
    root.insert("track".into(), Value::Object(track));
    root.insert("car".into(), Value::Object(car));
    root.insert("chunk".into(), Value::Object(chunk));
    root.insert("outputs".into(), Value::Object(outputs));

    if let Ok(value) = to_value(&summary.indexes) {
        root.insert("indexes".into(), value);
    }

    Value::Object(root)
}

fn write_summary_json(path: &Path, value: &Value) -> Result<(), String> {
    let rendered =
        to_string_pretty(value).map_err(|err| format!("failed to render summary JSON: {err}"))?;
    fs::write(path, rendered.as_bytes())
        .map_err(|err| format!("failed to write summary JSON `{}`: {err}", path.display()))
}

enum StatusOutputFormat {
    Table,
    Json,
}

enum ReportOutputFormat {
    Markdown,
    Json,
}

fn por_status(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut manifest_hex: Option<String> = None;
    let mut provider_hex: Option<String> = None;
    let mut epoch: Option<u64> = None;
    let mut status_filter: Option<String> = None;
    let mut limit: Option<u32> = None;
    let mut page_token: Option<String> = None;
    let mut format_label: String = "table".to_string();

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--manifest" => manifest_hex = Some(value.to_ascii_lowercase()),
            "--provider" => provider_hex = Some(value.to_ascii_lowercase()),
            "--epoch" => {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|err| format!("invalid `--epoch` value: {err}"))?;
                epoch = Some(parsed);
            }
            "--status" => status_filter = Some(value.to_string()),
            "--limit" => {
                let parsed = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|err| format!("invalid `--limit` value: {err}"))?;
                if parsed == 0 {
                    return Err("`--limit` must be greater than zero".into());
                }
                limit = Some(parsed);
            }
            "--page-token" => page_token = Some(value.to_string()),
            "--format" => format_label = value.to_string(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por status`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por status`".to_string()
    })?;
    let output_format = match format_label.trim().to_ascii_lowercase().as_str() {
        "table" => StatusOutputFormat::Table,
        "json" => StatusOutputFormat::Json,
        other => {
            return Err(format!(
                "unsupported `--format` value `{other}`; expected table|json"
            ));
        }
    };

    if let Some(hex) = manifest_hex.as_ref() {
        parse_digest_hex(hex).map_err(|err| {
            format!("invalid `--manifest` digest `{hex}` supplied to `por status`: {err}")
        })?;
    }
    if let Some(hex) = provider_hex.as_ref() {
        parse_digest_hex(hex).map_err(|err| {
            format!("invalid `--provider` digest `{hex}` supplied to `por status`: {err}")
        })?;
    }
    let status_param = if let Some(label) = status_filter.as_ref() {
        Some(
            PorChallengeOutcome::parse(label)
                .map_err(|err| {
                    format!("invalid `--status` value `{label}` supplied to `por status`: {err}")
                })?
                .as_str()
                .to_string(),
        )
    } else {
        None
    };

    let mut endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join("v1/sorafs/por/status")
        .map_err(|err| format!("failed to resolve PoR status endpoint: {err}"))?;

    let mut serializer = Serializer::new(String::new());
    if let Some(hex) = manifest_hex {
        serializer.append_pair("manifest", hex.trim());
    }
    if let Some(hex) = provider_hex {
        serializer.append_pair("provider", hex.trim());
    }
    if let Some(epoch) = epoch {
        serializer.append_pair("epoch", &epoch.to_string());
    }
    if let Some(status) = status_param {
        serializer.append_pair("status", status.as_str());
    }
    if let Some(limit) = limit {
        serializer.append_pair("limit", &limit.to_string());
    }
    if let Some(token) = page_token {
        serializer.append_pair("page_token", token.trim());
    }
    let query = serializer.finish();
    if !query.is_empty() {
        endpoint.set_query(Some(&query));
    }

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .get(endpoint.clone())
        .header("Accept", "application/x-norito, application/json")
        .send()
        .map_err(|err| format!("failed to request PoR status from `{endpoint}`: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read PoR status response: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "Torii responded with status {status} for `por status`: {}",
            body_snippet(&body)
        ));
    }
    let statuses: Vec<PorChallengeStatusV1> = decode_from_bytes(&body)
        .map_err(|err| format!("failed to decode PoR status records: {err}"))?;

    match output_format {
        StatusOutputFormat::Table => {
            let rendered = render_status_table(&statuses);
            println!("{rendered}");
        }
        StatusOutputFormat::Json => {
            let value = to_value(&statuses)
                .map_err(|err| format!("failed to serialise status JSON: {err}"))?;
            let pretty = to_string_pretty(&value)
                .map_err(|err| format!("failed to pretty-print status JSON: {err}"))?;
            println!("{pretty}");
        }
    }
    Ok(())
}

fn por_trigger(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut manifest_hex: Option<String> = None;
    let mut provider_hex: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut auth_token_path: Option<PathBuf> = None;
    let mut requested_samples: Option<u16> = None;
    let mut deadline_secs: Option<u32> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--manifest" => manifest_hex = Some(value.to_ascii_lowercase()),
            "--provider" => provider_hex = Some(value.to_ascii_lowercase()),
            "--reason" => reason = Some(value.to_string()),
            "--auth-token" => auth_token_path = Some(PathBuf::from(value)),
            "--samples" => {
                let parsed = value
                    .trim()
                    .parse::<u16>()
                    .map_err(|err| format!("invalid `--samples` value: {err}"))?;
                if parsed == 0 {
                    return Err("`--samples` must be greater than zero".into());
                }
                requested_samples = Some(parsed);
            }
            "--deadline-secs" => {
                let parsed = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|err| format!("invalid `--deadline-secs` value: {err}"))?;
                if parsed == 0 {
                    return Err("`--deadline-secs` must be greater than zero".into());
                }
                deadline_secs = Some(parsed);
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por trigger`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por trigger`".to_string()
    })?;
    let manifest_hex = manifest_hex.ok_or_else(|| {
        "missing required `--manifest=HEX32` for `sorafs_cli por trigger`".to_string()
    })?;
    let provider_hex = provider_hex.ok_or_else(|| {
        "missing required `--provider=HEX32` for `sorafs_cli por trigger`".to_string()
    })?;
    let reason = reason.ok_or_else(|| {
        "missing required `--reason=TEXT` for `sorafs_cli por trigger`".to_string()
    })?;
    let token_path = auth_token_path.ok_or_else(|| {
        "missing required `--auth-token=PATH` for `sorafs_cli por trigger`".to_string()
    })?;

    let manifest_digest =
        parse_digest_hex(&manifest_hex).map_err(|err| format!("invalid manifest digest: {err}"))?;
    let provider_id =
        parse_digest_hex(&provider_hex).map_err(|err| format!("invalid provider digest: {err}"))?;
    if reason.trim().is_empty() {
        return Err("`--reason` must not be empty".into());
    }

    let challenge = ManualPorChallengeV1 {
        version: sorafs_manifest::MANUAL_POR_CHALLENGE_VERSION_V1,
        manifest_digest,
        provider_id,
        requested_samples,
        requested_deadline_secs: deadline_secs,
        reason,
    };
    challenge
        .validate()
        .map_err(|err| format!("manual PoR challenge invalid: {err}"))?;

    let token_bytes = fs::read(&token_path).map_err(|err| {
        format!(
            "failed to read challenge auth token `{}`: {err}",
            token_path.display()
        )
    })?;
    let token: ChallengeAuthTokenV1 = match decode_from_bytes(&token_bytes) {
        Ok(token) => token,
        Err(norito::Error::SchemaMismatch) => {
            let view = norito::core::from_bytes_view(&token_bytes)
                .map_err(|err| format!("failed to decode challenge auth token: {err}"))?;
            let payload = view.as_bytes();
            let (token, used) =
                norito::core::decode_field_canonical::<ChallengeAuthTokenV1>(payload)
                    .map_err(|err| format!("failed to decode challenge auth token: {err}"))?;
            if used != payload.len() {
                return Err(format!(
                    "failed to decode challenge auth token: trailing bytes after canonical payload ({used} of {} bytes consumed)",
                    payload.len()
                ));
            }
            token
        }
        Err(err) => return Err(format!("failed to decode challenge auth token: {err}")),
    };
    token
        .validate(&challenge.manifest_digest, &challenge.provider_id)
        .map_err(|err| format!("challenge auth token rejected: {err}"))?;

    let request = ManualPorTriggerRequestV1 {
        version: POR_TRIGGER_REQUEST_VERSION_V1,
        challenge,
        auth_token: token,
    };
    let body = to_bytes(&request)
        .map_err(|err| format!("failed to encode manual challenge request: {err}"))?;

    let endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join("v1/sorafs/por/trigger")
        .map_err(|err| format!("failed to resolve PoR trigger endpoint: {err}"))?;

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .post(endpoint.clone())
        .header(CONTENT_TYPE, "application/x-norito")
        .header("Accept", "application/json, application/x-norito")
        .body(body)
        .send()
        .map_err(|err| format!("failed to submit manual PoR challenge to `{endpoint}`: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read manual challenge response: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "manual PoR challenge request failed with status {status}: {}",
            body_snippet(&body)
        ));
    }
    if body.is_empty() {
        println!("manual PoR challenge submitted successfully.");
        return Ok(());
    }
    println!("{}", render_por_trigger_response(&body)?);
    Ok(())
}

fn render_por_trigger_response(body: &[u8]) -> Result<String, String> {
    if let Ok(value) = norito::json::from_slice::<Value>(body) {
        let pretty = to_string_pretty(&value)
            .map_err(|err| format!("failed to format manual challenge response JSON: {err}"))?;
        Ok(pretty)
    } else if let Ok(text) = std::str::from_utf8(body) {
        Ok(text.trim().to_string())
    } else {
        Ok(format!(
            "manual PoR challenge accepted ({} bytes response payload).",
            body.len()
        ))
    }
}

fn por_export(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut start_epoch: Option<u64> = None;
    let mut end_epoch: Option<u64> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--out" => out_path = Some(PathBuf::from(value)),
            "--start-epoch" => {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|err| format!("invalid `--start-epoch` value: {err}"))?;
                start_epoch = Some(parsed);
            }
            "--end-epoch" => {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|err| format!("invalid `--end-epoch` value: {err}"))?;
                end_epoch = Some(parsed);
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por export`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por export`".to_string()
    })?;
    let out_path = out_path
        .ok_or_else(|| "missing required `--out=PATH` for `sorafs_cli por export`".to_string())?;

    let mut endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join("v1/sorafs/por/export")
        .map_err(|err| format!("failed to resolve PoR export endpoint: {err}"))?;
    let mut serializer = Serializer::new(String::new());
    if let Some(start) = start_epoch {
        serializer.append_pair("start_epoch", &start.to_string());
    }
    if let Some(end) = end_epoch {
        serializer.append_pair("end_epoch", &end.to_string());
    }
    let query = serializer.finish();
    if !query.is_empty() {
        endpoint.set_query(Some(&query));
    }

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .get(endpoint.clone())
        .header("Accept", "application/octet-stream")
        .send()
        .map_err(|err| format!("failed to request PoR export from `{endpoint}`: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read PoR export response: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "PoR export failed with status {status}: {}",
            body_snippet(&body)
        ));
    }
    fs::write(&out_path, &body).map_err(|err| {
        format!(
            "failed to write export artefact to `{}`: {err}",
            out_path.display()
        )
    })?;
    println!("exported {} bytes to `{}`.", body.len(), out_path.display());
    Ok(())
}

fn por_report(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut week_label: Option<String> = None;
    let mut format_label: String = "markdown".to_string();

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--week" => week_label = Some(value.to_string()),
            "--format" => format_label = value.to_string(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por report`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por report`".to_string()
    })?;
    let week_label = week_label.ok_or_else(|| {
        "missing required `--week=YYYY-Www` for `sorafs_cli por report`".to_string()
    })?;
    let iso_week = parse_iso_week_arg(&week_label)?;

    let output_format = match format_label.trim().to_ascii_lowercase().as_str() {
        "markdown" => ReportOutputFormat::Markdown,
        "json" => ReportOutputFormat::Json,
        other => {
            return Err(format!(
                "unsupported `--format` value `{other}`; expected markdown|json"
            ));
        }
    };

    let endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join(&format!("v1/sorafs/por/report/{iso_week}"))
        .map_err(|err| format!("failed to resolve PoR report endpoint: {err}"))?;

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .get(endpoint.clone())
        .header("Accept", "application/x-norito, application/json")
        .send()
        .map_err(|err| format!("failed to request PoR report from `{endpoint}`: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read PoR report response: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "PoR report fetch failed with status {status}: {}",
            body_snippet(&body)
        ));
    }
    let report: PorWeeklyReportV1 = decode_from_bytes(&body)
        .map_err(|err| format!("failed to decode PoR weekly report: {err}"))?;
    report
        .validate()
        .map_err(|err| format!("weekly report failed validation: {err}"))?;

    match output_format {
        ReportOutputFormat::Markdown => {
            let rendered = render_report_markdown(&report);
            println!("{rendered}");
        }
        ReportOutputFormat::Json => {
            let value = to_value(&report)
                .map_err(|err| format!("failed to serialise report JSON: {err}"))?;
            let pretty = to_string_pretty(&value)
                .map_err(|err| format!("failed to pretty-print report JSON: {err}"))?;
            println!("{pretty}");
        }
    }
    Ok(())
}

fn build_from_file(
    input: &Path,
    descriptor: &ChunkerProfileDescriptor,
    car_out: &Path,
    plan_out: Option<&PathBuf>,
    summary_out: Option<&PathBuf>,
    handle: &str,
) -> Result<(), String> {
    let payload = fs::read(input)
        .map_err(|err| format!("failed to read payload `{}`: {err}", input.display()))?;
    let plan = CarBuildPlan::single_file_with_profile(&payload, descriptor.profile)
        .map_err(|err| format!("failed to chunk payload: {err}"))?;
    emit_car_and_artifacts(
        InputSummary::File {
            path: input.to_path_buf(),
            bytes: payload.len() as u64,
        },
        descriptor,
        handle,
        plan,
        Cursor::new(payload),
        car_out,
        plan_out,
        summary_out,
    )
}

fn build_from_directory(
    input: &Path,
    descriptor: &ChunkerProfileDescriptor,
    car_out: &Path,
    plan_out: Option<&PathBuf>,
    summary_out: Option<&PathBuf>,
    handle: &str,
) -> Result<(), String> {
    let (plan, payload) = CarBuildPlan::from_directory_with_profile(input, descriptor.profile)
        .map_err(|err| format!("failed to build directory plan: {err}"))?;
    emit_car_and_artifacts(
        InputSummary::Directory {
            path: input.to_path_buf(),
            file_count: plan.files.len() as u64,
        },
        descriptor,
        handle,
        plan,
        Cursor::new(payload),
        car_out,
        plan_out,
        summary_out,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_car_and_artifacts<R: io::Read>(
    input: InputSummary,
    descriptor: &ChunkerProfileDescriptor,
    handle: &str,
    mut plan: CarBuildPlan,
    mut payload: R,
    car_out: &Path,
    plan_out: Option<&PathBuf>,
    summary_out: Option<&PathBuf>,
) -> Result<(), String> {
    ensure_parent_dir(car_out)?;
    let car_file = File::create(car_out)
        .map_err(|err| format!("failed to create `{}`: {err}", car_out.display()))?;
    let mut writer = BufWriter::new(car_file);
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload, &mut writer)
        .map_err(format_car_error)?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush `{}`: {err}", car_out.display()))?;

    if stats.chunk_profile != descriptor.profile {
        return Err("emitted CAR used unexpected chunk profile".to_string());
    }

    if let Some(plan_path) = plan_out {
        ensure_parent_dir(plan_path)?;
        let spec_json = chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
            .map_err(|err| format!("failed to render chunk plan: {err}"))?;
        write_text(plan_path, spec_json.as_bytes())?;
    }

    let summary = render_summary(&input, descriptor, handle, &plan, &stats, car_out);
    let rendered =
        to_string_pretty(&summary).map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");

    if let Some(summary_path) = summary_out {
        ensure_parent_dir(summary_path)?;
        write_text(summary_path, rendered.as_bytes())?;
    }

    // Drop payload bytes held in the plan to free memory before returning.
    plan.chunks.shrink_to_fit();
    Ok(())
}

fn render_summary(
    input: &InputSummary,
    descriptor: &ChunkerProfileDescriptor,
    handle: &str,
    plan: &CarBuildPlan,
    stats: &sorafs_car::CarWriteStats,
    car_path: &Path,
) -> Value {
    let mut obj = Map::new();
    obj.insert("chunker_handle".into(), Value::from(handle));
    obj.insert(
        "chunker_profile_id".into(),
        Value::from(descriptor.id.0 as u64),
    );
    obj.insert(
        "chunker_profile_canonical".into(),
        Value::from(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )),
    );
    obj.insert("payload_bytes".into(), Value::from(plan.content_length));
    obj.insert("chunk_count".into(), Value::from(plan.chunks.len() as u64));
    obj.insert("file_count".into(), Value::from(plan.files.len() as u64));
    obj.insert("car_size".into(), Value::from(stats.car_size));
    obj.insert(
        "car_payload_digest_hex".into(),
        Value::from(hex_encode(stats.car_payload_digest.as_bytes())),
    );
    obj.insert(
        "car_digest_hex".into(),
        Value::from(hex_encode(stats.car_archive_digest.as_bytes())),
    );
    obj.insert(
        "car_cid_hex".into(),
        Value::from(hex_encode(&stats.car_cid)),
    );
    obj.insert(
        "root_cids_hex".into(),
        Value::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| Value::from(hex_encode(cid)))
                .collect(),
        ),
    );
    obj.insert(
        "output_car".into(),
        Value::from(car_path.display().to_string()),
    );
    match input {
        InputSummary::File { path, bytes } => {
            obj.insert("input_kind".into(), Value::from("file"));
            obj.insert("input_path".into(), Value::from(path.display().to_string()));
            obj.insert("input_bytes".into(), Value::from(*bytes));
        }
        InputSummary::Directory { path, file_count } => {
            obj.insert("input_kind".into(), Value::from("directory"));
            obj.insert("input_path".into(), Value::from(path.display().to_string()));
            obj.insert("input_file_count".into(), Value::from(*file_count));
        }
    }
    Value::Object(obj)
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create `{}`: {err}", parent.display()))?;
    }
    Ok(())
}

fn anonymity_policy_label(policy: AnonymityPolicy) -> &'static str {
    match policy {
        AnonymityPolicy::GuardPq => "anon-guard-pq",
        AnonymityPolicy::MajorityPq => "anon-majority-pq",
        AnonymityPolicy::StrictPq => "anon-strict-pq",
    }
}

fn write_text(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|err| format!("failed to write `{}`: {err}", path.display()))
}

fn format_car_error(err: CarWriteError) -> String {
    match err {
        CarWriteError::Io(io_err) => format!("streaming payload failed: {io_err}"),
        other => format!("failed to emit CAR: {other}"),
    }
}

fn usage() -> String {
    "Usage:
  sorafs_cli norito build --source=PATH --bytecode-out=PATH [--abi-version=N] [--summary-out=PATH]
  sorafs_cli deploy --payload=PATH --client-config=PATH [--torii-url=URL] [--submitted-epoch=EPOCH] [--name=NAME] [--out-dir=PATH] [--gateway-base-url=URL] [--pin-torii-url=URL...] [--no-peer-discovery] [--summary-out=PATH]
  sorafs_cli car pack --input=PATH --car-out=PATH [--chunker-handle=HANDLE] [--plan-out=PATH] [--summary-out=PATH]
  sorafs_cli manifest build --summary=PATH --manifest-out=PATH [--manifest-json-out=PATH] [--pin-min-replicas=N] [--pin-storage-class=hot|warm|cold] [--pin-retention-epoch=EPOCH] [--metadata key=value]
  sorafs_cli manifest sign --manifest=PATH (--bundle-out=PATH | --signature-out=PATH) [--summary=PATH | --chunk-plan=PATH | --chunk-digest-sha3=HEX] [--identity-token=JWT | --identity-token-env=VAR | --identity-token-file=PATH | --identity-token-provider=github-actions [--identity-token-audience=AUD]] [--include-token=true|false] [--issued-at=UNIX]
  sorafs_cli manifest verify-signature --manifest=PATH (--bundle=PATH | (--signature=PATH --public-key-hex=HEX)) [--summary=PATH | --chunk-plan=PATH | --chunk-digest-sha3=HEX] [--expect-token-hash=HEX]
  sorafs_cli manifest submit --manifest=PATH --torii-url=URL (--submitted-epoch=EPOCH | --resolve-submitted-epoch=true) (--chunk-plan=PATH | --chunk-digest-sha3=HEX) --authority=ACCOUNT [--network-prefix=U16] (--private-key=KEY | --private-key-file=PATH) [--gas-asset-id=ASSET_ID] [--alias-namespace=NS --alias-name=NAME --alias-proof=PATH] [--successor-of=HEX] [--summary-out=PATH] [--response-out=PATH]
  sorafs_cli manifest proposal --manifest=PATH --submitted-epoch=EPOCH (--chunk-plan=PATH | --chunk-digest-sha3=HEX) --proposal-out=PATH [--successor-of=HEX] [--alias-hint=TEXT]
  sorafs_cli storage prepare --manifest=PATH --payload=PATH --payload-out=PATH --files-out=PATH [--summary-out=PATH]
  sorafs_cli storage pin --manifest=PATH --payload=PATH --torii-url=URL [--summary-out=PATH] [--response-out=PATH]
  sorafs_cli fetch --plan=PATH --manifest-id=HEX [--chunker-handle=HANDLE] [--manifest-envelope=BASE64] [--manifest-report=PATH|-] [--manifest-cid=HEX] [--client-id=ID] [--telemetry-region=REGION] [--rollout-phase=canary|ramp|default] [--transport-policy=soranet-first|soranet-strict|direct-only] [--transport-policy-override=soranet-first|soranet-strict|direct-only] [--anonymity-policy=stage-a|stage-b|stage-c|anon-guard-pq|anon-majority-pq|anon-strict-pq] [--anonymity-policy-override=stage-a|stage-b|stage-c|anon-guard-pq|anon-majority-pq|anon-strict-pq] [--write-mode=read-only|upload-pq-only] [--scoreboard-out=PATH] [--scoreboard-now=UNIX_SECS] [--telemetry-source-label=LABEL] [--profile=hot|warm|cold] [--orchestrator-config=PATH] [--taikai-cache-config=PATH] [--output=PATH] [--json-out=PATH] [--local-proxy-mode=bridge|metadata-only] [--local-proxy-norito-spool=PATH] [--max-peers=N] [--retry-budget=N] [--expected-cache-version=VERSION] [--moderation-key-b64=BASE64] --provider name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64 [...]
  sorafs_cli proof stream --manifest=PATH (--torii-url=URL | --gateway-url=URL | --endpoint=URL) (--provider-id-hex=HEX32 | --provider-id=ID) [--proof-kind=por|pdp|potr] [--samples=N] [--sample-seed=SEED] [--deadline-ms=N] [--tier=hot|warm|archive] [--nonce-b64=BASE64] [--orchestrator-job-id-hex=HEX] [--stream-token=TOKEN] [--bearer-token-env=VAR] [--por-root-hex=HEX32] [--summary-out=PATH] [--governance-evidence-dir=DIR] [--emit-events=true|false] [--max-failures=N] [--max-verification-failures=N]
  sorafs_cli proof verify --manifest=PATH --car=PATH [--chunk-plan=PATH] [--summary-out=PATH]
  sorafs_cli reputation publish --torii-url=URL --snapshot=PATH [--summary-out=PATH]
  sorafs_cli reputation snapshot --torii-url=URL [--output=PATH] [--summary-out=PATH]
  sorafs_cli reputation fetch --torii-url=URL --provider-id=ID [--format=table|json] [--summary-out=PATH]
  sorafs_cli reputation watch --torii-url=URL [--since=N] [--limit=N] [--max-polls=N] [--poll-interval-ms=N] [--summary-out=PATH]
  sorafs_cli reputation verify --snapshot=PATH [--provider-id=ID --proof=PATH] [--summary-out=PATH]
  sorafs_cli por status --torii-url=URL [--manifest=HEX32] [--provider=HEX32] [--epoch=N] [--status=pending|verified|failed|repaired|forced] [--format=table|json]
  sorafs_cli por trigger --torii-url=URL --manifest=HEX32 --provider=HEX32 --reason=TEXT --auth-token=PATH [--samples=N] [--deadline-secs=N]
  sorafs_cli por export --torii-url=URL --out=PATH [--start-epoch=N] [--end-epoch=N]
  sorafs_cli por report --torii-url=URL --week=YYYY-Www [--format=markdown|json]
  sorafs_cli proxy set-mode --orchestrator-config=PATH --mode=bridge|metadata-only [--json-out=PATH] [--config-out=PATH] [--dry-run]
  sorafs_cli taikai bundle --payload=PATH --car-out=PATH --envelope-out=PATH --event-id=NAME --stream-id=NAME --rendition-id=NAME --track-kind=video|audio|data --codec=CODEC --bitrate-kbps=KBPS --segment-sequence=N --segment-start-pts=N --segment-duration=N --wallclock-unix-ms=N --manifest-hash=HEX --storage-ticket=HEX [--indexes-out=PATH] [--ingest-metadata-out=PATH] [--summary-out=PATH] [--resolution=WxH] [--audio-layout=mono|stereo|5.1|7.1|custom:<label>] [--ingest-latency-ms=N] [--live-edge-drift-ms=N] [--ingest-node-id=ID] [--metadata-json=PATH]
  sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]
  sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]
  sorafs_cli moderation honey-audit --manifest-id=HEX --honey=HEX [--honey=HEX...] --provider name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64 [...] [--chunker-handle=HANDLE] [--expected-cache-version=VERSION] [--moderation-key-b64=BASE64] [--require-proof] [--json-out=PATH] [--markdown-out=PATH]
  sorafs_cli appeal quote --class=content|access|fraud|other [--backlog=N] [--evidence-mb=N] [--urgency=normal|high] [--panel-size=N] [--format=table|json] [--config=PATH|-]
  sorafs_cli governance dag list --root=DIR [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag show --node=PATH [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag verify --root=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag export --root=DIR --out=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag build --root=DIR --out=DIR --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH] [--car-out=PATH [--car-plan-out=PATH] [--car-chunker-handle=HANDLE]]
  sorafs_cli governance dag verify-build --root=DIR [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag rebuild-head --root=DIR --head-out=PATH --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX] [--car=PATH] [--mirror-index=PATH] [--generated-at=UNIX_SECS]
  sorafs_cli governance dag checkpoint-verify --checkpoint=PATH [--root=DIR] [--car=PATH] [--mirror-index=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint-recover --checkpoint=PATH --root=DIR --out=PATH [--car=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag mirror-build --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag mirror-query --index=PATH (--head | --block-cid=CID|hex:HEX | --node-cid=CID|hex:HEX) [--format=table|json]"
        .to_string()
}

fn reputation_usage() -> String {
    "Usage:
  sorafs_cli reputation publish --torii-url=URL --snapshot=PATH [--summary-out=PATH]
  sorafs_cli reputation snapshot --torii-url=URL [--output=PATH] [--summary-out=PATH]
  sorafs_cli reputation fetch --torii-url=URL --provider-id=ID [--format=table|json] [--summary-out=PATH]
  sorafs_cli reputation watch --torii-url=URL [--since=N] [--limit=N] [--max-polls=N] [--poll-interval-ms=N] [--summary-out=PATH]
  sorafs_cli reputation verify --snapshot=PATH [--provider-id=ID --proof=PATH] [--summary-out=PATH]"
        .to_string()
}

fn fetch_usage() -> String {
    "Usage:
  sorafs_cli fetch --plan=PATH --manifest-id=HEX --provider name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64 [additional --provider entries...] [--chunker-handle=HANDLE] [--manifest-envelope=BASE64] [--manifest-report=PATH|-] [--manifest-cid=HEX] [--client-id=ID] [--telemetry-region=REGION] [--rollout-phase=canary|ramp|default] [--transport-policy=soranet-first|soranet-strict|direct-only] [--transport-policy-override=soranet-first|soranet-strict|direct-only] [--anonymity-policy=stage-a|stage-b|stage-c|anon-guard-pq|anon-majority-pq|anon-strict-pq] [--anonymity-policy-override=stage-a|stage-b|stage-c|anon-guard-pq|anon-majority-pq|anon-strict-pq] [--write-mode=read-only|upload-pq-only] [--scoreboard-out=PATH] [--scoreboard-now=UNIX_SECS] [--telemetry-source-label=LABEL] [--profile=hot|warm|cold] [--orchestrator-config=PATH] [--taikai-cache-config=PATH] [--output=PATH] [--json-out=PATH] [--local-proxy-mode=bridge|metadata-only] [--local-proxy-norito-spool=PATH] [--local-proxy-manifest-out=PATH] [--max-peers=N] [--retry-budget=N] [--expected-cache-version=VERSION] [--moderation-key-b64=BASE64]"
        .to_string()
}

fn taikai_usage() -> String {
    "Usage:
  sorafs_cli taikai bundle --payload=PATH --car-out=PATH --envelope-out=PATH --event-id=NAME --stream-id=NAME --rendition-id=NAME --track-kind=video|audio|data --codec=CODEC --bitrate-kbps=KBPS --segment-sequence=N --segment-start-pts=N --segment-duration=N --wallclock-unix-ms=N --manifest-hash=HEX --storage-ticket=HEX [--indexes-out=PATH] [--ingest-metadata-out=PATH] [--summary-out=PATH] [--resolution=WxH] [--audio-layout=mono|stereo|5.1|7.1|custom:<label>] [--ingest-latency-ms=N] [--live-edge-drift-ms=N] [--ingest-node-id=ID] [--metadata-json=PATH]"
        .to_string()
}

#[derive(Clone, Copy)]
struct GatewayProviderCounts {
    direct: usize,
    gateway: usize,
}

impl GatewayProviderCounts {
    const fn new(direct: usize, gateway: usize) -> Self {
        Self { direct, gateway }
    }

    fn direct_u64(self) -> u64 {
        u64::try_from(self.direct).unwrap_or(u64::MAX)
    }

    fn gateway_u64(self) -> u64 {
        u64::try_from(self.gateway).unwrap_or(u64::MAX)
    }

    fn mix_label(self) -> &'static str {
        match (self.direct > 0, self.gateway > 0) {
            (true, true) => "mixed",
            (true, false) => "direct-only",
            (false, true) => "gateway-only",
            (false, false) => "none",
        }
    }
}

struct GatewayScoreboardMetadataInput<'a> {
    provider_counts: GatewayProviderCounts,
    max_peers: Option<usize>,
    retry_budget: Option<usize>,
    manifest_envelope_present: bool,
    gateway_manifest_id: Option<&'a str>,
    gateway_manifest_cid: Option<&'a str>,
    transport_policy: Option<TransportPolicy>,
    transport_policy_override: Option<TransportPolicy>,
    anonymity_policy: Option<AnonymityPolicy>,
    anonymity_policy_override: Option<AnonymityPolicy>,
    write_mode: WriteModeHint,
    scoreboard_now: Option<u64>,
    telemetry_source: Option<&'a str>,
}

#[derive(Clone, Copy)]
enum FetchCacheProfile {
    Warm,
    Cold,
}

impl FetchCacheProfile {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "hot" | "warm" => Some(Self::Warm),
            "cold" => Some(Self::Cold),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Cold => "cold",
        }
    }
}

struct CliPolicyLabels {
    effective_label: &'static str,
    override_flag: bool,
    override_label: Option<&'static str>,
}

fn summarise_policy<T>(
    requested: Option<T>,
    override_policy: Option<T>,
    label_fn: impl Fn(T) -> &'static str,
) -> CliPolicyLabels
where
    T: Copy + Default,
{
    let effective = override_policy.or(requested).unwrap_or_default();
    CliPolicyLabels {
        effective_label: label_fn(effective),
        override_flag: override_policy.is_some(),
        override_label: override_policy.map(label_fn),
    }
}

fn summarise_transport_policy(
    requested: Option<TransportPolicy>,
    override_policy: Option<TransportPolicy>,
) -> CliPolicyLabels {
    summarise_policy(requested, override_policy, TransportPolicy::label)
}

fn summarise_anonymity_policy(
    requested: Option<AnonymityPolicy>,
    override_policy: Option<AnonymityPolicy>,
) -> CliPolicyLabels {
    summarise_policy(requested, override_policy, AnonymityPolicy::label)
}

fn option_usize_to_value(value: Option<usize>) -> Value {
    value
        .and_then(|val| u64::try_from(val).ok())
        .map(Value::from)
        .unwrap_or(Value::Null)
}

fn build_gateway_scoreboard_metadata(input: &GatewayScoreboardMetadataInput<'_>) -> Value {
    let mut metadata = Map::new();
    metadata.insert("version".into(), Value::from(SORAFS_CLI_VERSION));
    metadata.insert("use_scoreboard".into(), Value::from(true));
    metadata.insert("allow_implicit_metadata".into(), Value::from(false));
    metadata.insert(
        "provider_count".into(),
        Value::from(input.provider_counts.direct_u64()),
    );
    metadata.insert(
        "gateway_provider_count".into(),
        Value::from(input.provider_counts.gateway_u64()),
    );
    metadata.insert(
        "provider_mix".into(),
        Value::from(input.provider_counts.mix_label()),
    );
    metadata.insert("max_parallel".into(), Value::Null);
    metadata.insert("max_peers".into(), option_usize_to_value(input.max_peers));
    metadata.insert(
        "retry_budget".into(),
        option_usize_to_value(input.retry_budget),
    );
    metadata.insert("provider_failure_threshold".into(), Value::Null);
    metadata.insert(
        "assume_now".into(),
        input.scoreboard_now.map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "telemetry_source".into(),
        input.telemetry_source.map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "gateway_manifest_id".into(),
        input.gateway_manifest_id.map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "gateway_manifest_cid".into(),
        input.gateway_manifest_cid.map_or(Value::Null, Value::from),
    );
    metadata.insert(
        "gateway_manifest_provided".into(),
        Value::from(input.manifest_envelope_present),
    );

    let transport_labels =
        summarise_transport_policy(input.transport_policy, input.transport_policy_override);
    metadata.insert(
        "transport_policy".into(),
        Value::from(transport_labels.effective_label),
    );
    metadata.insert(
        "transport_policy_override".into(),
        Value::from(transport_labels.override_flag),
    );
    metadata.insert(
        "transport_policy_override_label".into(),
        transport_labels
            .override_label
            .map_or(Value::Null, Value::from),
    );

    let anonymity_labels =
        summarise_anonymity_policy(input.anonymity_policy, input.anonymity_policy_override);
    metadata.insert(
        "anonymity_policy".into(),
        Value::from(anonymity_labels.effective_label),
    );
    metadata.insert(
        "anonymity_policy_override".into(),
        Value::from(anonymity_labels.override_flag),
    );
    metadata.insert(
        "anonymity_policy_override_label".into(),
        anonymity_labels
            .override_label
            .map_or(Value::Null, Value::from),
    );
    let write_mode_label = input.write_mode.label().replace('_', "-");
    metadata.insert("write_mode".into(), Value::from(write_mode_label));
    metadata.insert(
        "write_mode_enforces_pq".into(),
        Value::from(input.write_mode.enforces_pq_only()),
    );

    Value::Object(metadata)
}

fn insert_telemetry_source(summary: &mut Value, telemetry_source: Option<&str>) {
    if let Some(label) = telemetry_source
        && let Some(obj) = summary.as_object_mut()
    {
        obj.insert("telemetry_source".into(), Value::from(label));
    }
}

fn por_usage() -> String {
    "Usage:
  sorafs_cli por status --torii-url=URL [--manifest=HEX32] [--provider=HEX32] [--epoch=N] [--status=pending|verified|failed|repaired|forced] [--limit=N] [--page-token=TOKEN] [--format=table|json]
  sorafs_cli por trigger --torii-url=URL --manifest=HEX32 --provider=HEX32 --reason=TEXT --auth-token=PATH [--samples=N] [--deadline-secs=N]
  sorafs_cli por export --torii-url=URL --out=PATH [--start-epoch=N] [--end-epoch=N]
  sorafs_cli por report --torii-url=URL --week=YYYY-Www [--format=markdown|json]"
        .to_string()
}

fn proxy_usage() -> String {
    "Usage:
  sorafs_cli proxy set-mode --orchestrator-config=PATH --mode=bridge|metadata-only [--json-out=PATH] [--config-out=PATH] [--dry-run]"
        .to_string()
}

fn appeal_usage() -> String {
    "Usage:
  sorafs_cli appeal quote --class=content|access|fraud|other [--backlog=N] [--evidence-mb=N] [--urgency=normal|high] [--panel-size=N] [--format=table|json] [--config=PATH|-]
  sorafs_cli appeal settle --deposit=XOR --outcome=uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated [--panel-size=N] [--format=table|json] [--config=PATH|-]
  sorafs_cli appeal disburse --deposit=XOR --outcome=uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated --refund-account=ID --treasury-account=ID --escrow-account=ID --juror=ID [--juror=ID...] [--no-show=ID...] [--panel-size=N] [--format=table|json] [--config=PATH|-]"
        .to_string()
}

fn governance_usage() -> String {
    "Usage:
  sorafs_cli governance dag list --root=DIR [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag show --node=PATH [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag verify --root=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag export --root=DIR --out=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag build --root=DIR --out=DIR --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH] [--car-out=PATH [--car-plan-out=PATH] [--car-chunker-handle=HANDLE]]
  sorafs_cli governance dag verify-build --root=DIR [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag rebuild-head --root=DIR --head-out=PATH --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX] [--car=PATH] [--mirror-index=PATH] [--generated-at=UNIX_SECS]
  sorafs_cli governance dag checkpoint-verify --checkpoint=PATH [--root=DIR] [--car=PATH] [--mirror-index=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint-recover --checkpoint=PATH --root=DIR --out=PATH [--car=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag mirror-build --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag mirror-query --index=PATH (--head | --block-cid=CID|hex:HEX | --node-cid=CID|hex:HEX) [--format=table|json]"
        .to_string()
}

fn moderation_usage() -> String {
    "Usage:
  sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]
  sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]
  sorafs_cli moderation honey-audit --manifest-id=HEX --honey=HEX [--honey=HEX...] --provider name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64 [...] [--chunker-handle=HANDLE] [--expected-cache-version=VERSION] [--moderation-key-b64=BASE64] [--require-proof] [--json-out=PATH] [--markdown-out=PATH]

Validates governance-signed AI moderation reproducibility manifests and adversarial corpus registries before gateways adopt them. Use `honey-audit` to probe gateways with denylisted digests and emit JSON/Markdown evidence for policy enforcement."
        .to_string()
}

fn fetch_gateway(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(fetch_usage());
    }

    let mut plan_source: Option<JsonSource> = None;
    let mut manifest_id_hex: Option<String> = None;
    let mut chunker_handle_hint: Option<String> = None;
    let mut manifest_envelope: Option<String> = None;
    let mut manifest_report_source: Option<JsonSource> = None;
    let mut manifest_cid_hex: Option<String> = None;
    let mut expected_cache_version: Option<String> = None;
    let mut moderation_token_key_b64: Option<String> = None;
    let mut client_id: Option<String> = None;
    let mut telemetry_region: Option<String> = None;
    let mut rollout_phase: Option<RolloutPhase> = None;
    let mut orchestrator_config_source: Option<JsonSource> = None;
    let mut taikai_cache_source: Option<JsonSource> = None;
    let mut transport_policy: Option<TransportPolicy> = None;
    let mut anonymity_policy: Option<AnonymityPolicy> = None;
    let mut transport_policy_override: Option<TransportPolicy> = None;
    let mut anonymity_policy_override: Option<AnonymityPolicy> = None;
    let mut write_mode: Option<WriteModeHint> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut local_proxy_manifest_out: Option<PathBuf> = None;
    let mut local_proxy_mode_override: Option<ProxyMode> = None;
    let mut local_proxy_spool_override: Option<String> = None;
    let mut local_proxy_kaigi_spool_override: Option<String> = None;
    let mut local_proxy_kaigi_policy_override: Option<String> = None;
    let mut max_peers: Option<usize> = None;
    let mut retry_budget: Option<usize> = None;
    let mut provider_specs: Vec<GatewayProviderSpec> = Vec::new();
    let mut scoreboard_out: Option<PathBuf> = None;
    let mut scoreboard_now: Option<u64> = None;
    let mut telemetry_source_label: Option<String> = None;
    let mut cache_profile: Option<FetchCacheProfile> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(fetch_usage());
        }
        if let Some(rest) = arg.strip_prefix("--plan=") {
            plan_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--manifest-id=") {
            manifest_id_hex = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--chunker-handle=") {
            chunker_handle_hint = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--manifest-envelope=") {
            manifest_envelope = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--manifest-report=") {
            manifest_report_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--manifest-cid=") {
            manifest_cid_hex = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--expected-cache-version=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--expected-cache-version` must not be empty".into());
            }
            expected_cache_version = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--moderation-key-b64=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--moderation-key-b64` must not be empty".into());
            }
            moderation_token_key_b64 = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--client-id=") {
            client_id = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--telemetry-region=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--telemetry-region` must not be empty".into());
            }
            telemetry_region = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--rollout-phase=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--rollout-phase` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase().replace('-', "_");
            let parsed = RolloutPhase::parse(&normalized).ok_or_else(|| {
                "`--rollout-phase` must be one of canary|ramp|default (stage-a|stage-b|stage-c aliases accepted)"
                    .to_string()
            })?;
            rollout_phase = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--transport-policy=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--transport-policy` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase().replace('-', "_");
            let parsed = TransportPolicy::parse(&normalized).ok_or_else(|| {
                "`--transport-policy` must be one of soranet-first|soranet-strict|direct-only"
                    .to_string()
            })?;
            transport_policy = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--anonymity-policy=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--anonymity-policy` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase().replace('-', "_");
            let parsed = AnonymityPolicy::parse(&normalized).ok_or_else(|| {
                "`--anonymity-policy` must be one of stage-a|stage-b|stage-c|anon-guard-pq|anon-majority-pq|anon-strict-pq".to_string()
            })?;
            anonymity_policy = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--write-mode=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--write-mode` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase().replace('-', "_");
            let parsed = WriteModeHint::parse(&normalized).ok_or_else(|| {
                "`--write-mode` must be one of read-only|read_only|upload-pq-only|upload_pq_only"
                    .to_string()
            })?;
            write_mode = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--transport-policy-override=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--transport-policy-override` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase().replace('-', "_");
            let parsed = TransportPolicy::parse(&normalized).ok_or_else(|| {
                "`--transport-policy-override` must be one of soranet-first|soranet-strict|direct-only"
                    .to_string()
            })?;
            transport_policy_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--anonymity-policy-override=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--anonymity-policy-override` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase().replace('-', "_");
            let parsed = AnonymityPolicy::parse(&normalized).ok_or_else(|| {
                "`--anonymity-policy-override` must be one of stage-a|stage-b|stage-c|anon-guard-pq|anon-majority-pq|anon-strict-pq"
                    .to_string()
            })?;
            anonymity_policy_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--scoreboard-out=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--scoreboard-out` must not be empty".into());
            }
            scoreboard_out = Some(PathBuf::from(trimmed));
        } else if let Some(rest) = arg.strip_prefix("--scoreboard-now=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--scoreboard-now` must not be empty".into());
            }
            let parsed = trimmed
                .parse::<u64>()
                .map_err(|err| format!("`--scoreboard-now` must be an unsigned integer: {err}"))?;
            scoreboard_now = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--telemetry-source-label=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--telemetry-source-label` must not be empty".into());
            }
            telemetry_source_label = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--profile=") {
            let normalized = rest.trim().to_ascii_lowercase().replace('-', "_");
            let parsed = FetchCacheProfile::parse(&normalized).ok_or_else(|| {
                "`--profile` must be one of hot|warm|cold for `sorafs_cli fetch`".to_string()
            })?;
            cache_profile = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--orchestrator-config=") {
            orchestrator_config_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--policy=") {
            orchestrator_config_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--taikai-cache-config=") {
            taikai_cache_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--output=") {
            output_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-manifest-out=") {
            local_proxy_manifest_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-mode=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-mode` must not be empty".into());
            }
            let parsed = ProxyMode::parse(trimmed).ok_or_else(|| {
                "`--local-proxy-mode` must be one of bridge|metadata-only".to_string()
            })?;
            local_proxy_mode_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-norito-spool=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-norito-spool` must not be empty".into());
            }
            local_proxy_spool_override = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-kaigi-spool=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-kaigi-spool` must not be empty".into());
            }
            local_proxy_kaigi_spool_override = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-kaigi-policy=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-kaigi-policy` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase();
            match normalized.as_str() {
                "public" | "authenticated" => {
                    local_proxy_kaigi_policy_override = Some(normalized);
                }
                _ => {
                    return Err(
                        "`--local-proxy-kaigi-policy` must be `public` or `authenticated`".into(),
                    );
                }
            }
        } else if let Some(rest) = arg.strip_prefix("--max-peers=") {
            max_peers = Some(parse_usize(rest, "--max-peers")?);
        } else if let Some(rest) = arg.strip_prefix("--retry-budget=") {
            retry_budget = Some(parse_usize(rest, "--retry-budget")?);
        } else if let Some(rest) = arg.strip_prefix("--provider=") {
            provider_specs.push(parse_gateway_provider_spec(rest)?);
        } else {
            return Err(format!(
                "unrecognised option `{arg}` for `sorafs_cli fetch`"
            ));
        }
    }

    let plan_source = plan_source
        .ok_or_else(|| "missing required `--plan=PATH` for `sorafs_cli fetch`".to_string())?;
    let manifest_id_hex = manifest_id_hex
        .ok_or_else(|| "missing required `--manifest-id=HEX` for `sorafs_cli fetch`".to_string())?;
    if manifest_id_hex.len() != 64 || !manifest_id_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("`--manifest-id` must be a 32-byte hex string".to_string());
    }
    if let Some(cid) = &manifest_cid_hex
        && (cid.len() != 64 || !cid.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("`--manifest-cid` must be a 32-byte hex string".to_string());
    }
    if provider_specs.is_empty() {
        return Err("provide at least one `--provider` entry".to_string());
    }

    let has_manifest_report = manifest_report_source.is_some();
    let manifest_report = if let Some(source) = manifest_report_source.take() {
        Some(source.read()?)
    } else {
        None
    };
    if let Some(report) = manifest_report.as_ref() {
        if manifest_envelope.is_none()
            && let Some(encoded) = report.get("manifest_b64").and_then(Value::as_str)
            && !encoded.trim().is_empty()
        {
            manifest_envelope = Some(encoded.trim().to_string());
        }
        if manifest_cid_hex.is_none()
            && let Some(cid) = report.get("manifest_id_hex").and_then(Value::as_str)
        {
            manifest_cid_hex = Some(cid.trim().to_ascii_lowercase());
        }
    }

    let plan_json = plan_source.read()?;
    let plan_with_handle = build_plan_from_specs(&plan_json, chunker_handle_hint.as_deref())?;
    let plan = plan_with_handle.plan;
    let chunker_handle = plan_with_handle.chunker_handle;

    let gateway_config = GatewayFetchConfig {
        manifest_id_hex: manifest_id_hex.clone(),
        chunker_handle: chunker_handle.clone(),
        manifest_envelope_b64: manifest_envelope.clone(),
        client_id: client_id.clone(),
        expected_manifest_cid_hex: manifest_cid_hex.clone(),
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: expected_cache_version.clone(),
        moderation_token_key_b64: moderation_token_key_b64.clone(),
    };

    let provider_inputs: Vec<GatewayProviderInput> = provider_specs
        .iter()
        .map(|spec| GatewayProviderInput {
            name: spec.name.clone(),
            provider_id_hex: spec.provider_id_hex.clone(),
            base_url: spec.base_url.clone(),
            stream_token_b64: spec.stream_token_b64.clone(),
            privacy_events_url: spec.privacy_events_url.clone(),
        })
        .collect();

    let context = GatewayFetchContext::new(gateway_config.clone(), provider_inputs.clone())
        .map_err(|err| format!("failed to construct gateway context: {err}"))?;

    let context_providers = context.providers();
    if context_providers.is_empty() {
        return Err("gateway context did not expose any providers".to_string());
    }

    let metadata: Vec<ProviderMetadata> = context_providers
        .iter()
        .map(|provider| {
            let alias = provider.id().as_str().to_string();
            let mut meta = provider
                .metadata()
                .cloned()
                .unwrap_or_else(ProviderMetadata::new);
            meta.provider_id = Some(alias.clone());
            if !meta.profile_aliases.iter().any(|entry| entry == &alias) {
                meta.profile_aliases.push(alias.clone());
            }
            if meta.range_capability.is_none() {
                meta.range_capability = Some(RangeCapability {
                    max_chunk_span: u32::MAX,
                    min_granularity: 1,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: true,
                });
            }
            if meta.stream_budget.is_none()
                && let Some(max_streams) = meta.max_streams
            {
                meta.stream_budget = Some(StreamBudget {
                    max_in_flight: max_streams,
                    max_bytes_per_sec: 0,
                    burst_bytes: None,
                });
            }
            meta
        })
        .collect();

    let telemetry_snapshot = TelemetrySnapshot::default();
    let mut orchestrator_config = if let Some(source) = orchestrator_config_source {
        let value = source.read()?;
        orchestrator_config_from_json(&value)
            .map_err(|err| format!("failed to parse orchestrator config JSON: {err}"))?
    } else {
        OrchestratorConfig::default()
    };
    if let Some(path) = scoreboard_out {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create scoreboard directory `{}`: {err}",
                    parent.display()
                )
            })?;
        }
        orchestrator_config.scoreboard.persist_path = Some(path);
    }
    if let Some(now) = scoreboard_now {
        orchestrator_config.scoreboard.now_unix_secs = now;
    } else if orchestrator_config.scoreboard.persist_path.is_some()
        && orchestrator_config.scoreboard.now_unix_secs == 0
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_secs();
        orchestrator_config.scoreboard.now_unix_secs = now;
    }
    if let Some(mode) = write_mode {
        orchestrator_config = orchestrator_config.with_write_mode(mode);
    }
    let provider_counts = GatewayProviderCounts::new(0, provider_inputs.len());
    let scoreboard_metadata = build_gateway_scoreboard_metadata(&GatewayScoreboardMetadataInput {
        provider_counts,
        max_peers,
        retry_budget,
        manifest_envelope_present: manifest_envelope.is_some(),
        gateway_manifest_id: Some(manifest_id_hex.as_str()),
        gateway_manifest_cid: manifest_cid_hex.as_deref(),
        transport_policy,
        transport_policy_override,
        anonymity_policy,
        anonymity_policy_override,
        write_mode: orchestrator_config.write_mode,
        scoreboard_now,
        telemetry_source: telemetry_source_label.as_deref(),
    });
    orchestrator_config.scoreboard.persist_metadata = Some(scoreboard_metadata);
    if let Some(phase) = rollout_phase {
        orchestrator_config = orchestrator_config.with_rollout_phase(phase);
    }
    if let Some(source) = taikai_cache_source {
        let value = source.read()?;
        match parse_taikai_cache_override(value)? {
            Some(cache) => {
                orchestrator_config.taikai_cache = Some(cache);
            }
            None => {
                orchestrator_config.taikai_cache = None;
            }
        }
    }
    if let Some(proxy_cfg) = orchestrator_config.local_proxy.as_mut() {
        if let Some(mode) = local_proxy_mode_override {
            proxy_cfg.proxy_mode = mode;
        }
        if let Some(spool) = local_proxy_spool_override.clone() {
            proxy_cfg.norito_bridge = Some(ProxyNoritoBridgeConfig {
                spool_dir: spool,
                extension: Some("norito".to_string()),
            });
        }
        if matches!(proxy_cfg.proxy_mode, ProxyMode::Bridge) && proxy_cfg.norito_bridge.is_none() {
            proxy_cfg.norito_bridge = Some(ProxyNoritoBridgeConfig {
                spool_dir: PROVISION_SPOOL_DIR.to_string(),
                extension: Some("norito".to_string()),
            });
        }
        if matches!(proxy_cfg.proxy_mode, ProxyMode::Bridge) && proxy_cfg.kaigi_bridge.is_none() {
            proxy_cfg.kaigi_bridge = Some(ProxyKaigiBridgeConfig {
                spool_dir: PROVISION_SPOOL_DIR.to_string(),
                extension: Some("norito".to_string()),
                room_policy: Some("public".to_string()),
            });
        }
        if let Some(policy) = local_proxy_kaigi_policy_override.clone() {
            let bridge = proxy_cfg
                .kaigi_bridge
                .get_or_insert_with(|| ProxyKaigiBridgeConfig {
                    spool_dir: PROVISION_SPOOL_DIR.to_string(),
                    extension: Some("norito".to_string()),
                    room_policy: None,
                });
            bridge.room_policy = Some(policy);
            if bridge.extension.is_none() {
                bridge.extension = Some("norito".to_string());
            }
        }
        if let Some(spool) = local_proxy_kaigi_spool_override.clone() {
            let bridge = proxy_cfg
                .kaigi_bridge
                .get_or_insert_with(|| ProxyKaigiBridgeConfig {
                    spool_dir: spool.clone(),
                    extension: Some("norito".to_string()),
                    room_policy: None,
                });
            bridge.spool_dir = spool;
            if bridge.extension.is_none() {
                bridge.extension = Some("norito".to_string());
            }
        }
    } else if local_proxy_mode_override.is_some() || local_proxy_spool_override.is_some() {
        return Err("`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, and `--local-proxy-kaigi-policy` require `local_proxy` in the orchestrator config".to_string());
    } else if local_proxy_kaigi_spool_override.is_some()
        || local_proxy_kaigi_policy_override.is_some()
    {
        return Err("`--local-proxy-kaigi-spool` and `--local-proxy-kaigi-policy` require `local_proxy` in the orchestrator config".to_string());
    }
    let local_proxy_snapshot = orchestrator_config.local_proxy.clone();
    if local_proxy_manifest_out.is_some() {
        let Some(proxy_cfg) = local_proxy_snapshot.as_ref() else {
            return Err(
                "--local-proxy-manifest-out requires `local_proxy` in the orchestrator config"
                    .to_string(),
            );
        };
        if !proxy_cfg.emit_browser_manifest {
            return Err(
                "--local-proxy-manifest-out requires `local_proxy.emit_browser_manifest` to be true"
                    .to_string(),
            );
        }
        #[cfg(not(feature = "local-quic-proxy"))]
        {
            return Err(
                "--local-proxy-manifest-out requires local QUIC proxy runtime support; rebuild `sorafs_cli` with the `local-quic-proxy` feature"
                    .to_string(),
            );
        }
    }
    let mut fetch_options = orchestrator_config.fetch.clone();
    let scoreboard = sorafs_car::scoreboard::build_scoreboard(
        &plan,
        &metadata,
        &telemetry_snapshot,
        &orchestrator_config.scoreboard,
    )
    .map_err(|err| format!("failed to build provider scoreboard: {err}"))?;

    let eligible_count = scoreboard
        .entries()
        .iter()
        .filter(|entry| matches!(entry.eligibility, Eligibility::Eligible))
        .count();
    if eligible_count == 0 {
        return Err("no eligible providers available after capability checks".to_string());
    }

    let ineligible_providers: Vec<Value> = scoreboard
        .entries()
        .iter()
        .filter_map(|entry| match &entry.eligibility {
            Eligibility::Ineligible(reason) => {
                let mut obj = Map::new();
                obj.insert(
                    "provider".into(),
                    Value::from(entry.provider.id().as_str().to_string()),
                );
                obj.insert("reason".into(), Value::from(reason.to_string()));
                Some(Value::Object(obj))
            }
            Eligibility::Eligible => None,
        })
        .collect();

    if let Some(limit) = max_peers {
        let limit = limit.max(1);
        fetch_options.global_parallel_limit = Some(
            fetch_options
                .global_parallel_limit
                .map_or(limit, |existing| existing.min(limit)),
        );
    }
    if let Some(budget) = retry_budget {
        fetch_options.per_chunk_retry_limit = Some(budget);
    }
    // Preserve legacy CLI behaviour for tests and offline flows: unless callers explicitly
    // request manifest verification inputs, do not require the gateway manifest endpoint.
    let explicit_manifest_verification = manifest_envelope.is_some()
        || manifest_cid_hex.is_some()
        || expected_cache_version.is_some();
    if has_manifest_report || !explicit_manifest_verification {
        fetch_options.verify_lengths = false;
        fetch_options.verify_digests = false;
    }

    if let Some(limit) = max_peers {
        let limit = limit.max(1);
        orchestrator_config.max_providers = std::num::NonZeroUsize::new(limit);
    }
    let mut telemetry_region_effective = telemetry_region
        .map(|region| region.trim().to_string())
        .filter(|value| !value.is_empty());
    if telemetry_region_effective.is_none() {
        telemetry_region_effective = orchestrator_config.telemetry_region.clone();
    }
    orchestrator_config.telemetry_region = telemetry_region_effective.clone();
    orchestrator_config.fetch = fetch_options;
    if let Some(policy) = transport_policy {
        orchestrator_config.transport_policy = policy;
    }
    if let Some(policy) = anonymity_policy {
        orchestrator_config.anonymity_policy = policy;
        orchestrator_config.anonymity_policy_override = Some(policy);
    }
    if let Some(policy) = transport_policy_override {
        orchestrator_config.policy_override.transport_policy = Some(policy);
    }
    let requested_anonymity_override = anonymity_policy_override;

    let rollout_phase = orchestrator_config.rollout_phase;
    let write_mode = orchestrator_config.write_mode;
    let runtime =
        Runtime::new().map_err(|err| format!("failed to initialise Tokio runtime: {err}"))?;
    let session = runtime
        .block_on(fetch_via_gateway(
            orchestrator_config,
            &plan,
            gateway_config,
            provider_inputs,
            Some(&telemetry_snapshot),
            max_peers,
        ))
        .map_err(|err| format!("fetch failed: {err}"))?;

    let outcome = &session.outcome;

    if let Some(path) = output_path {
        ensure_parent_dir(&path)?;
        let assembled = outcome.assemble_payload();
        fs::write(&path, &assembled)
            .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    }

    let mut summary = build_fetch_summary(
        manifest_id_hex.as_str(),
        &chunker_handle,
        &plan,
        &session,
        FetchSummaryOptions {
            client_id: client_id.as_deref(),
            rollout_phase,
            write_mode,
            cache_profile,
        },
    );
    if let Some(region) = telemetry_region_effective.as_deref()
        && let Some(obj) = summary.as_object_mut()
    {
        obj.insert("telemetry_region".into(), Value::from(region));
    }
    insert_telemetry_source(&mut summary, telemetry_source_label.as_deref());
    if let Some(obj) = summary.as_object_mut() {
        obj.insert(
            "ineligible_providers".into(),
            Value::Array(ineligible_providers),
        );
        if let Some(policy) = requested_anonymity_override {
            obj.insert("anonymity_policy".into(), Value::from(policy.label()));
            obj.insert("anonymity_policy_override".into(), Value::from(true));
            obj.insert(
                "anonymity_policy_override_label".into(),
                Value::from(policy.label()),
            );
        }
        if let Some(proxy_cfg) = local_proxy_snapshot.as_ref() {
            obj.insert(
                "local_proxy_mode".into(),
                Value::from(proxy_cfg.proxy_mode.as_str()),
            );
            if let Some(bridge) = proxy_cfg.norito_bridge.as_ref() {
                obj.insert(
                    "local_proxy_norito_spool".into(),
                    Value::from(bridge.spool_dir.clone()),
                );
            }
            if let Some(bridge) = proxy_cfg.kaigi_bridge.as_ref() {
                obj.insert(
                    "local_proxy_kaigi_spool".into(),
                    Value::from(bridge.spool_dir.clone()),
                );
                let policy = bridge
                    .room_policy
                    .as_deref()
                    .unwrap_or("public")
                    .to_string();
                obj.insert("local_proxy_kaigi_policy".into(), Value::from(policy));
            }
        }
    }
    if let Some(path) = local_proxy_manifest_out {
        let manifest = session.local_proxy_manifest.as_ref().ok_or_else(|| {
            "--local-proxy-manifest-out requires `local_proxy.emit_browser_manifest = true` and an active local proxy runtime".to_string()
        })?;
        ensure_parent_dir(&path)?;
        let manifest_value =
            to_value(manifest).expect("local proxy manifest should serialise to JSON");
        let manifest_json =
            to_string_pretty(&manifest_value).expect("local proxy manifest should emit valid JSON");
        fs::write(&path, manifest_json.as_bytes())
            .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    }
    let summary_text = to_string_pretty(&summary).expect("fetch summary should be serialisable");
    println!("{summary_text}");
    if let Some(path) = json_out {
        ensure_parent_dir(&path)?;
        fs::write(&path, summary_text.as_bytes())
            .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    }

    Ok(())
}

fn proxy_set_mode(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(proxy_usage());
    }

    let mut config_path: Option<PathBuf> = None;
    let mut requested_mode: Option<ProxyMode> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut config_out: Option<PathBuf> = None;
    let mut dry_run = false;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(proxy_usage());
        }
        if let Some(rest) = arg.strip_prefix("--orchestrator-config=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--orchestrator-config` must not be empty".into());
            }
            if trimmed == "-" {
                return Err("`--orchestrator-config` requires a file path; stdin is not supported for remediation".into());
            }
            config_path = Some(PathBuf::from(trimmed));
        } else if let Some(rest) = arg.strip_prefix("--mode=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--mode` must not be empty".into());
            }
            let parsed = ProxyMode::parse(trimmed)
                .ok_or_else(|| "`--mode` must be one of bridge|metadata-only".to_string())?;
            requested_mode = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--json-out` must not be empty".into());
            }
            json_out = Some(PathBuf::from(trimmed));
        } else if let Some(rest) = arg.strip_prefix("--config-out=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--config-out` must not be empty".into());
            }
            config_out = Some(PathBuf::from(trimmed));
        } else if arg == "--dry-run" {
            dry_run = true;
        } else {
            return Err(format!(
                "unrecognised option `{arg}` for `sorafs_cli proxy set-mode`"
            ));
        }
    }

    let config_path =
        config_path.ok_or_else(|| "missing required `--orchestrator-config=PATH`".to_string())?;
    let requested_mode = requested_mode
        .ok_or_else(|| "missing required `--mode=bridge|metadata-only`".to_string())?;

    let config_bytes = fs::read_to_string(&config_path)
        .map_err(|err| format!("failed to read `{}`: {err}", config_path.display()))?;
    let config_value: Value = norito::json::from_str(&config_bytes)
        .map_err(|err| format!("failed to parse orchestrator config JSON: {err}"))?;
    let mut orchestrator_config = orchestrator_config_from_json(&config_value)
        .map_err(|err| format!("failed to decode orchestrator config structure: {err}"))?;
    let (previous_mode, telemetry_label, bind_addr, guard_cache_key_hex) = {
        let proxy_cfg = orchestrator_config.local_proxy.as_mut().ok_or_else(|| {
            "orchestrator config does not enable `local_proxy`; remediation is unavailable"
                .to_string()
        })?;
        let prev = proxy_cfg.proxy_mode.clone();
        proxy_cfg.proxy_mode = requested_mode.clone();
        let label = proxy_cfg.telemetry_label.clone();
        let addr = proxy_cfg.bind_addr.clone();
        let guard_key = proxy_cfg.guard_cache_key_hex.clone();
        (prev, label, addr, guard_key)
    };
    let effective_mode = orchestrator_config
        .local_proxy
        .as_ref()
        .expect("local proxy must be present")
        .proxy_mode
        .clone();

    let target_config_path = config_out.as_ref().unwrap_or(&config_path);
    if !dry_run {
        let config_value = orchestrator_config_to_json(&orchestrator_config);
        let config_json = norito::json::to_json_pretty(&config_value)
            .map_err(|err| format!("failed to render orchestrator config JSON: {err}"))?;
        write_text(target_config_path, config_json.as_bytes())?;
    }

    let mut summary = Map::new();
    summary.insert(
        "mode_previous".into(),
        Value::String(previous_mode.as_str().to_string()),
    );
    summary.insert(
        "mode_effective".into(),
        Value::String(effective_mode.as_str().to_string()),
    );
    summary.insert(
        "mode_requested".into(),
        Value::String(requested_mode.as_str().to_string()),
    );
    summary.insert("dry_run".into(), Value::from(dry_run));
    summary.insert(
        "config_path".into(),
        Value::String(config_path.to_string_lossy().into()),
    );
    if !dry_run {
        summary.insert(
            "config_written".into(),
            Value::String(target_config_path.to_string_lossy().into()),
        );
    } else {
        summary.insert("config_written".into(), Value::Null);
    }
    if let Some(label) = telemetry_label {
        summary.insert("telemetry_label".into(), Value::String(label));
    }
    summary.insert("bind_addr".into(), Value::String(bind_addr));
    if let Some(guard_key) = guard_cache_key_hex {
        summary.insert("guard_cache_key_hex".into(), Value::String(guard_key));
    }

    let summary_json = norito::json::to_json_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary JSON: {err}"))?;

    if let Some(path) = json_out {
        write_text(&path, summary_json.as_bytes())?;
    } else {
        println!("{summary_json}");
    }

    Ok(())
}

fn moderation_validate_repro(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation validate-repro`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation validate-repro`".to_string()
    })?;
    let bytes = fs::read(&manifest_path)
        .map_err(|err| format!("failed to read `{}`: {err}", manifest_path.display()))?;

    let manifest: ModerationReproManifestV1 = match format.as_str() {
        "json" => norito::json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse JSON reproducibility manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).map_err(|err| {
            format!(
                "failed to decode Norito reproducibility manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        other => {
            return Err(format!(
                "unsupported `--format={other}` for `sorafs_cli moderation validate-repro` (expected `json` or `norito`)"
            ));
        }
    };

    let summary = manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;

    println!(
        "reproducibility manifest {} validated (models={}, signers={}, issued_at={})",
        hex_encode(summary.manifest_id),
        summary.model_count,
        summary.signer_count,
        summary.issued_at_unix
    );

    Ok(())
}

fn moderation_validate_corpus(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation validate-corpus`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation validate-corpus`".to_string()
    })?;
    let bytes = fs::read(&manifest_path)
        .map_err(|err| format!("failed to read `{}`: {err}", manifest_path.display()))?;

    let manifest: AdversarialCorpusManifestV1 = match format.as_str() {
        "json" => norito::json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse JSON adversarial corpus manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).map_err(|err| {
            format!(
                "failed to decode Norito adversarial corpus manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        other => {
            return Err(format!(
                "unsupported `--format={other}` for `sorafs_cli moderation validate-corpus` (expected `json` or `norito`)"
            ));
        }
    };

    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;

    let family_count = manifest.families.len();
    let variant_count: usize = manifest
        .families
        .iter()
        .map(|family| family.variants.len())
        .sum();
    let cohort = manifest.cohort_label.as_deref().unwrap_or("-");
    println!(
        "adversarial corpus manifest validated (issued_at={}, cohort={}, families={}, variants={})",
        manifest.issued_at_unix, cohort, family_count, variant_count
    );

    Ok(())
}

fn moderation_honey_audit(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }

    let mut manifest_id_hex: Option<String> = None;
    let mut chunker_handle = DEFAULT_CHUNKER_HANDLE.to_string();
    let mut expected_cache_version: Option<String> = None;
    let mut moderation_token_key_b64: Option<String> = None;
    let mut honey_digests: Vec<String> = Vec::new();
    let mut provider_specs: Vec<GatewayProviderSpec> = Vec::new();
    let mut json_out: Option<PathBuf> = None;
    let mut markdown_out: Option<PathBuf> = None;
    let mut require_proof = false;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        if let Some(rest) = arg.strip_prefix("--manifest-id=") {
            manifest_id_hex = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--chunker-handle=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--chunker-handle` must not be empty".into());
            }
            chunker_handle = trimmed.to_string();
        } else if let Some(rest) = arg.strip_prefix("--expected-cache-version=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--expected-cache-version` must not be empty".into());
            }
            expected_cache_version = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--moderation-key-b64=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--moderation-key-b64` must not be empty".into());
            }
            moderation_token_key_b64 = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--honey=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--honey` digests must not be empty".into());
            }
            honey_digests.push(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--markdown-out=") {
            markdown_out = Some(PathBuf::from(rest));
        } else if arg == "--require-proof" {
            require_proof = true;
        } else if let Some(rest) = arg.strip_prefix("--provider") {
            if let Some(spec) = rest.strip_prefix('=') {
                provider_specs.push(parse_gateway_provider_spec(spec)?);
            } else {
                return Err("expected `--provider name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64`".to_string());
            }
        } else {
            return Err(moderation_usage());
        }
    }

    let manifest_id_hex = manifest_id_hex.ok_or_else(|| {
        "missing required `--manifest-id` for `sorafs_cli moderation honey-audit`".to_string()
    })?;
    if manifest_id_hex.len() != 64 || !manifest_id_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("`--manifest-id` must be a 32-byte hex string".to_string());
    }
    if honey_digests.is_empty() {
        return Err("provide at least one `--honey=HEX` digest to probe".to_string());
    }
    if provider_specs.is_empty() {
        return Err("provide at least one `--provider` entry".to_string());
    }

    let mut specs = Vec::with_capacity(honey_digests.len());
    for (idx, digest_hex) in honey_digests.iter().enumerate() {
        let digest = hex::decode(digest_hex.trim())
            .map_err(|err| format!("invalid `--honey` digest `{digest_hex}`: {err}"))?;
        if digest.len() != 32 {
            return Err(format!(
                "`--honey` digest `{digest_hex}` must decode to 32 bytes"
            ));
        }
        let mut digest_bytes = [0u8; 32];
        digest_bytes.copy_from_slice(&digest);
        specs.push(ChunkFetchSpec {
            chunk_index: idx,
            offset: 0,
            length: 0,
            digest: digest_bytes,
            taikai_segment_hint: None,
        });
    }

    let gateway_config = GatewayFetchConfig {
        manifest_id_hex: manifest_id_hex.clone(),
        chunker_handle: chunker_handle.clone(),
        manifest_envelope_b64: None,
        client_id: None,
        expected_manifest_cid_hex: None,
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: expected_cache_version.clone(),
        moderation_token_key_b64: moderation_token_key_b64.clone(),
    };

    let provider_inputs: Vec<GatewayProviderInput> = provider_specs
        .iter()
        .map(|spec| GatewayProviderInput {
            name: spec.name.clone(),
            provider_id_hex: spec.provider_id_hex.clone(),
            base_url: spec.base_url.clone(),
            stream_token_b64: spec.stream_token_b64.clone(),
            privacy_events_url: spec.privacy_events_url.clone(),
        })
        .collect();

    let context = GatewayFetchContext::new(gateway_config, provider_inputs)
        .map_err(|err| format!("failed to construct gateway context: {err}"))?;
    let providers = context.providers();
    if providers.is_empty() {
        return Err("gateway context did not expose any providers".to_string());
    }

    let mut validator = PolicyEvidenceValidator::new();
    if let Some(version) = expected_cache_version.as_deref() {
        validator = validator.with_expected_cache_version(version);
    }
    if require_proof || moderation_token_key_b64.is_some() {
        validator = validator.require_moderation_proof();
    }

    let runtime =
        Runtime::new().map_err(|err| format!("failed to initialise Tokio runtime: {err}"))?;

    let mut digest_reports = Vec::new();
    for spec in &specs {
        let reports = runtime
            .block_on(run_honey_probe(
                &context.fetcher(),
                &providers,
                spec,
                &validator,
            ))
            .map_err(|err| {
                format!(
                    "honey probe failed for digest {}: {err}",
                    hex::encode(spec.digest)
                )
            })?;
        digest_reports.push((spec.clone(), reports));
    }

    for (spec, reports) in &digest_reports {
        println!("digest {}:", hex::encode(spec.digest));
        for report in reports {
            let evidence = &report.policy.evidence;
            let proof_state = if report.policy.moderation_proof.is_some() {
                "verified"
            } else if report.policy.evidence.proof_token_b64.is_some() {
                "present"
            } else {
                "missing"
            };
            println!(
                "  - provider {} cache_version={:?} code={:?} status={} proof={proof_state}",
                report.provider_id,
                evidence
                    .cache_version
                    .as_deref()
                    .or(evidence.denylist_version.as_deref()),
                evidence.code,
                evidence.canonical_status
            );
        }
    }

    if let Some(path) = json_out {
        let to_value_opt = |opt: &Option<String>| {
            opt.as_ref()
                .map(|value| Value::from(value.clone()))
                .unwrap_or(Value::Null)
        };
        let digests: Vec<Value> = digest_reports
            .iter()
            .map(|(spec, reports)| {
                let providers: Vec<Value> = reports
                    .iter()
                    .map(|report| {
                        let evidence = &report.policy.evidence;
                        let mut map = Map::new();
                        map.insert("provider".into(), Value::from(report.provider_id.clone()));
                        map.insert(
                            "cache_version".into(),
                            to_value_opt(&evidence.cache_version),
                        );
                        map.insert(
                            "denylist_version".into(),
                            to_value_opt(&evidence.denylist_version),
                        );
                        map.insert(
                            "code".into(),
                            evidence
                                .code
                                .as_ref()
                                .map(|value| Value::from(value.clone()))
                                .unwrap_or(Value::Null),
                        );
                        map.insert(
                            "observed_status".into(),
                            Value::from(evidence.observed_status.as_u16()),
                        );
                        map.insert(
                            "canonical_status".into(),
                            Value::from(evidence.canonical_status.as_u16()),
                        );
                        map.insert(
                            "proof_token_present".into(),
                            Value::from(evidence.proof_token_b64.is_some()),
                        );
                        map.insert(
                            "proof_verified".into(),
                            Value::from(report.policy.proof.is_some()),
                        );
                        map.insert(
                            "moderation_proof_verified".into(),
                            Value::from(report.policy.moderation_proof.is_some()),
                        );
                        Value::Object(map)
                    })
                    .collect();
                let mut digest_map = Map::new();
                digest_map.insert("digest_hex".into(), Value::from(hex::encode(spec.digest)));
                digest_map.insert("reports".into(), Value::Array(providers));
                Value::Object(digest_map)
            })
            .collect();

        let mut summary = Map::new();
        summary.insert(
            "manifest_id_hex".into(),
            Value::from(manifest_id_hex.clone()),
        );
        summary.insert("chunker_handle".into(), Value::from(chunker_handle.clone()));
        summary.insert(
            "expected_cache_version".into(),
            to_value_opt(&expected_cache_version),
        );
        summary.insert(
            "moderation_proof_required".into(),
            Value::from(require_proof || moderation_token_key_b64.is_some()),
        );
        summary.insert("provider_count".into(), Value::from(providers.len() as u64));
        summary.insert("digests".into(), Value::Array(digests));
        let summary = Value::Object(summary);
        let rendered =
            to_string_pretty(&summary).map_err(|err| format!("failed to render JSON: {err}"))?;
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    }

    if let Some(path) = markdown_out {
        let mut md = String::from("# Honey Audit Report\n\n");
        md.push_str(&format!(
            "- manifest: `{}`\n- chunker: `{}`\n- expected cache version: `{}`\n- providers: {}\n\n",
            manifest_id_hex,
            chunker_handle,
            expected_cache_version
                .as_deref()
                .unwrap_or("unspecified"),
            providers.len()
        ));
        for (spec, reports) in &digest_reports {
            md.push_str(&format!("## digest `{}`\n", hex::encode(spec.digest)));
            for report in reports {
                let evidence = &report.policy.evidence;
                md.push_str(&format!(
                    "- {}: status={} code={:?} cache_version={:?} moderation_proof={}\n",
                    report.provider_id,
                    evidence.canonical_status,
                    evidence.code,
                    evidence
                        .cache_version
                        .as_deref()
                        .or(evidence.denylist_version.as_deref()),
                    if report.policy.moderation_proof.is_some() {
                        "verified"
                    } else {
                        "absent"
                    }
                ));
            }
            md.push('\n');
        }
        write_text(&path, md.as_bytes())?;
    }

    Ok(())
}

fn appeal_quote(raw_args: Vec<String>) -> Result<(), String> {
    let mut class: Option<AppealClass> = None;
    let mut backlog: u32 = 0;
    let mut evidence_size_mb: u32 = 0;
    let mut panel_size_override: Option<u32> = None;
    let mut urgency = AppealUrgency::Normal;
    let mut format = String::from("table");
    let mut config_source: Option<JsonSource> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(appeal_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--class" => {
                class = Some(
                    value
                        .parse::<AppealClass>()
                        .map_err(|err| err.to_string())?,
                );
            }
            "--backlog" => backlog = parse_u32_arg("backlog", value, CONTEXT_APPEAL_QUOTE)?,
            "--evidence-mb" => {
                evidence_size_mb = parse_u32_arg("evidence-mb", value, CONTEXT_APPEAL_QUOTE)?;
            }
            "--panel-size" => {
                panel_size_override =
                    Some(parse_u32_arg("panel-size", value, CONTEXT_APPEAL_QUOTE)?);
            }
            "--urgency" => {
                urgency = value
                    .parse::<AppealUrgency>()
                    .map_err(|err| err.to_string())?;
            }
            "--format" => format = value.trim().to_ascii_lowercase(),
            "--config" => {
                config_source = Some(JsonSource::from_arg(value).map_err(|err| {
                    format!("failed to parse --config for `{CONTEXT_APPEAL_QUOTE}`: {err}")
                })?)
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `{CONTEXT_APPEAL_QUOTE}`"
                ));
            }
        }
    }

    let config = if let Some(source) = config_source {
        let value = source.read()?;
        AppealPricingConfig::from_manifest_value(&value)
            .map_err(|err| format!("failed to parse appeal pricing config: {err}"))?
    } else {
        AppealPricingConfig::baseline_v1()
    };
    let class = class.ok_or_else(|| {
        format!(
            "missing required `--class=content|access|fraud|other` for `{CONTEXT_APPEAL_QUOTE}`"
        )
    })?;
    let panel_size = panel_size_override.unwrap_or_else(|| config.default_panel_size());
    let quote = config
        .quote(AppealQuoteInput {
            class,
            backlog,
            evidence_size_mb,
            urgency,
            panel_size,
        })
        .map_err(|err| err.to_string())?;
    let class_cfg = config
        .class_config(class)
        .expect("quoted class must have a configuration entry");
    let valid_until_unix = compute_valid_until(config.quote_ttl_secs());

    let context = AppealQuoteInputs {
        config: &config,
        class,
        urgency,
        backlog,
        evidence_size_mb,
        panel_size,
        quote: &quote,
        valid_until_unix,
    };

    match format.as_str() {
        "json" => print_appeal_quote_json(&context),
        "table" | "text" | "" => {
            print_appeal_quote_table(class_cfg, &context);
            Ok(())
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{CONTEXT_APPEAL_QUOTE}` (expected table|json)"
        )),
    }
}

fn appeal_settle(raw_args: Vec<String>) -> Result<(), String> {
    let mut deposit: Option<Decimal> = None;
    let mut verdict: Option<AppealVerdict> = None;
    let mut panel_size_override: Option<u32> = None;
    let mut format = String::from("table");
    let mut config_source: Option<JsonSource> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(appeal_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--deposit" => {
                deposit = Some(parse_decimal_arg("deposit", value, CONTEXT_APPEAL_SETTLE)?);
            }
            "--outcome" => {
                verdict = Some(parse_appeal_verdict(value)?);
            }
            "--panel-size" => {
                panel_size_override =
                    Some(parse_u32_arg("panel-size", value, CONTEXT_APPEAL_SETTLE)?);
            }
            "--format" => format = value.trim().to_ascii_lowercase(),
            "--config" => {
                config_source = Some(JsonSource::from_arg(value).map_err(|err| {
                    format!("failed to parse --config for `{CONTEXT_APPEAL_SETTLE}`: {err}")
                })?)
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `{CONTEXT_APPEAL_SETTLE}`"
                ));
            }
        }
    }

    let config = if let Some(source) = config_source {
        let value = source.read()?;
        AppealSettlementConfig::from_manifest_value(&value)
            .map_err(|err| format!("failed to parse appeal settlement config: {err}"))?
    } else {
        AppealSettlementConfig::baseline_v1()
    };
    let deposit = deposit.ok_or_else(|| {
        format!("missing required `--deposit` (XOR) for `{CONTEXT_APPEAL_SETTLE}`")
    })?;
    let verdict = verdict
        .ok_or_else(|| format!("missing required `--outcome` for `{CONTEXT_APPEAL_SETTLE}`"))?;
    let panel_size = panel_size_override.unwrap_or_else(|| config.default_panel_size());
    let breakdown = config
        .settle(deposit, panel_size, verdict)
        .map_err(|err| match err {
            AppealSettlementError::MissingDecisionRule { decision } => {
                format!("settlement config is missing a rule for `{decision}`")
            }
            AppealSettlementError::InvalidDeposit | AppealSettlementError::InvalidPanelSize => {
                err.to_string()
            }
        })?;

    let context = AppealSettlementInputs {
        config: &config,
        deposit_xor: deposit,
        panel_size,
        verdict,
        breakdown,
    };

    match format.as_str() {
        "json" => print_appeal_settlement_json(&context),
        "table" | "text" | "" => {
            print_appeal_settlement_table(&context);
            Ok(())
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{CONTEXT_APPEAL_SETTLE}` (expected table|json)"
        )),
    }
}

fn appeal_disburse(raw_args: Vec<String>) -> Result<(), String> {
    let mut deposit: Option<Decimal> = None;
    let mut verdict: Option<AppealVerdict> = None;
    let mut panel_size_override: Option<u32> = None;
    let mut format = String::from("table");
    let mut config_source: Option<JsonSource> = None;
    let mut refund_account: Option<AccountId> = None;
    let mut treasury_account: Option<AccountId> = None;
    let mut escrow_account: Option<AccountId> = None;
    let mut jurors: Vec<AccountId> = Vec::new();
    let mut no_shows: Vec<AccountId> = Vec::new();

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(appeal_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--deposit" => {
                deposit = Some(parse_decimal_arg(
                    "deposit",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--outcome" => verdict = Some(parse_appeal_verdict(value)?),
            "--panel-size" => {
                panel_size_override =
                    Some(parse_u32_arg("panel-size", value, CONTEXT_APPEAL_DISBURSE)?);
            }
            "--format" => format = value.trim().to_ascii_lowercase(),
            "--config" => {
                config_source = Some(JsonSource::from_arg(value).map_err(|err| {
                    format!("failed to parse --config for `{CONTEXT_APPEAL_DISBURSE}`: {err}")
                })?)
            }
            "--refund-account" => {
                refund_account = Some(parse_account_id_arg(
                    "refund-account",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--treasury-account" => {
                treasury_account = Some(parse_account_id_arg(
                    "treasury-account",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--escrow-account" => {
                escrow_account = Some(parse_account_id_arg(
                    "escrow-account",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--juror" => {
                jurors.push(parse_account_id_arg(
                    "juror",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--no-show" => {
                no_shows.push(parse_account_id_arg(
                    "no-show",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `{CONTEXT_APPEAL_DISBURSE}`"
                ));
            }
        }
    }

    let config = if let Some(source) = config_source {
        let value = source.read()?;
        AppealSettlementConfig::from_manifest_value(&value)
            .map_err(|err| format!("failed to parse appeal settlement config: {err}"))?
    } else {
        AppealSettlementConfig::baseline_v1()
    };
    let deposit = deposit.ok_or_else(|| {
        format!("missing required `--deposit` (XOR) for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    let verdict = verdict
        .ok_or_else(|| format!("missing required `--outcome` for `{CONTEXT_APPEAL_DISBURSE}`"))?;
    let refund_account = refund_account.ok_or_else(|| {
        format!("missing required `--refund-account` for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    let treasury_account = treasury_account.ok_or_else(|| {
        format!("missing required `--treasury-account` for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    let escrow_account = escrow_account.ok_or_else(|| {
        format!("missing required `--escrow-account` for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    if jurors.is_empty() {
        return Err(format!(
            "missing required `--juror` entries for `{CONTEXT_APPEAL_DISBURSE}` (supply at least one juror id)"
        ));
    }
    let panel_size = panel_size_override.unwrap_or_else(|| config.default_panel_size());

    let plan = config
        .disburse(AppealDisbursementInput {
            deposit_xor: deposit,
            panel_size,
            verdict,
            jurors: &jurors,
            no_shows: &no_shows,
            refund_account: &refund_account,
            treasury_account: &treasury_account,
            escrow_account: &escrow_account,
        })
        .map_err(|err| match err {
            AppealDisbursementError::Settlement(AppealSettlementError::MissingDecisionRule {
                decision,
            }) => format!("settlement config is missing a rule for `{decision}`"),
            other => other.to_string(),
        })?;

    let context = AppealDisbursementInputs {
        config: &config,
        plan,
    };
    match format.as_str() {
        "json" => print_appeal_disbursement_json(&context),
        "table" | "text" | "" => {
            print_appeal_disbursement_table(&context);
            Ok(())
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{CONTEXT_APPEAL_DISBURSE}` (expected table|json)"
        )),
    }
}

fn compute_valid_until(ttl_secs: u64) -> Option<u64> {
    if ttl_secs == 0 {
        return None;
    }
    let expiry = SystemTime::now().checked_add(Duration::from_secs(ttl_secs))?;
    Some(expiry.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

struct AppealQuoteInputs<'a> {
    config: &'a AppealPricingConfig,
    class: AppealClass,
    urgency: AppealUrgency,
    backlog: u32,
    evidence_size_mb: u32,
    panel_size: u32,
    quote: &'a AppealQuote,
    valid_until_unix: Option<u64>,
}

struct AppealSettlementInputs<'a> {
    config: &'a AppealSettlementConfig,
    deposit_xor: Decimal,
    panel_size: u32,
    verdict: AppealVerdict,
    breakdown: AppealSettlementBreakdown,
}

struct AppealDisbursementInputs<'a> {
    config: &'a AppealSettlementConfig,
    plan: AppealDisbursementPlan,
}

fn print_appeal_quote_table(class_cfg: &AppealClassConfig, ctx: &AppealQuoteInputs<'_>) {
    println!("Appeal deposit quote ({})", ctx.config.version());
    println!("  class: {:<8} urgency: {}", ctx.class, ctx.urgency);
    println!(
        "  deposit: {} XOR (raw {} XOR, min {}, max {})",
        format_decimal(ctx.quote.deposit_xor, 2),
        format_decimal(ctx.quote.breakdown.raw_deposit_xor, 2),
        format_decimal(class_cfg.min_deposit_xor, 2),
        format_decimal(class_cfg.max_deposit_xor, 2),
    );
    println!(
        "  backlog: {} (target {}), factor {}",
        ctx.backlog,
        class_cfg.backlog_target,
        format_decimal(ctx.quote.breakdown.backlog_factor, 4)
    );
    println!(
        "  evidence_size_mb: {} (divisor {}), size multiplier {}",
        ctx.evidence_size_mb,
        format_decimal(class_cfg.size_divisor_mb, 0),
        format_decimal(ctx.quote.breakdown.size_multiplier, 4),
    );
    println!(
        "  urgency multiplier: {}",
        format_decimal(ctx.quote.breakdown.urgency_multiplier, 4)
    );
    println!(
        "  panel multiplier: {} (panel {} / default {})",
        format_decimal(ctx.quote.breakdown.panel_multiplier, 4),
        ctx.panel_size,
        ctx.config.default_panel_size()
    );
    println!(
        "  surge multiplier: {}",
        format_decimal(ctx.quote.breakdown.surge_multiplier, 4)
    );
    if let Some(expiry) = ctx.valid_until_unix {
        println!("  valid until (unix): {expiry}");
    }
}

fn print_appeal_quote_json(ctx: &AppealQuoteInputs<'_>) -> Result<(), String> {
    let mut breakdown = Map::new();
    breakdown.insert(
        "base_rate_xor".into(),
        Value::String(format_decimal(ctx.quote.breakdown.base_rate_xor, 2)),
    );
    breakdown.insert(
        "backlog_factor".into(),
        Value::String(format_decimal(ctx.quote.breakdown.backlog_factor, 4)),
    );
    breakdown.insert(
        "size_multiplier".into(),
        Value::String(format_decimal(ctx.quote.breakdown.size_multiplier, 4)),
    );
    breakdown.insert(
        "urgency_multiplier".into(),
        Value::String(format_decimal(ctx.quote.breakdown.urgency_multiplier, 4)),
    );
    breakdown.insert(
        "panel_multiplier".into(),
        Value::String(format_decimal(ctx.quote.breakdown.panel_multiplier, 4)),
    );
    breakdown.insert(
        "surge_multiplier".into(),
        Value::String(format_decimal(ctx.quote.breakdown.surge_multiplier, 4)),
    );
    breakdown.insert(
        "raw_deposit_xor".into(),
        Value::String(format_decimal(ctx.quote.breakdown.raw_deposit_xor, 2)),
    );
    breakdown.insert(
        "min_deposit_xor".into(),
        Value::String(format_decimal(ctx.quote.breakdown.min_deposit_xor, 2)),
    );
    breakdown.insert(
        "max_deposit_xor".into(),
        Value::String(format_decimal(ctx.quote.breakdown.max_deposit_xor, 2)),
    );

    let mut root = Map::new();
    root.insert(
        "version".into(),
        Value::String(ctx.config.version().to_string()),
    );
    root.insert(
        "class".into(),
        Value::String(ctx.class.as_str().to_string()),
    );
    root.insert(
        "urgency".into(),
        Value::String(ctx.urgency.as_str().to_string()),
    );
    root.insert(
        "deposit_xor".into(),
        Value::String(format_decimal(ctx.quote.deposit_xor, 2)),
    );
    root.insert(
        "backlog_open_cases".into(),
        Value::Number(Number::from(ctx.backlog as u64)),
    );
    root.insert(
        "evidence_size_mb".into(),
        Value::Number(Number::from(ctx.evidence_size_mb as u64)),
    );
    root.insert(
        "panel_size".into(),
        Value::Number(Number::from(u64::from(ctx.panel_size))),
    );
    root.insert(
        "default_panel_size".into(),
        Value::Number(Number::from(ctx.config.default_panel_size() as u64)),
    );
    root.insert(
        "quote_ttl_secs".into(),
        Value::Number(Number::from(ctx.config.quote_ttl_secs())),
    );
    if let Some(expiry) = ctx.valid_until_unix {
        root.insert(
            "valid_until_unix".into(),
            Value::Number(Number::from(expiry)),
        );
    }
    root.insert("breakdown".into(), Value::Object(breakdown));

    let json = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to render JSON quote: {err}"))?;
    println!("{json}");
    Ok(())
}

fn print_appeal_settlement_table(ctx: &AppealSettlementInputs<'_>) {
    println!("Appeal settlement ({})", ctx.config.version());
    println!("  outcome: {}", ctx.verdict);
    println!("  deposit: {} XOR", format_decimal(ctx.deposit_xor, 2));
    println!(
        "  refund: {} XOR",
        format_decimal(ctx.breakdown.refund_xor, 2)
    );
    println!(
        "  treasury transfer: {} XOR",
        format_decimal(ctx.breakdown.treasury_xor, 2)
    );
    println!(
        "  held in escrow: {} XOR",
        format_decimal(ctx.breakdown.held_xor, 2)
    );
    println!(
        "  panel reward: {} jurors × {} XOR + bonus = {} XOR",
        ctx.panel_size,
        format_decimal(ctx.breakdown.panel_reward_per_juror_xor, 2),
        format_decimal(ctx.breakdown.panel_reward_total_xor, 2)
    );
}

fn print_appeal_settlement_json(ctx: &AppealSettlementInputs<'_>) -> Result<(), String> {
    let mut root = Map::new();
    root.insert(
        "version".into(),
        Value::String(ctx.config.version().to_string()),
    );
    root.insert("outcome".into(), Value::String(ctx.verdict.to_string()));
    root.insert(
        "deposit_xor".into(),
        Value::String(format_decimal(ctx.deposit_xor, 2)),
    );
    root.insert(
        "refund_xor".into(),
        Value::String(format_decimal(ctx.breakdown.refund_xor, 2)),
    );
    root.insert(
        "treasury_xor".into(),
        Value::String(format_decimal(ctx.breakdown.treasury_xor, 2)),
    );
    root.insert(
        "held_xor".into(),
        Value::String(format_decimal(ctx.breakdown.held_xor, 2)),
    );
    root.insert(
        "panel_size".into(),
        Value::Number(Number::from(u64::from(ctx.panel_size))),
    );
    root.insert(
        "panel_reward_per_juror_xor".into(),
        Value::String(format_decimal(ctx.breakdown.panel_reward_per_juror_xor, 2)),
    );
    root.insert(
        "panel_reward_total_xor".into(),
        Value::String(format_decimal(ctx.breakdown.panel_reward_total_xor, 2)),
    );
    let json = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to render JSON settlement: {err}"))?;
    println!("{json}");
    Ok(())
}

fn print_appeal_disbursement_table(ctx: &AppealDisbursementInputs<'_>) {
    println!("Appeal disbursement ({})", ctx.config.version());
    println!("  outcome: {}", ctx.plan.verdict);
    println!("  deposit: {} XOR", format_decimal(ctx.plan.deposit_xor, 2));
    println!(
        "  refund -> {}: {} XOR",
        ctx.plan.refund_account,
        format_decimal(ctx.plan.settlement.refund_xor, 2)
    );
    println!(
        "  treasury -> {}: {} XOR (deposit) + {} XOR (forfeited rewards) = {} XOR",
        ctx.plan.treasury_account,
        format_decimal(ctx.plan.settlement.treasury_xor, 2),
        format_decimal(ctx.plan.rewards_forfeited_treasury_xor, 2),
        format_decimal(ctx.plan.total_treasury_xor, 2)
    );
    println!(
        "  held in escrow -> {}: {} XOR",
        ctx.plan.escrow_account,
        format_decimal(ctx.plan.settlement.held_xor, 2)
    );
    println!(
        "  attendance: {}/{} jurors paid",
        ctx.plan.attending_count(),
        ctx.plan.panel_size
    );
    println!(
        "  panel rewards: {} XOR available; {} XOR paid; {} XOR forfeited to treasury",
        format_decimal(ctx.plan.rewards_available_xor, 2),
        format_decimal(ctx.plan.rewards_paid_total_xor, 2),
        format_decimal(ctx.plan.rewards_forfeited_treasury_xor, 2)
    );
    for payout in &ctx.plan.juror_payouts {
        println!(
            "    - {}: stipend {} XOR + bonus {} XOR = {} XOR",
            payout.juror,
            format_decimal(payout.stipend_xor, 2),
            format_decimal(payout.bonus_xor, 2),
            format_decimal(payout.total(), 2)
        );
    }
    if !ctx.plan.no_show_accounts.is_empty() {
        println!("  no-shows (forfeited rewards):");
        for account in &ctx.plan.no_show_accounts {
            println!("    - {}", account);
        }
    }
}

fn print_appeal_disbursement_json(ctx: &AppealDisbursementInputs<'_>) -> Result<(), String> {
    let mut root = Map::new();
    root.insert(
        "version".into(),
        Value::String(ctx.config.version().to_string()),
    );
    root.insert(
        "outcome".into(),
        Value::String(ctx.plan.verdict.to_string()),
    );
    root.insert(
        "deposit_xor".into(),
        Value::String(format_decimal(ctx.plan.deposit_xor, 2)),
    );
    root.insert(
        "panel_size".into(),
        Value::Number(Number::from(u64::from(ctx.plan.panel_size))),
    );

    let mut refund = Map::new();
    refund.insert(
        "account".into(),
        Value::String(ctx.plan.refund_account.to_string()),
    );
    refund.insert(
        "amount_xor".into(),
        Value::String(format_decimal(ctx.plan.settlement.refund_xor, 2)),
    );
    root.insert("refund".into(), Value::Object(refund));

    let mut treasury = Map::new();
    treasury.insert(
        "account".into(),
        Value::String(ctx.plan.treasury_account.to_string()),
    );
    treasury.insert(
        "deposit_component_xor".into(),
        Value::String(format_decimal(ctx.plan.settlement.treasury_xor, 2)),
    );
    treasury.insert(
        "forfeited_rewards_xor".into(),
        Value::String(format_decimal(ctx.plan.rewards_forfeited_treasury_xor, 2)),
    );
    treasury.insert(
        "total_xor".into(),
        Value::String(format_decimal(ctx.plan.total_treasury_xor, 2)),
    );
    root.insert("treasury".into(), Value::Object(treasury));

    let mut held = Map::new();
    held.insert(
        "account".into(),
        Value::String(ctx.plan.escrow_account.to_string()),
    );
    held.insert(
        "amount_xor".into(),
        Value::String(format_decimal(ctx.plan.settlement.held_xor, 2)),
    );
    root.insert("held".into(), Value::Object(held));

    let mut rewards = Map::new();
    rewards.insert(
        "available_xor".into(),
        Value::String(format_decimal(ctx.plan.rewards_available_xor, 2)),
    );
    rewards.insert(
        "paid_xor".into(),
        Value::String(format_decimal(ctx.plan.rewards_paid_total_xor, 2)),
    );
    rewards.insert(
        "forfeited_xor".into(),
        Value::String(format_decimal(ctx.plan.rewards_forfeited_treasury_xor, 2)),
    );
    rewards.insert(
        "attending".into(),
        Value::Number(Number::from(ctx.plan.attending_count() as u64)),
    );
    rewards.insert(
        "no_shows".into(),
        Value::Array(
            ctx.plan
                .no_show_accounts
                .iter()
                .map(|acct| Value::String(acct.to_string()))
                .collect(),
        ),
    );
    let participants: Vec<Value> = ctx
        .plan
        .juror_payouts
        .iter()
        .map(|payout| {
            let mut entry = Map::new();
            entry.insert("account".into(), Value::String(payout.juror.to_string()));
            entry.insert(
                "stipend_xor".into(),
                Value::String(format_decimal(payout.stipend_xor, 2)),
            );
            entry.insert(
                "bonus_xor".into(),
                Value::String(format_decimal(payout.bonus_xor, 2)),
            );
            entry.insert(
                "total_xor".into(),
                Value::String(format_decimal(payout.total(), 2)),
            );
            Value::Object(entry)
        })
        .collect();
    rewards.insert("participants".into(), Value::Array(participants));
    root.insert("rewards".into(), Value::Object(rewards));

    let json = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to render JSON disbursement: {err}"))?;
    println!("{json}");
    Ok(())
}

fn format_decimal(value: Decimal, places: u32) -> String {
    value.round_dp(places).to_string()
}

#[cfg(test)]
mod manifest_tests {
    use ed25519_dalek::SigningKey;
    use iroha_crypto::{Algorithm, PublicKey};
    use iroha_data_model::account::AccountId;
    use norito::json::{Map, Value};
    use sorafs_orchestrator::proxy::LocalQuicProxyConfig;
    use tempfile::TempDir;

    use super::*;

    fn account_string(label: u8) -> String {
        let seed = [label; ed25519_dalek::SECRET_KEY_LENGTH];
        let signer = SigningKey::from_bytes(&seed);
        let pk_bytes = signer.verifying_key().to_bytes();
        let pk =
            PublicKey::from_bytes(Algorithm::Ed25519, pk_bytes.as_slice()).expect("public key");
        AccountId::new(pk).to_string()
    }

    #[test]
    fn proxy_set_mode_updates_config_file() {
        let temp = TempDir::new().expect("tempdir");
        let config_path = temp.path().join("orchestrator.json");
        let json_out_path = temp.path().join("summary.json");

        let config = OrchestratorConfig {
            local_proxy: Some(LocalQuicProxyConfig {
                bind_addr: "127.0.0.1:0".into(),
                telemetry_label: Some("test-proxy".into()),
                proxy_mode: ProxyMode::Bridge,
                ..LocalQuicProxyConfig::default()
            }),
            ..OrchestratorConfig::default()
        };
        let config_value = orchestrator_config_to_json(&config);
        let config_json = norito::json::to_json_pretty(&config_value).expect("render config json");
        fs::write(&config_path, config_json).expect("write config");

        proxy_set_mode(vec![
            format!("--orchestrator-config={}", config_path.display()),
            "--mode=metadata-only".into(),
            format!("--json-out={}", json_out_path.display()),
        ])
        .expect("proxy set mode succeeds");

        let updated_json = fs::read_to_string(&config_path).expect("read updated config file");
        let updated_value: Value =
            norito::json::from_str(&updated_json).expect("parse updated config");
        let updated_config =
            orchestrator_config_from_json(&updated_value).expect("decode updated config");
        assert_eq!(
            updated_config
                .local_proxy
                .as_ref()
                .expect("local proxy")
                .proxy_mode,
            ProxyMode::MetadataOnly
        );

        let summary_json = fs::read_to_string(&json_out_path).expect("read summary json");
        let summary_value: Value =
            norito::json::from_str(&summary_json).expect("parse summary json");
        let summary_map = summary_value.as_object().expect("summary to be an object");
        assert_eq!(
            summary_map.get("mode_effective").and_then(Value::as_str),
            Some("metadata-only")
        );
        assert_eq!(
            summary_map.get("mode_previous").and_then(Value::as_str),
            Some("bridge")
        );
    }

    #[test]
    fn moderation_validate_corpus_accepts_valid_manifest() {
        use iroha_data_model::sorafs::moderation::{
            ADVERSARIAL_CORPUS_VERSION_V1, AdversarialCorpusManifestV1,
            AdversarialPerceptualFamilyV1, AdversarialPerceptualVariantV1,
        };

        let manifest = AdversarialCorpusManifestV1 {
            schema_version: ADVERSARIAL_CORPUS_VERSION_V1,
            issued_at_unix: 1_740_000_000,
            cohort_label: Some("test-cohort".to_string()),
            families: vec![AdversarialPerceptualFamilyV1 {
                family_id: [0x11; 16],
                description: "jpeg jitter corpus".to_string(),
                variants: vec![AdversarialPerceptualVariantV1 {
                    variant_id: [0x22; 16],
                    attack_vector: "jpeg_jitter".to_string(),
                    reference_cid_b64: None,
                    perceptual_hash: Some([0x33; 32]),
                    hamming_radius: 8,
                    embedding_digest: None,
                    notes: Some("sample variant".to_string()),
                }],
            }],
        };

        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("corpus.json");
        let manifest_json = norito::json::to_json_pretty(&manifest).expect("render manifest json");
        fs::write(&manifest_path, manifest_json).expect("write manifest json");

        moderation_validate_corpus(vec![format!("--manifest={}", manifest_path.display())])
            .expect("corpus manifest validated");
    }

    #[test]
    fn taikai_cache_override_accepts_raw_object() {
        let value = sample_taikai_cache_value();
        let parsed = parse_taikai_cache_override(value).expect("parse succeeds");
        let config = parsed.expect("cache config present");
        assert_eq!(config.hot_capacity_bytes, 8_388_608);
        assert_eq!(config.qos.priority_rate_bps, 83_886_080);
    }

    #[test]
    fn taikai_cache_override_accepts_wrapped_object() {
        let raw = sample_taikai_cache_value();
        let mut map = Map::new();
        map.insert("taikai_cache".into(), raw);
        let parsed = parse_taikai_cache_override(Value::Object(map)).expect("wrapped cache parses");
        assert!(parsed.is_some());
    }

    #[test]
    fn taikai_cache_override_allows_null() {
        let parsed = parse_taikai_cache_override(Value::Null).expect("null parses");
        assert!(parsed.is_none());
    }

    #[test]
    fn taikai_cache_override_rejects_invalid_payload() {
        let invalid = norito::json::from_str(r#"{"hot_capacity_bytes": 1}"#).expect("parse");
        let err = parse_taikai_cache_override(invalid).expect_err("invalid config rejected");
        assert!(
            err.contains("failed to parse Taikai cache config"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn appeal_disburse_requires_juror_list() {
        let refund_account = account_string(10);
        let treasury_account = account_string(11);
        let escrow_account = account_string(12);
        let err = appeal_disburse(vec![
            "--deposit=100".into(),
            "--outcome=overturn".into(),
            format!("--refund-account={refund_account}"),
            format!("--treasury-account={treasury_account}"),
            format!("--escrow-account={escrow_account}"),
        ])
        .expect_err("juror roster required");
        assert!(
            err.contains("missing required `--juror`"),
            "unexpected error: {err}"
        );
    }

    fn sample_taikai_cache_value() -> Value {
        norito::json::from_str(
            r#"{
                "hot_capacity_bytes": 8388608,
                "hot_retention_secs": 45,
                "warm_capacity_bytes": 33554432,
                "warm_retention_secs": 180,
                "cold_capacity_bytes": 268435456,
                "cold_retention_secs": 3600,
                "qos": {
                    "priority_rate_bps": 83886080,
                    "standard_rate_bps": 41943040,
                    "bulk_rate_bps": 12582912,
                    "burst_multiplier": 4
                }
            }"#,
        )
        .expect("sample Taikai cache JSON parses")
    }
}

enum InputSummary {
    File { path: PathBuf, bytes: u64 },
    Directory { path: PathBuf, file_count: u64 },
}

#[derive(Clone, Debug)]
struct GatewayProviderSpec {
    name: String,
    provider_id_hex: String,
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
}

struct PlanWithHandle {
    plan: CarBuildPlan,
    chunker_handle: String,
}

fn manifest_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut summary_source: Option<JsonSource> = None;
    let mut manifest_out: Option<PathBuf> = None;
    let mut manifest_json_out: Option<PathBuf> = None;
    let mut pin_min_replicas: Option<u16> = None;
    let mut pin_storage_class: Option<StorageClass> = None;
    let mut pin_retention_epoch: Option<u64> = None;
    let mut metadata_entries: Vec<(String, String)> = Vec::new();

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--summary" => summary_source = Some(JsonSource::from_arg(value)?),
            "--manifest-out" => manifest_out = Some(PathBuf::from(value)),
            "--manifest-json-out" => manifest_json_out = Some(PathBuf::from(value)),
            "--pin-min-replicas" => {
                let replicas: u16 = value
                    .parse()
                    .map_err(|err| format!("invalid --pin-min-replicas value: {err}"))?;
                pin_min_replicas = Some(replicas);
            }
            "--pin-storage-class" => pin_storage_class = Some(parse_storage_class(value)?),
            "--pin-retention-epoch" => {
                let epoch: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid --pin-retention-epoch value: {err}"))?;
                pin_retention_epoch = Some(epoch);
            }
            "--metadata" => {
                let (k, v) = value
                    .split_once('=')
                    .ok_or_else(|| "--metadata expects key=value".to_string())?;
                metadata_entries.push((k.to_string(), v.to_string()));
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli manifest build`"
                ));
            }
        }
    }

    let summary_source = summary_source.ok_or_else(|| {
        "missing required `--summary=PATH` for `sorafs_cli manifest build`".to_string()
    })?;
    let manifest_out = manifest_out.ok_or_else(|| {
        "missing required `--manifest-out=PATH` for `sorafs_cli manifest build`".to_string()
    })?;

    let summary_json = summary_source.read()?;
    let summary_obj = summary_json
        .as_object()
        .ok_or_else(|| "summary must be a JSON object".to_string())?;

    let chunker_handle = summary_obj
        .get("chunker_handle")
        .and_then(Value::as_str)
        .ok_or_else(|| "summary missing `chunker_handle`".to_string())?;
    let descriptor =
        manifest_chunker_registry::lookup_by_handle(chunker_handle).ok_or_else(|| {
            format!(
                "summary references unknown chunker handle `{chunker_handle}`; refresh the registry"
            )
        })?;

    let content_length = summary_obj
        .get("payload_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| "summary missing `payload_bytes`".to_string())?;
    let car_size = summary_obj
        .get("car_size")
        .and_then(Value::as_u64)
        .ok_or_else(|| "summary missing `car_size`".to_string())?;
    let car_digest_hex = summary_obj
        .get("car_digest_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| "summary missing `car_digest_hex`".to_string())?;
    let car_digest = parse_digest_hex(car_digest_hex)
        .map_err(|err| format!("invalid `car_digest_hex` in summary: {err}"))?;

    let root_cids = summary_obj
        .get("root_cids_hex")
        .and_then(Value::as_array)
        .ok_or_else(|| "summary missing `root_cids_hex` array".to_string())?;
    let first_root_hex = root_cids
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| "summary `root_cids_hex` array is empty".to_string())?;
    let root_cid = parse_hex_vec(first_root_hex)
        .map_err(|err| format!("invalid root CID hex `{first_root_hex}`: {err}"))?;

    let chunking_profile = ChunkingProfileV1::from_descriptor(descriptor);
    let pin_policy = PinPolicy {
        min_replicas: pin_min_replicas.unwrap_or(1),
        storage_class: pin_storage_class.unwrap_or_default(),
        retention_epoch: pin_retention_epoch.unwrap_or(0),
    };

    let mut builder = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_profile(chunking_profile)
        .content_length(content_length)
        .car_digest(car_digest)
        .car_size(car_size)
        .pin_policy(pin_policy);

    if !metadata_entries.is_empty() {
        builder = builder.extend_metadata(metadata_entries.clone());
    }

    let manifest = builder.build().map_err(format_manifest_error)?;
    let manifest_bytes = manifest
        .encode()
        .map_err(|err| format!("failed to encode manifest: {err}"))?;
    ensure_parent_dir(&manifest_out)?;
    fs::write(&manifest_out, &manifest_bytes)
        .map_err(|err| format!("failed to write `{}`: {err}", manifest_out.display()))?;

    if let Some(json_path) = manifest_json_out.as_ref() {
        ensure_parent_dir(json_path)?;
        let rendered = to_string_pretty(
            &norito::json::to_value(&manifest)
                .map_err(|err| format!("failed to serialise manifest JSON: {err}"))?,
        )
        .map_err(|err| format!("failed to render manifest JSON: {err}"))?;
        write_text(json_path, rendered.as_bytes())?;
    }

    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;

    let mut summary = Map::new();
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_out.display().to_string()),
    );
    summary.insert(
        "manifest_digest_hex".into(),
        Value::from(hex_encode(manifest_digest.as_bytes())),
    );
    summary.insert("chunker_handle".into(), Value::from(chunker_handle));
    summary.insert(
        "chunker_profile_id".into(),
        Value::from(descriptor.id.0 as u64),
    );
    summary.insert(
        "pin_policy".into(),
        Value::Object(pin_policy_json(&pin_policy)),
    );
    if let Some(json_path) = manifest_json_out {
        summary.insert(
            "manifest_json_path".into(),
            Value::from(json_path.display().to_string()),
        );
    }
    if !metadata_entries.is_empty() {
        summary.insert(
            "metadata_kv".into(),
            Value::Array(
                metadata_entries
                    .into_iter()
                    .map(|(k, v)| {
                        let mut kv = Map::new();
                        kv.insert("key".into(), Value::from(k));
                        kv.insert("value".into(), Value::from(v));
                        Value::Object(kv)
                    })
                    .collect(),
            ),
        );
    }

    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render manifest summary: {err}"))?;
    println!("{rendered}");
    Ok(())
}

fn norito_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut source_spec: Option<String> = None;
    let mut bytecode_out: Option<PathBuf> = None;
    let mut abi_version: Option<u8> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--source" => source_spec = Some(value.to_string()),
            "--bytecode-out" => bytecode_out = Some(PathBuf::from(value)),
            "--abi-version" => {
                let parsed: u8 = value
                    .parse()
                    .map_err(|err| format!("invalid `--abi-version` value: {err}"))?;
                abi_version = Some(parsed);
            }
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli norito build`"
                ));
            }
        }
    }

    let source_spec = source_spec.ok_or_else(|| {
        "missing required `--source=PATH` for `sorafs_cli norito build`".to_string()
    })?;
    let bytecode_out = bytecode_out.ok_or_else(|| {
        "missing required `--bytecode-out=PATH` for `sorafs_cli norito build`".to_string()
    })?;
    let abi_version = abi_version.unwrap_or(1);
    if abi_version != 1 {
        return Err(format!("unsupported abi_version {abi_version}; expected 1"));
    }

    let (source_text, source_path) = if source_spec == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|err| format!("failed to read Kotodama source from stdin: {err}"))?;
        (buf, None)
    } else {
        let path = PathBuf::from(&source_spec);
        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read Kotodama source `{}`: {err}", path.display()))?;
        (contents, Some(path))
    };

    let compiler = Compiler::new().with_abi_version(abi_version);
    let bytecode = compiler
        .compile_source(&source_text)
        .map_err(|err| format!("failed to compile Kotodama source: {err}"))?;

    ensure_parent_dir(&bytecode_out)?;
    fs::write(&bytecode_out, &bytecode)
        .map_err(|err| format!("failed to write `{}`: {err}", bytecode_out.display()))?;

    let mut summary = Map::new();
    summary.insert(
        "bytecode_path".into(),
        Value::from(bytecode_out.display().to_string()),
    );
    summary.insert("bytecode_len".into(), Value::from(bytecode.len() as u64));
    summary.insert(
        "bytecode_blake3_hex".into(),
        Value::from(hex_encode(blake3_hash(&bytecode).as_bytes())),
    );
    summary.insert("abi_version".into(), Value::from(abi_version as u64));
    match &source_path {
        Some(path) => {
            summary.insert("source_kind".into(), Value::from("file"));
            summary.insert(
                "source_path".into(),
                Value::from(path.display().to_string()),
            );
        }
        None => {
            summary.insert("source_kind".into(), Value::from("stdin"));
        }
    }

    let summary_value = Value::Object(summary);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}

fn manifest_submit(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut chunk_digest_hex_arg: Option<String> = None;
    let mut torii_url: Option<String> = None;
    let mut submitted_epoch: Option<u64> = None;
    let mut resolve_submitted_epoch = false;
    let mut authority_str: Option<String> = None;
    let mut authority_network_prefix: Option<u16> = None;
    let mut private_key_inline: Option<String> = None;
    let mut private_key_path: Option<PathBuf> = None;
    let mut gas_asset_id: Option<String> = None;
    let mut alias_namespace: Option<String> = None;
    let mut alias_name: Option<String> = None;
    let mut alias_proof_path: Option<PathBuf> = None;
    let mut successor_hex: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut response_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--chunk-digest-sha3" => chunk_digest_hex_arg = Some(value.to_string()),
            "--torii-url" => torii_url = Some(value.to_string()),
            "--submitted-epoch" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--submitted-epoch` value: {err}"))?;
                submitted_epoch = Some(parsed);
            }
            "--resolve-submitted-epoch" => {
                resolve_submitted_epoch = parse_bool_flag(value, "--resolve-submitted-epoch")?;
            }
            "--authority" => authority_str = Some(value.to_string()),
            "--network-prefix" => {
                authority_network_prefix = Some(parse_u16_arg(
                    "--network-prefix",
                    value,
                    "sorafs_cli manifest submit",
                )?);
            }
            "--private-key" => {
                if private_key_path.is_some() {
                    return Err(
                        "`--private-key` and `--private-key-file` are mutually exclusive"
                            .to_string(),
                    );
                }
                private_key_inline = Some(value.to_string());
            }
            "--private-key-file" => {
                if private_key_inline.is_some() {
                    return Err(
                        "`--private-key` and `--private-key-file` are mutually exclusive"
                            .to_string(),
                    );
                }
                private_key_path = Some(PathBuf::from(value));
            }
            "--gas-asset-id" => gas_asset_id = Some(value.to_string()),
            "--alias-namespace" => alias_namespace = Some(value.to_string()),
            "--alias-name" => alias_name = Some(value.to_string()),
            "--alias-proof" => alias_proof_path = Some(PathBuf::from(value)),
            "--successor-of" => successor_hex = Some(value.to_string()),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--response-out" => response_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli manifest submit`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli manifest submit`".to_string()
    })?;
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli manifest submit`".to_string()
    })?;
    if submitted_epoch.is_some() && resolve_submitted_epoch {
        return Err(
            "`--submitted-epoch` and `--resolve-submitted-epoch` are mutually exclusive"
                .to_string(),
        );
    }
    let authority_str = authority_str.ok_or_else(|| {
        "missing required `--authority=ACCOUNT_ID` for `sorafs_cli manifest submit`".to_string()
    })?;
    authority_network_prefix =
        authority_network_prefix.or_else(|| infer_i105_network_prefix(&authority_str));

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());

    let torii_base_url =
        Url::parse(&torii_url).map_err(|err| format!("invalid `--torii-url` value: {err}"))?;

    let plan_specs = if let Some(source) = chunk_plan_source {
        let value = source.read()?;
        Some(
            chunk_fetch_specs_from_json(&value)
                .map_err(|err| format!("failed to parse chunk plan JSON: {err}"))?,
        )
    } else {
        None
    };

    let plan_chunk_count = plan_specs.as_ref().map(|specs| specs.len() as u64);
    let plan_digest = plan_specs
        .as_ref()
        .map(|specs| chunk_digest_sha3_from_specs(specs));
    let chunk_digest_arg_supplied = chunk_digest_hex_arg.is_some();

    let chunk_digest = match (chunk_digest_hex_arg, plan_digest) {
        (Some(hex), _) => {
            parse_digest_hex(&hex).map_err(|err| format!("invalid `--chunk-digest-sha3`: {err}"))?
        }
        (None, Some(expected)) => expected,
        (None, None) => {
            return Err(
                "must provide either `--chunk-plan` or `--chunk-digest-sha3` for `sorafs_cli manifest submit`"
                    .to_string(),
            )
        }
    };

    if chunk_digest_arg_supplied && manifest.car_digest != chunk_digest {
        let expected_hex = hex_encode(manifest.car_digest);
        let provided_hex = hex_encode(chunk_digest);
        return Err(format!(
            "chunk digest `{provided_hex}` does not match manifest CAR digest `{expected_hex}`; regenerate the manifest or chunk plan so they originate from the same CAR build"
        ));
    }
    let manifest_car_digest_hex = hex_encode(manifest.car_digest);
    let authority = parse_account_id_arg_with_prefix(
        "--authority",
        &authority_str,
        "sorafs_cli manifest submit",
        authority_network_prefix,
    )
    .map_err(|err| {
        err.strip_prefix(
            "failed to parse `--authority` for `sorafs_cli manifest submit` as account id: ",
        )
        .map_or_else(
            || format!("invalid authority: {err}"),
            |reason| format!("invalid authority: {reason}"),
        )
    })?;
    let authority_literal = authority_payload_literal(&authority, authority_network_prefix)?;
    let private_key = match (private_key_inline, private_key_path) {
        (Some(inline), None) => parse_private_key_inline(&inline)?,
        (None, Some(path)) => load_private_key_from_file(&path)?,
        (Some(_), Some(_)) => {
            return Err(
                "`--private-key` and `--private-key-file` are mutually exclusive".to_string(),
            );
        }
        (None, None) => {
            return Err(
                "missing private key: supply `--private-key` or `--private-key-file`".to_string(),
            );
        }
    };

    let alias_inputs = alias_inputs_from_flags(alias_namespace, alias_name, alias_proof_path)?;

    let successor_digest = match successor_hex.as_ref() {
        Some(hex) => Some(
            parse_digest_hex(hex)
                .map_err(|err| format!("invalid `--successor-of` value: {err}"))?,
        ),
        None => None,
    };

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let submitted_epoch = match submitted_epoch {
        Some(epoch) => epoch,
        None if resolve_submitted_epoch => {
            resolve_submitted_epoch_from_status(&client, &torii_base_url)?
        }
        None => {
            return Err(
                "missing required `--submitted-epoch=EPOCH` for `sorafs_cli manifest submit`"
                    .to_string(),
            );
        }
    };

    let submission = submit_pin_register_with_fallback(
        &ManifestSubmitRequest {
            client: &client,
            torii_base_url: &torii_base_url,
            authority: &authority,
            private_key: &private_key,
            gas_asset_id: gas_asset_id.as_deref(),
            alias_inputs: alias_inputs.as_ref(),
            api_version_hint: None,
        },
        &authority_literal,
        &manifest,
        chunk_digest,
        submitted_epoch,
        successor_digest,
    )?;

    if let Some(path) = response_out {
        ensure_parent_dir(&path)?;
        fs::write(&path, &submission.response_bytes)
            .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    }

    let mut summary = Map::new();
    summary.insert("torii_url".into(), Value::from(torii_url));
    summary.insert(
        "torii_endpoint".into(),
        Value::from(submission.endpoint_used.clone()),
    );
    summary.insert(
        "torii_endpoint_requested".into(),
        Value::from(submission.endpoint_requested),
    );
    summary.insert(
        "status".into(),
        Value::from(submission.status.as_u16() as u64),
    );
    summary.insert("submitted_epoch".into(), Value::from(submitted_epoch));
    summary.insert("authority".into(), Value::from(authority_literal));
    summary.insert(
        "submission_mode".into(),
        Value::from(submission.submission_mode),
    );
    if let Some(asset_id) = gas_asset_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        summary.insert("gas_asset_id".into(), Value::from(asset_id));
    }
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest_hex.clone()),
    );
    summary.insert(
        "manifest_car_digest_hex".into(),
        Value::from(manifest_car_digest_hex.clone()),
    );
    summary.insert(
        "chunk_digest_sha3_hex".into(),
        Value::from(hex_encode(chunk_digest)),
    );
    summary.insert(
        "chunker_handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )),
    );
    summary.insert(
        "pin_policy".into(),
        Value::Object(pin_policy_json(&manifest.pin_policy)),
    );
    if let Some(label) = chunk_plan_label {
        summary.insert("chunk_plan".into(), Value::from(label));
    }
    if let Some(count) = plan_chunk_count {
        summary.insert("chunk_plan_chunk_count".into(), Value::from(count));
    }
    if let Some(alias) = alias_inputs {
        summary.insert("alias_namespace".into(), Value::from(alias.namespace));
        summary.insert("alias_name".into(), Value::from(alias.name));
    }
    if let Some(hex) = successor_hex.as_ref() {
        summary.insert("successor_of_hex".into(), Value::from(hex.clone()));
    }
    if let Some(reason) = submission.fallback_reason {
        summary.insert("fallback_reason".into(), Value::from(reason));
    }
    if let Some(chain_id) = submission.chain_id_hint {
        summary.insert("chain_id".into(), Value::from(chain_id));
    }
    summary.insert("torii_response".into(), submission.response_value);

    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }
    if let Some(message) = submission.failure_message {
        return Err(message);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StorageFileEntryOwned {
    path: Vec<String>,
    size: u64,
}

fn storage_prepare(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut payload_path: Option<PathBuf> = None;
    let mut payload_out: Option<PathBuf> = None;
    let mut files_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--payload-out" => payload_out = Some(PathBuf::from(value)),
            "--files-out" => files_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli storage prepare`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let payload_path = payload_path.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let payload_out = payload_out.ok_or_else(|| {
        "missing required `--payload-out=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let files_out = files_out.ok_or_else(|| {
        "missing required `--files-out=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    let manifest_id_hex = manifest_root_cid_hex(&manifest)?;

    let (payload_bytes, files, payload_kind) = load_storage_pin_payload(&payload_path, &manifest)?;
    let payload_bytes_len = u64::try_from(payload_bytes.len())
        .map_err(|_| "payload exceeds host limits".to_string())?;
    let payload_file_count = files.as_ref().map_or(0_u64, |entries| {
        u64::try_from(entries.len()).unwrap_or(u64::MAX)
    });

    ensure_parent_dir(&payload_out)?;
    fs::write(&payload_out, &payload_bytes)
        .map_err(|err| format!("failed to write `{}`: {err}", payload_out.display()))?;

    let files_value = storage_files_to_json_value(files.as_deref());
    let files_rendered = to_string_pretty(&files_value)
        .map_err(|err| format!("failed to render storage files JSON: {err}"))?;
    ensure_parent_dir(&files_out)?;
    write_text(&files_out, files_rendered.as_bytes())?;

    let mut summary = Map::new();
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary.insert(
        "payload_path".into(),
        Value::from(payload_path.display().to_string()),
    );
    summary.insert(
        "payload_out".into(),
        Value::from(payload_out.display().to_string()),
    );
    summary.insert(
        "files_out".into(),
        Value::from(files_out.display().to_string()),
    );
    summary.insert("payload_kind".into(), Value::from(payload_kind));
    summary.insert("payload_bytes".into(), Value::from(payload_bytes_len));
    summary.insert("payload_file_count".into(), Value::from(payload_file_count));
    summary.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest_hex),
    );
    summary.insert("manifest_id_hex".into(), Value::from(manifest_id_hex));
    summary.insert(
        "chunker_handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )),
    );

    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }

    Ok(())
}

fn storage_pin(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut payload_path: Option<PathBuf> = None;
    let mut torii_url: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut response_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--torii-url" => torii_url = Some(value.to_string()),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--response-out" => response_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli storage pin`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli storage pin`".to_string()
    })?;
    let payload_path = payload_path.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli storage pin`".to_string()
    })?;
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli storage pin`".to_string()
    })?;

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    let manifest_id_hex = manifest_root_cid_hex(&manifest)?;

    let (payload_bytes, files, payload_kind) = load_storage_pin_payload(&payload_path, &manifest)?;
    let payload_bytes_len = u64::try_from(payload_bytes.len())
        .map_err(|_| "payload exceeds host limits".to_string())?;
    let payload_file_count = files.as_ref().map_or(0_u64, |entries| {
        u64::try_from(entries.len()).unwrap_or(u64::MAX)
    });

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = submit_storage_pin_request(
        &client,
        &torii_url,
        &manifest_bytes,
        &payload_bytes,
        files.as_deref(),
    )?;

    if let Some(path) = response_out {
        ensure_parent_dir(&path)?;
        fs::write(&path, &response.response_bytes)
            .map_err(|err| format!("failed to write `{}`: {err}", path.display()))?;
    }

    let response_text = String::from_utf8_lossy(&response.response_bytes);
    if !response.success() {
        return Err(storage_pin_failure_message(
            response.status,
            response_text.as_ref(),
        ));
    }

    let mut summary = Map::new();
    summary.insert("torii_url".into(), Value::from(torii_url));
    summary.insert("torii_endpoint".into(), Value::from(response.endpoint));
    summary.insert(
        "status".into(),
        Value::from(response.status.as_u16() as u64),
    );
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary.insert(
        "payload_path".into(),
        Value::from(payload_path.display().to_string()),
    );
    summary.insert("payload_kind".into(), Value::from(payload_kind));
    summary.insert("payload_bytes".into(), Value::from(payload_bytes_len));
    summary.insert("payload_file_count".into(), Value::from(payload_file_count));
    summary.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest_hex),
    );
    summary.insert("manifest_id_hex".into(), Value::from(manifest_id_hex));
    summary.insert(
        "chunker_handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )),
    );
    summary.insert(
        "already_stored".into(),
        Value::from(response.already_stored),
    );
    summary.insert("torii_response".into(), response.response_value);

    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }

    Ok(())
}

fn manifest_root_cid_hex(manifest: &ManifestV1) -> Result<String, String> {
    if manifest.root_cid.is_empty() {
        return Err("manifest root_cid is empty".to_string());
    }

    Ok(hex_encode(&manifest.root_cid))
}

fn chunk_profile_from_manifest(manifest: &ManifestV1) -> Result<ChunkProfile, String> {
    Ok(ChunkProfile {
        min_size: usize::try_from(manifest.chunking.min_size)
            .map_err(|_| "manifest chunking.min_size exceeds host limits".to_string())?,
        target_size: usize::try_from(manifest.chunking.target_size)
            .map_err(|_| "manifest chunking.target_size exceeds host limits".to_string())?,
        max_size: usize::try_from(manifest.chunking.max_size)
            .map_err(|_| "manifest chunking.max_size exceeds host limits".to_string())?,
        break_mask: u64::from(manifest.chunking.break_mask),
    })
}

type StoragePinPayload = (Vec<u8>, Option<Vec<StorageFileEntryOwned>>, &'static str);

fn load_storage_pin_payload(
    input: &Path,
    manifest: &ManifestV1,
) -> Result<StoragePinPayload, String> {
    let metadata = fs::metadata(input)
        .map_err(|err| format!("failed to access payload `{}`: {err}", input.display()))?;

    if metadata.is_dir() {
        let profile = chunk_profile_from_manifest(manifest)?;
        let (plan, payload) = CarBuildPlan::from_directory_with_profile(input, profile)
            .map_err(|err| format!("failed to build directory payload plan: {err}"))?;
        let files = plan
            .files
            .iter()
            .map(|file| StorageFileEntryOwned {
                path: file.path.clone(),
                size: file.size,
            })
            .collect();
        return Ok((payload, Some(files), "directory"));
    }

    if metadata.is_file() {
        let payload = fs::read(input)
            .map_err(|err| format!("failed to read payload `{}`: {err}", input.display()))?;
        return Ok((payload, None, "file"));
    }

    Err("payload input must be a file or directory".to_string())
}

fn storage_files_to_json_value(files: Option<&[StorageFileEntryOwned]>) -> Value {
    match files {
        Some(entries) => Value::Array(
            entries
                .iter()
                .map(|entry| {
                    Value::Object(Map::from_iter([
                        (
                            "path".into(),
                            Value::Array(entry.path.iter().cloned().map(Value::from).collect()),
                        ),
                        ("size".into(), Value::from(entry.size)),
                    ]))
                })
                .collect(),
        ),
        None => Value::Null,
    }
}

fn should_fallback_manifest_submit_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED
    )
}

struct ManifestSubmitFallback {
    status: StatusCode,
    endpoint: String,
    response_bytes: Vec<u8>,
    response_value: Value,
    chain_id: String,
    failure_message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransactionConfirmationKind {
    Queued,
    Approved,
    Committed,
    Applied,
    Rejected,
    Expired,
}

struct TransactionConfirmation {
    kind: TransactionConfirmationKind,
    block_height: Option<u64>,
    pipeline_response: Value,
    rejection_message: Option<String>,
}

struct ManifestSubmitRequest<'a> {
    client: &'a HttpClient,
    torii_base_url: &'a Url,
    authority: &'a AccountId,
    private_key: &'a PrivateKey,
    gas_asset_id: Option<&'a str>,
    alias_inputs: Option<&'a AliasInputs>,
    api_version_hint: Option<&'a str>,
}

fn sign_manifest_submit_fallback_transaction(
    builder: iroha_data_model::transaction::signed::TransactionBuilder,
    private_key: &PrivateKey,
) -> Result<iroha_data_model::transaction::signed::SignedTransaction, String> {
    builder
        .try_sign(private_key)
        .map_err(|err| format!("failed to sign fallback manifest transaction: {err}"))
}

fn submit_manifest_via_transaction_endpoint(
    request: &ManifestSubmitRequest<'_>,
    manifest: &ManifestV1,
    chunk_digest: [u8; 32],
    submitted_epoch: u64,
    successor_digest: Option<[u8; 32]>,
) -> Result<ManifestSubmitFallback, String> {
    use iroha_data_model::{
        ChainId,
        metadata::Metadata,
        prelude::{InstructionBox, Json, TransactionBuilder},
        sorafs::pin_registry::{ManifestAliasBinding, ManifestDigest},
    };

    let chain_id = resolve_chain_id_from_sorafs_registry(request.client, request.torii_base_url)?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let chunker = chunker_handle_from_profile(&manifest.chunking);
    let policy = convert_pin_policy(&manifest.pin_policy);
    let alias = request.alias_inputs.map(|inputs| ManifestAliasBinding {
        namespace: inputs.namespace.clone(),
        name: inputs.name.clone(),
        proof: inputs.proof.clone(),
    });
    let successor_of = successor_digest.map(ManifestDigest::new);
    let instruction = iroha_data_model::isi::sorafs::RegisterPinManifest {
        digest: ManifestDigest::new(*manifest_digest.as_bytes()),
        chunker,
        chunk_digest_sha3_256: chunk_digest,
        content_length: manifest.content_length,
        policy,
        submitted_epoch,
        alias,
        successor_of,
    };
    let mut tx_metadata = Metadata::default();
    if let Some(asset_id) = request
        .gas_asset_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let gas_asset_key =
            Name::from_str("gas_asset_id").expect("static metadata key `gas_asset_id`");
        tx_metadata.insert(gas_asset_key, Json::new(asset_id.to_owned()));
    }
    let transaction = sign_manifest_submit_fallback_transaction(
        TransactionBuilder::new(ChainId::from(chain_id.clone()), request.authority.clone())
            .with_instructions([InstructionBox::from(instruction)])
            .with_metadata(tx_metadata),
        request.private_key,
    )?;
    let tx_hash_hex = hex_encode(transaction.hash().as_ref());
    let tx_endpoint = request
        .torii_base_url
        .join("transaction")
        .map_err(|err| format!("failed to build Torii transaction endpoint URL: {err}"))?;
    let mut request_builder = request
        .client
        .post(tx_endpoint.as_str())
        .header(CONTENT_TYPE, "application/x-norito");
    if let Some(version) = request
        .api_version_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request_builder = request_builder.header("x-iroha-api-version", version);
    }
    let response = request_builder
        .body(transaction.encode_versioned())
        .send()
        .map_err(|err| format!("failed to submit fallback transaction to Torii: {err}"))?;
    let status = response.status();
    let response_bytes = response
        .bytes()
        .map_err(|err| format!("failed to read fallback Torii response: {err}"))?
        .to_vec();
    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&response_bytes);
        return Err(format!(
            "Torii transaction endpoint `{}` returned {status}: {body_text}",
            tx_endpoint
        ));
    }

    let confirmation = wait_for_transaction_confirmation(
        request.client,
        request.torii_base_url,
        &tx_hash_hex,
    )
    .map_err(|err| {
        format!(
            "submitted fallback transaction `{tx_hash_hex}` but failed to confirm the result: {err}"
        )
    })?;

    let mut map = Map::new();
    map.insert(
        "status".into(),
        Value::from(match confirmation.kind {
            TransactionConfirmationKind::Queued => "QUEUED",
            TransactionConfirmationKind::Approved => "APPROVED",
            TransactionConfirmationKind::Committed => "COMMITTED",
            TransactionConfirmationKind::Applied => "APPLIED",
            TransactionConfirmationKind::Rejected => "REJECTED",
            TransactionConfirmationKind::Expired => "EXPIRED",
        }),
    );
    map.insert("transport".into(), Value::from("transaction_fallback"));
    map.insert("tx_hash_hex".into(), Value::from(tx_hash_hex.clone()));
    map.insert(
        "transaction_endpoint".into(),
        Value::from(tx_endpoint.as_str().to_string()),
    );
    map.insert("chain_id".into(), Value::from(chain_id.clone()));
    map.insert(
        "confirmation_source".into(),
        Value::from("v1/pipeline/transactions/status"),
    );
    if let Some(block_height) = confirmation.block_height {
        map.insert("block_height".into(), Value::from(block_height));
    }
    if let Some(message) = confirmation.rejection_message.clone() {
        map.insert("rejection_message".into(), Value::from(message));
    }
    map.insert("pipeline_response".into(), confirmation.pipeline_response);
    let response_value = Value::Object(map);
    let response_bytes = to_string_pretty(&response_value)
        .map_err(|err| format!("failed to render fallback response JSON: {err}"))?
        .into_bytes();
    let failure_message = match confirmation.kind {
        TransactionConfirmationKind::Rejected => Some(match confirmation.rejection_message {
            Some(message) => {
                format!("fallback transaction `{tx_hash_hex}` was rejected: {message}")
            }
            None => format!("fallback transaction `{tx_hash_hex}` was rejected"),
        }),
        TransactionConfirmationKind::Expired => Some(format!(
            "fallback transaction `{tx_hash_hex}` expired before commit"
        )),
        _ => None,
    };

    Ok(ManifestSubmitFallback {
        status,
        endpoint: tx_endpoint.as_str().to_string(),
        response_bytes,
        response_value,
        chain_id,
        failure_message,
    })
}

fn wait_for_transaction_confirmation(
    client: &HttpClient,
    torii_base_url: &Url,
    tx_hash_hex: &str,
) -> Result<TransactionConfirmation, String> {
    let status_endpoint = torii_base_url
        .join("v1/pipeline/transactions/status")
        .map_err(|err| format!("failed to build transaction status endpoint URL: {err}"))?;
    let deadline =
        Instant::now() + Duration::from_millis(DEFAULT_MANIFEST_SUBMIT_CONFIRM_TIMEOUT_MS);
    let poll_interval = Duration::from_millis(DEFAULT_MANIFEST_SUBMIT_CONFIRM_POLL_MS);
    let mut last_observed_status = "QUEUED".to_string();

    loop {
        let response = client
            .get(status_endpoint.as_str())
            .query(&[("hash", tx_hash_hex)])
            .header("Accept", "application/json")
            .send()
            .map_err(|err| {
                format!(
                    "failed to query transaction status at `{}`: {err}",
                    status_endpoint
                )
            })?;
        let status = response.status();
        let response_bytes = response
            .bytes()
            .map_err(|err| format!("failed to read transaction status response: {err}"))?
            .to_vec();

        match status {
            StatusCode::OK | StatusCode::ACCEPTED => {
                if !response_bytes.is_empty() {
                    let value: Value = from_slice(&response_bytes).map_err(|err| {
                        format!(
                            "failed to decode transaction status JSON from `{}`: {err}",
                            status_endpoint
                        )
                    })?;
                    let confirmation = parse_transaction_confirmation(&value).ok_or_else(|| {
                        format!(
                            "transaction status payload from `{}` does not expose a status kind",
                            status_endpoint
                        )
                    })?;
                    last_observed_status = match confirmation.kind {
                        TransactionConfirmationKind::Queued => "QUEUED".to_string(),
                        TransactionConfirmationKind::Approved => "APPROVED".to_string(),
                        TransactionConfirmationKind::Committed => "COMMITTED".to_string(),
                        TransactionConfirmationKind::Applied => "APPLIED".to_string(),
                        TransactionConfirmationKind::Rejected => "REJECTED".to_string(),
                        TransactionConfirmationKind::Expired => "EXPIRED".to_string(),
                    };
                    match confirmation.kind {
                        TransactionConfirmationKind::Queued
                        | TransactionConfirmationKind::Approved => {}
                        TransactionConfirmationKind::Rejected => {
                            let explorer_rejection_message = fetch_transaction_rejection_message(
                                client,
                                torii_base_url,
                                tx_hash_hex,
                            );
                            let detailed_rejection_message = explorer_rejection_message
                                .clone()
                                .filter(|message| !looks_like_generic_rejection_message(message))
                                .or_else(|| confirmation.rejection_message.clone())
                                .or(explorer_rejection_message);
                            return Ok(TransactionConfirmation {
                                rejection_message: detailed_rejection_message,
                                ..confirmation
                            });
                        }
                        TransactionConfirmationKind::Committed
                        | TransactionConfirmationKind::Applied
                        | TransactionConfirmationKind::Expired => return Ok(confirmation),
                    }
                }
            }
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => {}
            other => {
                return Err(format!(
                    "transaction status endpoint `{}` returned {}: {}",
                    status_endpoint,
                    other,
                    String::from_utf8_lossy(&response_bytes)
                ));
            }
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "transaction `{tx_hash_hex}` remained {last_observed_status} after {}ms",
                DEFAULT_MANIFEST_SUBMIT_CONFIRM_TIMEOUT_MS
            ));
        }
        thread::sleep(poll_interval);
    }
}

fn parse_transaction_confirmation(payload: &Value) -> Option<TransactionConfirmation> {
    let status_value = payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?;
    let block_height = pipeline_status_block_height(payload, status_value);

    match status_value {
        Value::String(kind) => Some(TransactionConfirmation {
            kind: parse_transaction_confirmation_kind(kind)?,
            block_height,
            pipeline_response: payload.clone(),
            rejection_message: None,
        }),
        Value::Object(map) => {
            let kind = map.get("kind").and_then(Value::as_str)?;
            let rejection_message = map
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    map.get("content")
                        .and_then(Value::as_str)
                        .and_then(decode_rejection_message_hint)
                });
            Some(TransactionConfirmation {
                kind: parse_transaction_confirmation_kind(kind)?,
                block_height,
                pipeline_response: payload.clone(),
                rejection_message,
            })
        }
        _ => None,
    }
}

fn parse_transaction_confirmation_kind(kind: &str) -> Option<TransactionConfirmationKind> {
    match kind {
        "Queued" => Some(TransactionConfirmationKind::Queued),
        "Approved" => Some(TransactionConfirmationKind::Approved),
        "Committed" => Some(TransactionConfirmationKind::Committed),
        "Applied" => Some(TransactionConfirmationKind::Applied),
        "Rejected" => Some(TransactionConfirmationKind::Rejected),
        "Expired" => Some(TransactionConfirmationKind::Expired),
        _ => None,
    }
}

fn pipeline_status_block_height(payload: &Value, status_value: &Value) -> Option<u64> {
    match status_value {
        Value::Object(map) => map
            .get("block_height")
            .and_then(Value::as_u64)
            .or_else(|| {
                payload
                    .get("content")
                    .and_then(|content| content.get("block_height"))
                    .and_then(Value::as_u64)
            })
            .or_else(|| payload.get("block_height").and_then(Value::as_u64)),
        _ => payload
            .get("content")
            .and_then(|content| content.get("block_height"))
            .and_then(Value::as_u64)
            .or_else(|| payload.get("block_height").and_then(Value::as_u64)),
    }
}

fn fetch_transaction_rejection_message(
    client: &HttpClient,
    torii_base_url: &Url,
    tx_hash_hex: &str,
) -> Option<String> {
    let endpoint = torii_base_url
        .join(&format!("v1/explorer/transactions/{tx_hash_hex}"))
        .ok()?;
    let response = client
        .get(endpoint.as_str())
        .header("Accept", "application/json")
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.bytes().ok()?;
    let value: Value = from_slice(&body).ok()?;
    value
        .get("rejection_reason")
        .and_then(|reason| reason.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn looks_like_generic_rejection_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("validation failed") || lower.contains("invalid instruction parameter")
}

fn decode_rejection_message_hint(encoded: &str) -> Option<String> {
    let bytes = BASE64_STANDARD.decode(encoded.trim()).ok()?;
    extract_ascii_message(&bytes)
}

fn extract_ascii_message(bytes: &[u8]) -> Option<String> {
    let mut best = String::new();
    let mut current = String::new();

    for &byte in bytes {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte as char);
        } else {
            if current.len() > best.len() {
                best = current.clone();
            }
            current.clear();
        }
    }
    if current.len() > best.len() {
        best = current;
    }

    let trimmed = best.trim();
    if trimmed.len() >= 12 {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn decode_response_value_or_text(response: &[u8]) -> Value {
    match from_slice(response) {
        Ok(value) => value,
        Err(_) => Value::from(String::from_utf8_lossy(response).to_string()),
    }
}

fn resolve_chain_id_from_sorafs_registry(
    client: &HttpClient,
    torii_base_url: &Url,
) -> Result<String, String> {
    let mut last_error: Option<String> = None;
    for route in ["v1/sorafs/pin", "v1/sorafs/aliases"] {
        let endpoint = torii_base_url
            .join(route)
            .map_err(|err| format!("failed to build Torii registry endpoint URL: {err}"))?;
        let response = match client
            .get(endpoint.as_str())
            .header("Accept", "application/json")
            .send()
        {
            Ok(response) => response,
            Err(err) => {
                last_error = Some(format!("failed to query `{}`: {err}", endpoint));
                continue;
            }
        };
        let status = response.status();
        let response_bytes = response.bytes().map_err(|err| {
            format!(
                "failed to read registry response from `{}`: {err}",
                endpoint
            )
        })?;
        let body = response_bytes.to_vec();
        if !status.is_success() {
            last_error = Some(format!(
                "registry endpoint `{}` returned {}: {}",
                endpoint,
                status,
                String::from_utf8_lossy(&body)
            ));
            continue;
        }
        let value: Value = from_slice(&body)
            .map_err(|err| format!("failed to decode registry JSON from `{}`: {err}", endpoint))?;
        match resolve_chain_id_from_registry_value(&value) {
            Ok(chain_id) => return Ok(chain_id),
            Err(err) => {
                last_error = Some(format!(
                    "failed to resolve chain id from `{}`: {err}",
                    endpoint
                ));
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| "failed to resolve chain id from SoraFS registry endpoints".to_string()))
}

fn resolve_chain_id_from_registry_value(value: &Value) -> Result<String, String> {
    let chain_id = find_status_value(value, &["attestation", "chain_id"])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "registry JSON is missing `attestation.chain_id`".to_string())?;
    Ok(chain_id.to_string())
}

fn resolve_submitted_epoch_from_status(
    client: &HttpClient,
    torii_base_url: &Url,
) -> Result<u64, String> {
    let status_endpoint = torii_base_url
        .join("status")
        .map_err(|err| format!("failed to build Torii status endpoint URL: {err}"))?;
    let response = client
        .get(status_endpoint.as_str())
        .header("Accept", "application/json")
        .send()
        .map_err(|err| {
            format!(
                "failed to query Torii status at `{}`: {err}",
                status_endpoint
            )
        })?;
    let status = response.status();
    let response_bytes = response
        .bytes()
        .map_err(|err| format!("failed to read Torii status response: {err}"))?;
    let body = response_bytes.to_vec();

    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        return Err(format!(
            "Torii status endpoint `{}` returned {status}: {body_text}",
            status_endpoint
        ));
    }

    let status_value: Value = from_slice(&body).map_err(|err| {
        format!(
            "failed to decode Torii status JSON from `{}`: {err}",
            status_endpoint
        )
    })?;

    resolve_submitted_epoch_from_status_value(&status_value).map_err(|err| {
        format!(
            "failed to resolve submitted epoch from `{}`: {err}",
            status_endpoint
        )
    })
}

fn resolve_submitted_epoch_from_status_value(status_value: &Value) -> Result<u64, String> {
    if let Some(epoch) = find_status_epoch(status_value) {
        return Ok(epoch);
    }

    let blocks = status_value
        .get("blocks")
        .and_then(Value::as_u64)
        .ok_or_else(|| "status JSON is missing `blocks`".to_string())?;
    let epoch_length_blocks = find_status_value(status_value, &["sumeragi", "epoch_length_blocks"])
        .and_then(Value::as_u64)
        .or_else(|| {
            status_value
                .get("epoch_length_blocks")
                .and_then(Value::as_u64)
        })
        .ok_or_else(|| "status JSON is missing `sumeragi.epoch_length_blocks`".to_string())?;

    if epoch_length_blocks == 0 {
        return Err("`sumeragi.epoch_length_blocks` must be greater than zero".to_string());
    }

    Ok(blocks.saturating_sub(1) / epoch_length_blocks)
}

fn find_status_epoch(status_value: &Value) -> Option<u64> {
    [
        ["sumeragi", "epoch"].as_slice(),
        ["sumeragi", "current_epoch"].as_slice(),
        ["sumeragi", "membership", "epoch"].as_slice(),
        ["current_epoch"].as_slice(),
        ["epoch"].as_slice(),
    ]
    .into_iter()
    .find_map(|path| find_status_value(status_value, path).and_then(parse_status_epoch_value))
}

fn find_status_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.as_object()?.get(*segment)?;
    }
    Some(current)
}

fn parse_status_epoch_value(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        let object = value.as_object()?;
        object
            .get("height")
            .and_then(Value::as_u64)
            .or_else(|| object.get("index").and_then(Value::as_u64))
            .or_else(|| object.get("epoch").and_then(Value::as_u64))
    })
}

fn manifest_proposal(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut chunk_digest_hex_arg: Option<String> = None;
    let mut submitted_epoch: Option<u64> = None;
    let mut successor_hex: Option<String> = None;
    let mut alias_hint: Option<String> = None;
    let mut proposal_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--chunk-digest-sha3" => chunk_digest_hex_arg = Some(value.to_string()),
            "--submitted-epoch" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--submitted-epoch` value: {err}"))?;
                submitted_epoch = Some(parsed);
            }
            "--successor-of" => successor_hex = Some(value.to_string()),
            "--alias-hint" => alias_hint = Some(value.to_string()),
            "--proposal-out" => proposal_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unknown option `{key}` for `sorafs_cli manifest proposal`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli manifest proposal`".to_string()
    })?;
    let submitted_epoch = submitted_epoch.ok_or_else(|| {
        "missing required `--submitted-epoch=EPOCH` for `sorafs_cli manifest proposal`".to_string()
    })?;
    let proposal_out = proposal_out.ok_or_else(|| {
        "missing required `--proposal-out=PATH` for `sorafs_cli manifest proposal`".to_string()
    })?;

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;

    let plan_specs = if let Some(source) = chunk_plan_source {
        let value = source.read()?;
        Some(
            chunk_fetch_specs_from_json(&value)
                .map_err(|err| format!("failed to parse chunk plan JSON: {err}"))?,
        )
    } else {
        None
    };

    let chunk_digest = match (chunk_digest_hex_arg, plan_specs.as_ref()) {
        (Some(hex), Some(specs)) => {
            let parsed =
                parse_digest_hex(&hex).map_err(|err| format!("invalid `--chunk-digest-sha3`: {err}"))?;
            let expected = chunk_digest_sha3_from_specs(specs);
            if parsed != expected {
                return Err(
                    "`--chunk-digest-sha3` does not match digest derived from `--chunk-plan`"
                        .to_string(),
                );
            }
            parsed
        }
        (Some(hex), None) => {
            parse_digest_hex(&hex).map_err(|err| format!("invalid `--chunk-digest-sha3`: {err}"))?
        }
        (None, Some(specs)) => chunk_digest_sha3_from_specs(specs),
        (None, None) => {
            return Err(
                "must provide either `--chunk-plan` or `--chunk-digest-sha3` for `sorafs_cli manifest proposal`"
                    .to_string(),
            )
        }
    };

    let successor_bytes = match successor_hex {
        Some(hex) => Some(
            parse_digest_hex(&hex)
                .map_err(|err| format!("invalid `--successor-of` value: {err}"))?,
        ),
        None => None,
    };

    let proposal_value = build_manifest_proposal_summary(ManifestProposalSummary {
        manifest_path: &manifest_path,
        manifest: &manifest,
        manifest_digest: &manifest_digest,
        chunk_digest_sha3: chunk_digest,
        chunk_plan_label: chunk_plan_label.as_deref(),
        submitted_epoch,
        alias_hint: alias_hint.as_deref(),
        successor_bytes,
    })?;

    ensure_parent_dir(&proposal_out)?;
    let mut rendered = to_string_pretty(&proposal_value)
        .map_err(|err| format!("failed to render proposal JSON: {err}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    write_text(&proposal_out, rendered.as_bytes())
        .map_err(|err| format!("failed to write `{}`: {err}", proposal_out.display()))?;
    println!("wrote {}", proposal_out.display());
    Ok(())
}

fn proof_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut car_path: Option<PathBuf> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--car" => car_path = Some(PathBuf::from(value)),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli proof verify`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli proof verify`".to_string()
    })?;
    let car_path = car_path
        .ok_or_else(|| "missing required `--car=PATH` for `sorafs_cli proof verify`".to_string())?;

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;

    let car_bytes = fs::read(&car_path)
        .map_err(|err| format!("failed to read CAR archive `{}`: {err}", car_path.display()))?;
    let resolved_plan = if let Some(source) = chunk_plan_source {
        let plan_json = source.read()?;
        let chunker_handle = chunker_handle_from_profile(&manifest.chunking).to_handle();
        Some(build_plan_from_specs(&plan_json, Some(&chunker_handle))?)
    } else {
        None
    };
    let report = if let Some(plan_with_handle) = resolved_plan.as_ref() {
        CarVerifier::verify_full_car_with_plan(&manifest, &plan_with_handle.plan, &car_bytes)
    } else {
        CarVerifier::verify_full_car(&manifest, &car_bytes)
    }
    .map_err(|err| format!("failed to verify CAR archive: {err}"))?;

    let payload_digest_hex = hex_encode(report.chunk_store.payload_digest().as_bytes());
    let chunk_digest_sha3 = chunk_digest_sha3_from_chunks(report.chunk_store.chunks());
    let chunk_digest_hex = hex_encode(chunk_digest_sha3);
    let car_payload_digest_hex = hex_encode(report.stats.car_payload_digest.as_bytes());
    let car_digest_hex = hex_encode(report.stats.car_archive_digest.as_bytes());

    let mut summary = Map::new();
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary.insert(
        "car_path".into(),
        Value::from(car_path.display().to_string()),
    );
    summary.insert(
        "chunk_count".into(),
        Value::from(report.chunk_store.chunks().len() as u64),
    );
    if let Some(label) = chunk_plan_label {
        summary.insert("chunk_plan_source".into(), Value::from(label));
    }
    if let Some(plan_with_handle) = resolved_plan.as_ref() {
        summary.insert(
            "chunk_plan_chunk_count".into(),
            Value::from(plan_with_handle.plan.chunks.len() as u64),
        );
    }
    summary.insert(
        "payload_bytes".into(),
        Value::from(report.chunk_store.payload_len()),
    );
    summary.insert("payload_digest_hex".into(), Value::from(payload_digest_hex));
    summary.insert(
        "chunk_digest_sha3_hex".into(),
        Value::from(chunk_digest_hex),
    );
    summary.insert(
        "car_payload_digest_hex".into(),
        Value::from(car_payload_digest_hex),
    );
    summary.insert("car_digest_hex".into(), Value::from(car_digest_hex.clone()));
    summary.insert(
        "manifest_car_digest_hex".into(),
        Value::from(car_digest_hex),
    );
    summary.insert("car_size".into(), Value::from(report.stats.car_size));
    summary.insert(
        "chunker_handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )),
    );
    summary.insert(
        "manifest_digest_hex".into(),
        Value::from(hex_encode(manifest_digest.as_bytes())),
    );
    summary.insert(
        "pin_policy".into(),
        Value::Object(pin_policy_json(&manifest.pin_policy)),
    );
    summary.insert(
        "root_cids_hex".into(),
        Value::Array(
            report
                .stats
                .root_cids
                .iter()
                .map(|cid| Value::from(hex_encode(cid)))
                .collect(),
        ),
    );
    summary.insert("dag_codec".into(), Value::from(report.stats.dag_codec));
    summary.insert(
        "chunker_profile_id".into(),
        Value::from(u64::from(manifest.chunking.profile_id.0)),
    );
    summary.insert(
        "car_payload_bytes".into(),
        Value::from(report.stats.payload_bytes),
    );

    let summary_value = Value::Object(summary);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}

fn reputation_publish(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut snapshot_path: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--snapshot" => snapshot_path = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation publish`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation publish`".to_string()
    })?;
    let snapshot_path = snapshot_path.ok_or_else(|| {
        "missing required `--snapshot=PATH` for `sorafs_cli reputation publish`".to_string()
    })?;
    let snapshot = read_reputation_snapshot(&snapshot_path)?;
    let body = to_vec(&snapshot)
        .map_err(|err| format!("failed to encode reputation snapshot JSON: {err}"))?;
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let endpoint = reputation_endpoint(&torii_url, "v1/sorafs/reputation/latest")?;
    let response = client
        .post(endpoint.as_str())
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .map_err(|err| format!("failed to publish reputation snapshot to `{endpoint}`: {err}"))?;
    let value = read_json_response(response, "reputation publish", endpoint.as_str())?;
    emit_reputation_json(value, summary_out.as_deref())
}

fn reputation_snapshot(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut output: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--output" => output = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation snapshot`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation snapshot`".to_string()
    })?;
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let endpoint = reputation_endpoint(&torii_url, "v1/sorafs/reputation/latest")?;
    let response = client
        .get(endpoint.as_str())
        .header("Accept", "application/json")
        .send()
        .map_err(|err| format!("failed to fetch reputation snapshot from `{endpoint}`: {err}"))?;
    let value = read_json_response(response, "reputation snapshot", endpoint.as_str())?;
    let output_path = output.as_deref().or(summary_out.as_deref());
    emit_reputation_json(value, output_path)
}

fn reputation_fetch(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut format = ReputationFetchFormat::Table;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--provider-id" => provider_id = Some(value.to_string()),
            "--format" => format = ReputationFetchFormat::parse(value)?,
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation fetch`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation fetch`".to_string()
    })?;
    let provider_id = provider_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "missing required `--provider-id=ID` for `sorafs_cli reputation fetch`".to_string()
        })?;
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let route = format!("v1/sorafs/reputation/providers/{provider_id}");
    let endpoint = reputation_endpoint(&torii_url, &route)?;
    let response = client
        .get(endpoint.as_str())
        .header("Accept", "application/json")
        .send()
        .map_err(|err| {
            format!("failed to fetch reputation provider `{provider_id}` from `{endpoint}`: {err}")
        })?;
    let value = read_json_response(response, "reputation fetch", endpoint.as_str())?;
    if let Some(path) = summary_out.as_deref() {
        write_reputation_json(path, &value)?;
    }
    match format {
        ReputationFetchFormat::Json => {
            let rendered = to_string_pretty(&value)
                .map_err(|err| format!("failed to render reputation provider JSON: {err}"))?;
            println!("{rendered}");
        }
        ReputationFetchFormat::Table => {
            println!("{}", reputation_provider_table(&value)?);
        }
    }
    Ok(())
}

fn reputation_watch(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut since: Option<u64> = None;
    let mut limit: Option<u32> = None;
    let mut max_polls: Option<usize> = Some(1);
    let mut poll_interval_ms: u64 = 1_000;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--since" => {
                since = Some(parse_u64_arg(
                    "since",
                    value,
                    "sorafs_cli reputation watch",
                )?)
            }
            "--limit" => {
                limit = Some(parse_u32_arg(
                    "limit",
                    value,
                    "sorafs_cli reputation watch",
                )?)
            }
            "--max-polls" => {
                let parsed = parse_usize(value, "--max-polls")?;
                max_polls = (parsed != 0).then_some(parsed);
            }
            "--poll-interval-ms" => {
                poll_interval_ms =
                    parse_u64_arg("poll-interval-ms", value, "sorafs_cli reputation watch")?;
            }
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation watch`"
                ));
            }
        }
    }

    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation watch`".to_string()
    })?;
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let mut next_since = since;
    let mut polls = 0_usize;
    let final_value = loop {
        let endpoint = reputation_events_endpoint(&torii_url, next_since, limit)?;
        let response = client
            .get(endpoint.as_str())
            .header("Accept", "application/json")
            .send()
            .map_err(|err| format!("failed to watch reputation events from `{endpoint}`: {err}"))?;
        let value = read_json_response(response, "reputation watch", endpoint.as_str())?;
        if let Some(cursor) = value.get("next_since").and_then(Value::as_u64) {
            next_since = Some(cursor);
        }
        let rendered = to_string_pretty(&value)
            .map_err(|err| format!("failed to render reputation watch JSON: {err}"))?;
        println!("{rendered}");
        polls = polls.saturating_add(1);
        if max_polls.is_some_and(|max| polls >= max) {
            break value;
        }
        thread::sleep(Duration::from_millis(poll_interval_ms));
    };
    if let Some(path) = summary_out.as_deref() {
        write_reputation_json(path, &final_value)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ReputationFetchFormat {
    Table,
    Json,
}

impl ReputationFetchFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unsupported reputation fetch format `{other}`; expected `table` or `json`"
            )),
        }
    }
}

fn read_reputation_snapshot(path: &Path) -> Result<ReputationSnapshotV1, String> {
    let snapshot_bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read reputation snapshot `{}`: {err}",
            path.display()
        )
    })?;
    let snapshot: ReputationSnapshotV1 = decode_from_bytes(&snapshot_bytes)
        .map_err(|err| format!("failed to decode reputation snapshot: {err}"))?;
    snapshot
        .validate()
        .map_err(|err| format!("invalid reputation snapshot: {err}"))?;
    Ok(snapshot)
}

fn reputation_endpoint(torii_url: &str, route: &str) -> Result<Url, String> {
    Url::parse(torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join(route)
        .map_err(|err| format!("failed to build reputation endpoint URL: {err}"))
}

fn reputation_events_endpoint(
    torii_url: &str,
    since: Option<u64>,
    limit: Option<u32>,
) -> Result<Url, String> {
    let mut endpoint = reputation_endpoint(torii_url, "v1/sorafs/reputation/events")?;
    let mut serializer = Serializer::new(String::new());
    if let Some(since) = since {
        serializer.append_pair("since", &since.to_string());
    }
    if let Some(limit) = limit {
        serializer.append_pair("limit", &limit.to_string());
    }
    let query = serializer.finish();
    if !query.is_empty() {
        endpoint.set_query(Some(&query));
    }
    Ok(endpoint)
}

fn read_json_response(
    response: reqwest::blocking::Response,
    context: &str,
    endpoint: &str,
) -> Result<Value, String> {
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read {context} response from `{endpoint}`: {err}"))?
        .to_vec();
    if !status.is_success() {
        return Err(format!(
            "Torii {context} endpoint `{endpoint}` returned {status}: {}",
            String::from_utf8_lossy(&body)
        ));
    }
    from_slice(&body)
        .map_err(|err| format!("failed to decode {context} JSON from `{endpoint}`: {err}"))
}

fn emit_reputation_json(value: Value, output: Option<&Path>) -> Result<(), String> {
    let rendered = to_string_pretty(&value)
        .map_err(|err| format!("failed to render reputation JSON: {err}"))?;
    println!("{rendered}");
    if let Some(path) = output {
        write_reputation_json(path, &value)?;
    }
    Ok(())
}

fn write_reputation_json(path: &Path, value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|err| format!("failed to render reputation JSON: {err}"))?;
    ensure_parent_dir(path)?;
    write_text(path, rendered.as_bytes())
}

fn reputation_provider_table(value: &Value) -> Result<String, String> {
    let provider = value
        .get("provider")
        .and_then(Value::as_object)
        .ok_or_else(|| "reputation provider response is missing `provider` object".to_string())?;
    let proof = value
        .get("proof")
        .and_then(Value::as_object)
        .ok_or_else(|| "reputation provider response is missing `proof` object".to_string())?;
    let provider_id = provider
        .get("provider_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "reputation provider response is missing `provider.provider_id`".to_string()
        })?;
    let score_bps = provider
        .get("score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "reputation provider response is missing `provider.score_bps`".to_string()
        })?;
    let leaf_index = proof
        .get("leaf_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| "reputation provider response is missing `proof.leaf_index`".to_string())?;
    let sibling_count = proof
        .get("siblings_hex")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let merkle_root = value
        .get("merkle_root_hex")
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(format!(
        "provider_id\tscore_bps\tleaf_index\tproof_siblings\tmerkle_root_hex\n{provider_id}\t{score_bps}\t{leaf_index}\t{sibling_count}\t{merkle_root}"
    ))
}

fn reputation_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut snapshot_path: Option<PathBuf> = None;
    let mut provider_id: Option<String> = None;
    let mut proof_path: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--snapshot" => snapshot_path = Some(PathBuf::from(value)),
            "--provider-id" => provider_id = Some(value.to_string()),
            "--proof" => proof_path = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation verify`"
                ));
            }
        }
    }

    let snapshot_path = snapshot_path.ok_or_else(|| {
        "missing required `--snapshot=PATH` for `sorafs_cli reputation verify`".to_string()
    })?;
    if provider_id.is_some() != proof_path.is_some() {
        return Err(
            "`--provider-id=ID` and `--proof=PATH` must be supplied together for reputation proof verification"
                .to_string(),
        );
    }

    let snapshot = read_reputation_snapshot(&snapshot_path)?;

    let mut summary = Map::new();
    summary.insert(
        "snapshot_path".into(),
        Value::from(snapshot_path.display().to_string()),
    );
    summary.insert(
        "snapshot_id_hex".into(),
        Value::from(hex_encode(snapshot.snapshot_id)),
    );
    summary.insert(
        "generated_at_unix".into(),
        Value::from(snapshot.generated_at_unix),
    );
    if let Some(previous_snapshot_id) = snapshot.previous_snapshot_id {
        summary.insert(
            "previous_snapshot_id_hex".into(),
            Value::from(hex_encode(previous_snapshot_id)),
        );
    }
    summary.insert(
        "provider_count".into(),
        Value::from(snapshot.providers.len() as u64),
    );
    summary.insert(
        "merkle_root_hex".into(),
        Value::from(hex_encode(snapshot.merkle_root)),
    );
    summary.insert(
        "alpha_bps".into(),
        Value::from(u64::from(snapshot.alpha_bps)),
    );
    summary.insert(
        "current_score_weight_bps".into(),
        Value::from(u64::from(snapshot.current_score_weight_bps)),
    );
    summary.insert("valid".into(), Value::from(true));

    if let (Some(provider_id), Some(proof_path)) = (provider_id, proof_path) {
        let provider = snapshot
            .providers
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .ok_or_else(|| {
                format!("provider `{provider_id}` was not found in the reputation snapshot")
            })?;
        let proof_bytes = fs::read(&proof_path).map_err(|err| {
            format!(
                "failed to read reputation proof `{}`: {err}",
                proof_path.display()
            )
        })?;
        let proof: ReputationMerkleProofV1 = decode_from_bytes(&proof_bytes)
            .map_err(|err| format!("failed to decode reputation proof: {err}"))?;
        proof
            .verify(provider, snapshot.merkle_root)
            .map_err(|err| format!("invalid reputation proof: {err}"))?;

        summary.insert(
            "provider_id".into(),
            Value::from(provider.provider_id.clone()),
        );
        summary.insert(
            "provider_score_bps".into(),
            Value::from(u64::from(provider.score_bps)),
        );
        summary.insert(
            "proof_path".into(),
            Value::from(proof_path.display().to_string()),
        );
        summary.insert(
            "proof_leaf_index".into(),
            Value::from(u64::from(proof.leaf_index)),
        );
        summary.insert(
            "proof_sibling_count".into(),
            Value::from(proof.siblings.len() as u64),
        );
        summary.insert("proof_verified".into(), Value::from(true));
    }

    let summary_value = Value::Object(summary);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render reputation summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}

fn proof_stream(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut torii_url: Option<String> = None;
    let mut endpoint_url: Option<String> = None;
    let mut provider_id_hex: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut proof_kind_arg: Option<String> = None;
    let mut samples: Option<u32> = None;
    let mut sample_seed: Option<u64> = None;
    let mut deadline_ms: Option<u32> = None;
    let mut tier_arg: Option<String> = None;
    let mut nonce_b64: Option<String> = None;
    let mut orchestrator_job_id_hex: Option<String> = None;
    let mut stream_token: Option<String> = None;
    let mut bearer_token_env: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut evidence_dir: Option<PathBuf> = None;
    let mut por_root_hex: Option<String> = None;
    let mut emit_events = false;
    let mut max_failures: Option<u64> = None;
    let mut max_verification_failures: Option<u64> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--torii-url" => torii_url = Some(value.to_string()),
            "--gateway-url" | "--endpoint" => endpoint_url = Some(value.to_string()),
            "--provider-id-hex" => provider_id_hex = Some(value.to_string()),
            "--provider-id" => provider_id = Some(value.to_string()),
            "--proof-kind" => proof_kind_arg = Some(value.to_string()),
            "--samples" => {
                let parsed: u32 = value
                    .parse()
                    .map_err(|err| format!("invalid `--samples` value: {err}"))?;
                samples = Some(parsed);
            }
            "--sample-seed" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--sample-seed` value: {err}"))?;
                sample_seed = Some(parsed);
            }
            "--deadline-ms" => {
                let parsed: u32 = value
                    .parse()
                    .map_err(|err| format!("invalid `--deadline-ms` value: {err}"))?;
                deadline_ms = Some(parsed);
            }
            "--tier" => tier_arg = Some(value.to_string()),
            "--nonce-b64" => nonce_b64 = Some(value.to_string()),
            "--orchestrator-job-id-hex" => orchestrator_job_id_hex = Some(value.to_string()),
            "--stream-token" => stream_token = Some(value.to_string()),
            "--bearer-token-env" => bearer_token_env = Some(value.to_string()),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--governance-evidence-dir" => evidence_dir = Some(PathBuf::from(value)),
            "--por-root-hex" => por_root_hex = Some(value.to_string()),
            "--emit-events" => emit_events = parse_bool_flag(value, "--emit-events")?,
            "--max-failures" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--max-failures` value: {err}"))?;
                max_failures = Some(parsed);
            }
            "--max-verification-failures" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--max-verification-failures` value: {err}"))?;
                max_verification_failures = Some(parsed);
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli proof stream`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli proof stream`".to_string()
    })?;
    let legacy_proof_stream_mode = endpoint_url.is_some() || provider_id.is_some();
    if torii_url.is_some() && endpoint_url.is_some() {
        return Err(
            "`--torii-url` cannot be combined with `--gateway-url`/`--endpoint`".to_string(),
        );
    }

    let (torii_url, endpoint) = if let Some(endpoint_raw) = endpoint_url {
        let endpoint_raw = endpoint_raw.trim().to_string();
        if endpoint_raw.is_empty() {
            return Err("`--gateway-url`/`--endpoint` must not be empty".to_string());
        }
        let endpoint = Url::parse(&endpoint_raw)
            .map_err(|err| format!("invalid proof stream endpoint `{endpoint_raw}`: {err}"))?;
        (endpoint_raw, endpoint)
    } else {
        let torii = torii_url.ok_or_else(|| {
            "missing required `--torii-url=URL` (or `--gateway-url=URL`/`--endpoint=URL`) for `sorafs_cli proof stream`".to_string()
        })?;
        let endpoint = Url::parse(&torii)
            .map_err(|err| format!("invalid `--torii-url` value `{torii}`: {err}"))?
            .join("v1/sorafs/proof/stream")
            .map_err(|err| format!("failed to resolve proof stream endpoint: {err}"))?;
        (torii, endpoint)
    };

    if provider_id_hex.is_some() && provider_id.is_some() {
        return Err("`--provider-id-hex` cannot be combined with `--provider-id`".to_string());
    }
    let provider_id = if let Some(raw_hex) = provider_id_hex {
        let normalized = raw_hex.trim().to_ascii_lowercase();
        let _ = parse_digest_hex(&normalized)
            .map_err(|err| format!("invalid `--provider-id-hex` value: {err}"))?;
        normalized
    } else if let Some(raw_id) = provider_id {
        let trimmed = raw_id.trim();
        if trimmed.is_empty() {
            return Err("`--provider-id` must not be empty".to_string());
        }
        trimmed.to_string()
    } else {
        return Err(
            "missing required `--provider-id-hex=HEX32` (or legacy `--provider-id=ID`) for `sorafs_cli proof stream`".to_string(),
        );
    };

    let proof_kind = proof_kind_arg
        .as_deref()
        .map(|kind| ProofKind::parse(kind.trim()))
        .transpose()?
        .unwrap_or_default();
    let deadline_ms_arg = deadline_ms;
    let (sample_count, deadline_ms) = match proof_kind {
        ProofKind::Por => {
            if deadline_ms_arg.is_some() {
                return Err("`--deadline-ms` may only be used with `--proof-kind=potr`".to_string());
            }
            let count = samples.unwrap_or(32);
            if count == 0 {
                return Err("`--samples` must be greater than zero".to_string());
            }
            (Some(count), None)
        }
        ProofKind::Pdp => {
            if deadline_ms_arg.is_some() {
                return Err("`--deadline-ms` may only be used with `--proof-kind=potr`".to_string());
            }
            let count = samples.unwrap_or(32);
            if count == 0 {
                return Err("`--samples` must be greater than zero".to_string());
            }
            (Some(count), None)
        }
        ProofKind::Potr => {
            if samples.is_some() {
                return Err("`--samples` is not supported for `--proof-kind=potr`".to_string());
            }
            let deadline = deadline_ms_arg.ok_or_else(|| {
                "`--deadline-ms` is required when `--proof-kind=potr`".to_string()
            })?;
            if deadline == 0 {
                return Err("`--deadline-ms` must be greater than zero".to_string());
            }
            (None, Some(deadline))
        }
    };
    let tier = tier_arg
        .as_deref()
        .map(|tier| ProofTier::parse(tier.trim()))
        .transpose()?;

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    let manifest_cid_hex = hex_encode(&manifest.root_cid);

    let nonce = if let Some(encoded) = nonce_b64 {
        decode_nonce_b64(&encoded)?
    } else {
        generate_proof_stream_nonce(
            manifest_digest.as_bytes(),
            proof_kind,
            sample_count,
            deadline_ms,
            Some(&provider_id),
        )
    };

    let request = ProofStreamRequest {
        manifest_digest_hex: manifest_digest_hex.clone(),
        provider_id_hex: Some(provider_id.clone()),
        proof_kind,
        sample_count,
        sample_seed,
        deadline_ms,
        tier,
        nonce,
        orchestrator_job_id_hex: orchestrator_job_id_hex.map(|id| id.trim().to_ascii_lowercase()),
    };
    let request_body = request.to_json_bytes()?;

    let client = HttpClient::builder()
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let mut builder = client
        .post(endpoint.clone())
        .header(CONTENT_TYPE, "application/json")
        .header("Accept", "application/x-ndjson")
        .header("Sora-Stream-Client", "sorafs_cli/stream/v2");

    if let Some(token) = stream_token.as_ref() {
        builder = builder.header("Sora-Stream-Token", token);
    }
    if let Some(env_var) = bearer_token_env.as_ref() {
        let value = env::var(env_var)
            .map_err(|err| format!("failed to read bearer token from `{env_var}`: {err}"))?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(format!("environment variable `{env_var}` is empty"));
        }
        builder = builder.header("Authorization", format!("Bearer {trimmed}"));
    }

    let response = builder
        .body(request_body)
        .send()
        .map_err(|err| format!("failed to initiate proof stream: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response
            .bytes()
            .map_err(|err| format!("failed to read proof stream error response: {err}"))?;
        let snippet = String::from_utf8_lossy(&body);
        return Err(format!(
            "gateway returned {status} when streaming proofs: {snippet}"
        ));
    }

    let expected_root = por_root_hex
        .as_deref()
        .map(|hex| parse_digest_hex(hex).map_err(|err| format!("invalid `--por-root-hex`: {err}")))
        .transpose()?;
    let mut verification_attempts: u64 = 0;
    let mut verification_failures: u64 = 0;

    let mut reader = BufReader::new(response);
    let mut line = String::new();
    let mut metrics = ProofStreamMetrics::default();
    let mut failure_samples: Vec<ProofStreamItem> = Vec::new();
    const FAILURE_SAMPLE_LIMIT: usize = 5;

    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|err| format!("failed to read proof stream response: {err}"))?;
        if bytes_read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut item = ProofStreamItem::from_ndjson(trimmed.as_bytes())?;
        if let Some(root) = expected_root.as_ref()
            && matches!(item.proof_kind, ProofKind::Por)
        {
            verification_attempts += 1;
            let verified = item
                .por_proof
                .as_ref()
                .map(|proof| proof.verify(root))
                .unwrap_or(false);
            if !verified {
                verification_failures += 1;
                if failure_samples.len() < FAILURE_SAMPLE_LIMIT {
                    failure_samples.push(item.clone());
                }
                item.failure_reason
                    .get_or_insert_with(|| "local_verification_failed".to_string());
            }
        }
        metrics.record(&item);
        if item.status.is_failure() && failure_samples.len() < FAILURE_SAMPLE_LIMIT {
            failure_samples.push(item.clone());
        }
        if emit_events {
            println!("{trimmed}");
        }
    }

    let summary = ProofStreamSummary::new(metrics.clone(), failure_samples.clone());
    let mut summary_map = Map::new();
    summary_map.insert("endpoint".into(), Value::from(endpoint.as_str()));
    summary_map.insert("torii_url".into(), Value::from(torii_url.clone()));
    summary_map.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary_map.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest_hex.clone()),
    );
    summary_map.insert(
        "manifest_cid_hex".into(),
        Value::from(manifest_cid_hex.clone()),
    );
    summary_map.insert("provider_id_hex".into(), Value::from(provider_id.clone()));
    summary_map.insert("provider_id".into(), Value::from(provider_id.clone()));
    summary_map.insert("proof_kind".into(), Value::from(proof_kind.as_str()));
    if let Some(count) = sample_count {
        summary_map.insert(
            "requested_sample_count".into(),
            Value::from(u64::from(count)),
        );
    }
    if let Some(seed) = sample_seed {
        summary_map.insert("requested_sample_seed".into(), Value::from(seed));
    }
    if let Some(deadline) = deadline_ms {
        summary_map.insert(
            "requested_deadline_ms".into(),
            Value::from(u64::from(deadline)),
        );
    }
    if let Some(tier) = tier {
        summary_map.insert("requested_tier".into(), Value::from(tier.as_str()));
    }
    summary_map.insert(
        "nonce_b64".into(),
        Value::from(BASE64_STANDARD.encode(request.nonce)),
    );
    let metrics_json = summary.metrics.to_json();
    summary_map.insert("metrics".into(), metrics_json.clone());
    summary_map.insert(
        "total_items".into(),
        Value::from(summary.metrics.item_total),
    );
    summary_map.insert(
        "success_count".into(),
        Value::from(summary.metrics.success_total),
    );
    summary_map.insert(
        "failure_count".into(),
        Value::from(summary.metrics.failure_total),
    );
    let mut legacy_breakdown = Map::new();
    for (reason, count) in &summary.metrics.failure_by_reason {
        legacy_breakdown.insert(reason.clone(), Value::from(*count));
    }
    summary_map.insert("failure_breakdown".into(), Value::Object(legacy_breakdown));
    if let Some(avg) = metrics_json
        .as_object()
        .and_then(|obj| obj.get("latency_ms"))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("average_ms"))
        .and_then(Value::as_f64)
    {
        summary_map.insert("average_latency_ms".into(), Value::from(avg));
    }
    let failure_budget = max_failures.unwrap_or({
        if legacy_proof_stream_mode {
            1
        } else {
            match proof_kind {
                ProofKind::Potr => 1,
                _ => 0,
            }
        }
    });
    summary_map.insert("allowed_failure_budget".into(), Value::from(failure_budget));
    let verification_budget = max_verification_failures.unwrap_or(0);
    summary_map.insert(
        "allowed_verification_failure_budget".into(),
        Value::from(verification_budget),
    );
    if let Some(root) = expected_root.as_ref() {
        summary_map.insert(
            "verification_root_hex".into(),
            Value::from(hex_encode(root)),
        );
        summary_map.insert(
            "verification_total".into(),
            Value::from(verification_attempts),
        );
        summary_map.insert(
            "verification_failures".into(),
            Value::from(verification_failures),
        );
    }
    if !failure_samples.is_empty() {
        let sample_json = failure_samples
            .iter()
            .take(FAILURE_SAMPLE_LIMIT)
            .map(ProofStreamItem::to_json)
            .collect::<Vec<_>>();
        summary_map.insert("failure_samples".into(), Value::Array(sample_json));
    }

    let summary_value = Value::Object(summary_map);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render proof stream summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        ensure_parent_dir(&path)?;
        write_text(&path, rendered.as_bytes())?;
    }
    if let Some(dir) = evidence_dir {
        write_proof_stream_evidence(
            &dir,
            &manifest_path,
            &manifest_digest_hex,
            &rendered,
            &torii_url,
        )?;
    }

    if summary.metrics.failure_total > failure_budget {
        return Err(format!(
            "proof stream reported {} gateway failures which exceeds the allowed maximum ({failure_budget})",
            summary.metrics.failure_total
        ));
    }
    if verification_failures > verification_budget {
        return Err(format!(
            "proof stream encountered {verification_failures} local verification failures which exceeds the allowed maximum ({verification_budget})"
        ));
    }

    Ok(())
}

fn write_proof_stream_evidence(
    dir: &Path,
    manifest_path: &Path,
    manifest_digest_hex: &str,
    summary_json: &str,
    torii_url: &str,
) -> Result<(), String> {
    prepare_clean_dir(dir)?;
    let summary_file_name = "proof_stream_summary.json";
    let summary_path = dir.join(summary_file_name);
    write_text(&summary_path, summary_json.as_bytes())?;

    let manifest_copy_name = manifest_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "manifest.norito".to_string());
    let manifest_copy_path = dir.join(&manifest_copy_name);
    fs::copy(manifest_path, &manifest_copy_path).map_err(|err| {
        format!(
            "failed to copy manifest `{}` into governance evidence directory `{}`: {err}",
            manifest_path.display(),
            dir.display()
        )
    })?;

    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64;

    let mut metadata = Map::new();
    metadata.insert("captured_at_unix_ms".into(), Value::from(captured_at_ms));
    metadata.insert("sorafs_cli_version".into(), Value::from(SORAFS_CLI_VERSION));
    metadata.insert("torii_url".into(), Value::from(torii_url.to_string()));
    metadata.insert(
        "manifest_source".into(),
        Value::from(manifest_path.display().to_string()),
    );
    metadata.insert("manifest_copy".into(), Value::from(manifest_copy_name));
    metadata.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_digest_hex.to_string()),
    );
    metadata.insert("summary_file".into(), Value::from(summary_file_name));
    let metadata_json = to_string_pretty(&Value::Object(metadata))
        .map_err(|err| format!("failed to render governance evidence metadata: {err}"))?;
    write_text(&dir.join("metadata.json"), metadata_json.as_bytes())?;
    Ok(())
}

fn prepare_clean_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        if !dir.is_dir() {
            return Err(format!(
                "governance evidence path `{}` must be a directory",
                dir.display()
            ));
        }
        let mut entries = fs::read_dir(dir)
            .map_err(|err| format!("failed to inspect `{}`: {err}", dir.display()))?;
        if entries.next().is_some() {
            return Err(format!(
                "governance evidence directory `{}` must be empty",
                dir.display()
            ));
        }
    } else {
        fs::create_dir_all(dir)
            .map_err(|err| format!("failed to create `{}`: {err}", dir.display()))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GovernanceDagOutputFormat {
    Table,
    Json,
}

impl GovernanceDagOutputFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unsupported governance DAG output format `{other}`; expected table|json"
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct GovernanceDagArtifact {
    path: PathBuf,
    rel_path: String,
    encoded_len: u64,
    blake3_hex: String,
    sidecar_status: String,
    sidecar_value: Option<String>,
    sidecar_error: Option<String>,
    node: Option<GovernanceDagNodeSummary>,
    decode_error: Option<String>,
    outcome: Option<ValidationOutcomeV1>,
}

#[derive(Debug, Clone)]
struct GovernanceDagNodeSummary {
    node_cid: Vec<u8>,
    node_cid_label: String,
    node_cid_hex: String,
    prev_cid: Option<Vec<u8>>,
    prev_cid_label: Option<String>,
    prev_cid_hex: Option<String>,
    timestamp: u64,
    publisher_peer_id: String,
    payload_kind: &'static str,
}

#[derive(Debug, Clone)]
struct GovernanceDagVerifyOptions {
    require_chain: bool,
    require_sidecars: bool,
    expected_head_cid: Option<Vec<u8>>,
}

fn governance_dag_list(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut format = GovernanceDagOutputFormat::Table;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--format" => format = GovernanceDagOutputFormat::parse(value)?,
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag list`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag list`".to_string()
    })?;
    let artifacts = load_governance_dag_inventory(&root)?;
    let summary = governance_dag_inventory_value(&root, &artifacts);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary)?;
    }
    match format {
        GovernanceDagOutputFormat::Json => print_governance_dag_json(&summary),
        GovernanceDagOutputFormat::Table => {
            print_governance_dag_inventory_table(&root, &artifacts);
            Ok(())
        }
    }
}

fn governance_dag_show(raw_args: Vec<String>) -> Result<(), String> {
    let mut node: Option<PathBuf> = None;
    let mut format = GovernanceDagOutputFormat::Table;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--node" => node = Some(PathBuf::from(value)),
            "--format" => format = GovernanceDagOutputFormat::parse(value)?,
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag show`"
                ));
            }
        }
    }

    let node = node.ok_or_else(|| {
        "missing required `--node=PATH` for `sorafs_cli governance dag show`".to_string()
    })?;
    let root = node.parent().unwrap_or_else(|| Path::new("."));
    let artifact = read_governance_dag_artifact(root, &node)?;
    let value = governance_dag_artifact_value(&artifact, true);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &value)?;
    }
    match format {
        GovernanceDagOutputFormat::Json => print_governance_dag_json(&value),
        GovernanceDagOutputFormat::Table => {
            print_governance_dag_artifact_table(&artifact);
            Ok(())
        }
    }
}

fn governance_dag_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut require_chain = false;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        match arg.as_str() {
            "--require-chain" => {
                require_chain = true;
                continue;
            }
            "--require-sidecars" => {
                require_sidecars = true;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag verify`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag verify`".to_string()
    })?;
    let artifacts = load_governance_dag_inventory(&root)?;
    let (ok, summary, _) = verify_governance_dag_inventory(
        &root,
        &artifacts,
        &GovernanceDagVerifyOptions {
            require_chain,
            require_sidecars,
            expected_head_cid: head_cid,
        },
    );
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary)?;
    }
    print_governance_dag_json(&summary)?;
    if ok {
        Ok(())
    } else {
        Err("governance DAG verification failed".to_string())
    }
}

fn governance_dag_export(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut require_chain = false;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        match arg.as_str() {
            "--require-chain" => {
                require_chain = true;
                continue;
            }
            "--require-sidecars" => {
                require_sidecars = true;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag export`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag export`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=DIR` for `sorafs_cli governance dag export`".to_string()
    })?;
    let artifacts = load_governance_dag_inventory(&root)?;
    let (ok, verify_summary, node_indices) = verify_governance_dag_inventory(
        &root,
        &artifacts,
        &GovernanceDagVerifyOptions {
            require_chain,
            require_sidecars,
            expected_head_cid: head_cid,
        },
    );
    if !ok {
        print_governance_dag_json(&verify_summary)?;
        return Err("governance DAG export refused invalid archive".to_string());
    }

    prepare_clean_dir(&out)?;
    let nodes_root = out.join("nodes");
    let mut exported_files = Vec::new();
    for index in node_indices {
        let artifact = &artifacts[index];
        let target = nodes_root.join(&artifact.rel_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create `{}`: {err}", parent.display()))?;
        }
        fs::copy(&artifact.path, &target).map_err(|err| {
            format!(
                "failed to copy governance node `{}` to `{}`: {err}",
                artifact.path.display(),
                target.display()
            )
        })?;
        let sidecar_path = target.with_extension("to.blake3");
        let mut sidecar = artifact.blake3_hex.clone();
        sidecar.push('\n');
        write_text(&sidecar_path, sidecar.as_bytes())?;

        let mut file = Map::new();
        file.insert(
            "path".into(),
            Value::from(format!("nodes/{}", artifact.rel_path)),
        );
        file.insert("blake3".into(), Value::from(artifact.blake3_hex.clone()));
        file.insert("encoded_len".into(), Value::from(artifact.encoded_len));
        if let Some(node) = &artifact.node {
            file.insert("node_cid".into(), Value::from(node.node_cid_label.clone()));
            file.insert("payload_kind".into(), Value::from(node.payload_kind));
        }
        exported_files.push(Value::Object(file));
    }

    let mut manifest = Map::new();
    manifest.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.export.v1"),
    );
    manifest.insert(
        "generated_at".into(),
        Value::from(governance_dag_now_secs()),
    );
    manifest.insert(
        "source_root".into(),
        Value::from(root.display().to_string()),
    );
    manifest.insert("verification".into(), verify_summary);
    manifest.insert("files".into(), Value::Array(exported_files));
    let manifest_value = Value::Object(manifest);
    write_governance_dag_json(&out.join("manifest.json"), &manifest_value)?;
    print_governance_dag_json(&manifest_value)
}

fn governance_dag_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut publisher_peer_id: Option<Vec<u8>> = None;
    let mut key_hex: Option<String> = None;
    let mut key_path: Option<PathBuf> = None;
    let mut generated_at: Option<u64> = None;
    let mut checkpoint_cid: Option<Vec<u8>> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut car_plan_out: Option<PathBuf> = None;
    let mut car_chunker_handle: Option<String> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--publisher-peer-id" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--publisher-peer-id` must not be empty".to_string());
                }
                publisher_peer_id = Some(trimmed.as_bytes().to_vec());
            }
            "--key-hex" => key_hex = Some(value.to_string()),
            "--key" => key_path = Some(PathBuf::from(value)),
            "--generated-at" => {
                generated_at = Some(parse_u64_arg(
                    "--generated-at",
                    value,
                    "sorafs_cli governance dag build",
                )?)
            }
            "--checkpoint-cid" => checkpoint_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--car-out" => car_out = Some(PathBuf::from(value)),
            "--car-plan-out" => car_plan_out = Some(PathBuf::from(value)),
            "--car-chunker-handle" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--car-chunker-handle` must not be empty".to_string());
                }
                car_chunker_handle = Some(trimmed.to_string());
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag build`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag build`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=DIR` for `sorafs_cli governance dag build`".to_string()
    })?;
    if car_out.is_none() && car_plan_out.is_some() {
        return Err(
            "`--car-plan-out=PATH` requires `--car-out=PATH` for `sorafs_cli governance dag build`"
                .to_string(),
        );
    }
    if car_out.is_none() && car_chunker_handle.is_some() {
        return Err(
            "`--car-chunker-handle=HANDLE` requires `--car-out=PATH` for `sorafs_cli governance dag build`"
                .to_string(),
        );
    }
    let publisher_peer_id = publisher_peer_id.ok_or_else(|| {
        "missing required `--publisher-peer-id=ID` for `sorafs_cli governance dag build`"
            .to_string()
    })?;
    let seed = load_governance_dag_build_seed(key_hex.as_deref(), key_path.as_deref())?;
    let signing_key = SigningKey::from_bytes(&seed);
    let generated_at = generated_at.unwrap_or_else(governance_dag_now_secs);

    let artifacts = load_governance_dag_inventory(&root)?;
    let (ok, verify_summary, node_indices) = verify_governance_dag_inventory(
        &root,
        &artifacts,
        &GovernanceDagVerifyOptions {
            require_chain: false,
            require_sidecars,
            expected_head_cid: None,
        },
    );
    if !ok {
        print_governance_dag_json(&verify_summary)?;
        return Err("governance DAG build refused invalid node archive".to_string());
    }

    let ordered_indices = governance_dag_build_order(&artifacts, &node_indices);
    prepare_clean_dir(&out)?;
    let blocks_dir = out.join("blocks");
    fs::create_dir_all(&blocks_dir)
        .map_err(|err| format!("failed to create `{}`: {err}", blocks_dir.display()))?;

    let mut blocks = Vec::<GovernanceDagBlockV1>::with_capacity(ordered_indices.len());
    let mut block_files = Vec::<Value>::new();
    let mut car_files = Vec::<FileEntry>::new();
    let mut prev_block_cid: Option<Vec<u8>> = None;
    for (sequence_usize, index) in ordered_indices.iter().enumerate() {
        let artifact = &artifacts[*index];
        let node = read_governance_log_node_file(&artifact.path)?;
        let sequence = u64::try_from(sequence_usize).unwrap_or(u64::MAX);
        let timestamp = node.timestamp;
        let block_cid = governance_dag_block_cid_v1(
            prev_block_cid.as_deref(),
            sequence,
            timestamp,
            &publisher_peer_id,
            &node,
        )
        .map_err(|err| format!("failed to derive governance DAG block CID: {err}"))?;
        let mut block = GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid,
            prev_block_cid: prev_block_cid.clone(),
            sequence,
            timestamp,
            publisher_peer_id: publisher_peer_id.clone(),
            node,
            block_signature: empty_governance_dag_ed25519_signature(),
        };
        sign_governance_dag_block_cli(&mut block, &signing_key)?;

        let block_bytes = to_bytes(&block)
            .map_err(|err| format!("failed to encode governance DAG block: {err}"))?;
        let block_file_name = format!("{sequence:020}-{}.to", hex_encode(&block.block_cid));
        let block_rel_path = format!("blocks/{block_file_name}");
        let block_path = blocks_dir.join(&block_file_name);
        write_text(&block_path, &block_bytes)?;
        let block_sidecar_bytes = write_governance_blake3_sidecar(&block_path, &block_bytes)?;
        car_files.push(governance_dag_car_file(
            &block_rel_path,
            block_bytes.clone(),
        )?);
        car_files.push(governance_dag_car_file(
            &format!("{block_rel_path}.blake3"),
            block_sidecar_bytes,
        )?);

        let mut block_value = Map::new();
        block_value.insert("path".into(), Value::from(block_rel_path));
        block_value.insert("sequence".into(), Value::from(sequence));
        block_value.insert(
            "block_cid_hex".into(),
            Value::from(hex_encode(&block.block_cid)),
        );
        block_value.insert(
            "prev_block_cid_hex".into(),
            block
                .prev_block_cid
                .as_ref()
                .map(hex_encode)
                .map_or(Value::Null, Value::from),
        );
        block_value.insert(
            "source_node_path".into(),
            Value::from(artifact.rel_path.clone()),
        );
        if let Some(node) = &artifact.node {
            block_value.insert("node_cid".into(), Value::from(node.node_cid_label.clone()));
            block_value.insert(
                "node_cid_hex".into(),
                Value::from(node.node_cid_hex.clone()),
            );
            block_value.insert("payload_kind".into(), Value::from(node.payload_kind));
        }
        block_value.insert(
            "encoded_blake3_hex".into(),
            Value::from(hex_encode(blake3_hash(&block_bytes).as_bytes())),
        );
        block_files.push(Value::Object(block_value));

        prev_block_cid = Some(block.block_cid.clone());
        blocks.push(block);
    }

    let head_block_cid = prev_block_cid.ok_or_else(|| {
        "governance DAG build found no validated governance nodes to build".to_string()
    })?;
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: head_block_cid.clone(),
        block_count: blocks.len() as u64,
        generated_at,
        publisher_peer_id: publisher_peer_id.clone(),
        checkpoint_cid,
        head_signature: empty_governance_dag_ed25519_signature(),
    };
    sign_governance_dag_head_cli(&mut head, &signing_key)?;
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .map_err(|err| format!("built governance DAG head failed validation: {err}"))?;

    let head_bytes =
        to_bytes(&head).map_err(|err| format!("failed to encode governance DAG head: {err}"))?;
    let head_path = out.join("head.to");
    write_text(&head_path, &head_bytes)?;
    let head_sidecar_bytes = write_governance_blake3_sidecar(&head_path, &head_bytes)?;
    car_files.push(governance_dag_car_file("head.to", head_bytes.clone())?);
    car_files.push(governance_dag_car_file(
        "head.to.blake3",
        head_sidecar_bytes,
    )?);

    let car_archive_summary = if let Some(car_path) = car_out.as_deref() {
        let handle = car_chunker_handle
            .as_deref()
            .unwrap_or(DEFAULT_CHUNKER_HANDLE);
        Some(write_governance_dag_car_archive(
            &out,
            car_path,
            car_plan_out.as_deref(),
            handle,
            car_files,
        )?)
    } else {
        None
    };

    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.build.v1"),
    );
    summary.insert(
        "source_root".into(),
        Value::from(root.display().to_string()),
    );
    summary.insert("output_root".into(), Value::from(out.display().to_string()));
    summary.insert("generated_at".into(), Value::from(generated_at));
    summary.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&publisher_peer_id).to_string()),
    );
    summary.insert(
        "publisher_public_key_hex".into(),
        Value::from(hex_encode(signing_key.verifying_key().to_bytes())),
    );
    summary.insert("block_count".into(), Value::from(blocks.len() as u64));
    summary.insert(
        "head_block_cid_hex".into(),
        Value::from(hex_encode(&head_block_cid)),
    );
    summary.insert("head_path".into(), Value::from("head.to"));
    summary.insert(
        "head_blake3_hex".into(),
        Value::from(hex_encode(blake3_hash(&head_bytes).as_bytes())),
    );
    if let Some(checkpoint) = &head.checkpoint_cid {
        summary.insert(
            "checkpoint_cid_hex".into(),
            Value::from(hex_encode(checkpoint)),
        );
    }
    summary.insert("blocks".into(), Value::Array(block_files));
    if let Some(car_summary) = car_archive_summary {
        summary.insert("car_archive".into(), car_summary);
    }
    summary.insert("input_verification".into(), verify_summary);
    let summary_value = Value::Object(summary);
    write_governance_dag_json(&out.join("manifest.json"), &summary_value)?;
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)
}

fn governance_dag_verify_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag verify-build`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag verify-build`".to_string()
    })?;
    let (ok, summary) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, head_cid.as_deref());
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary)?;
    }
    print_governance_dag_json(&summary)?;
    if ok {
        Ok(())
    } else {
        Err("governance DAG build verification failed".to_string())
    }
}

fn governance_dag_rebuild_head(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut head_out: Option<PathBuf> = None;
    let mut publisher_peer_id: Option<Vec<u8>> = None;
    let mut key_hex: Option<String> = None;
    let mut key_path: Option<PathBuf> = None;
    let mut generated_at: Option<u64> = None;
    let mut checkpoint_cid: Option<Vec<u8>> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--head-out" => head_out = Some(PathBuf::from(value)),
            "--publisher-peer-id" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--publisher-peer-id` must not be empty".to_string());
                }
                publisher_peer_id = Some(trimmed.as_bytes().to_vec());
            }
            "--key-hex" => key_hex = Some(value.to_string()),
            "--key" => key_path = Some(PathBuf::from(value)),
            "--generated-at" => {
                generated_at = Some(parse_u64_arg(
                    "--generated-at",
                    value,
                    "sorafs_cli governance dag rebuild-head",
                )?)
            }
            "--checkpoint-cid" => checkpoint_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag rebuild-head`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag rebuild-head`".to_string()
    })?;
    let head_out = head_out.ok_or_else(|| {
        "missing required `--head-out=PATH` for `sorafs_cli governance dag rebuild-head`"
            .to_string()
    })?;
    let publisher_peer_id = publisher_peer_id.ok_or_else(|| {
        "missing required `--publisher-peer-id=ID` for `sorafs_cli governance dag rebuild-head`"
            .to_string()
    })?;
    let seed = load_governance_dag_build_seed(key_hex.as_deref(), key_path.as_deref())?;
    let signing_key = SigningKey::from_bytes(&seed);
    let generated_at = generated_at.unwrap_or_else(governance_dag_now_secs);

    let (blocks, block_records, warnings) =
        load_governance_dag_block_snapshot(&root, require_sidecars)?;
    let head_block_cid = governance_dag_head_cid_from_blocks(&blocks)?;
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: head_block_cid.clone(),
        block_count: blocks.len() as u64,
        generated_at,
        publisher_peer_id: publisher_peer_id.clone(),
        checkpoint_cid,
        head_signature: empty_governance_dag_ed25519_signature(),
    };
    sign_governance_dag_head_cli(&mut head, &signing_key)?;
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .map_err(|err| format!("rebuilt governance DAG head failed validation: {err}"))?;

    let head_bytes = to_bytes(&head)
        .map_err(|err| format!("failed to encode rebuilt governance DAG head: {err}"))?;
    ensure_parent_dir(&head_out)?;
    write_text(&head_out, &head_bytes)?;
    write_governance_blake3_sidecar(&head_out, &head_bytes)?;

    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.head.rebuild.v1"),
    );
    summary.insert(
        "source_root".into(),
        Value::from(root.display().to_string()),
    );
    summary.insert(
        "head_path".into(),
        Value::from(head_out.display().to_string()),
    );
    summary.insert("generated_at".into(), Value::from(generated_at));
    summary.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&publisher_peer_id).to_string()),
    );
    summary.insert(
        "publisher_public_key_hex".into(),
        Value::from(hex_encode(signing_key.verifying_key().to_bytes())),
    );
    summary.insert("block_count".into(), Value::from(blocks.len() as u64));
    summary.insert(
        "head_block_cid".into(),
        Value::from(cid_display(&head_block_cid)),
    );
    summary.insert(
        "head_block_cid_hex".into(),
        Value::from(hex_encode(&head_block_cid)),
    );
    summary.insert(
        "head_blake3_hex".into(),
        Value::from(hex_encode(blake3_hash(&head_bytes).as_bytes())),
    );
    if let Some(checkpoint) = &head.checkpoint_cid {
        summary.insert(
            "checkpoint_cid_hex".into(),
            Value::from(hex_encode(checkpoint)),
        );
    }
    summary.insert("blocks".into(), Value::Array(block_records));
    summary.insert("warnings".into(), Value::Array(warnings));
    let summary_value = Value::Object(summary);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)
}

fn governance_dag_checkpoint(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut car_path: Option<PathBuf> = None;
    let mut mirror_index_path: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    let mut generated_at: Option<u64> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--car" => car_path = Some(PathBuf::from(value)),
            "--mirror-index" => mirror_index_path = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            "--generated-at" => {
                generated_at = Some(parse_u64_arg(
                    "--generated-at",
                    value,
                    "sorafs_cli governance dag checkpoint",
                )?)
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag checkpoint`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag checkpoint`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=PATH` for `sorafs_cli governance dag checkpoint`".to_string()
    })?;

    let (ok, verification) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, head_cid.as_deref());
    if !ok {
        print_governance_dag_json(&verification)?;
        return Err("governance DAG checkpoint refused invalid build snapshot".to_string());
    }

    let head_path = root.join("head.to");
    let (head_bytes, head_len, head_blake3_hex) =
        governance_dag_read_digest_file(&head_path, "governance DAG checkpoint head")?;
    let head = decode_from_bytes::<GovernanceDagHeadV1>(&head_bytes).map_err(|err| {
        format!(
            "failed to decode governance DAG checkpoint head `{}`: {err}",
            head_path.display()
        )
    })?;

    let mut head_value = Map::new();
    head_value.insert("path".into(), Value::from("head.to"));
    head_value.insert(
        "source_path".into(),
        Value::from(head_path.display().to_string()),
    );
    head_value.insert("encoded_len".into(), Value::from(head_len));
    head_value.insert("blake3".into(), Value::from(head_blake3_hex));
    head_value.insert(
        "head_block_cid".into(),
        Value::from(cid_display(&head.head_block_cid)),
    );
    head_value.insert(
        "head_block_cid_hex".into(),
        Value::from(hex_encode(&head.head_block_cid)),
    );
    head_value.insert("block_count".into(), Value::from(head.block_count));
    head_value.insert("generated_at".into(), Value::from(head.generated_at));
    head_value.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&head.publisher_peer_id).to_string()),
    );
    head_value.insert(
        "checkpoint_cid_hex".into(),
        head.checkpoint_cid
            .as_ref()
            .map(hex_encode)
            .map_or(Value::Null, Value::from),
    );

    let car_value = if let Some(path) = car_path.as_deref() {
        let (_, encoded_len, blake3_hex) =
            governance_dag_read_digest_file(path, "governance DAG checkpoint CAR")?;
        let mut value = Map::new();
        value.insert("path".into(), Value::from(path.display().to_string()));
        value.insert("encoded_len".into(), Value::from(encoded_len));
        value.insert("car_size".into(), Value::from(encoded_len));
        value.insert("blake3".into(), Value::from(blake3_hex));
        Some(Value::Object(value))
    } else {
        None
    };

    let mirror_index_value = if let Some(path) = mirror_index_path.as_deref() {
        let (bytes, encoded_len, blake3_hex) =
            governance_dag_read_digest_file(path, "governance DAG checkpoint mirror index")?;
        let index: Value = from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse governance DAG checkpoint mirror index `{}` as JSON: {err}",
                path.display()
            )
        })?;
        if index.get("schema").and_then(Value::as_str) != Some("sorafs.governance_dag.mirror.v1") {
            return Err(format!(
                "governance DAG checkpoint mirror index `{}` has unsupported schema",
                path.display()
            ));
        }
        let index_head_cid = index
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "governance DAG checkpoint mirror index `{}` is missing `head.head_block_cid_hex`",
                    path.display()
                )
            })?;
        let snapshot_head_cid = hex_encode(&head.head_block_cid);
        if index_head_cid != snapshot_head_cid {
            return Err(format!(
                "governance DAG checkpoint mirror index `{}` advertises head `{index_head_cid}` but snapshot advertises `{snapshot_head_cid}`",
                path.display()
            ));
        }
        let index_block_count = index
            .get("block_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "governance DAG checkpoint mirror index `{}` is missing `block_count`",
                    path.display()
                )
            })?;
        if index_block_count != head.block_count {
            return Err(format!(
                "governance DAG checkpoint mirror index `{}` advertises block_count `{index_block_count}` but head advertises `{}`",
                path.display(),
                head.block_count
            ));
        }

        let mut value = Map::new();
        value.insert("path".into(), Value::from(path.display().to_string()));
        value.insert("encoded_len".into(), Value::from(encoded_len));
        value.insert("blake3".into(), Value::from(blake3_hex));
        value.insert(
            "schema".into(),
            Value::from("sorafs.governance_dag.mirror.v1"),
        );
        value.insert("block_count".into(), Value::from(index_block_count));
        value.insert(
            "head_block_cid_hex".into(),
            Value::from(index_head_cid.to_string()),
        );
        Some(Value::Object(value))
    } else {
        None
    };

    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.checkpoint.v1"),
    );
    summary.insert(
        "source_root".into(),
        Value::from(root.display().to_string()),
    );
    summary.insert("output_path".into(), Value::from(out.display().to_string()));
    summary.insert(
        "generated_at".into(),
        Value::from(generated_at.unwrap_or_else(governance_dag_now_secs)),
    );
    summary.insert("require_sidecars".into(), Value::from(require_sidecars));
    summary.insert(
        "expected_head_cid".into(),
        head_cid
            .as_ref()
            .map(|cid| cid_display(cid))
            .map_or(Value::Null, Value::from),
    );
    summary.insert(
        "expected_head_cid_hex".into(),
        head_cid
            .as_ref()
            .map(hex_encode)
            .map_or(Value::Null, Value::from),
    );
    summary.insert("head".into(), Value::Object(head_value));
    summary.insert("block_count".into(), Value::from(head.block_count));
    if let Some(blocks) = verification.get("blocks").cloned() {
        summary.insert("blocks".into(), blocks);
    }
    if let Some(value) = car_value {
        summary.insert("car_archive".into(), value);
    }
    if let Some(value) = mirror_index_value {
        summary.insert("mirror_index".into(), value);
    }
    summary.insert("verification".into(), verification);
    let summary_value = Value::Object(summary);
    ensure_parent_dir(&out)?;
    write_governance_dag_json(&out, &summary_value)?;
    print_governance_dag_json(&summary_value)
}

fn governance_dag_checkpoint_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut checkpoint_path: Option<PathBuf> = None;
    let mut root_override: Option<PathBuf> = None;
    let mut car_override: Option<PathBuf> = None;
    let mut mirror_index_override: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--checkpoint" => checkpoint_path = Some(PathBuf::from(value)),
            "--root" => root_override = Some(PathBuf::from(value)),
            "--car" => car_override = Some(PathBuf::from(value)),
            "--mirror-index" => mirror_index_override = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag checkpoint-verify`"
                ));
            }
        }
    }

    let checkpoint_path = checkpoint_path.ok_or_else(|| {
        "missing required `--checkpoint=PATH` for `sorafs_cli governance dag checkpoint-verify`"
            .to_string()
    })?;
    let (checkpoint_bytes, checkpoint_len, checkpoint_blake3_hex) =
        governance_dag_read_digest_file(&checkpoint_path, "governance DAG checkpoint manifest")?;
    let checkpoint: Value = from_slice(&checkpoint_bytes).map_err(|err| {
        format!(
            "failed to parse governance DAG checkpoint `{}` as JSON: {err}",
            checkpoint_path.display()
        )
    })?;

    let mut errors = Vec::<Value>::new();
    if checkpoint.get("schema").and_then(Value::as_str)
        != Some("sorafs.governance_dag.checkpoint.v1")
    {
        errors.push(governance_dag_problem(
            checkpoint_path.to_string_lossy().as_ref(),
            "schema",
            "checkpoint manifest has unsupported schema",
        ));
    }

    let mut checkpoint_file_value = Map::new();
    checkpoint_file_value.insert(
        "path".into(),
        Value::from(checkpoint_path.display().to_string()),
    );
    checkpoint_file_value.insert("encoded_len".into(), Value::from(checkpoint_len));
    checkpoint_file_value.insert("blake3".into(), Value::from(checkpoint_blake3_hex));

    let expected_head_cid_hex = checkpoint
        .get("head")
        .and_then(|head| head.get("head_block_cid_hex"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let expected_head_cid = match expected_head_cid_hex.as_deref() {
        Some(value) => match hex::decode(value) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                errors.push(governance_dag_problem(
                    checkpoint_path.to_string_lossy().as_ref(),
                    "head_cid",
                    format!("checkpoint head_block_cid_hex is not valid hex: {err}"),
                ));
                None
            }
        },
        None => {
            errors.push(governance_dag_problem(
                checkpoint_path.to_string_lossy().as_ref(),
                "head_cid",
                "checkpoint manifest is missing `head.head_block_cid_hex`",
            ));
            None
        }
    };

    let root_path = match root_override {
        Some(path) => Some(path),
        None => checkpoint
            .get("source_root")
            .and_then(Value::as_str)
            .map(PathBuf::from),
    };
    let mut root_value = Map::new();
    let mut head_check = Value::Null;
    let mut root_verification = Value::Null;
    if let Some(root) = root_path.as_deref() {
        root_value.insert("path".into(), Value::from(root.display().to_string()));
        let (root_ok, verification) = verify_governance_dag_build_snapshot(
            root,
            require_sidecars,
            expected_head_cid.as_deref(),
        );
        root_value.insert("ok".into(), Value::from(root_ok));
        if !root_ok {
            errors.push(governance_dag_problem(
                root.to_string_lossy().as_ref(),
                "snapshot",
                "checkpoint root snapshot verification failed",
            ));
        }
        root_verification = verification;
        if let Some(expected_head) = checkpoint.get("head") {
            let head_path = root.join("head.to");
            head_check = governance_dag_checkpoint_file_check(
                "head",
                &head_path,
                expected_head,
                &["encoded_len"],
                &mut errors,
            );
        }
    } else {
        errors.push(governance_dag_problem(
            checkpoint_path.to_string_lossy().as_ref(),
            "source_root",
            "checkpoint verification requires `--root=DIR` or a checkpoint `source_root`",
        ));
    }
    root_value.insert("verification".into(), root_verification);

    let car_check = governance_dag_checkpoint_optional_artifact_check(
        &checkpoint,
        car_override.as_deref(),
        "car_archive",
        "governance DAG checkpoint CAR",
        &["encoded_len", "car_size"],
        &mut errors,
    )?;
    let mirror_check = governance_dag_checkpoint_optional_artifact_check(
        &checkpoint,
        mirror_index_override.as_deref(),
        "mirror_index",
        "governance DAG checkpoint mirror index",
        &["encoded_len"],
        &mut errors,
    )?;
    if let Some((path, value)) = mirror_check.as_ref() {
        governance_dag_checkpoint_validate_mirror_index(
            path,
            value,
            expected_head_cid_hex.as_deref(),
            checkpoint.get("block_count").and_then(Value::as_u64),
            &mut errors,
        );
    }

    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.checkpoint.verify.v1"),
    );
    summary.insert("ok".into(), Value::from(errors.is_empty()));
    summary.insert("require_sidecars".into(), Value::from(require_sidecars));
    summary.insert("checkpoint".into(), Value::Object(checkpoint_file_value));
    summary.insert(
        "expected_head_cid_hex".into(),
        expected_head_cid_hex.map_or(Value::Null, Value::from),
    );
    summary.insert("root".into(), Value::Object(root_value));
    summary.insert("head".into(), head_check);
    if let Some((_, value)) = car_check {
        summary.insert("car_archive".into(), value);
    }
    if let Some((_, value)) = mirror_check {
        summary.insert("mirror_index".into(), value);
    }
    summary.insert("errors".into(), Value::Array(errors.clone()));
    let summary_value = Value::Object(summary);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err("governance DAG checkpoint verification failed".to_string())
    }
}

fn governance_dag_checkpoint_recover(raw_args: Vec<String>) -> Result<(), String> {
    let mut checkpoint_path: Option<PathBuf> = None;
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut car_path: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--checkpoint" => checkpoint_path = Some(PathBuf::from(value)),
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--car" => car_path = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag checkpoint-recover`"
                ));
            }
        }
    }

    let checkpoint_path = checkpoint_path.ok_or_else(|| {
        "missing required `--checkpoint=PATH` for `sorafs_cli governance dag checkpoint-recover`"
            .to_string()
    })?;
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag checkpoint-recover`"
            .to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=PATH` for `sorafs_cli governance dag checkpoint-recover`"
            .to_string()
    })?;

    let (checkpoint_bytes, checkpoint_len, checkpoint_blake3_hex) =
        governance_dag_read_digest_file(&checkpoint_path, "governance DAG checkpoint manifest")?;
    let checkpoint: Value = from_slice(&checkpoint_bytes).map_err(|err| {
        format!(
            "failed to parse governance DAG checkpoint `{}` as JSON: {err}",
            checkpoint_path.display()
        )
    })?;

    let mut errors = Vec::<Value>::new();
    if checkpoint.get("schema").and_then(Value::as_str)
        != Some("sorafs.governance_dag.checkpoint.v1")
    {
        errors.push(governance_dag_problem(
            checkpoint_path.to_string_lossy().as_ref(),
            "schema",
            "checkpoint manifest has unsupported schema",
        ));
    }

    let mut checkpoint_file_value = Map::new();
    checkpoint_file_value.insert(
        "path".into(),
        Value::from(checkpoint_path.display().to_string()),
    );
    checkpoint_file_value.insert("encoded_len".into(), Value::from(checkpoint_len));
    checkpoint_file_value.insert("blake3".into(), Value::from(checkpoint_blake3_hex));

    let expected_head_cid_hex = checkpoint
        .get("head")
        .and_then(|head| head.get("head_block_cid_hex"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let expected_head_cid = match expected_head_cid_hex.as_deref() {
        Some(value) => match hex::decode(value) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                errors.push(governance_dag_problem(
                    checkpoint_path.to_string_lossy().as_ref(),
                    "head_cid",
                    format!("checkpoint head_block_cid_hex is not valid hex: {err}"),
                ));
                None
            }
        },
        None => {
            errors.push(governance_dag_problem(
                checkpoint_path.to_string_lossy().as_ref(),
                "head_cid",
                "checkpoint manifest is missing `head.head_block_cid_hex`",
            ));
            None
        }
    };

    let (root_ok, root_verification) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, expected_head_cid.as_deref());
    if !root_ok {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "snapshot",
            "checkpoint recovery root snapshot verification failed",
        ));
    }
    let head_check = if let Some(expected_head) = checkpoint.get("head") {
        governance_dag_checkpoint_file_check(
            "head",
            &root.join("head.to"),
            expected_head,
            &["encoded_len"],
            &mut errors,
        )
    } else {
        Value::Null
    };
    let car_check = governance_dag_checkpoint_optional_artifact_check(
        &checkpoint,
        car_path.as_deref(),
        "car_archive",
        "governance DAG checkpoint CAR",
        &["encoded_len", "car_size"],
        &mut errors,
    )?;

    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.checkpoint.recover.v1"),
    );
    summary.insert("require_sidecars".into(), Value::from(require_sidecars));
    summary.insert("checkpoint".into(), Value::Object(checkpoint_file_value));
    summary.insert(
        "expected_head_cid_hex".into(),
        expected_head_cid_hex
            .clone()
            .map_or(Value::Null, Value::from),
    );
    let mut root_value = Map::new();
    root_value.insert("path".into(), Value::from(root.display().to_string()));
    root_value.insert("ok".into(), Value::from(root_ok));
    root_value.insert("verification".into(), root_verification);
    summary.insert("root".into(), Value::Object(root_value));
    summary.insert("head".into(), head_check);
    if let Some((_, value)) = car_check {
        summary.insert("car_archive".into(), value);
    }

    if errors.is_empty() {
        let index = governance_dag_mirror_index_value(
            &root,
            require_sidecars,
            expected_head_cid.as_deref(),
        )?;
        write_governance_dag_json(&out, &index)?;
        let (index_bytes, encoded_len, blake3_hex) =
            governance_dag_read_digest_file(&out, "recovered governance DAG mirror index")?;
        let index_value: Value = from_slice(&index_bytes).map_err(|err| {
            format!(
                "failed to parse recovered governance DAG mirror index `{}` as JSON: {err}",
                out.display()
            )
        })?;
        let mut recovered = Map::new();
        recovered.insert("path".into(), Value::from(out.display().to_string()));
        recovered.insert("encoded_len".into(), Value::from(encoded_len));
        recovered.insert("blake3".into(), Value::from(blake3_hex));
        recovered.insert(
            "schema".into(),
            Value::from("sorafs.governance_dag.mirror.v1"),
        );
        recovered.insert(
            "head_block_cid_hex".into(),
            index_value
                .get("head")
                .and_then(|head| head.get("head_block_cid_hex"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        recovered.insert(
            "block_count".into(),
            index_value
                .get("block_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        summary.insert("recovered_mirror_index".into(), Value::Object(recovered));
    }

    summary.insert("ok".into(), Value::from(errors.is_empty()));
    summary.insert("errors".into(), Value::Array(errors.clone()));
    let summary_value = Value::Object(summary);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err("governance DAG checkpoint recovery failed".to_string())
    }
}

fn governance_dag_mirror_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag mirror-build`"
                ));
            }
        }
    }

    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag mirror-build`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=PATH` for `sorafs_cli governance dag mirror-build`".to_string()
    })?;
    let (ok, verification) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, head_cid.as_deref());
    if !ok {
        print_governance_dag_json(&verification)?;
        return Err("governance DAG mirror index refused invalid build snapshot".to_string());
    }

    let index = governance_dag_mirror_index_value(&root, require_sidecars, head_cid.as_deref())?;
    write_governance_dag_json(&out, &index)?;
    print_governance_dag_json(&index)
}

enum GovernanceDagMirrorQuery {
    Head,
    BlockCid(Vec<u8>),
    NodeCid(Vec<u8>),
}

fn governance_dag_mirror_query(raw_args: Vec<String>) -> Result<(), String> {
    let mut index_path: Option<PathBuf> = None;
    let mut query: Option<GovernanceDagMirrorQuery> = None;
    let mut format = GovernanceDagOutputFormat::Table;

    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--head" {
            set_governance_dag_mirror_query(&mut query, GovernanceDagMirrorQuery::Head)?;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--index" => index_path = Some(PathBuf::from(value)),
            "--block-cid" => set_governance_dag_mirror_query(
                &mut query,
                GovernanceDagMirrorQuery::BlockCid(parse_governance_cid_arg(value)?),
            )?,
            "--node-cid" => set_governance_dag_mirror_query(
                &mut query,
                GovernanceDagMirrorQuery::NodeCid(parse_governance_cid_arg(value)?),
            )?,
            "--format" => format = GovernanceDagOutputFormat::parse(value)?,
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag mirror-query`"
                ));
            }
        }
    }

    let index_path = index_path.ok_or_else(|| {
        "missing required `--index=PATH` for `sorafs_cli governance dag mirror-query`".to_string()
    })?;
    let query = query.ok_or_else(|| {
        "missing mirror query selector; provide `--head`, `--block-cid=...`, or `--node-cid=...`"
            .to_string()
    })?;
    let index = read_governance_dag_json_file(&index_path)?;
    if index.get("schema").and_then(Value::as_str) != Some("sorafs.governance_dag.mirror.v1") {
        return Err(format!(
            "governance DAG mirror index `{}` has unsupported schema",
            index_path.display()
        ));
    }
    let (found, query_value) = governance_dag_mirror_query_value(&index_path, &index, &query)?;
    match format {
        GovernanceDagOutputFormat::Json => print_governance_dag_json(&query_value)?,
        GovernanceDagOutputFormat::Table => print_governance_dag_mirror_query_table(&query_value),
    }
    if found {
        Ok(())
    } else {
        Err("governance DAG mirror query returned no match".to_string())
    }
}

fn set_governance_dag_mirror_query(
    slot: &mut Option<GovernanceDagMirrorQuery>,
    value: GovernanceDagMirrorQuery,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(
            "governance DAG mirror query accepts exactly one of `--head`, `--block-cid`, or `--node-cid`"
                .to_string(),
        );
    }
    *slot = Some(value);
    Ok(())
}

fn governance_dag_mirror_index_value(
    root: &Path,
    require_sidecars: bool,
    expected_head_cid: Option<&[u8]>,
) -> Result<Value, String> {
    let head_path = root.join("head.to");
    let head_bytes = fs::read(&head_path).map_err(|err| {
        format!(
            "failed to read governance DAG mirror head `{}`: {err}",
            head_path.display()
        )
    })?;
    let head_blake3_hex = hex_encode(blake3_hash(&head_bytes).as_bytes());
    let head = decode_from_bytes::<GovernanceDagHeadV1>(&head_bytes).map_err(|err| {
        format!(
            "failed to decode governance DAG mirror head `{}`: {err}",
            head_path.display()
        )
    })?;

    let blocks_dir = root.join("blocks");
    let mut block_paths = Vec::<PathBuf>::new();
    collect_governance_dag_to_files(&blocks_dir, &mut block_paths)?;
    block_paths.sort();
    let mut blocks = Vec::<(String, String, String, GovernanceDagBlockV1)>::new();
    for path in block_paths {
        let rel_path = governance_dag_relative_path(root, &path);
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "failed to read governance DAG mirror block `{}`: {err}",
                path.display()
            )
        })?;
        let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
        let (sidecar_status, _, sidecar_error) = governance_dag_sidecar_status(&path, &blake3_hex);
        match sidecar_status.as_str() {
            "mismatch" | "error" => {
                return Err(format!(
                    "governance DAG mirror block `{rel_path}` has invalid sidecar status `{sidecar_status}`{}",
                    sidecar_error
                        .as_deref()
                        .map(|err| format!(": {err}"))
                        .unwrap_or_default()
                ));
            }
            "missing" if require_sidecars => {
                return Err(format!(
                    "governance DAG mirror block `{rel_path}` is missing required sidecar"
                ));
            }
            _ => {}
        }
        let block = decode_from_bytes::<GovernanceDagBlockV1>(&bytes).map_err(|err| {
            format!(
                "failed to decode governance DAG mirror block `{}`: {err}",
                path.display()
            )
        })?;
        blocks.push((rel_path, blake3_hex, sidecar_status, block));
    }
    blocks.sort_by(|left, right| {
        left.3
            .sequence
            .cmp(&right.3.sequence)
            .then_with(|| left.0.cmp(&right.0))
    });
    let decoded_blocks = blocks
        .iter()
        .map(|(_, _, _, block)| block.clone())
        .collect::<Vec<_>>();
    validate_governance_dag_head_against_chain_v1(&head, &decoded_blocks)
        .map_err(|err| format!("governance DAG mirror snapshot failed validation: {err}"))?;
    if let Some(expected) = expected_head_cid
        && head.head_block_cid != expected
    {
        return Err(format!(
            "governance DAG mirror expected head CID `{}` but snapshot advertises `{}`",
            cid_display(expected),
            cid_display(&head.head_block_cid)
        ));
    }

    let mut by_block_cid_hex = Map::new();
    let mut by_node_cid_hex = Map::new();
    let mut block_values = Vec::<Value>::new();
    for (position, (path, blake3_hex, sidecar_status, block)) in blocks.iter().enumerate() {
        let block_cid_hex = hex_encode(&block.block_cid);
        let node_cid_hex = hex_encode(&block.node.node_cid);
        by_block_cid_hex.insert(block_cid_hex.clone(), Value::from(position as u64));
        by_node_cid_hex.insert(node_cid_hex.clone(), Value::from(position as u64));

        let mut block_value = Map::new();
        block_value.insert("position".into(), Value::from(position as u64));
        block_value.insert("path".into(), Value::from(path.clone()));
        block_value.insert("sequence".into(), Value::from(block.sequence));
        block_value.insert("timestamp".into(), Value::from(block.timestamp));
        block_value.insert(
            "publisher_peer_id".into(),
            Value::from(String::from_utf8_lossy(&block.publisher_peer_id).to_string()),
        );
        block_value.insert(
            "block_cid".into(),
            Value::from(cid_display(&block.block_cid)),
        );
        block_value.insert("block_cid_hex".into(), Value::from(block_cid_hex));
        block_value.insert(
            "prev_block_cid_hex".into(),
            block
                .prev_block_cid
                .as_ref()
                .map(hex_encode)
                .map_or(Value::Null, Value::from),
        );
        block_value.insert(
            "node_cid".into(),
            Value::from(cid_display(&block.node.node_cid)),
        );
        block_value.insert("node_cid_hex".into(), Value::from(node_cid_hex));
        block_value.insert(
            "payload_kind".into(),
            Value::from(governance_payload_kind_cli(&block.node.payload)),
        );
        block_value.insert("blake3".into(), Value::from(blake3_hex.clone()));
        block_value.insert("sidecar_status".into(), Value::from(sidecar_status.clone()));
        block_values.push(Value::Object(block_value));
    }

    let mut head_value = Map::new();
    head_value.insert("path".into(), Value::from("head.to"));
    head_value.insert(
        "head_block_cid".into(),
        Value::from(cid_display(&head.head_block_cid)),
    );
    head_value.insert(
        "head_block_cid_hex".into(),
        Value::from(hex_encode(&head.head_block_cid)),
    );
    head_value.insert("block_count".into(), Value::from(head.block_count));
    head_value.insert("generated_at".into(), Value::from(head.generated_at));
    head_value.insert(
        "publisher_peer_id".into(),
        Value::from(String::from_utf8_lossy(&head.publisher_peer_id).to_string()),
    );
    head_value.insert("blake3".into(), Value::from(head_blake3_hex));
    head_value.insert(
        "checkpoint_cid_hex".into(),
        head.checkpoint_cid
            .as_ref()
            .map(hex_encode)
            .map_or(Value::Null, Value::from),
    );

    let mut root_value = Map::new();
    root_value.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.mirror.v1"),
    );
    root_value.insert(
        "source_root".into(),
        Value::from(root.display().to_string()),
    );
    root_value.insert(
        "generated_at".into(),
        Value::from(governance_dag_now_secs()),
    );
    root_value.insert("require_sidecars".into(), Value::from(require_sidecars));
    root_value.insert("head".into(), Value::Object(head_value));
    root_value.insert("block_count".into(), Value::from(block_values.len() as u64));
    root_value.insert("blocks".into(), Value::Array(block_values));
    root_value.insert("by_block_cid_hex".into(), Value::Object(by_block_cid_hex));
    root_value.insert("by_node_cid_hex".into(), Value::Object(by_node_cid_hex));
    Ok(Value::Object(root_value))
}

fn governance_dag_mirror_query_value(
    index_path: &Path,
    index: &Value,
    query: &GovernanceDagMirrorQuery,
) -> Result<(bool, Value), String> {
    let mut result = Map::new();
    result.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.mirror.query.v1"),
    );
    result.insert(
        "index".into(),
        Value::from(index_path.display().to_string()),
    );

    match query {
        GovernanceDagMirrorQuery::Head => {
            result.insert("query".into(), Value::from("head"));
            let head = index.get("head").cloned().unwrap_or(Value::Null);
            let found = !matches!(head, Value::Null);
            result.insert("found".into(), Value::from(found));
            result.insert("head".into(), head);
            Ok((found, Value::Object(result)))
        }
        GovernanceDagMirrorQuery::BlockCid(cid) => {
            let cid_hex = hex_encode(cid);
            result.insert("query".into(), Value::from("block_cid"));
            result.insert("cid".into(), Value::from(cid_display(cid)));
            result.insert("cid_hex".into(), Value::from(cid_hex.clone()));
            let block = governance_dag_mirror_lookup_block(index, "by_block_cid_hex", &cid_hex)?;
            let found = !matches!(block, Value::Null);
            result.insert("found".into(), Value::from(found));
            result.insert("block".into(), block);
            Ok((found, Value::Object(result)))
        }
        GovernanceDagMirrorQuery::NodeCid(cid) => {
            let cid_hex = hex_encode(cid);
            result.insert("query".into(), Value::from("node_cid"));
            result.insert("cid".into(), Value::from(cid_display(cid)));
            result.insert("cid_hex".into(), Value::from(cid_hex.clone()));
            let block = governance_dag_mirror_lookup_block(index, "by_node_cid_hex", &cid_hex)?;
            let found = !matches!(block, Value::Null);
            result.insert("found".into(), Value::from(found));
            result.insert("block".into(), block);
            Ok((found, Value::Object(result)))
        }
    }
}

fn governance_dag_mirror_lookup_block(
    index: &Value,
    map_name: &str,
    cid_hex: &str,
) -> Result<Value, String> {
    let Some(position) = index
        .get(map_name)
        .and_then(Value::as_object)
        .and_then(|map| map.get(cid_hex))
        .and_then(Value::as_u64)
    else {
        return Ok(Value::Null);
    };
    let blocks = index
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance DAG mirror index is missing `blocks` array".to_string())?;
    let position = usize::try_from(position)
        .map_err(|_| "governance DAG mirror block position exceeds host limits".to_string())?;
    Ok(blocks.get(position).cloned().unwrap_or(Value::Null))
}

fn read_governance_dag_json_file(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;
    from_slice(&bytes).map_err(|err| format!("failed to parse `{}` as JSON: {err}", path.display()))
}

fn governance_dag_read_digest_file(
    path: &Path,
    label: &str,
) -> Result<(Vec<u8>, u64, String), String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read {label} `{}`: {err}", path.display()))?;
    let encoded_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
    Ok((bytes, encoded_len, blake3_hex))
}

fn governance_dag_checkpoint_file_check(
    artifact: &str,
    path: &Path,
    expected: &Value,
    size_fields: &[&str],
    errors: &mut Vec<Value>,
) -> Value {
    let mut value = Map::new();
    value.insert("path".into(), Value::from(path.display().to_string()));
    match governance_dag_read_digest_file(path, artifact) {
        Ok((_, encoded_len, blake3_hex)) => {
            value.insert("encoded_len".into(), Value::from(encoded_len));
            value.insert("blake3".into(), Value::from(blake3_hex.clone()));

            let expected_blake3 = expected.get("blake3").and_then(Value::as_str);
            value.insert(
                "expected_blake3".into(),
                expected_blake3.map_or(Value::Null, Value::from),
            );
            let digest_ok = expected_blake3 == Some(blake3_hex.as_str());
            value.insert("digest_ok".into(), Value::from(digest_ok));
            if !digest_ok {
                errors.push(governance_dag_problem(
                    path.to_string_lossy().as_ref(),
                    format!("{artifact}_digest"),
                    "checkpoint artifact BLAKE3 digest does not match recorded value",
                ));
            }

            let expected_len = size_fields
                .iter()
                .find_map(|field| expected.get(*field).and_then(Value::as_u64));
            value.insert(
                "expected_len".into(),
                expected_len.map_or(Value::Null, Value::from),
            );
            let len_ok = expected_len == Some(encoded_len);
            value.insert("encoded_len_ok".into(), Value::from(len_ok));
            if !len_ok {
                errors.push(governance_dag_problem(
                    path.to_string_lossy().as_ref(),
                    format!("{artifact}_encoded_len"),
                    "checkpoint artifact length does not match recorded value",
                ));
            }
            value.insert("ok".into(), Value::from(digest_ok && len_ok));
        }
        Err(err) => {
            value.insert("ok".into(), Value::from(false));
            value.insert("read_error".into(), Value::from(err.clone()));
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                format!("{artifact}_read"),
                err,
            ));
        }
    }
    Value::Object(value)
}

fn governance_dag_checkpoint_optional_artifact_check(
    checkpoint: &Value,
    override_path: Option<&Path>,
    artifact: &str,
    label: &str,
    size_fields: &[&str],
    errors: &mut Vec<Value>,
) -> Result<Option<(PathBuf, Value)>, String> {
    let expected = checkpoint.get(artifact);
    let Some(expected) = expected else {
        if let Some(path) = override_path {
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                artifact,
                format!(
                    "`--{}` was supplied but checkpoint manifest has no `{artifact}` record",
                    artifact.replace('_', "-")
                ),
            ));
        }
        return Ok(None);
    };
    if matches!(expected, Value::Null) {
        if let Some(path) = override_path {
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                artifact,
                format!(
                    "`--{}` was supplied but checkpoint manifest has a null `{artifact}` record",
                    artifact.replace('_', "-")
                ),
            ));
        }
        return Ok(None);
    }

    let path = if let Some(path) = override_path {
        path.to_path_buf()
    } else {
        let Some(recorded) = expected.get("path").and_then(Value::as_str) else {
            errors.push(governance_dag_problem(
                artifact,
                artifact,
                "checkpoint artifact record is missing `path`",
            ));
            return Ok(None);
        };
        PathBuf::from(recorded)
    };
    let mut value =
        governance_dag_checkpoint_file_check(artifact, &path, expected, size_fields, errors);
    if let Value::Object(ref mut obj) = value {
        obj.insert("label".into(), Value::from(label.to_string()));
        obj.insert(
            "path_source".into(),
            Value::from(if override_path.is_some() {
                "override"
            } else {
                "checkpoint"
            }),
        );
    }
    Ok(Some((path, value)))
}

fn governance_dag_checkpoint_validate_mirror_index(
    path: &Path,
    check_value: &Value,
    expected_head_cid_hex: Option<&str>,
    expected_block_count: Option<u64>,
    errors: &mut Vec<Value>,
) {
    if check_value.get("ok").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let index = match read_governance_dag_json_file(path) {
        Ok(value) => value,
        Err(err) => {
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                "mirror_index_json",
                err,
            ));
            return;
        }
    };
    if index.get("schema").and_then(Value::as_str) != Some("sorafs.governance_dag.mirror.v1") {
        errors.push(governance_dag_problem(
            path.to_string_lossy().as_ref(),
            "mirror_index_schema",
            "mirror index has unsupported schema",
        ));
    }
    let index_head_cid_hex = index
        .get("head")
        .and_then(|head| head.get("head_block_cid_hex"))
        .and_then(Value::as_str);
    if index_head_cid_hex != expected_head_cid_hex {
        errors.push(governance_dag_problem(
            path.to_string_lossy().as_ref(),
            "mirror_index_head",
            "mirror index head does not match checkpoint head",
        ));
    }
    let index_block_count = index.get("block_count").and_then(Value::as_u64);
    if index_block_count != expected_block_count {
        errors.push(governance_dag_problem(
            path.to_string_lossy().as_ref(),
            "mirror_index_block_count",
            "mirror index block count does not match checkpoint block count",
        ));
    }
}

fn print_governance_dag_mirror_query_table(value: &Value) {
    let found = value.get("found").and_then(Value::as_bool).unwrap_or(false);
    println!("found: {found}");
    match value.get("query").and_then(Value::as_str) {
        Some("head") => {
            if let Some(head) = value.get("head").and_then(Value::as_object) {
                println!(
                    "head_block_cid_hex: {}",
                    head.get("head_block_cid_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "block_count: {}",
                    head.get("block_count")
                        .and_then(Value::as_u64)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "<missing>".to_string())
                );
            }
        }
        Some("block_cid" | "node_cid") => {
            if let Some(block) = value.get("block").and_then(Value::as_object) {
                println!(
                    "sequence: {}",
                    block
                        .get("sequence")
                        .and_then(Value::as_u64)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "<missing>".to_string())
                );
                println!(
                    "path: {}",
                    block
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "block_cid_hex: {}",
                    block
                        .get("block_cid_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "node_cid_hex: {}",
                    block
                        .get("node_cid_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "payload_kind: {}",
                    block
                        .get("payload_kind")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
            }
        }
        _ => {}
    }
}

type GovernanceDagBlockSnapshot = (Vec<GovernanceDagBlockV1>, Vec<Value>, Vec<Value>);

fn load_governance_dag_block_snapshot(
    root: &Path,
    require_sidecars: bool,
) -> Result<GovernanceDagBlockSnapshot, String> {
    let blocks_dir = root.join("blocks");
    if !blocks_dir.is_dir() {
        return Err(format!(
            "governance DAG block snapshot `{}` must contain a `blocks` directory",
            root.display()
        ));
    }
    let mut block_paths = Vec::<PathBuf>::new();
    collect_governance_dag_to_files(&blocks_dir, &mut block_paths)?;
    block_paths.sort();
    if block_paths.is_empty() {
        return Err(format!(
            "governance DAG block snapshot `{}` contains no `.to` block payloads",
            blocks_dir.display()
        ));
    }

    let mut warnings = Vec::<Value>::new();
    let mut decoded = Vec::<(String, String, String, GovernanceDagBlockV1)>::new();
    for path in block_paths {
        let rel_path = governance_dag_relative_path(root, &path);
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "failed to read governance DAG block `{}`: {err}",
                path.display()
            )
        })?;
        let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
        let (sidecar_status, _, sidecar_error) = governance_dag_sidecar_status(&path, &blake3_hex);
        match sidecar_status.as_str() {
            "mismatch" | "error" => {
                return Err(format!(
                    "governance DAG block `{rel_path}` has invalid sidecar status `{sidecar_status}`{}",
                    sidecar_error
                        .as_deref()
                        .map(|err| format!(": {err}"))
                        .unwrap_or_default()
                ));
            }
            "missing" if require_sidecars => {
                return Err(format!(
                    "governance DAG block `{rel_path}` is missing required sidecar"
                ));
            }
            "missing" => warnings.push(governance_dag_problem(
                &rel_path,
                "sidecar",
                "missing optional `.to.blake3` sidecar",
            )),
            _ => {}
        }
        let block = decode_from_bytes::<GovernanceDagBlockV1>(&bytes).map_err(|err| {
            format!(
                "failed to decode governance DAG block `{}`: {err}",
                path.display()
            )
        })?;
        decoded.push((rel_path, blake3_hex, sidecar_status, block));
    }

    decoded.sort_by(|left, right| {
        left.3
            .sequence
            .cmp(&right.3.sequence)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut blocks = Vec::<GovernanceDagBlockV1>::with_capacity(decoded.len());
    let mut records = Vec::<Value>::with_capacity(decoded.len());
    for (position, (path, blake3_hex, sidecar_status, block)) in decoded.into_iter().enumerate() {
        let mut record = Map::new();
        record.insert("position".into(), Value::from(position as u64));
        record.insert("path".into(), Value::from(path));
        record.insert("sequence".into(), Value::from(block.sequence));
        record.insert("timestamp".into(), Value::from(block.timestamp));
        record.insert(
            "block_cid".into(),
            Value::from(cid_display(&block.block_cid)),
        );
        record.insert(
            "block_cid_hex".into(),
            Value::from(hex_encode(&block.block_cid)),
        );
        record.insert(
            "prev_block_cid_hex".into(),
            block
                .prev_block_cid
                .as_ref()
                .map(hex_encode)
                .map_or(Value::Null, Value::from),
        );
        record.insert(
            "node_cid".into(),
            Value::from(cid_display(&block.node.node_cid)),
        );
        record.insert(
            "node_cid_hex".into(),
            Value::from(hex_encode(&block.node.node_cid)),
        );
        record.insert(
            "payload_kind".into(),
            Value::from(governance_payload_kind_cli(&block.node.payload)),
        );
        record.insert("blake3".into(), Value::from(blake3_hex));
        record.insert("sidecar_status".into(), Value::from(sidecar_status));
        blocks.push(block);
        records.push(Value::Object(record));
    }
    Ok((blocks, records, warnings))
}

fn governance_dag_head_cid_from_blocks(blocks: &[GovernanceDagBlockV1]) -> Result<Vec<u8>, String> {
    if blocks.is_empty() {
        return Err("governance DAG head rebuild requires at least one block".to_string());
    }
    let referenced = blocks
        .iter()
        .filter_map(|block| block.prev_block_cid.clone())
        .collect::<BTreeSet<_>>();
    let heads = blocks
        .iter()
        .filter(|block| !referenced.contains(&block.block_cid))
        .map(|block| block.block_cid.clone())
        .collect::<Vec<_>>();
    match heads.as_slice() {
        [head] => Ok(head.clone()),
        _ => Err(format!(
            "governance DAG head rebuild expected exactly one block-chain head, found {}",
            heads.len()
        )),
    }
}

fn governance_dag_car_file(rel_path: &str, data: Vec<u8>) -> Result<FileEntry, String> {
    let path = rel_path.split('/').map(str::to_string).collect::<Vec<_>>();
    if path.is_empty() || path.iter().any(|component| component.is_empty()) {
        return Err(format!(
            "invalid governance DAG CAR relative path `{rel_path}`"
        ));
    }
    Ok(FileEntry { path, data })
}

fn write_governance_dag_car_archive(
    snapshot_root: &Path,
    car_out: &Path,
    car_plan_out: Option<&Path>,
    chunker_handle: &str,
    files: Vec<FileEntry>,
) -> Result<Value, String> {
    let descriptor = chunker_registry::lookup_by_handle(chunker_handle).ok_or_else(|| {
        format!(
            "unknown governance DAG CAR chunker profile handle `{chunker_handle}`; see `sorafs_manifest_stub --list-chunker-profiles` for options"
        )
    })?;
    let (plan, payload) = CarBuildPlan::from_files_with_profile(files, descriptor.profile)
        .map_err(|err| format!("failed to build governance DAG CAR plan: {err}"))?;

    ensure_parent_dir(car_out)?;
    let car_file = File::create(car_out)
        .map_err(|err| format!("failed to create `{}`: {err}", car_out.display()))?;
    let mut writer = BufWriter::new(car_file);
    let mut payload_reader = Cursor::new(payload);
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload_reader, &mut writer)
        .map_err(format_car_error)?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush `{}`: {err}", car_out.display()))?;

    if stats.chunk_profile != descriptor.profile {
        return Err("emitted governance DAG CAR used unexpected chunk profile".to_string());
    }

    if let Some(plan_path) = car_plan_out {
        ensure_parent_dir(plan_path)?;
        let plan_json = chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
            .map_err(|err| format!("failed to render governance DAG CAR chunk plan: {err}"))?;
        write_text(plan_path, plan_json.as_bytes())?;
    }

    let files = plan
        .files
        .iter()
        .map(|file| {
            let mut obj = Map::new();
            obj.insert("path".into(), Value::from(file.path.join("/")));
            obj.insert("size".into(), Value::from(file.size));
            obj.insert("first_chunk".into(), Value::from(file.first_chunk as u64));
            obj.insert("chunk_count".into(), Value::from(file.chunk_count as u64));
            Value::Object(obj)
        })
        .collect::<Vec<_>>();

    let mut summary = Map::new();
    summary.insert("schema".into(), Value::from("sorafs.governance_dag.car.v1"));
    summary.insert(
        "snapshot_root".into(),
        Value::from(snapshot_root.display().to_string()),
    );
    summary.insert(
        "output_car".into(),
        Value::from(car_out.display().to_string()),
    );
    summary.insert(
        "chunker_handle".into(),
        Value::from(chunker_handle.to_string()),
    );
    summary.insert(
        "chunker_profile_id".into(),
        Value::from(descriptor.id.0 as u64),
    );
    summary.insert(
        "chunker_profile_canonical".into(),
        Value::from(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )),
    );
    summary.insert("payload_bytes".into(), Value::from(plan.content_length));
    summary.insert("chunk_count".into(), Value::from(plan.chunks.len() as u64));
    summary.insert("file_count".into(), Value::from(plan.files.len() as u64));
    summary.insert("files".into(), Value::Array(files));
    summary.insert("car_size".into(), Value::from(stats.car_size));
    summary.insert(
        "car_payload_digest_hex".into(),
        Value::from(hex_encode(stats.car_payload_digest.as_bytes())),
    );
    summary.insert(
        "car_digest_hex".into(),
        Value::from(hex_encode(stats.car_archive_digest.as_bytes())),
    );
    summary.insert(
        "car_cid_hex".into(),
        Value::from(hex_encode(&stats.car_cid)),
    );
    summary.insert(
        "root_cids_hex".into(),
        Value::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| Value::from(hex_encode(cid)))
                .collect(),
        ),
    );
    if let Some(plan_path) = car_plan_out {
        summary.insert(
            "chunk_plan_path".into(),
            Value::from(plan_path.display().to_string()),
        );
    }
    Ok(Value::Object(summary))
}

fn verify_governance_dag_build_snapshot(
    root: &Path,
    require_sidecars: bool,
    expected_head_cid: Option<&[u8]>,
) -> (bool, Value) {
    let mut errors = Vec::<Value>::new();
    let mut warnings = Vec::<Value>::new();
    let mut head_value = Map::new();
    let mut block_values = Vec::<Value>::new();
    let mut decoded_head: Option<GovernanceDagHeadV1> = None;
    let mut decoded_blocks = Vec::<(String, GovernanceDagBlockV1)>::new();

    if !root.is_dir() {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "root",
            "governance DAG build root must be a directory",
        ));
    }

    let head_path = root.join("head.to");
    let head_rel = governance_dag_relative_path(root, &head_path);
    head_value.insert("path".into(), Value::from(head_rel.clone()));
    head_value.insert(
        "source_path".into(),
        Value::from(head_path.display().to_string()),
    );
    match fs::read(&head_path) {
        Ok(bytes) => {
            let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
            let (sidecar_status, sidecar_value, sidecar_error) =
                governance_dag_sidecar_status(&head_path, &blake3_hex);
            head_value.insert(
                "encoded_len".into(),
                Value::from(u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            );
            head_value.insert("blake3".into(), Value::from(blake3_hex));
            head_value.insert("sidecar_status".into(), Value::from(sidecar_status.clone()));
            if let Some(value) = sidecar_value {
                head_value.insert("sidecar_blake3".into(), Value::from(value));
            }
            if let Some(error) = &sidecar_error {
                head_value.insert("sidecar_error".into(), Value::from(error.clone()));
            }
            push_governance_dag_sidecar_problem(
                &head_rel,
                &sidecar_status,
                sidecar_error.as_deref(),
                require_sidecars,
                &mut warnings,
                &mut errors,
            );

            match decode_from_bytes::<GovernanceDagHeadV1>(&bytes) {
                Ok(head) => {
                    head_value.insert("version".into(), Value::from(head.version));
                    head_value.insert("block_count".into(), Value::from(head.block_count));
                    head_value.insert("generated_at".into(), Value::from(head.generated_at));
                    head_value.insert(
                        "publisher_peer_id".into(),
                        Value::from(String::from_utf8_lossy(&head.publisher_peer_id).to_string()),
                    );
                    head_value.insert(
                        "head_block_cid".into(),
                        Value::from(cid_display(&head.head_block_cid)),
                    );
                    head_value.insert(
                        "head_block_cid_hex".into(),
                        Value::from(hex_encode(&head.head_block_cid)),
                    );
                    head_value.insert(
                        "checkpoint_cid_hex".into(),
                        head.checkpoint_cid
                            .as_ref()
                            .map(hex_encode)
                            .map_or(Value::Null, Value::from),
                    );
                    decoded_head = Some(head);
                }
                Err(err) => {
                    let message = format!("failed to decode GovernanceDagHeadV1: {err}");
                    head_value.insert("decode_error".into(), Value::from(message.clone()));
                    errors.push(governance_dag_problem(&head_rel, "decode_head", message));
                }
            }
        }
        Err(err) => {
            let message = format!("failed to read governance DAG head: {err}");
            head_value.insert("read_error".into(), Value::from(message.clone()));
            errors.push(governance_dag_problem(&head_rel, "head", message));
        }
    }

    let blocks_dir = root.join("blocks");
    let mut block_paths = Vec::<PathBuf>::new();
    if blocks_dir.is_dir() {
        if let Err(err) = collect_governance_dag_to_files(&blocks_dir, &mut block_paths) {
            errors.push(governance_dag_problem(
                &governance_dag_relative_path(root, &blocks_dir),
                "blocks",
                err,
            ));
        }
    } else {
        errors.push(governance_dag_problem(
            &governance_dag_relative_path(root, &blocks_dir),
            "blocks",
            "governance DAG build snapshot is missing the `blocks` directory",
        ));
    }
    block_paths.sort();

    if block_paths.is_empty() {
        errors.push(governance_dag_problem(
            &governance_dag_relative_path(root, &blocks_dir),
            "blocks",
            "no GovernanceDagBlockV1 `.to` payloads found",
        ));
    }

    for block_path in &block_paths {
        let rel_path = governance_dag_relative_path(root, block_path);
        let mut block_value = Map::new();
        block_value.insert("path".into(), Value::from(rel_path.clone()));
        block_value.insert(
            "source_path".into(),
            Value::from(block_path.display().to_string()),
        );
        match fs::read(block_path) {
            Ok(bytes) => {
                let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
                let (sidecar_status, sidecar_value, sidecar_error) =
                    governance_dag_sidecar_status(block_path, &blake3_hex);
                block_value.insert(
                    "encoded_len".into(),
                    Value::from(u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
                );
                block_value.insert("blake3".into(), Value::from(blake3_hex));
                block_value.insert("sidecar_status".into(), Value::from(sidecar_status.clone()));
                if let Some(value) = sidecar_value {
                    block_value.insert("sidecar_blake3".into(), Value::from(value));
                }
                if let Some(error) = &sidecar_error {
                    block_value.insert("sidecar_error".into(), Value::from(error.clone()));
                }
                push_governance_dag_sidecar_problem(
                    &rel_path,
                    &sidecar_status,
                    sidecar_error.as_deref(),
                    require_sidecars,
                    &mut warnings,
                    &mut errors,
                );

                match decode_from_bytes::<GovernanceDagBlockV1>(&bytes) {
                    Ok(block) => {
                        block_value.insert("version".into(), Value::from(block.version));
                        block_value.insert("sequence".into(), Value::from(block.sequence));
                        block_value.insert("timestamp".into(), Value::from(block.timestamp));
                        block_value.insert(
                            "publisher_peer_id".into(),
                            Value::from(
                                String::from_utf8_lossy(&block.publisher_peer_id).to_string(),
                            ),
                        );
                        block_value.insert(
                            "block_cid".into(),
                            Value::from(cid_display(&block.block_cid)),
                        );
                        block_value.insert(
                            "block_cid_hex".into(),
                            Value::from(hex_encode(&block.block_cid)),
                        );
                        block_value.insert(
                            "prev_block_cid_hex".into(),
                            block
                                .prev_block_cid
                                .as_ref()
                                .map(hex_encode)
                                .map_or(Value::Null, Value::from),
                        );
                        block_value.insert(
                            "node_cid".into(),
                            Value::from(cid_display(&block.node.node_cid)),
                        );
                        block_value.insert(
                            "node_cid_hex".into(),
                            Value::from(hex_encode(&block.node.node_cid)),
                        );
                        block_value.insert(
                            "payload_kind".into(),
                            Value::from(governance_payload_kind_cli(&block.node.payload)),
                        );
                        decoded_blocks.push((rel_path, block));
                    }
                    Err(err) => {
                        let message = format!("failed to decode GovernanceDagBlockV1: {err}");
                        block_value.insert("decode_error".into(), Value::from(message.clone()));
                        errors.push(governance_dag_problem(&rel_path, "decode_block", message));
                    }
                }
            }
            Err(err) => {
                let message = format!("failed to read governance DAG block: {err}");
                block_value.insert("read_error".into(), Value::from(message.clone()));
                errors.push(governance_dag_problem(&rel_path, "block", message));
            }
        }
        block_values.push(Value::Object(block_value));
    }

    decoded_blocks.sort_by(|left, right| {
        left.1
            .sequence
            .cmp(&right.1.sequence)
            .then_with(|| left.0.cmp(&right.0))
    });
    let blocks = decoded_blocks
        .into_iter()
        .map(|(_, block)| block)
        .collect::<Vec<_>>();
    if let Some(head) = decoded_head.as_ref() {
        if let Some(expected) = expected_head_cid
            && head.head_block_cid != expected
        {
            errors.push(governance_dag_problem(
                &head_rel,
                "head_cid",
                format!(
                    "expected head CID `{}` but snapshot advertises `{}`",
                    cid_display(expected),
                    cid_display(&head.head_block_cid)
                ),
            ));
        }
        if let Err(err) = validate_governance_dag_head_against_chain_v1(head, &blocks) {
            errors.push(governance_dag_problem(
                &head_rel,
                "head_chain",
                format!("governance DAG block/head validation failed: {err}"),
            ));
        }
    }

    let mut summary = Map::new();
    summary.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.build.verify.v1"),
    );
    summary.insert("root".into(), Value::from(root.display().to_string()));
    summary.insert("ok".into(), Value::from(errors.is_empty()));
    summary.insert("require_sidecars".into(), Value::from(require_sidecars));
    summary.insert(
        "expected_head_cid".into(),
        expected_head_cid
            .map(cid_display)
            .map_or(Value::Null, Value::from),
    );
    summary.insert(
        "expected_head_cid_hex".into(),
        expected_head_cid
            .map(hex_encode)
            .map_or(Value::Null, Value::from),
    );
    summary.insert("head".into(), Value::Object(head_value));
    summary.insert(
        "block_file_count".into(),
        Value::from(block_paths.len() as u64),
    );
    summary.insert("block_count".into(), Value::from(blocks.len() as u64));
    summary.insert("blocks".into(), Value::Array(block_values));
    summary.insert("warnings".into(), Value::Array(warnings));
    summary.insert("errors".into(), Value::Array(errors.clone()));
    (errors.is_empty(), Value::Object(summary))
}

fn load_governance_dag_inventory(root: &Path) -> Result<Vec<GovernanceDagArtifact>, String> {
    if !root.is_dir() {
        return Err(format!(
            "governance DAG root `{}` must be a directory",
            root.display()
        ));
    }
    let mut paths = Vec::new();
    collect_governance_dag_to_files(root, &mut paths)?;
    paths.sort();
    paths
        .iter()
        .map(|path| read_governance_dag_artifact(root, path))
        .collect()
}

fn collect_governance_dag_to_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|err| {
        format!(
            "failed to read governance DAG directory `{}`: {err}",
            root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read governance DAG directory entry in `{}`: {err}",
                root.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect governance DAG path `{}`: {err}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_governance_dag_to_files(&path, out)?;
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("to")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn governance_dag_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_governance_dag_artifact(root: &Path, path: &Path) -> Result<GovernanceDagArtifact, String> {
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read governance DAG artifact `{}`: {err}",
            path.display()
        )
    })?;
    let digest = blake3_hash(&bytes);
    let blake3_hex = digest.to_hex().to_string();
    let rel_path = governance_dag_relative_path(root, path);
    let (sidecar_status, sidecar_value, sidecar_error) =
        governance_dag_sidecar_status(path, &blake3_hex);

    let (node, decode_error, outcome) = match decode_from_bytes::<GovernanceLogNodeV1>(&bytes) {
        Ok(node) => {
            let summary = GovernanceDagNodeSummary::from_node(&node);
            let outcome = validate_governance_log_node_bytes(
                &bytes,
                rel_path.clone(),
                Some(node.node_cid.as_slice()),
                governance_dag_now_secs(),
            );
            (Some(summary), None, Some(outcome))
        }
        Err(err) => (None, Some(err.to_string()), None),
    };

    Ok(GovernanceDagArtifact {
        path: path.to_path_buf(),
        rel_path,
        encoded_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        blake3_hex,
        sidecar_status,
        sidecar_value,
        sidecar_error,
        node,
        decode_error,
        outcome,
    })
}

fn read_governance_log_node_file(path: &Path) -> Result<GovernanceLogNodeV1, String> {
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read governance log node `{}`: {err}",
            path.display()
        )
    })?;
    decode_from_bytes::<GovernanceLogNodeV1>(&bytes).map_err(|err| {
        format!(
            "failed to decode governance log node `{}`: {err}",
            path.display()
        )
    })
}

fn governance_dag_build_order(
    artifacts: &[GovernanceDagArtifact],
    node_indices: &[usize],
) -> Vec<usize> {
    let mut ordered = node_indices.to_vec();
    ordered.sort_by(|left, right| {
        let left_artifact = &artifacts[*left];
        let right_artifact = &artifacts[*right];
        let left_node = left_artifact.node.as_ref();
        let right_node = right_artifact.node.as_ref();
        left_node
            .map(|node| node.timestamp)
            .cmp(&right_node.map(|node| node.timestamp))
            .then_with(|| {
                left_node
                    .map(|node| node.node_cid.as_slice())
                    .cmp(&right_node.map(|node| node.node_cid.as_slice()))
            })
            .then_with(|| left_artifact.rel_path.cmp(&right_artifact.rel_path))
    });
    ordered
}

fn load_governance_dag_build_seed(
    key_hex: Option<&str>,
    key_path: Option<&Path>,
) -> Result<[u8; ed25519_dalek::SECRET_KEY_LENGTH], String> {
    let raw = match (key_hex, key_path) {
        (Some(_), Some(_)) => {
            return Err(
                "governance DAG build key flags are mutually exclusive; provide `--key-hex` or `--key`"
                    .to_string(),
            );
        }
        (Some(value), None) => value.trim().to_string(),
        (None, Some(path)) => fs::read_to_string(path)
            .map_err(|err| {
                format!(
                    "failed to read governance DAG signing key `{}`: {err}",
                    path.display()
                )
            })?
            .trim()
            .to_string(),
        (None, None) => {
            return Err(
                "missing governance DAG signing key; provide `--key-hex=HEX` or `--key=PATH`"
                    .to_string(),
            );
        }
    };
    let trimmed = raw.strip_prefix("ed25519:").unwrap_or(raw.as_str()).trim();
    let bytes = parse_hex_vec(trimmed).map_err(|err| {
        format!("failed to parse governance DAG Ed25519 seed hex for `--key-hex`/`--key`: {err}")
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        format!(
            "governance DAG Ed25519 seed must be {} bytes, found {} bytes",
            ed25519_dalek::SECRET_KEY_LENGTH,
            bytes.len()
        )
    })
}

fn empty_governance_dag_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

fn sign_governance_dag_block_cli(
    block: &mut GovernanceDagBlockV1,
    signing_key: &SigningKey,
) -> Result<(), String> {
    let payload = block
        .signature_payload_bytes()
        .map_err(|err| format!("failed to encode governance DAG block signing payload: {err}"))?;
    let signature = signing_key.sign(&payload);
    block.block_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    Ok(())
}

fn sign_governance_dag_head_cli(
    head: &mut GovernanceDagHeadV1,
    signing_key: &SigningKey,
) -> Result<(), String> {
    let payload = head
        .signature_payload_bytes()
        .map_err(|err| format!("failed to encode governance DAG head signing payload: {err}"))?;
    let signature = signing_key.sign(&payload);
    head.head_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    Ok(())
}

fn write_governance_blake3_sidecar(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let sidecar_path = path.with_extension("to.blake3");
    let mut digest = hex_encode(blake3_hash(bytes).as_bytes());
    digest.push('\n');
    let sidecar_bytes = digest.into_bytes();
    write_text(&sidecar_path, &sidecar_bytes)?;
    Ok(sidecar_bytes)
}

impl GovernanceDagNodeSummary {
    fn from_node(node: &GovernanceLogNodeV1) -> Self {
        Self {
            node_cid: node.node_cid.clone(),
            node_cid_label: cid_display(&node.node_cid),
            node_cid_hex: hex_encode(&node.node_cid),
            prev_cid: node.prev_cid.clone(),
            prev_cid_label: node.prev_cid.as_ref().map(|cid| cid_display(cid)),
            prev_cid_hex: node.prev_cid.as_ref().map(hex_encode),
            timestamp: node.timestamp,
            publisher_peer_id: String::from_utf8_lossy(&node.publisher_peer_id).to_string(),
            payload_kind: governance_payload_kind_cli(&node.payload),
        }
    }
}

fn governance_dag_sidecar_status(
    path: &Path,
    blake3_hex: &str,
) -> (String, Option<String>, Option<String>) {
    let sidecar_path = path.with_extension("to.blake3");
    match fs::read_to_string(&sidecar_path) {
        Ok(raw) => {
            let sidecar_value = raw.trim().to_string();
            let status = if sidecar_value == blake3_hex {
                "match"
            } else {
                "mismatch"
            };
            (status.to_string(), Some(sidecar_value), None)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => ("missing".to_string(), None, None),
        Err(err) => ("error".to_string(), None, Some(err.to_string())),
    }
}

fn push_governance_dag_sidecar_problem(
    path: &str,
    status: &str,
    error: Option<&str>,
    require_sidecars: bool,
    warnings: &mut Vec<Value>,
    errors: &mut Vec<Value>,
) {
    match status {
        "mismatch" => errors.push(governance_dag_problem(
            path,
            "sidecar",
            "BLAKE3 sidecar does not match encoded bytes",
        )),
        "error" => errors.push(governance_dag_problem(
            path,
            "sidecar",
            format!(
                "failed to inspect BLAKE3 sidecar: {}",
                error.unwrap_or("unknown error")
            ),
        )),
        "missing" if require_sidecars => errors.push(governance_dag_problem(
            path,
            "sidecar",
            "missing required `.to.blake3` sidecar",
        )),
        "missing" => warnings.push(governance_dag_problem(
            path,
            "sidecar",
            "missing optional `.to.blake3` sidecar",
        )),
        _ => {}
    }
}

fn governance_payload_kind_cli(payload: &GovernanceLogPayloadV1) -> &'static str {
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(_) => "provider_advert",
        GovernanceLogPayloadV1::ReplicationOrder(_) => "replication_order",
        GovernanceLogPayloadV1::PorChallenge(_) => "por_challenge",
        GovernanceLogPayloadV1::PorProof(_) => "por_proof",
        GovernanceLogPayloadV1::AuditVerdict(_) => "audit_verdict",
        GovernanceLogPayloadV1::DealSettlement(_) => "deal_settlement",
        GovernanceLogPayloadV1::ModerationBallotEvent(_) => "moderation_ballot_event",
        GovernanceLogPayloadV1::AppealFinanceReport(_) => "appeal_finance_report",
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(_) => "appeal_finance_weekly_rollup",
        GovernanceLogPayloadV1::ReputationSnapshot(_) => "reputation_snapshot",
    }
}

fn verify_governance_dag_inventory(
    root: &Path,
    artifacts: &[GovernanceDagArtifact],
    options: &GovernanceDagVerifyOptions,
) -> (bool, Value, Vec<usize>) {
    let mut errors = Vec::<Value>::new();
    let mut warnings = Vec::<Value>::new();
    let mut node_indices = Vec::<usize>::new();
    let mut node_by_cid = BTreeMap::<Vec<u8>, usize>::new();
    let mut referenced_prev = BTreeSet::<Vec<u8>>::new();

    for (index, artifact) in artifacts.iter().enumerate() {
        match artifact.sidecar_status.as_str() {
            "mismatch" | "error" => errors.push(governance_dag_problem(
                &artifact.rel_path,
                "sidecar",
                format!("BLAKE3 sidecar status is `{}`", artifact.sidecar_status),
            )),
            "missing" if options.require_sidecars => errors.push(governance_dag_problem(
                &artifact.rel_path,
                "sidecar",
                "missing required `.to.blake3` sidecar",
            )),
            "missing" => warnings.push(governance_dag_problem(
                &artifact.rel_path,
                "sidecar",
                "missing optional `.to.blake3` sidecar",
            )),
            _ => {}
        }

        if let Some(node) = &artifact.node {
            node_indices.push(index);
            if let Some(existing) = node_by_cid.insert(node.node_cid.clone(), index) {
                errors.push(governance_dag_problem(
                    &artifact.rel_path,
                    "duplicate_node_cid",
                    format!("node CID duplicates `{}`", artifacts[existing].rel_path),
                ));
            }
            if let Some(prev) = &node.prev_cid {
                referenced_prev.insert(prev.clone());
            }
            if !artifact
                .outcome
                .as_ref()
                .is_some_and(ValidationOutcomeV1::is_ok)
            {
                let code = artifact
                    .outcome
                    .as_ref()
                    .map(|outcome| outcome.code.as_str())
                    .unwrap_or("decode-error");
                errors.push(governance_dag_problem(
                    &artifact.rel_path,
                    "validation",
                    format!("governance node failed reference validation with `{code}`"),
                ));
            }
        }
    }

    if node_indices.is_empty() {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "inventory",
            "no GovernanceLogNodeV1 `.to` payloads found",
        ));
    }

    if options.require_chain {
        for index in &node_indices {
            let artifact = &artifacts[*index];
            let Some(node) = &artifact.node else {
                continue;
            };
            if let Some(prev) = &node.prev_cid
                && !node_by_cid.contains_key(prev)
            {
                errors.push(governance_dag_problem(
                    &artifact.rel_path,
                    "missing_parent",
                    format!(
                        "previous CID `{}` is not present in this archive",
                        cid_display(prev)
                    ),
                ));
            }
        }
    }

    let mut heads = Vec::<Vec<u8>>::new();
    for index in &node_indices {
        let Some(node) = &artifacts[*index].node else {
            continue;
        };
        if !referenced_prev.contains(&node.node_cid) {
            heads.push(node.node_cid.clone());
        }
    }
    heads.sort();

    if options.require_chain && heads.len() != 1 {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "head_count",
            format!(
                "expected exactly one governance DAG head, found {}",
                heads.len()
            ),
        ));
    }
    if let Some(expected_head) = &options.expected_head_cid
        && !heads.iter().any(|head| head == expected_head)
    {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "head_cid",
            format!(
                "expected head CID `{}` is not an archive head",
                cid_display(expected_head)
            ),
        ));
    }

    let mut summary = governance_dag_inventory_value(root, artifacts);
    if let Value::Object(ref mut obj) = summary {
        obj.insert("ok".into(), Value::from(errors.is_empty()));
        obj.insert("require_chain".into(), Value::from(options.require_chain));
        obj.insert(
            "require_sidecars".into(),
            Value::from(options.require_sidecars),
        );
        obj.insert(
            "head_cids".into(),
            Value::Array(
                heads
                    .iter()
                    .map(|cid| Value::from(cid_display(cid)))
                    .collect(),
            ),
        );
        obj.insert(
            "head_cid_hex".into(),
            Value::Array(
                heads
                    .iter()
                    .map(|cid| Value::from(hex_encode(cid)))
                    .collect(),
            ),
        );
        if let Some(expected) = &options.expected_head_cid {
            obj.insert(
                "expected_head_cid".into(),
                Value::from(cid_display(expected)),
            );
            obj.insert(
                "expected_head_cid_hex".into(),
                Value::from(hex_encode(expected)),
            );
        }
        obj.insert("warnings".into(), Value::Array(warnings));
        obj.insert("errors".into(), Value::Array(errors.clone()));
    }

    (errors.is_empty(), summary, node_indices)
}

fn governance_dag_problem(
    path: &str,
    kind: impl Into<String>,
    message: impl Into<String>,
) -> Value {
    let mut obj = Map::new();
    obj.insert("path".into(), Value::from(path.to_string()));
    obj.insert("kind".into(), Value::from(kind.into()));
    obj.insert("message".into(), Value::from(message.into()));
    Value::Object(obj)
}

fn governance_dag_inventory_value(root: &Path, artifacts: &[GovernanceDagArtifact]) -> Value {
    let node_count = artifacts
        .iter()
        .filter(|artifact| artifact.node.is_some())
        .count();
    let valid_node_count = artifacts
        .iter()
        .filter(|artifact| {
            artifact
                .outcome
                .as_ref()
                .is_some_and(ValidationOutcomeV1::is_ok)
        })
        .count();
    let sidecar_mismatch_count = artifacts
        .iter()
        .filter(|artifact| matches!(artifact.sidecar_status.as_str(), "mismatch" | "error"))
        .count();
    let sidecar_missing_count = artifacts
        .iter()
        .filter(|artifact| artifact.sidecar_status == "missing")
        .count();

    let mut obj = Map::new();
    obj.insert(
        "schema".into(),
        Value::from("sorafs.governance_dag.inventory.v1"),
    );
    obj.insert("root".into(), Value::from(root.display().to_string()));
    obj.insert("artifact_count".into(), Value::from(artifacts.len() as u64));
    obj.insert("node_count".into(), Value::from(node_count as u64));
    obj.insert(
        "valid_node_count".into(),
        Value::from(valid_node_count as u64),
    );
    obj.insert(
        "sidecar_mismatch_count".into(),
        Value::from(sidecar_mismatch_count as u64),
    );
    obj.insert(
        "sidecar_missing_count".into(),
        Value::from(sidecar_missing_count as u64),
    );
    obj.insert(
        "artifacts".into(),
        Value::Array(
            artifacts
                .iter()
                .map(|artifact| governance_dag_artifact_value(artifact, false))
                .collect(),
        ),
    );
    Value::Object(obj)
}

fn governance_dag_artifact_value(artifact: &GovernanceDagArtifact, include_outcome: bool) -> Value {
    let mut obj = Map::new();
    obj.insert("path".into(), Value::from(artifact.rel_path.clone()));
    obj.insert(
        "source_path".into(),
        Value::from(artifact.path.display().to_string()),
    );
    obj.insert("encoded_len".into(), Value::from(artifact.encoded_len));
    obj.insert("blake3".into(), Value::from(artifact.blake3_hex.clone()));
    obj.insert(
        "sidecar_status".into(),
        Value::from(artifact.sidecar_status.clone()),
    );
    if let Some(value) = &artifact.sidecar_value {
        obj.insert("sidecar_blake3".into(), Value::from(value.clone()));
    }
    if let Some(error) = &artifact.sidecar_error {
        obj.insert("sidecar_error".into(), Value::from(error.clone()));
    }
    if let Some(node) = &artifact.node {
        obj.insert("node".into(), governance_dag_node_value(node));
        if let Some(outcome) = &artifact.outcome {
            obj.insert(
                "validation_status".into(),
                Value::from(outcome.status.clone()),
            );
            obj.insert("validation_code".into(), Value::from(outcome.code.clone()));
            if include_outcome {
                obj.insert(
                    "validation_outcome".into(),
                    to_value(outcome).unwrap_or(Value::Null),
                );
            }
        }
    } else if let Some(error) = &artifact.decode_error {
        obj.insert(
            "validation_status".into(),
            Value::from("not_governance_node"),
        );
        obj.insert("decode_error".into(), Value::from(error.clone()));
    }
    Value::Object(obj)
}

fn governance_dag_node_value(node: &GovernanceDagNodeSummary) -> Value {
    let mut obj = Map::new();
    obj.insert("node_cid".into(), Value::from(node.node_cid_label.clone()));
    obj.insert(
        "node_cid_hex".into(),
        Value::from(node.node_cid_hex.clone()),
    );
    obj.insert(
        "prev_cid".into(),
        node.prev_cid_label.clone().map_or(Value::Null, Value::from),
    );
    obj.insert(
        "prev_cid_hex".into(),
        node.prev_cid_hex.clone().map_or(Value::Null, Value::from),
    );
    obj.insert("timestamp".into(), Value::from(node.timestamp));
    obj.insert(
        "publisher_peer_id".into(),
        Value::from(node.publisher_peer_id.clone()),
    );
    obj.insert("payload_kind".into(), Value::from(node.payload_kind));
    Value::Object(obj)
}

fn print_governance_dag_inventory_table(root: &Path, artifacts: &[GovernanceDagArtifact]) {
    println!("root: {}", root.display());
    println!("path\tkind\tvalidation\tsidecar\tblake3");
    for artifact in artifacts {
        let kind = artifact
            .node
            .as_ref()
            .map(|node| node.payload_kind)
            .unwrap_or("raw");
        let validation = artifact
            .outcome
            .as_ref()
            .map(|outcome| outcome.status.as_str())
            .unwrap_or("not_governance_node");
        println!(
            "{}\t{}\t{}\t{}\t{}",
            artifact.rel_path, kind, validation, artifact.sidecar_status, artifact.blake3_hex
        );
    }
}

fn print_governance_dag_artifact_table(artifact: &GovernanceDagArtifact) {
    println!("path: {}", artifact.rel_path);
    println!("encoded_len: {}", artifact.encoded_len);
    println!("blake3: {}", artifact.blake3_hex);
    println!("sidecar_status: {}", artifact.sidecar_status);
    if let Some(node) = &artifact.node {
        println!("node_cid: {}", node.node_cid_label);
        println!("node_cid_hex: {}", node.node_cid_hex);
        println!(
            "prev_cid: {}",
            node.prev_cid_label.as_deref().unwrap_or("<root>")
        );
        println!("timestamp: {}", node.timestamp);
        println!("publisher_peer_id: {}", node.publisher_peer_id);
        println!("payload_kind: {}", node.payload_kind);
    }
    if let Some(outcome) = &artifact.outcome {
        println!("validation_status: {}", outcome.status);
        println!("validation_code: {}", outcome.code);
        println!("validation_message: {}", outcome.message);
    } else if let Some(error) = &artifact.decode_error {
        println!("validation_status: not_governance_node");
        println!("decode_error: {error}");
    }
}

fn write_governance_dag_json(path: &Path, value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|err| format!("failed to render governance DAG JSON: {err}"))?;
    write_text(path, rendered.as_bytes())
}

fn print_governance_dag_json(value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|err| format!("failed to render governance DAG JSON: {err}"))?;
    println!("{rendered}");
    Ok(())
}

fn parse_governance_cid_arg(raw: &str) -> Result<Vec<u8>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("governance DAG CID argument must not be empty".to_string());
    }
    if let Some(hex) = trimmed.strip_prefix("hex:") {
        return hex::decode(hex)
            .map_err(|err| format!("failed to decode governance DAG hex CID `{trimmed}`: {err}"));
    }
    if trimmed.len().is_multiple_of(2) && trimmed.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return hex::decode(trimmed)
            .map_err(|err| format!("failed to decode governance DAG hex CID `{trimmed}`: {err}"));
    }
    Ok(trimmed.as_bytes().to_vec())
}

fn cid_display(cid: &[u8]) -> String {
    match std::str::from_utf8(cid) {
        Ok(value) if !value.is_empty() && value.chars().all(|ch| !ch.is_control()) => {
            value.to_string()
        }
        _ => format!("hex:{}", hex_encode(cid)),
    }
}

fn governance_dag_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn generate_proof_stream_nonce(
    manifest_digest: &[u8],
    proof_kind: ProofKind,
    sample_count: Option<u32>,
    deadline_ms: Option<u32>,
    provider_id: Option<&str>,
) -> [u8; 16] {
    let mut buffer = Vec::with_capacity(manifest_digest.len() + 48);
    buffer.extend_from_slice(manifest_digest);
    buffer.extend_from_slice(proof_kind.as_str().as_bytes());
    let count_bytes = sample_count.unwrap_or(0).to_le_bytes();
    buffer.extend_from_slice(&count_bytes);
    if let Some(deadline) = deadline_ms {
        buffer.extend_from_slice(&deadline.to_le_bytes());
    }
    if let Some(provider) = provider_id {
        buffer.extend_from_slice(provider.as_bytes());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_nanos()
        .to_le_bytes();
    buffer.extend_from_slice(&timestamp);
    let digest = blake3_hash(&buffer);
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&digest.as_bytes()[..16]);
    nonce
}

fn decode_nonce_b64(input: &str) -> Result<[u8; 16], String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("`--nonce-b64` may not be empty".to_string());
    }
    let decoded = BASE64_STANDARD
        .decode(trimmed.as_bytes())
        .map_err(|err| format!("invalid `--nonce-b64` value: {err}"))?;
    if decoded.len() != 16 {
        return Err(format!(
            "`--nonce-b64` must decode to 16 bytes, found {} bytes",
            decoded.len()
        ));
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&decoded);
    Ok(out)
}

fn manifest_sign(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut bundle_out: Option<PathBuf> = None;
    let mut signature_out: Option<PathBuf> = None;
    let mut identity_token_inline: Option<String> = None;
    let mut identity_token_env: Option<String> = None;
    let mut identity_token_file: Option<PathBuf> = None;
    let mut identity_token_provider: Option<IdentityTokenProvider> = None;
    let mut identity_token_audience: Option<String> = None;
    let mut include_token = false;
    let mut summary_source: Option<JsonSource> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut chunk_digest_hex_arg: Option<String> = None;
    let mut issued_at_unix: Option<u64> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--bundle-out" => bundle_out = Some(PathBuf::from(value)),
            "--signature-out" => signature_out = Some(PathBuf::from(value)),
            "--identity-token" => identity_token_inline = Some(value.to_string()),
            "--identity-token-env" => identity_token_env = Some(value.to_string()),
            "--identity-token-file" => identity_token_file = Some(PathBuf::from(value)),
            "--identity-token-provider" => {
                let provider = IdentityTokenProvider::parse(value)?;
                if identity_token_provider.replace(provider).is_some() {
                    return Err(
                        "`--identity-token-provider` may not be specified more than once"
                            .to_string(),
                    );
                }
            }
            "--identity-token-audience" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--identity-token-audience` may not be empty".to_string());
                }
                if identity_token_audience
                    .replace(trimmed.to_string())
                    .is_some()
                {
                    return Err(
                        "`--identity-token-audience` may not be specified more than once"
                            .to_string(),
                    );
                }
            }
            "--include-token" => include_token = parse_bool_flag(value, "--include-token")?,
            "--summary" => summary_source = Some(JsonSource::from_arg(value)?),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--chunk-digest-sha3" => chunk_digest_hex_arg = Some(value.to_string()),
            "--issued-at" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid `--issued-at` value: {err}"))?;
                issued_at_unix = Some(parsed);
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli manifest sign`"
                ));
            }
        }
    }

    if bundle_out.is_none() && signature_out.is_none() {
        return Err(
            "must provide at least one of `--bundle-out` or `--signature-out` for `sorafs_cli manifest sign`"
                .to_string(),
        );
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli manifest sign`".to_string()
    })?;

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());

    let chunk_digest_resolution = resolve_chunk_digest(
        summary_source,
        chunk_plan_source,
        chunk_plan_label,
        chunk_digest_hex_arg,
        true,
        "sorafs_cli manifest sign",
    )?;
    let chunk_digest_bytes = chunk_digest_resolution
        .digest
        .expect("chunk digest required when `require_digest` is true");
    let chunk_digest_hex = chunk_digest_resolution
        .hex
        .unwrap_or_else(|| hex_encode(chunk_digest_bytes));
    let plan_chunk_count = chunk_digest_resolution.plan_chunk_count;
    let chunk_plan_label = chunk_digest_resolution.plan_label;

    let (identity_token, token_source_label) = resolve_identity_token(
        identity_token_inline,
        identity_token_env,
        identity_token_file,
        identity_token_provider,
        identity_token_audience,
    )?;
    let token_hash = blake3_hash(identity_token.as_bytes());
    let token_hash_hex = hex_encode(token_hash.as_bytes());

    let (jwt_header, jwt_claims) = decode_jwt_sections(&identity_token)?;

    let seed = blake3_hash(identity_token.as_bytes());
    let signing_key = SigningKey::from_bytes(seed.as_bytes());
    let verifying_key = signing_key.verifying_key();
    let signature = signing_key.sign(manifest_digest.as_bytes());
    let signature_hex = hex_encode(signature.to_bytes());
    let public_key_bytes = verifying_key.to_bytes();
    let public_key_hex = hex_encode(public_key_bytes);

    let public_key_multihash = PublicKey::from_bytes(Algorithm::Ed25519, &public_key_bytes)
        .and_then(|key| key.try_to_prefixed_string())
        .map_err(|err| format!("failed to format public key multihash: {err}"))?;

    let issued_at = issued_at_unix.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_secs()
    });

    let mut bundle = Map::new();
    bundle.insert(
        "schema_version".into(),
        Value::from("sorafs-cli-manifest-sign/v2"),
    );
    bundle.insert("issued_at_unix".into(), Value::from(issued_at));

    let mut manifest_info = Map::new();
    manifest_info.insert(
        "path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    manifest_info.insert(
        "manifest_blake3_hex".into(),
        Value::from(manifest_digest_hex.clone()),
    );
    manifest_info.insert(
        "car_digest_hex".into(),
        Value::from(hex_encode(manifest.car_digest)),
    );
    manifest_info.insert("car_size".into(), Value::from(manifest.car_size));
    manifest_info.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::from(chunk_digest_hex.clone()),
    );
    if let Some(label) = &chunk_plan_label {
        manifest_info.insert("chunk_plan_source".into(), Value::from(label.clone()));
    }
    if let Some(count) = plan_chunk_count {
        manifest_info.insert("chunk_plan_chunk_count".into(), Value::from(count));
    }
    bundle.insert("manifest".into(), Value::Object(manifest_info));

    let mut signature_info = Map::new();
    signature_info.insert("algorithm".into(), Value::from("ed25519"));
    signature_info.insert("signature_hex".into(), Value::from(signature_hex.clone()));
    signature_info.insert("public_key_hex".into(), Value::from(public_key_hex.clone()));
    signature_info.insert(
        "public_key_multihash".into(),
        Value::from(public_key_multihash.clone()),
    );
    bundle.insert("signature".into(), Value::Object(signature_info));

    let mut identity_info = Map::new();
    identity_info.insert(
        "token_source".into(),
        Value::from(token_source_label.clone()),
    );
    identity_info.insert(
        "token_hash_blake3_hex".into(),
        Value::from(token_hash_hex.clone()),
    );
    identity_info.insert("token_included".into(), Value::from(include_token));
    if include_token {
        identity_info.insert("token".into(), Value::from(identity_token.clone()));
    }
    identity_info.insert("jwt_header".into(), jwt_header);
    identity_info.insert("jwt_claims".into(), jwt_claims);
    bundle.insert("identity".into(), Value::Object(identity_info));

    let bundle_value = Value::Object(bundle);
    let bundle_rendered = to_string_pretty(&bundle_value)
        .map_err(|err| format!("failed to render signature bundle: {err}"))?;

    if let Some(path) = bundle_out.as_ref() {
        ensure_parent_dir(path)?;
        write_text(path, bundle_rendered.as_bytes())?;
    }

    if let Some(path) = signature_out.as_ref() {
        ensure_parent_dir(path)?;
        let mut signature_text = signature_hex.clone();
        signature_text.push('\n');
        write_text(path, signature_text.as_bytes())?;
    }

    let mut summary = Map::new();
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary.insert(
        "manifest_blake3_hex".into(),
        Value::from(manifest_digest_hex),
    );
    summary.insert("signature_hex".into(), Value::from(signature_hex));
    summary.insert("public_key_hex".into(), Value::from(public_key_hex));
    summary.insert(
        "public_key_multihash".into(),
        Value::from(public_key_multihash),
    );
    summary.insert("issued_at_unix".into(), Value::from(issued_at));
    if let Some(path) = bundle_out {
        summary.insert(
            "bundle_path".into(),
            Value::from(path.display().to_string()),
        );
    }
    if let Some(path) = signature_out {
        summary.insert(
            "signature_path".into(),
            Value::from(path.display().to_string()),
        );
    }
    summary.insert(
        "identity_token_source".into(),
        Value::from(token_source_label),
    );
    summary.insert(
        "identity_token_hash_blake3_hex".into(),
        Value::from(token_hash_hex),
    );
    summary.insert("token_included".into(), Value::from(include_token));
    summary.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::from(chunk_digest_hex),
    );
    if let Some(count) = plan_chunk_count {
        summary.insert("chunk_plan_chunk_count".into(), Value::from(count));
    }
    if let Some(label) = chunk_plan_label {
        summary.insert("chunk_plan_source".into(), Value::from(label));
    }

    let summary_rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{summary_rendered}");

    Ok(())
}

fn manifest_verify_signature(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut bundle_path: Option<PathBuf> = None;
    let mut signature_path: Option<PathBuf> = None;
    let mut public_key_hex_arg: Option<String> = None;
    let mut summary_source: Option<JsonSource> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut chunk_digest_hex_arg: Option<String> = None;
    let mut expect_token_hash: Option<String> = None;

    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--bundle" => bundle_path = Some(PathBuf::from(value)),
            "--signature" => signature_path = Some(PathBuf::from(value)),
            "--public-key-hex" => public_key_hex_arg = Some(value.to_string()),
            "--summary" => summary_source = Some(JsonSource::from_arg(value)?),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--chunk-digest-sha3" => chunk_digest_hex_arg = Some(value.to_string()),
            "--expect-token-hash" => expect_token_hash = Some(value.to_string()),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli manifest verify-signature`"
                ));
            }
        }
    }

    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli manifest verify-signature`".to_string()
    })?;

    if bundle_path.is_none() && signature_path.is_none() {
        return Err(
            "provide either `--bundle` or `--signature`/`--public-key-hex` inputs for `sorafs_cli manifest verify-signature`"
                .to_string(),
        );
    }

    if signature_path.is_some() && public_key_hex_arg.is_none() && bundle_path.is_none() {
        return Err(
            "`--signature` requires `--public-key-hex` unless a bundle is also supplied"
                .to_string(),
        );
    }

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());

    let chunk_digest_resolution = resolve_chunk_digest(
        summary_source,
        chunk_plan_source,
        chunk_plan_label,
        chunk_digest_hex_arg,
        false,
        "sorafs_cli manifest verify-signature",
    )?;

    let mut final_chunk_digest_hex = chunk_digest_resolution.hex.clone();
    let mut final_chunk_plan_count = chunk_digest_resolution.plan_chunk_count;
    let mut final_chunk_plan_label = chunk_digest_resolution.plan_label.clone();

    let mut bundle_manifest_digest_hex: Option<String> = None;
    let mut bundle_chunk_digest_hex: Option<String> = None;
    let mut bundle_chunk_plan_count: Option<u64> = None;
    let mut bundle_chunk_plan_label: Option<String> = None;
    let mut bundle_signature_hex: Option<String> = None;
    let mut bundle_public_key_hex: Option<String> = None;
    let mut bundle_token_hash: Option<String> = None;
    let mut bundle_token_source: Option<String> = None;

    if let Some(bundle_path) = &bundle_path {
        let bytes = fs::read(bundle_path)
            .map_err(|err| format!("failed to read bundle `{}`: {err}", bundle_path.display()))?;
        let value: Value = from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse bundle `{}` as JSON: {err}",
                bundle_path.display()
            )
        })?;
        let bundle_obj = value
            .as_object()
            .ok_or_else(|| format!("bundle `{}` must be a JSON object", bundle_path.display()))?;
        let manifest_obj = bundle_obj
            .get("manifest")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                format!(
                    "bundle `{}` missing `manifest` object",
                    bundle_path.display()
                )
            })?;
        bundle_manifest_digest_hex = manifest_obj
            .get("manifest_blake3_hex")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        bundle_chunk_digest_hex = manifest_obj
            .get("chunk_digest_sha3_256_hex")
            .or_else(|| manifest_obj.get("chunk_digest_sha3_hex"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        bundle_chunk_plan_count = manifest_obj
            .get("chunk_plan_chunk_count")
            .and_then(Value::as_u64);
        bundle_chunk_plan_label = manifest_obj
            .get("chunk_plan_source")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        let signature_obj = bundle_obj
            .get("signature")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                format!(
                    "bundle `{}` missing `signature` object",
                    bundle_path.display()
                )
            })?;
        bundle_signature_hex = signature_obj
            .get("signature_hex")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        bundle_public_key_hex = signature_obj
            .get("public_key_hex")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        if let Some(identity_obj) = bundle_obj.get("identity").and_then(Value::as_object) {
            bundle_token_hash = identity_obj
                .get("token_hash_blake3_hex")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            bundle_token_source = identity_obj
                .get("token_source")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
        }
    }

    if let Some(bundle_digest_hex) = &bundle_manifest_digest_hex {
        if !bundle_digest_hex.eq_ignore_ascii_case(&manifest_digest_hex) {
            return Err("bundle manifest digest does not match manifest payload".to_string());
        }
    } else if bundle_path.is_some() {
        return Err("bundle missing manifest digest field".to_string());
    }

    if let Some(bundle_hex) = &bundle_chunk_digest_hex {
        if let Some(local_hex) = &chunk_digest_resolution.hex
            && !local_hex.eq_ignore_ascii_case(bundle_hex)
        {
            return Err("chunk digest mismatch between bundle and local inputs".to_string());
        }
        final_chunk_digest_hex.get_or_insert_with(|| bundle_hex.clone());
    }

    if let Some(bundle_count) = bundle_chunk_plan_count {
        if let Some(local_count) = final_chunk_plan_count
            && local_count != bundle_count
        {
            return Err("chunk plan count mismatch between bundle and local inputs".to_string());
        } else {
            final_chunk_plan_count = Some(bundle_count);
        }
    }

    if let Some(bundle_label) = bundle_chunk_plan_label {
        if let Some(local_label) = &final_chunk_plan_label {
            if local_label != &bundle_label {
                return Err("chunk plan label mismatch between bundle and local inputs".to_string());
            }
        } else {
            final_chunk_plan_label = Some(bundle_label);
        }
    }

    let mut signature_hex = bundle_signature_hex.clone();
    let mut signature_source = bundle_path.as_ref().and_then(|path| {
        signature_hex
            .as_ref()
            .map(|_| format!("bundle:{}", path.display()))
    });

    if let Some(path) = &signature_path {
        let contents = fs::read_to_string(path)
            .map_err(|err| format!("failed to read signature `{}`: {err}", path.display()))?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Err(format!("signature file `{}` is empty", path.display()));
        }
        if let Some(existing) = &signature_hex
            && !existing.eq_ignore_ascii_case(trimmed)
        {
            return Err("signature from file does not match bundle signature".to_string());
        }
        signature_hex = Some(trimmed.to_string());
        signature_source = Some(format!("file:{}", path.display()));
    }

    let signature_hex = signature_hex.ok_or_else(|| {
        "signature material not provided; pass `--signature` or a bundle generated by `manifest sign`"
            .to_string()
    })?;

    let mut public_key_source = None;
    if let (Some(arg_hex), Some(bundle_hex)) = (&public_key_hex_arg, &bundle_public_key_hex)
        && !arg_hex.eq_ignore_ascii_case(bundle_hex)
    {
        return Err("public key from CLI does not match bundle".to_string());
    }

    let public_key_hex = public_key_hex_arg
        .clone()
        .or_else(|| bundle_public_key_hex.clone())
        .ok_or_else(|| {
            "public key not provided; supply `--public-key-hex` or include it via `--bundle`"
                .to_string()
        })?;

    if public_key_hex_arg.is_some() {
        public_key_source = Some("arg:--public-key-hex".to_string());
    } else if let Some(bundle) = bundle_path.as_ref() {
        public_key_source = Some(format!("bundle:{}", bundle.display()));
    }

    let signature_bytes_vec = parse_hex_vec(&signature_hex)?;
    if signature_bytes_vec.len() != 64 {
        return Err(format!(
            "signature must be 64 bytes (128 hex chars), found {} bytes",
            signature_bytes_vec.len()
        ));
    }
    let signature_bytes: [u8; 64] = signature_bytes_vec
        .try_into()
        .map_err(|_| "failed to convert signature bytes".to_string())?;
    let signature = Signature::from_bytes(&signature_bytes);

    let public_key_bytes_vec = parse_hex_vec(&public_key_hex)?;
    if public_key_bytes_vec.len() != 32 {
        return Err(format!(
            "public key must be 32 bytes (64 hex chars), found {} bytes",
            public_key_bytes_vec.len()
        ));
    }
    let public_key_bytes: [u8; 32] = public_key_bytes_vec
        .try_into()
        .map_err(|_| "failed to convert public key bytes".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|err| format!("invalid public key: {err}"))?;

    verifying_key
        .verify(manifest_digest.as_bytes(), &signature)
        .map_err(|err| format!("signature verification failed: {err}"))?;

    if let Some(expected_hash) = expect_token_hash {
        let bundle_hash = bundle_token_hash
            .as_ref()
            .ok_or_else(|| "bundle does not expose `token_hash_blake3_hex`".to_string())?;
        if !bundle_hash.eq_ignore_ascii_case(&expected_hash) {
            return Err("bundle token hash does not match expected value".to_string());
        }
    }

    let mut summary = Map::new();
    summary.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    summary.insert(
        "manifest_blake3_hex".into(),
        Value::from(manifest_digest_hex.clone()),
    );
    summary.insert("verification_status".into(), Value::from("ok"));
    summary.insert("public_key_hex".into(), Value::from(public_key_hex.clone()));
    if let Some(source) = public_key_source {
        summary.insert("public_key_source".into(), Value::from(source));
    }
    summary.insert("signature_hex".into(), Value::from(signature_hex.clone()));
    if let Some(source) = signature_source {
        summary.insert("signature_source".into(), Value::from(source));
    }
    if let Some(bundle_path) = bundle_path {
        summary.insert(
            "bundle_path".into(),
            Value::from(bundle_path.display().to_string()),
        );
    }
    if let Some(signature_path) = signature_path {
        summary.insert(
            "signature_path".into(),
            Value::from(signature_path.display().to_string()),
        );
    }
    if let Some(chunk_hex) = final_chunk_digest_hex.or(bundle_chunk_digest_hex) {
        summary.insert("chunk_digest_sha3_256_hex".into(), Value::from(chunk_hex));
    }
    if let Some(count) = final_chunk_plan_count {
        summary.insert("chunk_plan_chunk_count".into(), Value::from(count));
    }
    if let Some(label) = final_chunk_plan_label {
        summary.insert("chunk_plan_source".into(), Value::from(label));
    }
    if let Some(token_hash) = bundle_token_hash {
        summary.insert(
            "bundle_token_hash_blake3_hex".into(),
            Value::from(token_hash),
        );
    }
    if let Some(token_source) = bundle_token_source {
        summary.insert("bundle_token_source".into(), Value::from(token_source));
    }

    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render verification summary: {err}"))?;
    println!("{rendered}");

    Ok(())
}

#[derive(Debug, Clone)]
struct AliasInputs {
    namespace: String,
    name: String,
    proof: Vec<u8>,
}

struct ChunkDigestResult {
    digest: Option<[u8; 32]>,
    hex: Option<String>,
    plan_chunk_count: Option<u64>,
    plan_label: Option<String>,
}

fn resolve_chunk_digest(
    summary_source: Option<JsonSource>,
    chunk_plan_source: Option<JsonSource>,
    chunk_plan_label: Option<String>,
    chunk_digest_hex_arg: Option<String>,
    require_digest: bool,
    context: &str,
) -> Result<ChunkDigestResult, String> {
    let mut chunk_digest: Option<[u8; 32]> = None;
    let mut chunk_digest_hex: Option<String> = None;
    let mut plan_chunk_count: Option<u64> = None;
    let mut plan_label = chunk_plan_label;

    if let Some(summary_src) = summary_source {
        let value = summary_src.read()?;
        if let Some(hex) = value
            .get("chunk_digest_sha3_256_hex")
            .or_else(|| value.get("chunk_digest_sha3_hex"))
            .and_then(Value::as_str)
        {
            let parsed = parse_digest_hex(hex)
                .map_err(|err| format!("invalid chunk digest in summary JSON: {err}"))?;
            chunk_digest = Some(parsed);
            chunk_digest_hex = Some(hex.to_string());
        }
        if let Some(count) = value.get("chunk_plan_chunk_count").and_then(Value::as_u64) {
            plan_chunk_count = Some(count);
        }
        if plan_label.is_none()
            && let Some(label) = value
                .get("chunk_plan")
                .or_else(|| value.get("chunk_plan_source"))
                .and_then(Value::as_str)
        {
            plan_label = Some(label.to_string());
        }
    }

    if let Some(source) = chunk_plan_source {
        let plan_json = source.read()?;
        let specs = chunk_fetch_specs_from_json(&plan_json)
            .map_err(|err| format!("failed to parse chunk plan JSON: {err}"))?;
        let digest = chunk_digest_sha3_from_specs(&specs);
        if let Some(existing) = chunk_digest
            && existing != digest
        {
            return Err("chunk plan digest does not match digest derived from summary".to_string());
        }
        chunk_digest = Some(digest);
        chunk_digest_hex = Some(hex_encode(digest));
        plan_chunk_count = Some(specs.len() as u64);
    }

    if let Some(hex) = chunk_digest_hex_arg {
        let parsed = parse_digest_hex(&hex)
            .map_err(|err| format!("invalid `--chunk-digest-sha3` value: {err}"))?;
        if let Some(existing) = chunk_digest
            && existing != parsed
        {
            return Err(
                "`--chunk-digest-sha3` does not match digest derived from summary or chunk plan"
                    .to_string(),
            );
        }
        chunk_digest = Some(parsed);
        chunk_digest_hex = Some(hex);
    }

    chunk_digest = match chunk_digest {
        Some(digest) => {
            if chunk_digest_hex.is_none() {
                chunk_digest_hex = Some(hex_encode(digest));
            }
            Some(digest)
        }
        None => {
            if require_digest {
                return Err(format!(
                    "missing chunk digest for {context}; supply `--chunk-plan`, `--chunk-digest-sha3`, or a summary containing the digest"
                ));
            }
            None
        }
    };

    Ok(ChunkDigestResult {
        digest: chunk_digest,
        hex: chunk_digest_hex,
        plan_chunk_count,
        plan_label,
    })
}

fn parse_private_key_inline(value: &str) -> Result<PrivateKey, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("`--private-key` may not be empty".to_string());
    }
    PrivateKey::from_str(trimmed).map_err(|err| format!("failed to parse private key: {err}"))
}

fn load_private_key_from_file(path: &Path) -> Result<PrivateKey, String> {
    let contents = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read private key from `{}`: {err}",
            path.display()
        )
    })?;
    parse_private_key_inline(&contents)
}

fn alias_inputs_from_flags(
    namespace: Option<String>,
    name: Option<String>,
    proof_path: Option<PathBuf>,
) -> Result<Option<AliasInputs>, String> {
    match (namespace, name, proof_path) {
        (None, None, None) => Ok(None),
        (Some(ns), Some(name), Some(path)) => {
            let proof = fs::read(&path)
                .map_err(|err| format!("failed to read alias proof `{}`: {err}", path.display()))?;
            if proof.is_empty() {
                return Err(format!("alias proof file `{}` is empty", path.display()));
            }
            Ok(Some(AliasInputs {
                namespace: ns,
                name,
                proof,
            }))
        }
        _ => Err(
            "alias namespace, name, and proof must be provided together for `--alias-*` flags"
                .to_string(),
        ),
    }
}

fn build_pin_register_payload(
    authority_literal: &str,
    private_key: PrivateKey,
    manifest: &ManifestV1,
    chunk_digest_sha3: [u8; 32],
    submitted_epoch: u64,
    alias: Option<&AliasInputs>,
    successor_of: Option<[u8; 32]>,
) -> Result<Value, String> {
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let chunker = &manifest.chunking;
    let policy = &manifest.pin_policy;

    let mut storage_class_map = Map::new();
    storage_class_map.insert(
        "type".into(),
        Value::from(match policy.storage_class {
            StorageClass::Hot => "Hot",
            StorageClass::Warm => "Warm",
            StorageClass::Cold => "Cold",
        }),
    );

    let mut pin_policy_map = Map::new();
    pin_policy_map.insert(
        "min_replicas".into(),
        Value::from(policy.min_replicas as u64),
    );
    pin_policy_map.insert("storage_class".into(), Value::from(storage_class_map));
    pin_policy_map.insert(
        "retention_epoch".into(),
        Value::from(policy.retention_epoch),
    );

    let mut map = Map::new();
    map.insert(
        "authority".into(),
        Value::from(authority_literal.to_owned()),
    );
    map.insert(
        "private_key".into(),
        to_value(&ExposedPrivateKey(private_key))
            .map_err(|err| format!("failed to serialise private key: {err}"))?,
    );
    map.insert(
        "chunker_profile_id".into(),
        Value::from(u64::from(chunker.profile_id.0)),
    );
    map.insert(
        "chunker_namespace".into(),
        Value::from(chunker.namespace.clone()),
    );
    map.insert("chunker_name".into(), Value::from(chunker.name.clone()));
    map.insert("chunker_semver".into(), Value::from(chunker.semver.clone()));
    map.insert(
        "chunker_multihash_code".into(),
        Value::from(chunker.multihash_code),
    );
    map.insert("pin_policy".into(), Value::from(pin_policy_map));
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(hex_encode(manifest_digest.as_bytes())),
    );
    map.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::from(hex_encode(chunk_digest_sha3)),
    );
    map.insert(
        "content_length".into(),
        Value::from(manifest.content_length),
    );
    map.insert("submitted_epoch".into(), Value::from(submitted_epoch));

    if let Some(alias) = alias {
        let mut alias_map = Map::new();
        alias_map.insert("namespace".into(), Value::from(alias.namespace.clone()));
        alias_map.insert("name".into(), Value::from(alias.name.clone()));
        alias_map.insert(
            "proof_base64".into(),
            Value::from(BASE64_STANDARD.encode(&alias.proof)),
        );
        map.insert("alias".into(), Value::from(alias_map));
    }

    if let Some(successor) = successor_of {
        map.insert(
            "successor_of_hex".into(),
            Value::from(hex_encode(successor)),
        );
    }

    Ok(Value::Object(map))
}

struct ManifestProposalSummary<'a> {
    manifest_path: &'a Path,
    manifest: &'a ManifestV1,
    manifest_digest: &'a blake3::Hash,
    chunk_digest_sha3: [u8; 32],
    chunk_plan_label: Option<&'a str>,
    submitted_epoch: u64,
    alias_hint: Option<&'a str>,
    successor_bytes: Option<[u8; 32]>,
}

fn build_manifest_proposal_summary(summary: ManifestProposalSummary<'_>) -> Result<Value, String> {
    let ManifestProposalSummary {
        manifest_path,
        manifest,
        manifest_digest,
        chunk_digest_sha3,
        chunk_plan_label,
        submitted_epoch,
        alias_hint,
        successor_bytes,
    } = summary;
    let chunker_handle = chunker_handle_from_profile(&manifest.chunking);
    let policy_dm = convert_pin_policy(&manifest.pin_policy);
    let register_value = build_register_instruction_value(
        manifest_digest,
        &chunker_handle,
        chunk_digest_sha3,
        &policy_dm,
        submitted_epoch,
        successor_bytes,
    );

    let mut map = Map::new();
    map.insert("proposal_version".into(), Value::from(1_u64));
    map.insert(
        "manifest_path".into(),
        Value::from(manifest_path.display().to_string()),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::from(hex_encode(manifest_digest.as_bytes())),
    );
    map.insert(
        "chunk_digest_sha3_hex".into(),
        Value::from(hex_encode(chunk_digest_sha3)),
    );
    map.insert(
        "chunker_handle".into(),
        Value::from(chunker_handle.to_handle()),
    );
    map.insert(
        "pin_policy".into(),
        Value::Object(pin_policy_json(&manifest.pin_policy)),
    );
    map.insert("submitted_epoch".into(), Value::from(submitted_epoch));
    if let Some(label) = chunk_plan_label {
        map.insert("chunk_plan_source".into(), Value::from(label));
    }
    if let Some(alias) = alias_hint {
        map.insert("alias_hint".into(), Value::from(alias));
    }
    if let Some(bytes) = successor_bytes {
        map.insert("successor_of_hex".into(), Value::from(hex_encode(bytes)));
    }
    map.insert("register_instruction".into(), register_value);
    Ok(Value::Object(map))
}

fn chunker_handle_from_profile(profile: &ChunkingProfileV1) -> ChunkerProfileHandle {
    ChunkerProfileHandle {
        profile_id: profile.profile_id.0,
        namespace: profile.namespace.clone(),
        name: profile.name.clone(),
        semver: profile.semver.clone(),
        multihash_code: profile.multihash_code,
    }
}

fn convert_pin_policy(policy: &sorafs_manifest::PinPolicy) -> RegistryPinPolicy {
    RegistryPinPolicy {
        min_replicas: policy.min_replicas,
        storage_class: convert_storage_class(&policy.storage_class),
        retention_epoch: policy.retention_epoch,
    }
}

fn convert_storage_class(class: &sorafs_manifest::StorageClass) -> RegistryStorageClass {
    match class {
        sorafs_manifest::StorageClass::Hot => RegistryStorageClass::Hot,
        sorafs_manifest::StorageClass::Warm => RegistryStorageClass::Warm,
        sorafs_manifest::StorageClass::Cold => RegistryStorageClass::Cold,
    }
}

fn build_register_instruction_value(
    manifest_digest: &blake3::Hash,
    chunker_handle: &ChunkerProfileHandle,
    chunk_digest_sha3: [u8; 32],
    policy: &RegistryPinPolicy,
    submitted_epoch: u64,
    successor_bytes: Option<[u8; 32]>,
) -> Value {
    let mut register_map = Map::new();
    register_map.insert(
        "digest_hex".into(),
        Value::from(hex_encode(manifest_digest.as_bytes())),
    );
    register_map.insert(
        "chunker_handle".into(),
        Value::from(chunker_handle.to_handle()),
    );
    register_map.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::from(hex_encode(chunk_digest_sha3)),
    );
    register_map.insert("submitted_epoch".into(), Value::from(submitted_epoch));
    register_map.insert("policy".into(), registry_pin_policy_to_value(policy));
    if let Some(bytes) = successor_bytes {
        register_map.insert("successor_of_hex".into(), Value::from(hex_encode(bytes)));
    }
    Value::Object(register_map)
}

fn registry_pin_policy_to_value(policy: &RegistryPinPolicy) -> Value {
    let mut map = Map::new();
    map.insert("min_replicas".into(), Value::from(policy.min_replicas));
    map.insert(
        "storage_class".into(),
        Value::from(match policy.storage_class {
            RegistryStorageClass::Hot => "hot",
            RegistryStorageClass::Warm => "warm",
            RegistryStorageClass::Cold => "cold",
        }),
    );
    map.insert(
        "retention_epoch".into(),
        Value::from(policy.retention_epoch),
    );
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::json::Map;
    use sorafs_manifest::{
        GovernanceProofs, PinPolicy as ManifestPinPolicy, StorageClass as ManifestStorageClass,
    };
    use sorafs_orchestrator::{PolicyReport, PolicyStatus};
    use tempfile::tempdir;

    use super::*;

    fn sample_manifest() -> ManifestV1 {
        let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
        ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
            .chunking_profile(ChunkingProfileV1::from_descriptor(descriptor))
            .content_length(1_024)
            .car_digest([0xAB; 32])
            .car_size(2_048)
            .pin_policy(ManifestPinPolicy {
                min_replicas: 3,
                storage_class: ManifestStorageClass::Warm,
                retention_epoch: 64,
            })
            .governance(GovernanceProofs {
                council_signatures: Vec::new(),
            })
            .extend_metadata([("release".into(), "test".into())])
            .build()
            .expect("manifest build")
    }

    fn fixture_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive SoraFS CLI fixture key")
    }

    fn fixture_account(seed: u8) -> AccountId {
        AccountId::new(fixture_keypair(seed).public_key().clone())
    }

    #[test]
    fn fixture_account_uses_checked_seed_derivation() {
        let account = fixture_account(0x5A);
        let expected = AccountId::new(fixture_keypair(0x5A).public_key().clone());

        assert_eq!(account, expected);
    }

    #[test]
    fn render_por_trigger_response_formats_json_and_fallbacks() {
        let rendered = render_por_trigger_response(br#"{"status":"ok","accepted":true}"#)
            .expect("render JSON");
        let parsed: Value = norito::json::from_str(&rendered).expect("parse rendered JSON");
        assert_eq!(parsed.get("status").and_then(Value::as_str), Some("ok"));
        assert_eq!(parsed.get("accepted").and_then(Value::as_bool), Some(true));

        let text = render_por_trigger_response(b" accepted\n").expect("render text");
        assert_eq!(text, "accepted");

        let binary = render_por_trigger_response(&[0xFF, 0x00]).expect("render binary fallback");
        assert_eq!(
            binary,
            "manual PoR challenge accepted (2 bytes response payload)."
        );
    }

    #[test]
    fn load_storage_pin_payload_uses_canonical_directory_ordering() {
        let tempdir = tempdir().expect("tempdir");
        let payload_dir = tempdir.path().join("site");
        fs::create_dir_all(payload_dir.join("assets")).expect("create payload dir");
        fs::write(payload_dir.join("index.html"), "<html>hayahi</html>").expect("write index");
        fs::write(
            payload_dir.join("assets").join("app.js"),
            "console.log('hayahi');",
        )
        .expect("write script");

        let manifest = sample_manifest();
        let profile = chunk_profile_from_manifest(&manifest).expect("chunk profile");
        let (expected_plan, expected_payload) =
            CarBuildPlan::from_directory_with_profile(&payload_dir, profile)
                .expect("build canonical directory payload");

        let (payload, files, payload_kind) =
            load_storage_pin_payload(&payload_dir, &manifest).expect("load storage payload");

        assert_eq!(payload_kind, "directory");
        assert_eq!(payload, expected_payload);

        let files = files.expect("directory payload should include file entries");
        let expected_files = expected_plan
            .files
            .iter()
            .map(|file| StorageFileEntryOwned {
                path: file.path.clone(),
                size: file.size,
            })
            .collect::<Vec<_>>();
        assert_eq!(files, expected_files);
    }

    #[test]
    fn manifest_submit_fallback_transaction_checked_signing_verifies() {
        use iroha_data_model::{
            ChainId,
            prelude::{InstructionBox, TransactionBuilder},
            sorafs::pin_registry::ManifestDigest,
        };

        let manifest = sample_manifest();
        let manifest_digest = manifest.digest().expect("manifest digest");
        let instruction = iroha_data_model::isi::sorafs::RegisterPinManifest::new(
            ManifestDigest::new(*manifest_digest.as_bytes()),
            chunker_handle_from_profile(&manifest.chunking),
            [0xCC; 32],
            manifest.content_length,
            convert_pin_policy(&manifest.pin_policy),
            42,
            None,
            Some(ManifestDigest::new([0xDD; 32])),
        );
        let keypair = fixture_keypair(0xA5);
        let authority = AccountId::new(keypair.public_key().clone());

        let tx = sign_manifest_submit_fallback_transaction(
            TransactionBuilder::new(ChainId::from("test-chain".to_owned()), authority.clone())
                .with_instructions([InstructionBox::from(instruction)]),
            keypair.private_key(),
        )
        .expect("checked fallback transaction signing");

        assert_eq!(tx.authority(), &authority);
        tx.verify_signature()
            .expect("checked fallback transaction signature should verify");
    }

    #[test]
    fn parse_account_id_arg_with_prefix_accepts_matching_i105_discriminant() {
        let account = fixture_account(0x5A);
        let encoded = account
            .to_i105_for_discriminant(369)
            .expect("encode i105 with taira discriminant");

        let parsed = parse_account_id_arg_with_prefix(
            "--authority",
            &encoded,
            "sorafs_cli manifest submit",
            Some(369),
        )
        .expect("parse authority with explicit discriminant");

        assert_eq!(parsed, account);
    }

    #[test]
    fn parse_account_id_arg_accepts_taira_i105_without_explicit_prefix() {
        let account = fixture_account(0x59);
        let encoded = account
            .to_i105_for_discriminant(369)
            .expect("encode i105 with taira discriminant");

        let parsed = parse_account_id_arg("--authority", &encoded, "sorafs_cli manifest submit")
            .expect("parse taira authority without explicit discriminant");

        assert_eq!(parsed, account);
    }

    #[test]
    fn parse_account_id_arg_with_prefix_rejects_mismatched_i105_discriminant() {
        let account = fixture_account(0x6B);
        let encoded = account
            .to_i105_for_discriminant(369)
            .expect("encode i105 with taira discriminant");

        let err = parse_account_id_arg_with_prefix(
            "--authority",
            &encoded,
            "sorafs_cli manifest submit",
            Some(753),
        )
        .expect_err("mismatched discriminant should fail");

        assert!(
            err.contains("ERR_UNEXPECTED_NETWORK_PREFIX"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn authority_payload_literal_preserves_explicit_i105_discriminant() {
        let _guard = iroha_data_model::account::address::ChainDiscriminantGuard::enter(753);
        let account = fixture_account(0x7C);
        let expected = account
            .to_i105_for_discriminant(369)
            .expect("encode taira authority");

        let literal =
            authority_payload_literal(&account, Some(369)).expect("render authority payload");

        assert_eq!(literal, expected);
    }

    #[test]
    fn build_pin_register_payload_uses_supplied_authority_literal() {
        let manifest = sample_manifest();
        let account = fixture_account(0x8D);
        let authority_literal = account
            .to_i105_for_discriminant(369)
            .expect("encode taira authority");

        let payload = build_pin_register_payload(
            &authority_literal,
            fixture_keypair(0x9E).private_key().clone(),
            &manifest,
            [0xCD; 32],
            99,
            None,
            None,
        )
        .expect("build payload");

        assert_eq!(
            payload["authority"].as_str().expect("authority literal"),
            authority_literal
        );
        assert_eq!(
            payload["content_length"].as_u64().expect("content length"),
            manifest.content_length
        );
    }

    #[test]
    fn proposal_summary_contains_register_instruction() {
        let manifest = sample_manifest();
        let digest = manifest.digest().expect("digest");
        let summary = build_manifest_proposal_summary(ManifestProposalSummary {
            manifest_path: Path::new("/tmp/manifest.to"),
            manifest: &manifest,
            manifest_digest: &digest,
            chunk_digest_sha3: [0xCD; 32],
            chunk_plan_label: Some("plan.json"),
            submitted_epoch: 99,
            alias_hint: Some("docs.sora.link"),
            successor_bytes: None,
        })
        .expect("proposal summary");

        assert_eq!(summary["proposal_version"], Value::from(1_u64));
        assert_eq!(
            summary["manifest_digest_hex"].as_str().expect("digest hex"),
            hex_encode(digest.as_bytes())
        );
        assert_eq!(
            summary["chunk_plan_source"].as_str().expect("plan label"),
            "plan.json"
        );
        assert_eq!(
            summary["alias_hint"].as_str().expect("alias hint"),
            "docs.sora.link"
        );
        assert!(
            summary
                .get("register_instruction")
                .expect("register instruction")
                .is_object(),
            "register instruction serialized as object"
        );
    }

    #[test]
    fn resolve_chain_id_from_registry_value_reads_attestation_chain_id() {
        let mut attestation = Map::new();
        attestation.insert("chain_id".into(), Value::from("test-chain"));
        let mut root = Map::new();
        root.insert("attestation".into(), Value::Object(attestation));

        assert_eq!(
            resolve_chain_id_from_registry_value(&Value::Object(root)).expect("chain id"),
            "test-chain"
        );
    }

    #[test]
    fn resolve_chain_id_from_registry_value_rejects_missing_attestation_chain_id() {
        let err = resolve_chain_id_from_registry_value(&Value::Object(Map::new()))
            .expect_err("missing attestation chain id should fail");
        assert!(
            err.contains("attestation.chain_id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn manifest_submit_status_fallback_detects_missing_route() {
        assert!(should_fallback_manifest_submit_status(
            StatusCode::METHOD_NOT_ALLOWED
        ));
        assert!(should_fallback_manifest_submit_status(
            StatusCode::NOT_FOUND
        ));
        assert!(should_fallback_manifest_submit_status(
            StatusCode::NOT_IMPLEMENTED
        ));
        assert!(!should_fallback_manifest_submit_status(
            StatusCode::BAD_REQUEST
        ));
    }

    #[test]
    fn storage_class_conversion_matches_variants() {
        assert!(matches!(
            convert_storage_class(&ManifestStorageClass::Hot),
            RegistryStorageClass::Hot
        ));
        assert!(matches!(
            convert_storage_class(&ManifestStorageClass::Warm),
            RegistryStorageClass::Warm
        ));
        assert!(matches!(
            convert_storage_class(&ManifestStorageClass::Cold),
            RegistryStorageClass::Cold
        ));
    }

    #[test]
    fn proof_stream_evidence_helper_writes_bundle() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("sample_manifest.norito");
        fs::write(&manifest_path, b"norito-data").expect("write manifest");
        let evidence_dir = temp.path().join("evidence");
        let summary_json = r#"{"proof_kind":"por"}"#;

        write_proof_stream_evidence(
            &evidence_dir,
            &manifest_path,
            "deadbeef",
            summary_json,
            "https://torii.sora",
        )
        .expect("evidence bundle");

        let summary_path = evidence_dir.join("proof_stream_summary.json");
        assert!(summary_path.exists(), "summary file created");
        let written_summary = fs::read_to_string(&summary_path).expect("read summary");
        assert!(
            written_summary.contains("\"proof_kind\""),
            "summary data preserved"
        );

        let metadata_path = evidence_dir.join("metadata.json");
        let metadata_bytes = fs::read(&metadata_path).expect("read metadata");
        let metadata_value: Value =
            norito::json::from_slice(&metadata_bytes).expect("metadata json");
        assert_eq!(
            metadata_value["manifest_digest_hex"],
            Value::from("deadbeef")
        );
        assert_eq!(
            metadata_value["manifest_copy"],
            Value::from("sample_manifest.norito")
        );
        let manifest_copy_path = evidence_dir.join("sample_manifest.norito");
        assert_eq!(
            fs::read(&manifest_copy_path).expect("read copied manifest"),
            b"norito-data"
        );
    }

    #[test]
    fn gateway_scoreboard_metadata_records_telemetry_source() {
        let metadata = build_gateway_scoreboard_metadata(&GatewayScoreboardMetadataInput {
            provider_counts: GatewayProviderCounts::new(0, 2),
            max_peers: Some(3),
            retry_budget: None,
            manifest_envelope_present: true,
            gateway_manifest_id: Some("feedface"),
            gateway_manifest_cid: None,
            transport_policy: Some(TransportPolicy::SoranetPreferred),
            transport_policy_override: None,
            anonymity_policy: Some(AnonymityPolicy::GuardPq),
            anonymity_policy_override: None,
            write_mode: WriteModeHint::ReadOnly,
            scoreboard_now: Some(1_234),
            telemetry_source: Some("otel::ci"),
        });
        let map = metadata.as_object().expect("metadata object");
        assert_eq!(
            map.get("telemetry_source").and_then(Value::as_str),
            Some("otel::ci")
        );
        assert_eq!(
            map.get("provider_mix").and_then(Value::as_str),
            Some("gateway-only")
        );
        assert_eq!(
            map.get("write_mode").and_then(Value::as_str),
            Some("read-only")
        );
        assert_eq!(
            map.get("write_mode_enforces_pq").and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn fetch_summary_records_write_mode_hint() {
        let outcome = sorafs_car::multi_fetch::FetchOutcome {
            chunks: Vec::new(),
            chunk_receipts: Vec::new(),
            provider_reports: Vec::new(),
        };
        let policy_report = PolicyReport {
            policy: AnonymityPolicy::StrictPq,
            effective_policy: AnonymityPolicy::StrictPq,
            total_candidates: 1,
            pq_candidates: 1,
            selected_soranet_total: 1,
            selected_pq: 1,
            status: PolicyStatus::Met,
            fallback_reason: None,
        };
        let plan = CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: blake3_hash(&[]),
            content_length: 0,
            chunks: Vec::new(),
            files: Vec::new(),
        };
        let session = FetchSession {
            outcome,
            policy_report,
            local_proxy_manifest: None,
            car_verification: None,
            taikai_cache_stats: None,
            taikai_cache_queue: None,
        };
        let summary = build_fetch_summary(
            "deadbeef",
            "sorafs.sf1@1.0.0",
            &plan,
            &session,
            FetchSummaryOptions {
                client_id: None,
                rollout_phase: RolloutPhase::Default,
                write_mode: WriteModeHint::UploadPqOnly,
                cache_profile: None,
            },
        );
        let map = summary.as_object().expect("summary object");
        assert_eq!(
            map.get("write_mode").and_then(Value::as_str),
            Some("upload-pq-only")
        );
        assert_eq!(
            map.get("write_mode_enforces_pq").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn insert_telemetry_source_injects_label() {
        let mut summary = Value::Object(Map::new());
        insert_telemetry_source(&mut summary, Some("otel::staging"));
        let object = summary.as_object().expect("summary object");
        assert_eq!(
            object.get("telemetry_source").and_then(Value::as_str),
            Some("otel::staging")
        );

        insert_telemetry_source(&mut summary, None);
        let object = summary.as_object().expect("summary object");
        assert_eq!(
            object.get("telemetry_source").and_then(Value::as_str),
            Some("otel::staging"),
            "missing label should not remove existing value"
        );
    }
}

fn chunk_digest_sha3_from_specs(specs: &[ChunkFetchSpec]) -> [u8; 32] {
    let mut ordered = specs.to_vec();
    ordered.sort_by_key(|spec| spec.chunk_index);
    let mut hasher = Sha3_256::new();
    for spec in ordered {
        hasher.update(spec.offset.to_le_bytes());
        hasher.update(u64::from(spec.length).to_le_bytes());
        hasher.update(spec.digest);
    }
    hasher.finalize().into()
}

fn chunk_digest_sha3_from_chunks(chunks: &[StoredChunk]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    for chunk in chunks {
        hasher.update(chunk.offset.to_le_bytes());
        hasher.update(u64::from(chunk.length).to_le_bytes());
        hasher.update(chunk.blake3);
    }
    hasher.finalize().into()
}

fn parse_bool_flag(value: &str, flag: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!("{flag} expects a boolean value (true|false)")),
    }
}

fn decode_jwt_sections(token: &str) -> Result<(Value, Value), String> {
    let mut segments = token.split('.');
    let header_segment = segments
        .next()
        .ok_or_else(|| "identity token missing header segment".to_string())?;
    let claims_segment = segments
        .next()
        .ok_or_else(|| "identity token missing payload segment".to_string())?;
    let signature_segment = segments
        .next()
        .ok_or_else(|| "identity token missing signature segment".to_string())?;
    if segments.next().is_some() {
        return Err("identity token contains unexpected extra segments".to_string());
    }
    if signature_segment.trim().is_empty() {
        return Err("identity token signature segment is empty".to_string());
    }

    let header_bytes = decode_base64url_segment(header_segment)
        .map_err(|err| format!("failed to decode identity token header: {err}"))?;
    let claims_bytes = decode_base64url_segment(claims_segment)
        .map_err(|err| format!("failed to decode identity token payload: {err}"))?;

    let header_value = if header_bytes.is_empty() {
        Value::Object(Map::new())
    } else {
        from_slice(&header_bytes)
            .map_err(|err| format!("failed to parse identity token header JSON: {err}"))?
    };
    let claims_value = if claims_bytes.is_empty() {
        Value::Object(Map::new())
    } else {
        from_slice(&claims_bytes)
            .map_err(|err| format!("failed to parse identity token payload JSON: {err}"))?
    };

    Ok((header_value, claims_value))
}

fn decode_base64url_segment(segment: &str) -> Result<Vec<u8>, String> {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    BASE64_URL_SAFE_NO_PAD
        .decode(trimmed.as_bytes())
        .or_else(|_| {
            let mut padded = trimmed.to_string();
            while !padded.len().is_multiple_of(4) {
                padded.push('=');
            }
            BASE64_URL_SAFE.decode(padded.as_bytes())
        })
        .map_err(|err| format!("{err}"))
}

fn build_plan_from_specs(
    plan_json: &Value,
    chunker_handle_hint: Option<&str>,
) -> Result<PlanWithHandle, String> {
    let mut chunk_specs = chunk_fetch_specs_from_json(plan_json)
        .map_err(|err| format!("failed to parse chunk fetch specs: {err}"))?;
    if chunk_specs.is_empty() {
        return Err("chunk fetch plan contained no entries".into());
    }

    chunk_specs.sort_by_key(|spec| spec.chunk_index);
    for (idx, spec) in chunk_specs.iter().enumerate() {
        if spec.chunk_index != idx {
            return Err(format!(
                "chunk fetch specs missing chunk index {} (found {})",
                idx, spec.chunk_index
            ));
        }
    }

    let content_length = chunk_specs
        .iter()
        .map(|spec| spec.offset + u64::from(spec.length))
        .max()
        .ok_or_else(|| "failed to derive content length from chunk fetch specs".to_string())?;

    let (chunk_profile, resolved_handle) = if let Some(handle) = chunker_handle_hint {
        let trimmed = handle.trim();
        let descriptor = chunker_registry::lookup_by_handle(trimmed).ok_or_else(|| {
            format!(
                "unknown chunker handle `{trimmed}`; register the profile or provide a valid handle"
            )
        })?;
        (descriptor.profile, trimmed.to_string())
    } else if let Some(descriptor) = chunker_registry::lookup_by_profile(
        ChunkProfile::DEFAULT,
        chunker_registry::DEFAULT_MULTIHASH_CODE,
    ) {
        (
            descriptor.profile,
            format!(
                "{}.{}@{}",
                descriptor.namespace, descriptor.name, descriptor.semver
            ),
        )
    } else {
        (ChunkProfile::DEFAULT, DEFAULT_CHUNKER_HANDLE.to_string())
    };

    let plan = CarBuildPlan {
        chunk_profile,
        payload_digest: blake3_hash(&[]),
        content_length,
        chunks: chunk_specs
            .iter()
            .map(|spec| CarChunk {
                offset: spec.offset,
                length: spec.length,
                digest: spec.digest,
                taikai_segment_hint: spec.taikai_segment_hint.clone(),
            })
            .collect(),
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: chunk_specs.len(),
            size: content_length,
        }],
    };

    Ok(PlanWithHandle {
        plan,
        chunker_handle: resolved_handle,
    })
}

struct FetchSummaryOptions<'a> {
    client_id: Option<&'a str>,
    rollout_phase: RolloutPhase,
    write_mode: WriteModeHint,
    cache_profile: Option<FetchCacheProfile>,
}

fn build_fetch_summary(
    manifest_id_hex: &str,
    chunker_handle: &str,
    plan: &CarBuildPlan,
    session: &FetchSession,
    options: FetchSummaryOptions<'_>,
) -> Value {
    let outcome = &session.outcome;
    let policy_report = &session.policy_report;
    let mut root = Map::new();
    root.insert("manifest_id_hex".into(), Value::from(manifest_id_hex));
    root.insert("chunker_handle".into(), Value::from(chunker_handle));
    root.insert(
        "rollout_phase".into(),
        Value::from(options.rollout_phase.label()),
    );
    let write_mode_label = options.write_mode.label().replace('_', "-");
    root.insert("write_mode".into(), Value::from(write_mode_label));
    root.insert(
        "write_mode_enforces_pq".into(),
        Value::from(options.write_mode.enforces_pq_only()),
    );
    if let Some(client) = options.client_id {
        root.insert("client_id".into(), Value::from(client));
    }
    if let Some(profile) = options.cache_profile {
        let label = profile.label();
        root.insert("cache_profile".into(), Value::from(label));
        root.insert("cache_state".into(), Value::from(label));
    }
    root.insert("chunk_count".into(), Value::from(plan.chunks.len() as u64));
    root.insert("content_length".into(), Value::from(plan.content_length));
    let assembled_bytes: u64 = outcome.chunks.iter().map(|chunk| chunk.len() as u64).sum();
    root.insert("assembled_bytes".into(), Value::from(assembled_bytes));

    let provider_reports = outcome
        .provider_reports
        .iter()
        .map(|report| {
            let mut obj = Map::new();
            obj.insert(
                "provider".into(),
                Value::from(report.provider.id().as_str()),
            );
            obj.insert("successes".into(), Value::from(report.successes as u64));
            obj.insert("failures".into(), Value::from(report.failures as u64));
            obj.insert("disabled".into(), Value::from(report.disabled));
            Value::Object(obj)
        })
        .collect();
    root.insert("provider_reports".into(), Value::Array(provider_reports));

    let receipts = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| {
            let mut obj = Map::new();
            obj.insert(
                "chunk_index".into(),
                Value::from(receipt.chunk_index as u64),
            );
            obj.insert("provider".into(), Value::from(receipt.provider.as_str()));
            obj.insert("attempts".into(), Value::from(receipt.attempts as u64));
            Value::Object(obj)
        })
        .collect();
    root.insert("chunk_receipts".into(), Value::Array(receipts));
    if let Some(manifest) = &session.local_proxy_manifest {
        let manifest_json =
            to_value(manifest).expect("local proxy manifest should serialise to JSON");
        root.insert("local_proxy_manifest".into(), manifest_json);
    }
    if let Some(stats) = session.taikai_cache_stats {
        root.insert(
            "taikai_cache_summary".into(),
            taikai_cache_stats_to_value(stats),
        );
    }
    if let Some(queue_stats) = session.taikai_cache_queue {
        root.insert(
            "taikai_cache_queue".into(),
            taikai_cache_queue_to_value(queue_stats),
        );
    }
    if let Some(verification) = &session.car_verification {
        root.insert(
            "manifest_digest_hex".into(),
            Value::from(hex_encode(verification.manifest_digest.as_bytes())),
        );
        root.insert(
            "manifest_payload_digest_hex".into(),
            Value::from(hex_encode(verification.manifest_payload_digest.as_bytes())),
        );
        root.insert(
            "manifest_car_digest_hex".into(),
            Value::from(hex_encode(verification.manifest_car_digest)),
        );
        root.insert(
            "manifest_content_length".into(),
            Value::from(verification.manifest_content_length),
        );
        root.insert(
            "manifest_chunk_count".into(),
            Value::from(verification.manifest_chunk_count),
        );
        root.insert(
            "manifest_chunk_profile_handle".into(),
            Value::from(verification.chunk_profile_handle.clone()),
        );
        let governance_signatures: Vec<Value> = verification
            .manifest_governance
            .council_signatures
            .iter()
            .map(|signature| {
                let mut obj = Map::new();
                obj.insert(
                    "signer_hex".into(),
                    Value::from(hex_encode(signature.signer)),
                );
                obj.insert(
                    "signature_hex".into(),
                    Value::from(hex_encode(&signature.signature)),
                );
                Value::Object(obj)
            })
            .collect();
        let mut governance_obj = Map::new();
        governance_obj.insert(
            "council_signatures".into(),
            Value::Array(governance_signatures),
        );
        root.insert("manifest_governance".into(), Value::Object(governance_obj));

        let mut car_obj = Map::new();
        car_obj.insert("size".into(), Value::from(verification.car_stats.car_size));
        car_obj.insert(
            "payload_digest_hex".into(),
            Value::from(hex_encode(
                verification.car_stats.car_payload_digest.as_bytes(),
            )),
        );
        car_obj.insert(
            "archive_digest_hex".into(),
            Value::from(hex_encode(
                verification.car_stats.car_archive_digest.as_bytes(),
            )),
        );
        car_obj.insert(
            "cid_hex".into(),
            Value::from(hex_encode(&verification.car_stats.car_cid)),
        );
        car_obj.insert(
            "root_cids_hex".into(),
            Value::Array(
                verification
                    .car_stats
                    .root_cids
                    .iter()
                    .map(|cid| Value::from(hex_encode(cid)))
                    .collect(),
            ),
        );
        car_obj.insert("verified".into(), Value::from(true));
        car_obj.insert(
            "por_leaf_count".into(),
            Value::from(verification.por_leaf_count as u64),
        );
        root.insert("car_archive".into(), Value::Object(car_obj));
    }

    root.insert(
        "anonymity_policy".into(),
        Value::from(anonymity_policy_label(policy_report.policy).to_string()),
    );
    root.insert(
        "anonymity_status".into(),
        Value::from(policy_report.status_label()),
    );
    root.insert(
        "anonymity_reason".into(),
        Value::from(policy_report.reason_label()),
    );
    root.insert(
        "anonymity_soranet_selected".into(),
        Value::from(policy_report.selected_soranet_total as u64),
    );
    root.insert(
        "anonymity_pq_selected".into(),
        Value::from(policy_report.selected_pq as u64),
    );
    root.insert(
        "anonymity_classical_selected".into(),
        Value::from(policy_report.selected_classical() as u64),
    );
    root.insert(
        "anonymity_classical_ratio".into(),
        Value::from(policy_report.classical_ratio()),
    );
    root.insert(
        "anonymity_pq_ratio".into(),
        Value::from(policy_report.pq_ratio()),
    );
    root.insert(
        "anonymity_candidate_ratio".into(),
        Value::from(policy_report.candidate_ratio()),
    );
    root.insert(
        "anonymity_deficit_ratio".into(),
        Value::from(policy_report.deficit_ratio()),
    );
    root.insert(
        "anonymity_supply_delta".into(),
        Value::from(policy_report.supply_delta_ratio()),
    );
    root.insert(
        "anonymity_brownout".into(),
        Value::from(policy_report.is_brownout()),
    );
    root.insert(
        "anonymity_brownout_effective".into(),
        Value::from(policy_report.should_flag_brownout()),
    );
    root.insert(
        "anonymity_uses_classical".into(),
        Value::from(policy_report.uses_classical()),
    );

    Value::Object(root)
}

fn taikai_cache_stats_to_value(stats: TaikaiCacheStatsSnapshot) -> Value {
    let mut map = Map::new();
    map.insert(
        "hits".into(),
        tier_counts_value(stats.hits.hot, stats.hits.warm, stats.hits.cold),
    );
    map.insert("misses".into(), Value::from(stats.misses));
    map.insert(
        "inserts".into(),
        tier_counts_value(stats.inserts.hot, stats.inserts.warm, stats.inserts.cold),
    );
    let mut evictions = Map::new();
    evictions.insert(
        "hot".into(),
        reason_counts_value(stats.evictions.hot.expired, stats.evictions.hot.capacity),
    );
    evictions.insert(
        "warm".into(),
        reason_counts_value(stats.evictions.warm.expired, stats.evictions.warm.capacity),
    );
    evictions.insert(
        "cold".into(),
        reason_counts_value(stats.evictions.cold.expired, stats.evictions.cold.capacity),
    );
    map.insert("evictions".into(), Value::Object(evictions));
    map.insert(
        "promotions".into(),
        promotion_counts_value(
            stats.promotions.warm_to_hot,
            stats.promotions.cold_to_warm,
            stats.promotions.cold_to_hot,
        ),
    );
    map.insert(
        "qos_denials".into(),
        qos_counts_value(
            stats.qos_denials.priority,
            stats.qos_denials.standard,
            stats.qos_denials.bulk,
        ),
    );
    Value::Object(map)
}

fn taikai_cache_queue_to_value(stats: TaikaiPullQueueStats) -> Value {
    let mut map = Map::new();
    map.insert(
        "pending_segments".into(),
        Value::from(stats.pending_segments),
    );
    map.insert("pending_bytes".into(), Value::from(stats.pending_bytes));
    map.insert("pending_batches".into(), Value::from(stats.pending_batches));
    map.insert(
        "in_flight_batches".into(),
        Value::from(stats.in_flight_batches),
    );
    map.insert("hedged_batches".into(), Value::from(stats.hedged_batches));
    map.insert(
        "shaper_denials".into(),
        qos_counts_value(
            stats.shaper_denials.priority,
            stats.shaper_denials.standard,
            stats.shaper_denials.bulk,
        ),
    );
    map.insert(
        "dropped_segments".into(),
        Value::from(stats.dropped_segments),
    );
    map.insert("failovers".into(), Value::from(stats.failovers));
    map.insert("open_circuits".into(), Value::from(stats.open_circuits));
    Value::Object(map)
}

fn tier_counts_value(hot: u64, warm: u64, cold: u64) -> Value {
    let mut map = Map::new();
    map.insert("hot".into(), Value::from(hot));
    map.insert("warm".into(), Value::from(warm));
    map.insert("cold".into(), Value::from(cold));
    Value::Object(map)
}

fn reason_counts_value(expired: u64, capacity: u64) -> Value {
    let mut map = Map::new();
    map.insert("expired".into(), Value::from(expired));
    map.insert("capacity".into(), Value::from(capacity));
    Value::Object(map)
}

fn promotion_counts_value(warm_to_hot: u64, cold_to_warm: u64, cold_to_hot: u64) -> Value {
    let mut map = Map::new();
    map.insert("warm_to_hot".into(), Value::from(warm_to_hot));
    map.insert("cold_to_warm".into(), Value::from(cold_to_warm));
    map.insert("cold_to_hot".into(), Value::from(cold_to_hot));
    Value::Object(map)
}

fn qos_counts_value(priority: u64, standard: u64, bulk: u64) -> Value {
    let mut map = Map::new();
    map.insert("priority".into(), Value::from(priority));
    map.insert("standard".into(), Value::from(standard));
    map.insert("bulk".into(), Value::from(bulk));
    Value::Object(map)
}

fn parse_gateway_provider_spec(value: &str) -> Result<GatewayProviderSpec, String> {
    let mut name: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut stream_token: Option<String> = None;
    let mut privacy_events_url: Option<String> = None;

    for pair in value.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair
            .split_once('=')
            .ok_or_else(|| "--provider expects comma-separated key=value pairs".to_string())?;
        let val = val.trim();
        match key {
            "name" => {
                if val.is_empty() {
                    return Err("--provider name must not be empty".into());
                }
                name = Some(val.to_string());
            }
            "provider-id" | "provider_id" => {
                if val.len() != 64 || !val.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("--provider provider-id must be 32-byte hex".into());
                }
                provider_id = Some(val.to_ascii_lowercase());
            }
            "base-url" | "base_url" => {
                if val.is_empty() {
                    return Err("--provider base-url must not be empty".into());
                }
                base_url = Some(val.to_string());
            }
            "stream-token" | "stream_token" => {
                if val.is_empty() {
                    return Err("--provider stream-token must not be empty".into());
                }
                stream_token = Some(val.to_string());
            }
            "privacy-url" | "privacy_url" => {
                if val.is_empty() {
                    return Err("--provider privacy-url must not be empty".into());
                }
                privacy_events_url = Some(val.to_string());
            }
            other => return Err(format!("unknown key `{other}` in --provider argument")),
        }
    }

    let name = name.ok_or_else(|| "--provider requires a `name=` entry".to_string())?;
    let provider_id_hex =
        provider_id.ok_or_else(|| "--provider requires a `provider-id=` entry".to_string())?;
    let base_url = base_url.ok_or_else(|| "--provider requires a `base-url=` entry".to_string())?;
    let stream_token_b64 =
        stream_token.ok_or_else(|| "--provider requires a `stream-token=` entry".to_string())?;

    Ok(GatewayProviderSpec {
        name,
        provider_id_hex,
        base_url,
        stream_token_b64,
        privacy_events_url,
    })
}

fn parse_usize(raw: &str, flag: &str) -> Result<usize, String> {
    raw.trim()
        .parse::<usize>()
        .map_err(|err| format!("invalid {flag} value `{raw}`: {err}"))
}

fn parse_taikai_cache_override(value: Value) -> Result<Option<TaikaiCacheConfig>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let inner = match value {
        Value::Object(mut map) => {
            if let Some(embedded) = map.remove("taikai_cache") {
                embedded
            } else {
                Value::Object(map)
            }
        }
        other => other,
    };
    let mut wrapper = Map::new();
    wrapper.insert("taikai_cache".into(), inner);
    let parsed = orchestrator_config_from_json(&Value::Object(wrapper))
        .map_err(|err| format!("failed to parse Taikai cache config: {err}"))?;
    Ok(parsed.taikai_cache)
}

fn parse_storage_class(value: &str) -> Result<StorageClass, String> {
    match value.to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        _ => Err(format!(
            "invalid storage class `{value}`; expected hot|warm|cold"
        )),
    }
}

fn parse_iso_week_arg(raw: &str) -> Result<PorReportIsoWeek, String> {
    let trimmed = raw.trim();
    let (year_part, week_part) = if let Some((year, week)) = trimmed.split_once("-W") {
        (year, week)
    } else if let Some((year, week)) = trimmed.split_once('W') {
        (year, week)
    } else if let Some((year, week)) = trimmed.split_once('-') {
        (year, week)
    } else {
        return Err(format!(
            "invalid ISO week `{trimmed}`; expected format YYYY-Www"
        ));
    };
    let year = year_part.trim().parse::<u16>().map_err(|err| {
        format!("invalid ISO week year `{year_part}` supplied to `--week`: {err}")
    })?;
    let week = week_part.trim().parse::<u8>().map_err(|err| {
        format!("invalid ISO week number `{week_part}` supplied to `--week`: {err}")
    })?;
    let week_id = PorReportIsoWeek { year, week };
    week_id
        .validate()
        .map_err(|err| format!("invalid ISO week `{trimmed}`: {err}"))?;
    Ok(week_id)
}

fn render_status_table(entries: &[PorChallengeStatusV1]) -> String {
    if entries.is_empty() {
        return "No PoR challenges found.".to_string();
    }
    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        "{:<12} {:<12} {:<8} {:>8} {:>6} {:>12} {:>12} FAILURE",
        "CHALLENGE", "PROVIDER", "STATUS", "SAMPLES", "FORCED", "ISSUED", "RESPONDED"
    );
    for entry in entries {
        let challenge = hex_prefix(&entry.challenge_id, 12);
        let provider = hex_prefix(&entry.provider_id, 12);
        let status = entry.status.as_str();
        let samples = entry.sample_count;
        let forced = bool_label(entry.forced);
        let issued = entry.issued_at.to_string();
        let responded = entry
            .responded_at
            .map(|ts| ts.to_string())
            .unwrap_or_else(|| "-".to_string());
        let failure = entry
            .failure_reason
            .as_deref()
            .map(|reason| truncate_with_ellipsis(reason, 40))
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            &mut out,
            "{challenge:<12} {provider:<12} {status:<8} {samples:>8} {forced:>6} \
             {issued:>12} {responded:>12} {failure}"
        );
    }
    out
}

fn render_report_markdown(report: &PorWeeklyReportV1) -> String {
    const MICRO_XOR_PER_XOR: f64 = 1_000_000.0;
    let mut out = String::new();
    let _ = writeln!(&mut out, "# PoR Weekly Health — {}", report.cycle);
    let _ = writeln!(&mut out, "\nGenerated (unix): {}", report.generated_at);
    let _ = writeln!(&mut out, "\n## Aggregate Metrics");
    let _ = writeln!(&mut out, "- Total challenges: {}", report.challenges_total);
    let _ = writeln!(&mut out, "- Verified: {}", report.challenges_verified);
    let _ = writeln!(&mut out, "- Failed: {}", report.challenges_failed);
    let _ = writeln!(
        &mut out,
        "- Forced challenges: {}",
        report.forced_challenges
    );
    let _ = writeln!(&mut out, "- Repairs enqueued: {}", report.repairs_enqueued);
    let _ = writeln!(
        &mut out,
        "- Repairs completed: {}",
        report.repairs_completed
    );
    if let Some(mean) = report.mean_latency_ms {
        let _ = writeln!(&mut out, "- Mean latency: {mean:.0} ms");
    }
    if let Some(p95) = report.p95_latency_ms {
        let _ = writeln!(&mut out, "- P95 latency: {p95:.0} ms");
    }

    if !report.top_offenders.is_empty() {
        let _ = writeln!(&mut out, "\n## Provider Summaries");
        let _ = writeln!(
            &mut out,
            "| Provider | Challenges | Successes | Failures | Forced | Success Rate | Pending Repairs | Repair Dispatched | Ticket | First Failure (unix) | p95 latency ms |"
        );
        let _ = writeln!(
            &mut out,
            "|----------|------------|-----------|---------|--------|--------------|-----------------|-------------------|--------|----------------------|----------------|"
        );
        for provider in &report.top_offenders {
            let provider_id = hex_prefix(&provider.provider_id, 12);
            let success_rate = provider.success_rate * 100.0;
            let ticket = provider.ticket_id.as_deref().unwrap_or("-");
            let first_failure = provider
                .first_failure_at
                .map(|ts| ts.to_string())
                .unwrap_or_else(|| "-".to_string());
            let latency = provider
                .last_success_latency_ms_p95
                .map(|ms| ms.to_string())
                .unwrap_or_else(|| "-".to_string());
            let _ = writeln!(
                &mut out,
                "| {} | {} | {} | {} | {} | {:.2}% | {} | {} | {} | {} | {} |",
                provider_id,
                provider.challenges,
                provider.successes,
                provider.failures,
                provider.forced,
                success_rate,
                provider.pending_repairs,
                bool_label(provider.repair_dispatched),
                ticket,
                first_failure,
                latency
            );
        }
    }

    if !report.providers_missing_vrf.is_empty() {
        let _ = writeln!(&mut out, "\n## Providers Missing VRF");
        for provider in &report.providers_missing_vrf {
            let _ = writeln!(&mut out, "- {}", hex_prefix(provider, 12));
        }
    }

    if !report.slashing_events.is_empty() {
        let _ = writeln!(&mut out, "\n## Slashing Events");
        for event in &report.slashing_events {
            let provider = hex_prefix(&event.provider_id, 12);
            let manifest = hex_prefix(&event.manifest_digest, 12);
            let penalty_xor = event.penalty_xor.as_micro() as f64 / MICRO_XOR_PER_XOR;
            let _ = writeln!(
                &mut out,
                "- Provider {} manifest {} penalty {:.6} XOR (verdict `{}`, decided unix {})",
                provider, manifest, penalty_xor, event.verdict_cid, event.decided_at
            );
        }
    }

    if let Some(notes) = report.notes.as_deref() {
        let _ = writeln!(&mut out, "\n## Notes\n{}", notes.trim());
    }
    out
}

fn truncate_with_ellipsis(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &value[..max_len - 3])
    }
}

fn bool_label(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

fn hex_prefix(bytes: &[u8], len: usize) -> String {
    let full = hex_encode(bytes);
    let end = len.min(full.len());
    full[..end].to_string()
}

fn body_snippet(body: &[u8]) -> String {
    if body.is_empty() {
        return "empty response body".to_string();
    }
    if let Ok(text) = std::str::from_utf8(body) {
        let trimmed = text.trim();
        truncate_with_ellipsis(trimmed, 120)
    } else {
        format!("{} bytes (binary payload)", body.len())
    }
}

fn parse_digest_hex(input: &str) -> Result<[u8; 32], String> {
    let bytes = parse_hex_vec(input)?;
    if bytes.len() != 32 {
        return Err(format!(
            "expected 32-byte digest, found {} bytes",
            bytes.len()
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_hex_vec(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) {
        return Err("hex string must contain an even number of characters".into());
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let mut iter = input.as_bytes().chunks_exact(2);
    for pair in &mut iter {
        let hi = hex_value(pair[0])?;
        let lo = hex_value(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit `{}`", byte as char)),
    }
}

fn format_manifest_error(err: ManifestBuildError) -> String {
    match err {
        ManifestBuildError::MissingField(field) => {
            format!("manifest missing required field `{field}`")
        }
    }
}

fn pin_policy_json(policy: &PinPolicy) -> Map {
    let mut obj = Map::new();
    let label = match policy.storage_class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    };
    obj.insert(
        "min_replicas".into(),
        Value::from(policy.min_replicas as u64),
    );
    obj.insert("storage_class".into(), Value::from(label));
    obj.insert(
        "retention_epoch".into(),
        Value::from(policy.retention_epoch),
    );
    obj
}

enum JsonSource {
    Stdin,
    File(PathBuf),
}

impl JsonSource {
    fn from_arg(arg: &str) -> Result<Self, String> {
        if arg == "-" {
            Ok(Self::Stdin)
        } else {
            Ok(Self::File(PathBuf::from(arg)))
        }
    }

    fn read(self) -> Result<Value, String> {
        match self {
            JsonSource::Stdin => {
                let mut buf = String::new();
                io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|err| format!("failed to read summary from stdin: {err}"))?;
                norito::json::from_str(&buf)
                    .map_err(|err| format!("failed to parse summary JSON from stdin: {err}"))
            }
            JsonSource::File(path) => {
                let file = File::open(&path)
                    .map_err(|err| format!("failed to open `{}`: {err}", path.display()))?;
                let mut reader = BufReader::new(file);
                let mut buf = String::new();
                reader
                    .read_to_string(&mut buf)
                    .map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;
                norito::json::from_str(&buf)
                    .map_err(|err| format!("failed to parse JSON from `{}`: {err}", path.display()))
            }
        }
    }
}
