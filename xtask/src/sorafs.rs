use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    convert::{TryFrom, TryInto},
    error::Error,
    fs,
    io::{self, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use eyre::{WrapErr, eyre};
use integration_tests::sorafs_gateway_conformance::HarnessContext;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    account::AccountAddress,
    isi::sorafs::{
        ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder, IssueReplicationOrder,
        RegisterPinManifest,
    },
    prelude::*,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
            ManifestDigest, PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId,
            ReplicationOrderRecord, ReplicationOrderStatus, StorageClass,
        },
        reserve::{ReserveDuration, ReservePolicyV1, ReserveQuote, ReserveTier},
    },
    taikai::{
        TaikaiCacheConfigV1, TaikaiCacheProfileV1, TaikaiCacheQosConfigV1, TaikaiCacheRolloutStage,
    },
};
use iroha_torii::sorafs::gateway::{AcmeAutomation, AcmeConfig, SelfSignedAcmeClient};
use mv::storage::StorageReadOnly;
use norito::{
    decode_from_bytes,
    derive::JsonDeserialize,
    json::{self, Map, Number, Value, to_string_pretty},
    to_bytes,
};
use reqwest::{
    Url,
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};
use sha2::{Digest, Sha256};
use sorafs_car::chunker_registry::{self, ChunkerProfileDescriptor};
use sorafs_chunker::fixtures::{FixtureProfile, to_hex};
use sorafs_manifest::{
    AdmissionRecord, AdvertEndpoint, AliasBindingV1, AvailabilityTier, CapabilityTlv,
    CapabilityType, CouncilSignature, EndpointAdmissionV1, EndpointAttestationKind,
    EndpointAttestationV1, EndpointKind, GatewayAuthorizationRecord, GatewayAuthorizationVerifier,
    PathDiversityPolicy, ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1,
    ProviderAdvertBodyV1, ProviderAdvertV1, ProviderCapabilityRangeV1, QosHints,
    REPLICATION_ORDER_VERSION_V1, RendezvousTopic, ReplicationAssignmentV1, ReplicationOrderSlaV1,
    ReplicationOrderV1, SignatureAlgorithm, StakePointer, StreamBudgetV1, TransportHintV1,
    TransportProtocol,
    alias_cache::{AliasCachePolicy, AliasProofEvaluation, AliasProofState, decode_alias_proof},
    compute_advert_body_digest, compute_envelope_digest, compute_proposal_digest,
    deal::{MICRO_XOR_PER_XOR, XorAmount},
    pin_registry::{AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest},
    verify_advert_against_record,
};
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{JsonTarget, normalize_path, workspace_root, write_json_output};

#[derive(Clone, Debug)]
pub struct ReserveMatrixOptions {
    pub capacities_gib: Vec<u64>,
    pub storage_classes: Vec<StorageClass>,
    pub tiers: Vec<ReserveTier>,
    pub durations: Vec<ReserveDuration>,
    pub reserve_balance: XorAmount,
    pub policy_json: Option<PathBuf>,
    pub policy_norito: Option<PathBuf>,
    pub label: Option<String>,
}

pub fn parse_storage_class_label(input: &str) -> eyre::Result<StorageClass> {
    match input.trim().to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        other => Err(eyre!(
            "unknown storage class `{other}` (expected hot|warm|cold)"
        )),
    }
}

pub fn parse_reserve_tier_label(input: &str) -> eyre::Result<ReserveTier> {
    match input.trim().to_ascii_lowercase().as_str() {
        "tier-a" | "a" => Ok(ReserveTier::TierA),
        "tier-b" | "b" => Ok(ReserveTier::TierB),
        "tier-c" | "c" => Ok(ReserveTier::TierC),
        other => Err(eyre!(
            "unknown reserve tier `{other}` (expected tier-a|tier-b|tier-c)"
        )),
    }
}

pub fn parse_reserve_duration_label(input: &str) -> eyre::Result<ReserveDuration> {
    match input.trim().to_ascii_lowercase().as_str() {
        "monthly" => Ok(ReserveDuration::Monthly),
        "quarterly" => Ok(ReserveDuration::Quarterly),
        "annual" | "yearly" => Ok(ReserveDuration::Annual),
        other => Err(eyre!(
            "unknown reserve duration `{other}` (expected monthly|quarterly|annual)"
        )),
    }
}

pub fn parse_xor_amount_decimal(input: &str) -> eyre::Result<XorAmount> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(eyre!("reserve balance must not be empty"));
    }
    if trimmed.starts_with('-') {
        return Err(eyre!("reserve balance must be non-negative"));
    }
    let mut parts = trimmed.split('.');
    let whole_part = parts.next().unwrap_or("");
    let fractional_part = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(eyre!(
            "reserve balance may contain at most one decimal separator"
        ));
    }
    if whole_part.is_empty() && fractional_part.is_empty() {
        return Err(eyre!("reserve balance must contain digits"));
    }
    if !whole_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(eyre!("reserve balance contains invalid characters"));
    }
    if !fractional_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(eyre!("reserve balance fractional part is invalid"));
    }
    if fractional_part.len() > 6 {
        return Err(eyre!(
            "reserve balance supports up to six fractional digits (micro XOR precision)"
        ));
    }

    let whole_value = if whole_part.is_empty() {
        0
    } else {
        whole_part
            .parse::<u128>()
            .wrap_err("failed to parse whole-number component")?
    };
    let mut fractional_value = 0u128;
    let mut digits = 0;
    for ch in fractional_part.chars() {
        digits += 1;
        fractional_value = fractional_value * 10 + u128::from(ch as u8 - b'0');
    }
    if digits > 0 {
        for _ in digits..6 {
            fractional_value *= 10;
        }
    }
    let base = whole_value
        .checked_mul(MICRO_XOR_PER_XOR)
        .ok_or_else(|| eyre!("reserve balance exceeds supported range"))?;
    let total = base
        .checked_add(fractional_value)
        .ok_or_else(|| eyre!("reserve balance exceeds supported range"))?;
    Ok(XorAmount::from_micro(total))
}

const fn storage_class_label(class: StorageClass) -> &'static str {
    match class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    }
}

const fn reserve_tier_label(tier: ReserveTier) -> &'static str {
    match tier {
        ReserveTier::TierA => "tier-a",
        ReserveTier::TierB => "tier-b",
        ReserveTier::TierC => "tier-c",
    }
}

const fn reserve_duration_label(duration: ReserveDuration) -> &'static str {
    match duration {
        ReserveDuration::Monthly => "monthly",
        ReserveDuration::Quarterly => "quarterly",
        ReserveDuration::Annual => "annual",
    }
}

pub fn reserve_matrix_report(options: ReserveMatrixOptions) -> Result<json::Value, Box<dyn Error>> {
    if options.capacities_gib.is_empty() {
        return Err(eyre!("reserve matrix requires at least one --capacity").into());
    }
    if options.storage_classes.is_empty() {
        return Err(eyre!("reserve matrix requires at least one --storage-class").into());
    }
    if options.tiers.is_empty() {
        return Err(eyre!("reserve matrix requires at least one --tier").into());
    }
    if options.durations.is_empty() {
        return Err(eyre!("reserve matrix requires at least one --duration").into());
    }

    let (policy, policy_source) = load_reserve_policy_from_paths(
        options.policy_json.as_deref(),
        options.policy_norito.as_deref(),
    )?;
    let policy_bytes =
        to_bytes(&policy).map_err(|err| eyre!("failed to encode reserve policy: {err}"))?;
    let policy_sha256 = hex::encode(Sha256::digest(&policy_bytes));
    let reserve_balance_micro = u64::try_from(options.reserve_balance.as_micro())
        .map_err(|_| eyre!("reserve balance exceeds supported range"))?;

    let mut matrix_entries = Vec::new();
    for &storage_class in &options.storage_classes {
        for &tier in &options.tiers {
            for &duration in &options.durations {
                for &capacity in &options.capacities_gib {
                    let quote = policy
                        .quote(storage_class, capacity, duration, tier, options.reserve_balance)
                        .map_err(|err| {
                            eyre!(
                                "failed to compute reserve quote for storage_class={:?} tier={:?} duration={:?} capacity_gib={}: {err}",
                                storage_class,
                                tier,
                                duration,
                                capacity
                            )
                        })?;
                    let entry = build_matrix_entry_value(
                        storage_class,
                        tier,
                        duration,
                        capacity,
                        options.reserve_balance,
                        &quote,
                    )?;
                    matrix_entries.push(entry);
                }
            }
        }
    }

    let mut payload = json::Map::new();
    payload.insert(
        "capacities_gib".into(),
        json::Value::Array(
            options
                .capacities_gib
                .iter()
                .map(|c| json::Value::from(*c))
                .collect(),
        ),
    );
    payload.insert(
        "storage_classes".into(),
        json::Value::Array(
            options
                .storage_classes
                .iter()
                .map(|class| json::Value::from(storage_class_label(*class)))
                .collect(),
        ),
    );
    payload.insert(
        "tiers".into(),
        json::Value::Array(
            options
                .tiers
                .iter()
                .map(|tier| json::Value::from(reserve_tier_label(*tier)))
                .collect(),
        ),
    );
    payload.insert(
        "durations".into(),
        json::Value::Array(
            options
                .durations
                .iter()
                .map(|duration| json::Value::from(reserve_duration_label(*duration)))
                .collect(),
        ),
    );
    payload.insert(
        "reserve_balance_micro_xor".into(),
        json::Value::from(reserve_balance_micro),
    );
    payload.insert(
        "policy_json".into(),
        options
            .policy_json
            .as_ref()
            .map(|p| json::Value::from(p.display().to_string()))
            .unwrap_or(json::Value::Null),
    );
    payload.insert(
        "policy_norito".into(),
        options
            .policy_norito
            .as_ref()
            .map(|p| json::Value::from(p.display().to_string()))
            .unwrap_or(json::Value::Null),
    );
    payload.insert(
        "label".into(),
        options
            .label
            .map(json::Value::from)
            .unwrap_or(json::Value::Null),
    );
    payload.insert(
        "policy_version".into(),
        json::Value::from(u64::from(policy.version)),
    );
    payload.insert("policy_sha256".into(), json::Value::from(policy_sha256));
    payload.insert("policy_source".into(), json::Value::from(policy_source));
    payload.insert(
        "matrix_entry_count".into(),
        json::Value::from(matrix_entries.len() as u64),
    );
    payload.insert("matrix".into(), json::Value::Array(matrix_entries));
    Ok(json::Value::Object(payload))
}

fn load_reserve_policy_from_paths(
    json_path: Option<&Path>,
    norito_path: Option<&Path>,
) -> Result<(ReservePolicyV1, String), Box<dyn Error>> {
    match (json_path, norito_path) {
        (Some(_), Some(_)) => {
            Err(eyre!("only one of --policy-json or --policy-norito may be supplied").into())
        }
        (Some(path), None) => {
            let contents = fs::read_to_string(path).map_err(|err| {
                eyre!(
                    "failed to read reserve policy JSON `{}`: {err}",
                    path.display()
                )
            })?;
            let policy: ReservePolicyV1 = norito::json::from_str(&contents).map_err(|err| {
                eyre!(
                    "failed to parse reserve policy JSON `{}`: {err}",
                    path.display()
                )
            })?;
            Ok((policy, format!("policy JSON `{}`", path.display())))
        }
        (None, Some(path)) => {
            let bytes = fs::read(path).map_err(|err| {
                eyre!(
                    "failed to read reserve policy Norito `{}`: {err}",
                    path.display()
                )
            })?;
            let policy = decode_from_bytes::<ReservePolicyV1>(&bytes).map_err(|err| {
                eyre!(
                    "failed to decode reserve policy Norito `{}`: {err}",
                    path.display()
                )
            })?;
            Ok((policy, format!("policy Norito `{}`", path.display())))
        }
        (None, None) => Ok((
            ReservePolicyV1::default(),
            "embedded default policy".to_string(),
        )),
    }
}

fn build_matrix_entry_value(
    storage_class: StorageClass,
    tier: ReserveTier,
    duration: ReserveDuration,
    capacity_gib: u64,
    reserve_balance: XorAmount,
    quote: &ReserveQuote,
) -> Result<json::Value, Box<dyn Error>> {
    let inputs_value =
        matrix_inputs_value(storage_class, tier, duration, capacity_gib, reserve_balance)?;
    let quote_value = norito::json::to_value(quote)
        .map_err(|err| eyre!("failed to serialize reserve quote JSON: {err}"))?;
    let projection_value = norito::json::to_value(&quote.ledger_projection())
        .map_err(|err| eyre!("failed to serialize reserve ledger projection: {err}"))?;
    let mut entry = json::Map::new();
    entry.insert(
        "storage_class".into(),
        json::Value::from(storage_class_label(storage_class)),
    );
    entry.insert("tier".into(), json::Value::from(reserve_tier_label(tier)));
    entry.insert(
        "duration".into(),
        json::Value::from(reserve_duration_label(duration)),
    );
    entry.insert("capacity_gib".into(), json::Value::from(capacity_gib));
    entry.insert("inputs".into(), inputs_value);
    entry.insert("quote".into(), quote_value);
    entry.insert("ledger_projection".into(), projection_value);
    Ok(json::Value::Object(entry))
}

fn matrix_inputs_value(
    storage_class: StorageClass,
    tier: ReserveTier,
    duration: ReserveDuration,
    capacity_gib: u64,
    reserve_balance: XorAmount,
) -> Result<json::Value, Box<dyn Error>> {
    let mut inputs = json::Map::new();
    inputs.insert(
        "storage_class".into(),
        json::Value::from(storage_class_label(storage_class)),
    );
    inputs.insert("tier".into(), json::Value::from(reserve_tier_label(tier)));
    inputs.insert(
        "duration".into(),
        json::Value::from(reserve_duration_label(duration)),
    );
    inputs.insert("capacity_gib".into(), json::Value::from(capacity_gib));
    let reserve_value = norito::json::to_value(&reserve_balance)
        .map_err(|err| eyre!("failed to serialize reserve balance: {err}"))?;
    inputs.insert("reserve_balance".into(), reserve_value);
    Ok(json::Value::Object(inputs))
}

mod gateway_fixture;

const DEFAULT_PROFILE_HANDLE: &str = "sorafs.sf1@1.0.0";

#[derive(Clone)]
pub struct FetchFixtureOptions {
    pub signatures_source: FetchSource,
    pub manifest_source: Option<FetchSource>,
    pub output_dir: PathBuf,
    pub profile_handle: String,
    pub allow_unsigned: bool,
}

#[derive(Clone)]
pub struct AdoptionCheckOptions {
    pub scoreboard_paths: Vec<PathBuf>,
    pub summary_paths: Vec<PathBuf>,
    pub min_eligible_providers: usize,
    pub require_positive_weight: bool,
    pub allow_single_source_fallback: bool,
    pub require_telemetry_source: bool,
    pub allow_implicit_metadata: bool,
    pub require_direct_only: bool,
    pub require_telemetry_region: bool,
}

#[derive(Debug, Serialize)]
pub struct AdoptionCheckReport {
    pub scoreboard_reports: Vec<ScoreboardReport>,
    pub total_evaluated: usize,
    pub min_providers_required: usize,
    pub single_source_override_used: bool,
    pub implicit_metadata_override_used: bool,
}

#[derive(Debug, Serialize)]
pub struct ScoreboardReport {
    pub scoreboard_path: String,
    pub summary_path: String,
    pub provider_count: usize,
    pub eligible_providers: usize,
    pub active_providers: usize,
    pub min_required: usize,
    pub weight_sum: f64,
    pub summary_provider_count: u64,
    pub summary_gateway_provider_count: u64,
    pub summary_provider_mix: String,
    pub summary_transport_policy: String,
    pub summary_transport_policy_override: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_transport_policy_override_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ScoreboardMetadata>,
}

#[derive(Debug, Serialize, Default)]
pub struct ScoreboardMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestrator_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_scoreboard: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_implicit_metadata: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_provider_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_mix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parallel: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_peers: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_failure_threshold: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assume_now: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_manifest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_manifest_cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_manifest_provided: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_policy_override: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_policy_override_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymity_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymity_policy_override: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymity_policy_override_label: Option<String>,
}

#[derive(Clone)]
pub struct ScoreboardDiffOptions {
    pub previous_scoreboard: PathBuf,
    pub current_scoreboard: PathBuf,
    pub threshold_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct ScoreboardDiffReport {
    pub previous_scoreboard: String,
    pub current_scoreboard: String,
    pub threshold_percent: f64,
    pub total_providers: usize,
    pub changed_providers: Vec<ProviderWeightDelta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderWeightDelta {
    pub provider_id: String,
    pub previous_weight: f64,
    pub current_weight: f64,
    pub delta: f64,
    pub delta_percent: f64,
    pub was_added: bool,
    pub was_removed: bool,
    pub exceeds_threshold: bool,
}

#[derive(Clone)]
pub struct BurnInCheckOptions {
    pub log_paths: Vec<PathBuf>,
    pub required_window_days: u64,
    pub min_pq_ratio: f64,
    pub max_no_provider_errors: u64,
    pub max_brownout_ratio: f64,
    pub min_fetches: u64,
}

#[derive(Debug, Serialize)]
pub struct BurnInSummary {
    pub sources: Vec<String>,
    pub coverage_days: f64,
    pub first_timestamp: String,
    pub last_timestamp: String,
    pub fetches_total: u64,
    pub successful_fetches: u64,
    pub brownout_fetches: u64,
    pub brownout_ratio: f64,
    pub stall_event_ratio: f64,
    pub stall_events: u64,
    pub stall_chunks: u64,
    pub stall_chunk_ratio: f64,
    pub pq_ratio_min: f64,
    pub pq_ratio_avg: f64,
    pub failure_reasons: BTreeMap<String, u64>,
}

#[derive(Clone)]
pub struct TaikaiCacheBundleOptions {
    pub profile_paths: Vec<PathBuf>,
    pub output_root: PathBuf,
}

#[derive(Clone)]
pub enum FetchSource {
    File(PathBuf),
    Url(Url),
}

#[derive(Clone)]
pub struct GatewayAttestOptions {
    pub output_dir: PathBuf,
    pub signing_key_path: PathBuf,
    pub signer_account: String,
    pub gateway_target: Option<String>,
}

#[derive(Clone)]
pub struct GatewayProbeOptions {
    pub request: Option<GatewayProbeRequest>,
    pub headers_path: Option<PathBuf>,
    pub gar_path: PathBuf,
    pub gar_keys: Vec<(String, Vec<u8>)>,
    pub cache_max_age: Option<u64>,
    pub cache_swr: Option<u64>,
    pub now_override: Option<u64>,
    pub host_override: Option<String>,
    pub require_tls_state: bool,
    pub report_target: Option<JsonTarget>,
    pub summary_path: Option<PathBuf>,
    pub drill: Option<DrillLogConfig>,
    pub pagerduty: Option<PagerDutyConfig>,
}

#[derive(Clone)]
pub struct DrillLogConfig {
    pub log_path: PathBuf,
    pub scenario: String,
    pub ic: Option<String>,
    pub scribe: Option<String>,
    pub notes: Option<String>,
    pub link: Option<String>,
}

#[derive(Clone)]
pub struct PagerDutyConfig {
    pub payload_path: PathBuf,
    pub routing_key: String,
    pub severity: String,
    pub source: String,
    pub component: Option<String>,
    pub group: Option<String>,
    pub class_name: Option<String>,
    pub dedup_key: Option<String>,
    pub links: Vec<PagerDutyLink>,
    pub endpoint_url: Option<Url>,
}

#[derive(Clone)]
pub struct PagerDutyLink {
    pub text: String,
    pub href: String,
}

#[derive(Clone)]
pub struct GatewayProbeRequest {
    pub url: String,
    pub method: String,
    pub timeout_secs: Option<u64>,
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Debug)]
pub enum GatewayCliCommand {
    TlsRenew(GatewayTlsRenewOptions),
    TlsRevoke(GatewayTlsRevokeOptions),
    KeyRotate(GatewayKeyRotateOptions),
    DenylistPack(GatewayDenylistPackOptions),
    DenylistDiff(GatewayDenylistDiffOptions),
    DenylistVerify(GatewayDenylistVerifyOptions),
    RoutePlan(Box<GatewayRoutePlanOptions>),
}

#[derive(Debug, Clone)]
pub struct GatewayTlsRenewOptions {
    pub hostnames: Vec<String>,
    pub account_email: Option<String>,
    pub directory_url: String,
    pub dns_provider_id: Option<String>,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct GatewayTlsRevokeOptions {
    pub bundle_dir: PathBuf,
    pub archive_dir: Option<PathBuf>,
    pub reason: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct GatewayKeyRotateOptions {
    pub kind: String,
    pub output_path: PathBuf,
    pub public_out: Option<PathBuf>,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct GatewayDenylistPackOptions {
    pub input_path: PathBuf,
    pub output_dir: PathBuf,
    pub label: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct GatewayDenylistDiffOptions {
    pub old_bundle: PathBuf,
    pub new_bundle: PathBuf,
    pub report_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct GatewayDenylistVerifyOptions {
    pub bundle: PathBuf,
    pub norito_path: Option<PathBuf>,
    pub root_path: Option<PathBuf>,
    pub report_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct GatewayRoutePlanOptions {
    pub manifest_json: PathBuf,
    pub output_path: PathBuf,
    pub headers_out: Option<PathBuf>,
    pub alias: Option<String>,
    pub hostname: Option<String>,
    pub route_label: Option<String>,
    pub proof_status: Option<String>,
    pub release_tag: Option<String>,
    pub cutover_window: Option<String>,
    pub rollback_manifest: Option<PathBuf>,
    pub rollback_headers_out: Option<PathBuf>,
    pub rollback_route_label: Option<String>,
    pub rollback_release_tag: Option<String>,
    pub include_csp: bool,
    pub include_permissions: bool,
    pub include_hsts: bool,
    pub now: OffsetDateTime,
}

#[derive(Debug, Serialize)]
struct GatewayRoutePlan {
    version: u32,
    generated_at: String,
    manifest_json: String,
    alias: Option<String>,
    hostname: Option<String>,
    release_tag: Option<String>,
    cutover_window: Option<String>,
    content_cid: String,
    route_binding: String,
    headers: BTreeMap<String, String>,
    headers_template: String,
    headers_path: Option<String>,
    rollback: Option<GatewayRouteRollback>,
}

#[derive(Debug, Serialize)]
struct GatewayRouteRollback {
    manifest_json: String,
    release_tag: Option<String>,
    content_cid: String,
    route_binding: String,
    headers_template: String,
    headers_path: Option<String>,
}

#[derive(Debug)]
struct RouteBindingContext {
    manifest_json: PathBuf,
    alias: Option<String>,
    hostname: Option<String>,
    route_label: Option<String>,
    proof_status: Option<String>,
    include_csp: bool,
    include_permissions: bool,
    include_hsts: bool,
    generated_at: OffsetDateTime,
}

struct RouteBindingOutput {
    content_cid: String,
    route_binding: String,
    headers: BTreeMap<String, String>,
    headers_template: String,
}

pub struct GatewayTlsRenewOutcome {
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
    pub ech_config_path: PathBuf,
    pub fingerprint_hex: String,
    pub expiry_rfc3339: String,
    pub hostnames: Vec<String>,
}

pub struct GatewayTlsRevokeOutcome {
    pub archive_dir: PathBuf,
    pub archived_files: Vec<PathBuf>,
    pub timestamp_epoch: u64,
    pub reason: Option<String>,
}

pub struct GatewayKeyRotateOutcome {
    pub private_key_path: PathBuf,
    pub public_key_hex: String,
    pub public_key_prefixed: String,
    pub public_out_path: Option<PathBuf>,
}

pub struct GatewayDenylistPackOutcome {
    pub bundle_path: PathBuf,
    pub norito_path: PathBuf,
    pub root_path: PathBuf,
    pub root_hex: String,
    pub entry_count: usize,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
struct DenylistBundleV1 {
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    label: Option<String>,
    generated_at: String,
    root_hex: String,
    entry_count: u32,
    entries: Vec<DenylistBundleEntry>,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
struct DenylistBundleEntry {
    index: u32,
    kind: String,
    digest_hex: String,
    canonical_json: String,
    proof: Vec<MerkleProofNode>,
}

#[derive(Debug, Serialize)]
struct DenylistDiffEntry {
    kind: String,
    digest_hex: String,
    canonical_json: String,
}

#[derive(Debug, Serialize)]
pub struct DenylistDiffReport {
    old_label: Option<String>,
    new_label: Option<String>,
    old_root: String,
    new_root: String,
    old_entry_count: u32,
    new_entry_count: u32,
    added: Vec<DenylistDiffEntry>,
    removed: Vec<DenylistDiffEntry>,
    unchanged_entries: usize,
}

#[derive(Debug, Serialize)]
pub struct GatewayDenylistVerifyReport {
    bundle_path: String,
    label: Option<String>,
    generated_at: String,
    root_hex: String,
    entry_count: u32,
    norito_verified: bool,
    norito_path: Option<String>,
    root_file_verified: bool,
    root_file_path: Option<String>,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
struct MerkleProofNode {
    direction: MerkleSiblingPosition,
    digest_hex: String,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
enum MerkleSiblingPosition {
    Left,
    Right,
}

#[derive(Debug)]
struct CanonicalEntry {
    kind: String,
    canonical_json: String,
    digest: [u8; 32],
}

pub fn parse_gateway_cli<I>(args: I) -> Result<GatewayCliCommand, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let Some(subcommand) = iter.next() else {
        gateway_cli_usage();
        return Err("sorafs-gateway requires a subcommand (tls|key)".into());
    };

    match subcommand.as_str() {
        "tls" => {
            let remaining: Vec<String> = iter.collect();
            parse_gateway_tls_cli(remaining)
        }
        "route" => {
            let remaining: Vec<String> = iter.collect();
            parse_gateway_route_cli(remaining)
        }
        "key" => {
            let remaining: Vec<String> = iter.collect();
            parse_gateway_key_cli(remaining)
        }
        "denylist" => {
            let remaining: Vec<String> = iter.collect();
            parse_gateway_denylist_cli(remaining)
        }
        "-h" | "--help" => {
            gateway_cli_usage();
            Err("sorafs-gateway usage displayed".into())
        }
        other => {
            gateway_cli_usage();
            Err(format!("unknown sorafs-gateway subcommand `{other}`").into())
        }
    }
}

pub fn run_gateway_cli(command: GatewayCliCommand) -> Result<(), Box<dyn Error>> {
    match command {
        GatewayCliCommand::TlsRenew(options) => {
            let outcome = gateway_tls_renew(options)?;
            println!("generated TLS bundle:");
            println!("  hosts        : {}", outcome.hostnames.join(", "));
            println!("  fingerprint  : {}", outcome.fingerprint_hex);
            println!("  not_after    : {}", outcome.expiry_rfc3339);
            println!("  certificate  : {}", outcome.certificate_path.display());
            println!("  private key  : {}", outcome.private_key_path.display());
            println!("  ech config   : {}", outcome.ech_config_path.display());
        }
        GatewayCliCommand::TlsRevoke(options) => {
            let outcome = gateway_tls_revoke(options)?;
            println!("archived TLS bundle:");
            println!("  archive dir : {}", outcome.archive_dir.display());
            println!("  timestamp   : {}", outcome.timestamp_epoch);
            if let Some(reason) = outcome.reason.as_deref() {
                println!("  reason      : {reason}");
            }
            for path in &outcome.archived_files {
                println!("  archived    : {}", path.display());
            }
        }
        GatewayCliCommand::KeyRotate(options) => {
            let outcome = gateway_key_rotate(options)?;
            println!("generated token-signing key:");
            println!("  private key : {}", outcome.private_key_path.display());
            println!("  public hex  : {}", outcome.public_key_hex);
            println!("  public mh   : {}", outcome.public_key_prefixed);
            if let Some(path) = outcome.public_out_path {
                println!("  public file : {}", path.display());
            }
        }
        GatewayCliCommand::DenylistPack(options) => {
            let outcome = gateway_denylist_pack(options)?;
            println!(
                "packed {} denylist entries (root={})",
                outcome.entry_count, outcome.root_hex
            );
            println!("  bundle json : {}", outcome.bundle_path.display());
            println!("  bundle to   : {}", outcome.norito_path.display());
            println!("  root file   : {}", outcome.root_path.display());
        }
        GatewayCliCommand::DenylistDiff(options) => {
            let report = gateway_denylist_diff(&options)?;
            println!(
                "denylist diff: {} ({} entries) -> {} ({} entries)",
                report.old_root, report.old_entry_count, report.new_root, report.new_entry_count
            );
            if let Some(label) = report.old_label.as_deref() {
                println!("  old label   : {label}");
            }
            if let Some(label) = report.new_label.as_deref() {
                println!("  new label   : {label}");
            }
            println!("  added       : {}", report.added.len());
            println!("  removed     : {}", report.removed.len());
            println!("  unchanged   : {}", report.unchanged_entries);
            if !report.added.is_empty() {
                println!("  added entries:");
                for entry in &report.added {
                    println!("    - {} {}", entry.kind, entry.digest_hex);
                }
            }
            if !report.removed.is_empty() {
                println!("  removed entries:");
                for entry in &report.removed {
                    println!("    - {} {}", entry.kind, entry.digest_hex);
                }
            }
            if let Some(path) = options.report_path.as_ref() {
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    fs::create_dir_all(parent)?;
                }
                let json = serde_json::to_vec_pretty(&report)
                    .map_err(|err| format!("failed to encode diff report: {err}"))?;
                fs::write(path, json).map_err(|err| {
                    format!("failed to write diff report `{}`: {err}", path.display())
                })?;
                println!("  report json : {}", path.display());
            }
        }
        GatewayCliCommand::DenylistVerify(options) => {
            let report = gateway_denylist_verify(&options)?;
            println!("denylist bundle verified:");
            println!("  bundle     : {}", report.bundle_path);
            if let Some(label) = report.label.as_deref() {
                println!("  label      : {label}");
            }
            println!("  generated  : {}", report.generated_at);
            println!("  root       : {}", report.root_hex);
            println!("  entries    : {}", report.entry_count);
            if let Some(path) = report.norito_path.as_deref() {
                println!("  norito     : {path} (verified)");
            } else {
                println!("  norito     : not supplied");
            }
            if let Some(path) = report.root_file_path.as_deref() {
                println!("  root file  : {path} (verified)");
            } else {
                println!("  root file  : not supplied");
            }
            if let Some(path) = options.report_path.as_ref() {
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    fs::create_dir_all(parent)?;
                }
                let json = serde_json::to_vec_pretty(&report)
                    .map_err(|err| format!("failed to encode verify report: {err}"))?;
                fs::write(path, json).map_err(|err| {
                    format!("failed to write verify report `{}`: {err}", path.display())
                })?;
                println!("  report json: {}", path.display());
            }
        }
        GatewayCliCommand::RoutePlan(options) => {
            run_gateway_route_plan(*options)?;
        }
    }

    Ok(())
}

fn parse_gateway_tls_cli<I>(args: I) -> Result<GatewayCliCommand, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let Some(action) = iter.next() else {
        gateway_cli_usage();
        return Err("sorafs-gateway tls requires an action (renew|revoke)".into());
    };

    match action.as_str() {
        "renew" => {
            let mut hostnames = Vec::new();
            let mut host_files = Vec::new();
            let mut account_email = None;
            let mut directory_url = None;
            let mut dns_provider_id = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut force = false;
            let mut pending = iter.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--host" => {
                        let Some(value) = pending.next() else {
                            return Err("--host requires a value".into());
                        };
                        let normalized = normalize_tls_host(&value)
                            .map_err(|err| format!("invalid --host value: {err}"))?;
                        hostnames.push(normalized);
                    }
                    "--hosts-from" => {
                        let Some(value) = pending.next() else {
                            return Err("--hosts-from requires a JSON path".into());
                        };
                        host_files.push(crate::normalize_path(Path::new(&value))?);
                    }
                    "--out" | "--output" => {
                        let Some(value) = pending.next() else {
                            return Err("--out requires a directory path".into());
                        };
                        output_dir = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--account-email" => {
                        let Some(value) = pending.next() else {
                            return Err("--account-email requires an address".into());
                        };
                        account_email = Some(value);
                    }
                    "--directory-url" => {
                        let Some(value) = pending.next() else {
                            return Err("--directory-url requires a value".into());
                        };
                        directory_url = Some(value);
                    }
                    "--dns-provider-id" => {
                        let Some(value) = pending.next() else {
                            return Err("--dns-provider-id requires a value".into());
                        };
                        dns_provider_id = Some(value);
                    }
                    "--force" => force = true,
                    "-h" | "--help" => {
                        gateway_cli_usage();
                        return Err("sorafs-gateway tls renew usage displayed".into());
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-gateway tls renew: {flag}").into()
                        );
                    }
                }
            }

            for path in host_files {
                let additional = load_tls_hosts_from_file(&path)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if additional.is_empty() {
                    return Err(format!(
                        "host fixture `{}` did not contain any entries",
                        path.display()
                    )
                    .into());
                }
                println!(
                    "[sorafs-gateway tls renew] loaded {} host(s) from {}",
                    additional.len(),
                    path.display()
                );
                hostnames.extend(additional);
            }
            dedup_tls_hosts(&mut hostnames);

            if hostnames.is_empty() {
                return Err(
                    "sorafs-gateway tls renew requires at least one --host or --hosts-from entry"
                        .into(),
                );
            }
            let output_dir = output_dir
                .ok_or_else(|| "sorafs-gateway tls renew requires --out <dir>".to_string())?;
            let directory_url =
                directory_url.unwrap_or_else(|| AcmeConfig::default().directory_url);

            Ok(GatewayCliCommand::TlsRenew(GatewayTlsRenewOptions {
                hostnames,
                account_email,
                directory_url,
                dns_provider_id,
                output_dir,
                force,
            }))
        }
        "revoke" => {
            let mut bundle_dir: Option<PathBuf> = None;
            let mut archive_dir: Option<PathBuf> = None;
            let mut reason: Option<String> = None;
            let mut force = false;
            let mut pending = iter.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--out" | "--bundle" | "--bundle-dir" => {
                        let Some(value) = pending.next() else {
                            return Err("--out requires a directory path".into());
                        };
                        bundle_dir = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--archive-dir" => {
                        let Some(value) = pending.next() else {
                            return Err("--archive-dir requires a directory path".into());
                        };
                        archive_dir = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--reason" => {
                        let Some(value) = pending.next() else {
                            return Err("--reason requires a value".into());
                        };
                        reason = Some(value);
                    }
                    "--force" => force = true,
                    "-h" | "--help" => {
                        gateway_cli_usage();
                        return Err("sorafs-gateway tls revoke usage displayed".into());
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-gateway tls revoke: {flag}").into(),
                        );
                    }
                }
            }

            let bundle_dir = bundle_dir
                .ok_or_else(|| "sorafs-gateway tls revoke requires --out <dir>".to_string())?;
            Ok(GatewayCliCommand::TlsRevoke(GatewayTlsRevokeOptions {
                bundle_dir,
                archive_dir,
                reason,
                force,
            }))
        }
        "-h" | "--help" => {
            gateway_cli_usage();
            Err("sorafs-gateway tls usage displayed".into())
        }
        other => {
            gateway_cli_usage();
            Err(format!("unknown sorafs-gateway tls action `{other}`").into())
        }
    }
}

fn parse_gateway_route_cli<I>(args: I) -> Result<GatewayCliCommand, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let Some(action) = iter.next() else {
        gateway_cli_usage();
        return Err("sorafs-gateway route requires an action (plan)".into());
    };

    match action.as_str() {
        "plan" => {
            let mut manifest_json: Option<PathBuf> = None;
            let mut output_path: Option<PathBuf> = None;
            let mut headers_out: Option<PathBuf> = None;
            let mut alias: Option<String> = None;
            let mut hostname: Option<String> = None;
            let mut route_label: Option<String> = None;
            let mut proof_status: Option<String> = None;
            let mut release_tag: Option<String> = None;
            let mut cutover_window: Option<String> = None;
            let mut rollback_manifest: Option<PathBuf> = None;
            let mut rollback_headers_out: Option<PathBuf> = None;
            let mut rollback_route_label: Option<String> = None;
            let mut rollback_release_tag: Option<String> = None;
            let mut include_csp = true;
            let mut include_permissions = true;
            let mut include_hsts = true;

            for arg in iter {
                if let Some(rest) = arg.strip_prefix("--manifest-json=") {
                    manifest_json = Some(PathBuf::from(rest));
                } else if let Some(rest) = arg.strip_prefix("--out=") {
                    output_path = Some(PathBuf::from(rest));
                } else if let Some(rest) = arg.strip_prefix("--headers-out=") {
                    headers_out = Some(PathBuf::from(rest));
                } else if let Some(rest) = arg.strip_prefix("--alias=") {
                    alias = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--hostname=") {
                    hostname = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--route-label=") {
                    route_label = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--proof-status=") {
                    proof_status = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--release-tag=") {
                    release_tag = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--cutover-window=") {
                    cutover_window = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--rollback-manifest-json=") {
                    rollback_manifest = Some(PathBuf::from(rest));
                } else if let Some(rest) = arg.strip_prefix("--rollback-route-label=") {
                    rollback_route_label = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--rollback-release-tag=") {
                    rollback_release_tag = Some(rest.to_string());
                } else if let Some(rest) = arg.strip_prefix("--rollback-headers-out=") {
                    rollback_headers_out = Some(PathBuf::from(rest));
                } else if arg == "--no-csp" {
                    include_csp = false;
                } else if arg == "--no-permissions-policy" {
                    include_permissions = false;
                } else if arg == "--no-hsts" {
                    include_hsts = false;
                } else if arg == "-h" || arg == "--help" {
                    gateway_cli_usage();
                    return Err("sorafs-gateway route plan usage displayed".into());
                } else {
                    return Err(format!("unknown flag for sorafs-gateway route plan: {arg}").into());
                }
            }

            let manifest_json = manifest_json.ok_or_else(|| {
                "--manifest-json=<path> is required for sorafs-gateway route plan".to_string()
            })?;
            let hostname = hostname.ok_or_else(|| {
                "--hostname=<host> is required for sorafs-gateway route plan".to_string()
            })?;
            let output_path = output_path
                .unwrap_or_else(|| PathBuf::from("artifacts/sorafs_gateway/route_plan.json"));
            let resolved_headers_out = headers_out.or_else(|| {
                output_path
                    .parent()
                    .map(|parent| parent.join("gateway.route.headers.txt"))
            });
            let resolved_rollback_headers = rollback_headers_out.or_else(|| {
                output_path
                    .parent()
                    .map(|parent| parent.join("gateway.route.rollback.headers.txt"))
            });

            Ok(GatewayCliCommand::RoutePlan(Box::new(
                GatewayRoutePlanOptions {
                    manifest_json,
                    output_path,
                    headers_out: resolved_headers_out,
                    alias,
                    hostname: Some(hostname),
                    route_label,
                    proof_status,
                    release_tag,
                    cutover_window,
                    rollback_manifest,
                    rollback_headers_out: resolved_rollback_headers,
                    rollback_route_label,
                    rollback_release_tag,
                    include_csp,
                    include_permissions,
                    include_hsts,
                    now: OffsetDateTime::now_utc(),
                },
            )))
        }
        other => {
            gateway_cli_usage();
            Err(format!("unknown sorafs-gateway route action `{other}`").into())
        }
    }
}

fn run_gateway_route_plan(options: GatewayRoutePlanOptions) -> Result<(), Box<dyn Error>> {
    let binding_context = RouteBindingContext {
        manifest_json: options.manifest_json.clone(),
        alias: options.alias.clone(),
        hostname: options.hostname.clone(),
        route_label: options.route_label.clone(),
        proof_status: options.proof_status.clone(),
        include_csp: options.include_csp,
        include_permissions: options.include_permissions,
        include_hsts: options.include_hsts,
        generated_at: options.now,
    };
    let primary_binding = build_route_binding(&binding_context)?;
    write_optional_output(&options.headers_out, &primary_binding.headers_template)?;

    let rollback = if let Some(rollback_manifest) = options.rollback_manifest.clone() {
        let rollback_context = RouteBindingContext {
            manifest_json: rollback_manifest.clone(),
            alias: options.alias.clone(),
            hostname: options.hostname.clone(),
            route_label: options
                .rollback_route_label
                .clone()
                .or(options.route_label.clone()),
            proof_status: options.proof_status.clone(),
            include_csp: options.include_csp,
            include_permissions: options.include_permissions,
            include_hsts: options.include_hsts,
            generated_at: options.now,
        };
        let binding = build_route_binding(&rollback_context)?;
        write_optional_output(&options.rollback_headers_out, &binding.headers_template)?;
        Some(GatewayRouteRollback {
            manifest_json: rollback_manifest.display().to_string(),
            release_tag: options.rollback_release_tag.clone(),
            content_cid: binding.content_cid,
            route_binding: binding.route_binding,
            headers_template: binding.headers_template,
            headers_path: options
                .rollback_headers_out
                .as_ref()
                .map(|path| path.display().to_string()),
        })
    } else {
        None
    };

    if let Some(parent) = options.output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let generated_at = options
        .now
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format timestamp: {err}"))?;
    let plan = GatewayRoutePlan {
        version: 1,
        generated_at,
        manifest_json: options.manifest_json.display().to_string(),
        alias: options.alias,
        hostname: options.hostname,
        release_tag: options.release_tag,
        cutover_window: options.cutover_window,
        content_cid: primary_binding.content_cid,
        route_binding: primary_binding.route_binding,
        headers: primary_binding.headers,
        headers_template: primary_binding.headers_template,
        headers_path: options
            .headers_out
            .as_ref()
            .map(|path| path.display().to_string()),
        rollback,
    };
    let payload = serde_json::to_string_pretty(&plan)?;
    fs::write(&options.output_path, format!("{payload}\n"))
        .map_err(|err| format!("failed to write {}: {err}", options.output_path.display()))?;
    println!("wrote {}", options.output_path.display());
    Ok(())
}

const ROUTE_HEADER_ORDER: &[&str] = &[
    "Sora-Name",
    "Sora-Content-CID",
    "Sora-Proof",
    "Sora-Proof-Status",
    "Sora-Route-Binding",
    "Content-Security-Policy",
    "Strict-Transport-Security",
    "Permissions-Policy",
];

const DEFAULT_ROUTE_CSP: &str = "default-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'";
const DEFAULT_ROUTE_PERMISSIONS: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), clipboard-read=(self), clipboard-write=(self), encrypted-media=(), fullscreen=(self), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), speaker-selection=(), usb=(), xr-spatial-tracking=()";
const DEFAULT_ROUTE_HSTS_MAX_AGE: u32 = 63_072_000;

fn build_route_binding(
    context: &RouteBindingContext,
) -> Result<RouteBindingOutput, Box<dyn Error>> {
    let manifest_bytes = fs::read(&context.manifest_json).map_err(|err| {
        format!(
            "failed to read manifest JSON from `{}`: {err}",
            context.manifest_json.display()
        )
    })?;
    let manifest: JsonValue = serde_json::from_slice(&manifest_bytes).map_err(|err| {
        format!(
            "failed to parse manifest JSON from `{}`: {err}",
            context.manifest_json.display()
        )
    })?;
    let root_bytes = manifest_root_bytes(&manifest)?;
    if root_bytes.is_empty() {
        return Err("manifest root CID payload was empty".into());
    }
    let content_cid = format!("b{}", encode_base32_lower(&root_bytes));
    let mut headers = BTreeMap::new();
    headers.insert("Sora-Content-CID".into(), content_cid.clone());

    if let Some(alias) = context.alias.as_deref() {
        headers.insert("Sora-Name".into(), alias.to_string());
        let proof_payload = serde_json::json!({
            "alias": alias,
            "manifest": content_cid,
        });
        let proof_bytes = serde_json::to_vec(&proof_payload)
            .map_err(|err| format!("failed to encode proof payload: {err}"))?;
        headers.insert("Sora-Proof".into(), BASE64_STD.encode(proof_bytes));
        let status = context
            .proof_status
            .clone()
            .unwrap_or_else(|| "ok".to_string());
        headers.insert("Sora-Proof-Status".into(), status);
    }

    let hostname = context.hostname.as_ref().ok_or_else(|| {
        "hostname must be supplied for sorafs-gateway route plan (--hostname)".to_string()
    })?;
    let generated_at = context
        .generated_at
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format timestamp: {err}"))?;
    let mut binding_parts = vec![
        format!("host={hostname}"),
        format!("cid={content_cid}"),
        format!("generated_at={generated_at}"),
    ];
    if let Some(label) = context.route_label.as_deref() {
        binding_parts.push(format!("label={label}"));
    }
    let route_binding = binding_parts.join(";");
    headers.insert("Sora-Route-Binding".into(), route_binding.clone());

    if context.include_csp {
        headers.insert("Content-Security-Policy".into(), DEFAULT_ROUTE_CSP.into());
    }
    if context.include_hsts {
        headers.insert(
            "Strict-Transport-Security".into(),
            format!(
                "max-age={}; includeSubDomains; preload",
                DEFAULT_ROUTE_HSTS_MAX_AGE
            ),
        );
    }
    if context.include_permissions {
        headers.insert(
            "Permissions-Policy".into(),
            DEFAULT_ROUTE_PERMISSIONS.into(),
        );
    }

    let headers_template = format_headers_template(&headers);
    Ok(RouteBindingOutput {
        content_cid,
        route_binding,
        headers,
        headers_template,
    })
}

fn manifest_root_bytes(manifest: &JsonValue) -> Result<Vec<u8>, Box<dyn Error>> {
    if let Some(array) = manifest.get("root_cid").and_then(|value| value.as_array()) {
        let mut bytes = Vec::with_capacity(array.len());
        for value in array {
            let number = value.as_i64().ok_or_else(|| {
                format!("root_cid entries must be integers, found {value:?} instead")
            })?;
            if !(0..=255).contains(&number) {
                return Err(format!(
                    "root_cid entries must be between 0 and 255 inclusive (found {number})"
                )
                .into());
            }
            bytes.push(number as u8);
        }
        return Ok(bytes);
    }
    if let Some(array) = manifest
        .get("root_cids_hex")
        .and_then(|value| value.as_array())
    {
        for value in array {
            if let Some(decoded) = value
                .as_str()
                .and_then(|hex_str| hex::decode(hex_str.trim()).ok())
                .filter(|decoded| !decoded.is_empty())
            {
                return Ok(decoded);
            }
        }
    }
    if let Some(hex_value) = manifest
        .get("root_cid_hex")
        .and_then(|value| value.as_str())
    {
        return hex::decode(hex_value.trim())
            .map_err(|err| format!("failed to decode root_cid_hex value: {err}").into());
    }

    Err("manifest JSON is missing `root_cid`, `root_cids_hex`, or `root_cid_hex` fields".into())
}

fn encode_base32_lower(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if bytes.is_empty() {
        return String::new();
    }
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut output = String::new();
    for &byte in bytes {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1F) as usize;
            output.push(ALPHABET[index] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1F) as usize;
        output.push(ALPHABET[index] as char);
    }
    output
}

fn format_headers_template(headers: &BTreeMap<String, String>) -> String {
    let mut lines = Vec::new();
    for &key in ROUTE_HEADER_ORDER {
        if let Some(value) = headers.get(key) {
            lines.push(format!("{key}: {value}"));
        }
    }
    for (key, value) in headers {
        if ROUTE_HEADER_ORDER.contains(&key.as_str()) {
            continue;
        }
        lines.push(format!("{key}: {value}"));
    }
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

fn write_optional_output(path: &Option<PathBuf>, contents: &str) -> Result<(), Box<dyn Error>> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }
    Ok(())
}

fn parse_gateway_key_cli<I>(args: I) -> Result<GatewayCliCommand, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let Some(action) = iter.next() else {
        gateway_cli_usage();
        return Err("sorafs-gateway key requires an action (rotate)".into());
    };

    match action.as_str() {
        "rotate" => {
            let mut kind: Option<String> = None;
            let mut output_path: Option<PathBuf> = None;
            let mut public_out: Option<PathBuf> = None;
            let mut force = false;
            let mut pending = iter.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--kind" => {
                        let Some(value) = pending.next() else {
                            return Err("--kind requires a value".into());
                        };
                        kind = Some(value);
                    }
                    "--out" | "--output" => {
                        let Some(value) = pending.next() else {
                            return Err("--out requires a path".into());
                        };
                        output_path = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--public-out" => {
                        let Some(value) = pending.next() else {
                            return Err("--public-out requires a path".into());
                        };
                        public_out = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--force" => force = true,
                    "-h" | "--help" => {
                        gateway_cli_usage();
                        return Err("sorafs-gateway key rotate usage displayed".into());
                    }
                    flag => {
                        return Err(
                            format!("unknown flag for sorafs-gateway key rotate: {flag}").into(),
                        );
                    }
                }
            }

            let kind = kind.unwrap_or_else(|| "token-signing".to_string());
            let output_path = output_path
                .ok_or_else(|| "sorafs-gateway key rotate requires --out <path>".to_string())?;
            Ok(GatewayCliCommand::KeyRotate(GatewayKeyRotateOptions {
                kind,
                output_path,
                public_out,
                force,
            }))
        }
        "-h" | "--help" => {
            gateway_cli_usage();
            Err("sorafs-gateway key usage displayed".into())
        }
        other => {
            gateway_cli_usage();
            Err(format!("unknown sorafs-gateway key action `{other}`").into())
        }
    }
}

fn parse_gateway_denylist_cli<I>(args: I) -> Result<GatewayCliCommand, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let Some(action) = iter.next() else {
        gateway_cli_usage();
        return Err("sorafs-gateway denylist requires an action (pack|diff|verify)".into());
    };

    match action.as_str() {
        "pack" => {
            let mut input_path: Option<PathBuf> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut label: Option<String> = None;
            let mut force = false;
            let mut pending = iter.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--input" | "--denylist" => {
                        let Some(value) = pending.next() else {
                            return Err("--input requires a path".into());
                        };
                        input_path = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--out" | "--output" => {
                        let Some(value) = pending.next() else {
                            return Err("--out requires a directory path".into());
                        };
                        output_dir = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--label" => {
                        let Some(value) = pending.next() else {
                            return Err("--label requires a value".into());
                        };
                        label = Some(value);
                    }
                    "--force" => force = true,
                    "-h" | "--help" => {
                        gateway_cli_usage();
                        return Err("sorafs-gateway denylist usage displayed".into());
                    }
                    flag => {
                        gateway_cli_usage();
                        return Err(
                            format!("unknown flag for sorafs-gateway denylist: {flag}").into()
                        );
                    }
                }
            }

            let input_path = input_path.ok_or_else(|| {
                "--input <path> is required for sorafs-gateway denylist pack".to_string()
            })?;
            let output_dir = output_dir.unwrap_or_else(default_denylist_pack_dir);
            Ok(GatewayCliCommand::DenylistPack(
                GatewayDenylistPackOptions {
                    input_path,
                    output_dir,
                    label,
                    force,
                },
            ))
        }
        "diff" => {
            let mut old_bundle: Option<PathBuf> = None;
            let mut new_bundle: Option<PathBuf> = None;
            let mut report_path: Option<PathBuf> = None;
            let mut pending = iter.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--old" => {
                        let Some(value) = pending.next() else {
                            return Err("--old requires a bundle path".into());
                        };
                        old_bundle = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--new" => {
                        let Some(value) = pending.next() else {
                            return Err("--new requires a bundle path".into());
                        };
                        new_bundle = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--report-json" => {
                        let Some(value) = pending.next() else {
                            return Err("--report-json requires a path".into());
                        };
                        report_path = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "-h" | "--help" => {
                        gateway_cli_usage();
                        return Err("sorafs-gateway denylist usage displayed".into());
                    }
                    flag => {
                        gateway_cli_usage();
                        return Err(
                            format!("unknown flag for sorafs-gateway denylist: {flag}").into()
                        );
                    }
                }
            }

            let old_bundle = old_bundle.ok_or_else(|| {
                "--old <bundle.json> is required for sorafs-gateway denylist diff".to_string()
            })?;
            let new_bundle = new_bundle.ok_or_else(|| {
                "--new <bundle.json> is required for sorafs-gateway denylist diff".to_string()
            })?;
            Ok(GatewayCliCommand::DenylistDiff(
                GatewayDenylistDiffOptions {
                    old_bundle,
                    new_bundle,
                    report_path,
                },
            ))
        }
        "verify" => {
            let mut bundle: Option<PathBuf> = None;
            let mut norito_path: Option<PathBuf> = None;
            let mut root_path: Option<PathBuf> = None;
            let mut report_path: Option<PathBuf> = None;
            let mut pending = iter.peekable();
            while let Some(arg) = pending.next() {
                match arg.as_str() {
                    "--bundle" => {
                        let Some(value) = pending.next() else {
                            return Err("--bundle requires a bundle path".into());
                        };
                        bundle = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--norito" => {
                        let Some(value) = pending.next() else {
                            return Err("--norito requires a path".into());
                        };
                        norito_path = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--root" => {
                        let Some(value) = pending.next() else {
                            return Err("--root requires a path".into());
                        };
                        root_path = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "--report-json" => {
                        let Some(value) = pending.next() else {
                            return Err("--report-json requires a path".into());
                        };
                        report_path = Some(crate::normalize_path(Path::new(&value))?);
                    }
                    "-h" | "--help" => {
                        gateway_cli_usage();
                        return Err("sorafs-gateway denylist usage displayed".into());
                    }
                    flag => {
                        gateway_cli_usage();
                        return Err(
                            format!("unknown flag for sorafs-gateway denylist: {flag}").into()
                        );
                    }
                }
            }

            let bundle = bundle.ok_or_else(|| {
                "--bundle <bundle.json> is required for sorafs-gateway denylist verify".to_string()
            })?;
            Ok(GatewayCliCommand::DenylistVerify(
                GatewayDenylistVerifyOptions {
                    bundle,
                    norito_path,
                    root_path,
                    report_path,
                },
            ))
        }
        "-h" | "--help" => {
            gateway_cli_usage();
            Err("sorafs-gateway denylist usage displayed".into())
        }
        other => {
            gateway_cli_usage();
            Err(format!("unknown sorafs-gateway denylist action `{other}`").into())
        }
    }
}

pub fn gateway_tls_renew(
    options: GatewayTlsRenewOptions,
) -> Result<GatewayTlsRenewOutcome, Box<dyn Error>> {
    if options.hostnames.is_empty() {
        return Err("sorafs-gateway tls renew requires at least one hostname".into());
    }

    fs::create_dir_all(&options.output_dir)?;
    let mut automation = AcmeAutomation::new(
        AcmeConfig {
            enabled: true,
            hostnames: options.hostnames.clone(),
            account_email: options.account_email.clone(),
            directory_url: options.directory_url.clone(),
            dns_provider_id: options.dns_provider_id.clone(),
            ..AcmeConfig::default()
        },
        SelfSignedAcmeClient,
    );
    let bundle = automation
        .process(SystemTime::now())
        .map_err(|err| format!("failed to run TLS automation: {err}"))?
        .ok_or_else(|| "TLS automation did not produce a certificate bundle".to_string())?;

    let certificate_path = options.output_dir.join("fullchain.pem");
    write_file_with_mode(
        &certificate_path,
        bundle.certificate_pem.as_bytes(),
        options.force,
        0o600,
    )?;
    let private_key_path = options.output_dir.join("privkey.pem");
    write_file_with_mode(
        &private_key_path,
        bundle.private_key_pem.as_bytes(),
        options.force,
        0o600,
    )?;
    let ech_config_path = options.output_dir.join("ech.json");
    let ech_contents = if let Some(config) = bundle.ech_config.as_ref() {
        let encoded = BASE64_STD.encode(config);
        format!("{{\"ech_config_b64\":\"{encoded}\"}}\n")
    } else {
        "null\n".to_string()
    };
    write_file_with_mode(
        &ech_config_path,
        ech_contents.as_bytes(),
        options.force,
        0o640,
    )?;

    let fingerprint_hex = compute_self_signed_fingerprint(
        &options.hostnames,
        options.account_email.as_deref(),
        &options.directory_url,
    );
    let expiry_rfc3339 = OffsetDateTime::from(bundle.not_after)
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format certificate expiry: {err}"))?;

    Ok(GatewayTlsRenewOutcome {
        certificate_path,
        private_key_path,
        ech_config_path,
        fingerprint_hex,
        expiry_rfc3339,
        hostnames: options.hostnames.clone(),
    })
}

pub fn gateway_tls_revoke(
    options: GatewayTlsRevokeOptions,
) -> Result<GatewayTlsRevokeOutcome, Box<dyn Error>> {
    if !options.bundle_dir.exists() {
        return Err(format!(
            "bundle directory {} does not exist",
            options.bundle_dir.display()
        )
        .into());
    }

    let archive_dir = options
        .archive_dir
        .clone()
        .unwrap_or_else(|| options.bundle_dir.join("revoked"));
    fs::create_dir_all(&archive_dir)?;

    let timestamp_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock before UNIX epoch")?
        .as_secs();
    let suffix = format!(".revoked.{timestamp_epoch}");

    let mut archived_files = Vec::new();
    for name in ["fullchain.pem", "privkey.pem"] {
        let source = options.bundle_dir.join(name);
        if !source.exists() {
            if options.force {
                continue;
            }
            return Err(format!(
                "missing {} in {}; use --force to skip",
                name,
                options.bundle_dir.display()
            )
            .into());
        }
        let target = archive_dir.join(format!("{name}{suffix}"));
        if target.exists() && !options.force {
            return Err(format!(
                "archive target {} already exists; use --force to overwrite",
                target.display()
            )
            .into());
        }
        fs::rename(&source, &target)?;
        archived_files.push(target);
    }

    let ech_source = options.bundle_dir.join("ech.json");
    if ech_source.exists() {
        let target = archive_dir.join(format!("ech.json{suffix}"));
        if target.exists() && !options.force {
            return Err(format!(
                "archive target {} already exists; use --force to overwrite",
                target.display()
            )
            .into());
        }
        fs::rename(&ech_source, &target)?;
        archived_files.push(target);
    }

    let mut audit = Map::new();
    audit.insert(
        "timestamp_epoch".into(),
        Value::Number(timestamp_epoch.into()),
    );
    if let Some(reason) = options.reason.clone() {
        audit.insert("reason".into(), Value::String(reason.clone()));
    }
    let files_value = archived_files
        .iter()
        .map(|path| Value::String(path.display().to_string()))
        .collect::<Vec<_>>();
    audit.insert("archived_files".into(), Value::Array(files_value));
    let audit_json = to_string_pretty(&Value::Object(audit))
        .map_err(|err| format!("failed to encode revocation audit: {err}"))?;
    let audit_path = archive_dir.join(format!("revocation-{timestamp_epoch}.json"));
    write_file_with_mode(&audit_path, audit_json.as_bytes(), true, 0o600)?;
    archived_files.push(audit_path.clone());

    Ok(GatewayTlsRevokeOutcome {
        archive_dir,
        archived_files,
        timestamp_epoch,
        reason: options.reason,
    })
}

pub fn gateway_key_rotate(
    options: GatewayKeyRotateOptions,
) -> Result<GatewayKeyRotateOutcome, Box<dyn Error>> {
    if options.kind.as_str() != "token-signing" {
        return Err(format!(
            "unsupported key kind `{}` (expected token-signing)",
            options.kind
        )
        .into());
    }

    if let Some(parent) = options.output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (algorithm, private_bytes) = keypair.private_key().to_bytes();
    if algorithm != Algorithm::Ed25519 {
        return Err("unexpected algorithm for generated private key".into());
    }
    let private_hex = hex::encode(private_bytes);
    let private_contents = format!("{private_hex}\n");
    write_file_with_mode(
        &options.output_path,
        private_contents.as_bytes(),
        options.force,
        0o600,
    )?;

    let public = keypair.public_key();
    let (_, public_bytes) = public.to_bytes();
    let public_key_hex = hex::encode(public_bytes);
    let public_key_prefixed = public.to_prefixed_string();

    let mut public_out_path = None;
    if let Some(path) = options.public_out.as_ref() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut payload = Map::new();
        payload.insert("algorithm".into(), Value::String("ed25519".into()));
        payload.insert("key_hex".into(), Value::String(public_key_hex.clone()));
        payload.insert(
            "key_multihash".into(),
            Value::String(public_key_prefixed.clone()),
        );
        let json = to_string_pretty(&Value::Object(payload))
            .map_err(|err| format!("failed to encode public key JSON: {err}"))?;
        write_file_with_mode(path, json.as_bytes(), options.force, 0o640)?;
        public_out_path = Some(path.clone());
    }

    Ok(GatewayKeyRotateOutcome {
        private_key_path: options.output_path,
        public_key_hex,
        public_key_prefixed,
        public_out_path,
    })
}

pub fn gateway_denylist_pack(
    options: GatewayDenylistPackOptions,
) -> Result<GatewayDenylistPackOutcome, Box<dyn Error>> {
    if !options.input_path.exists() {
        return Err(format!(
            "denylist file `{}` does not exist",
            options.input_path.display()
        )
        .into());
    }
    fs::create_dir_all(&options.output_dir)?;

    let bytes = fs::read(&options.input_path).map_err(|err| {
        format!(
            "failed to read denylist `{}`: {err}",
            options.input_path.display()
        )
    })?;
    let raw: JsonValue = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse denylist JSON `{}`: {err}",
            options.input_path.display()
        )
    })?;
    let entries = raw
        .as_array()
        .ok_or_else(|| "denylist JSON must be an array of entry objects".to_string())?;
    if entries.is_empty() {
        return Err("denylist JSON contains no entries".into());
    }

    let mut canonical_entries = Vec::with_capacity(entries.len());
    for (index, value) in entries.iter().enumerate() {
        let canonical = canonicalize_entry(value, index)
            .map_err(|err| format!("failed to canonicalize entry #{index}: {err}"))?;
        canonical_entries.push(canonical);
    }

    let leaves: Vec<[u8; 32]> = canonical_entries.iter().map(|entry| entry.digest).collect();
    let layers = build_merkle_layers(&leaves)?;
    let root = layers
        .last()
        .and_then(|level| level.first().copied())
        .ok_or_else(|| "failed to compute Merkle root".to_string())?;
    let root_hex = hex::encode(root);

    let generated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format timestamp: {err}"))?;
    let timestamp_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock before UNIX epoch")?
        .as_secs();
    let slug = options
        .label
        .as_deref()
        .map(slugify_label)
        .filter(|slug| !slug.is_empty())
        .unwrap_or_else(|| "denylist".to_string());
    let base_name = format!("{slug}_{timestamp_epoch}");

    let bundle_entries = canonical_entries
        .iter()
        .enumerate()
        .map(|(index, entry)| DenylistBundleEntry {
            index: index as u32,
            kind: entry.kind.clone(),
            digest_hex: hex::encode(entry.digest),
            canonical_json: entry.canonical_json.clone(),
            proof: build_merkle_proof(&layers, index),
        })
        .collect();

    let bundle = DenylistBundleV1 {
        version: 1,
        label: options.label.clone(),
        generated_at: generated_at.clone(),
        root_hex: root_hex.clone(),
        entry_count: canonical_entries.len() as u32,
        entries: bundle_entries,
    };

    let bundle_json = serde_json::to_string_pretty(&bundle)
        .map_err(|err| format!("failed to serialize denylist bundle to JSON: {err}"))?;
    let bundle_path = options.output_dir.join(format!("{base_name}.json"));
    write_file_with_mode(&bundle_path, bundle_json.as_bytes(), options.force, 0o640)?;

    let norito_bytes = to_bytes(&bundle)
        .map_err(|err| format!("failed to encode denylist bundle as Norito: {err}"))?;
    let norito_path = options.output_dir.join(format!("{base_name}.to"));
    write_file_with_mode(&norito_path, &norito_bytes, options.force, 0o640)?;

    let root_path = options.output_dir.join(format!("{base_name}_root.txt"));
    let root_contents = format!("{root_hex}\nGenerated:{generated_at}\n");
    write_file_with_mode(&root_path, root_contents.as_bytes(), options.force, 0o644)?;

    Ok(GatewayDenylistPackOutcome {
        bundle_path,
        norito_path,
        root_path,
        root_hex,
        entry_count: canonical_entries.len(),
    })
}

pub fn gateway_denylist_diff(
    options: &GatewayDenylistDiffOptions,
) -> Result<DenylistDiffReport, Box<dyn Error>> {
    let old_bundle = load_denylist_bundle(&options.old_bundle)?;
    let new_bundle = load_denylist_bundle(&options.new_bundle)?;
    let old_index = index_bundle_entries(&old_bundle);
    let new_index = index_bundle_entries(&new_bundle);
    let old_keys: BTreeSet<&str> = old_index.keys().copied().collect();
    let new_keys: BTreeSet<&str> = new_index.keys().copied().collect();

    let mut added = Vec::new();
    for key in new_keys.difference(&old_keys) {
        if let Some(entry) = new_index.get(*key) {
            added.push(diff_entry_from(entry));
        }
    }

    let mut removed = Vec::new();
    for key in old_keys.difference(&new_keys) {
        if let Some(entry) = old_index.get(*key) {
            removed.push(diff_entry_from(entry));
        }
    }

    let unchanged_entries = new_keys.intersection(&old_keys).count();

    Ok(DenylistDiffReport {
        old_label: old_bundle.label.clone(),
        new_label: new_bundle.label.clone(),
        old_root: old_bundle.root_hex.clone(),
        new_root: new_bundle.root_hex.clone(),
        old_entry_count: old_bundle.entry_count,
        new_entry_count: new_bundle.entry_count,
        added,
        removed,
        unchanged_entries,
    })
}

pub fn gateway_denylist_verify(
    options: &GatewayDenylistVerifyOptions,
) -> Result<GatewayDenylistVerifyReport, Box<dyn Error>> {
    let bundle = load_denylist_bundle(&options.bundle)?;
    if bundle.version != 1 {
        return Err(format!(
            "unsupported denylist bundle version {}; expected 1",
            bundle.version
        )
        .into());
    }
    if bundle.entries.is_empty() {
        return Err("denylist bundle does not contain any entries".into());
    }
    if bundle.entry_count != bundle.entries.len() as u32 {
        return Err(format!(
            "entry_count {} does not match actual entry length {}",
            bundle.entry_count,
            bundle.entries.len()
        )
        .into());
    }

    let mut leaves = Vec::with_capacity(bundle.entries.len());
    for (index, entry) in bundle.entries.iter().enumerate() {
        if entry.index != index as u32 {
            return Err(format!(
                "entry #{index} has index {} but expected {}",
                entry.index, index
            )
            .into());
        }
        let parsed: JsonValue = serde_json::from_str(&entry.canonical_json)
            .map_err(|err| format!("failed to parse canonical_json for entry #{index}: {err}"))?;
        let canonical = canonicalize_json_value(&parsed);
        let canonical_bytes = serde_json::to_vec(&canonical).map_err(|err| {
            format!("failed to encode canonical entry #{index} for verification: {err}")
        })?;
        if canonical_bytes != entry.canonical_json.as_bytes() {
            return Err(format!("entry #{index} canonical JSON is not normalized").into());
        }
        let kind = canonical
            .get("kind")
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("entry #{index} missing `kind` field"))?;
        if kind != entry.kind {
            return Err(format!(
                "entry #{index} kind mismatch (`{}` vs `{kind}`)",
                entry.kind
            )
            .into());
        }
        let digest_hex = parse_digest_hex(&entry.digest_hex).map_err(|err| {
            format!(
                "entry #{index} digest `{}` invalid: {err}",
                entry.digest_hex
            )
        })?;
        let recomputed = hash_leaf(&canonical_bytes);
        if digest_hex != recomputed {
            return Err(format!(
                "entry #{index} digest mismatch (`{}` vs recomputed `{}`)",
                entry.digest_hex,
                hex::encode(recomputed)
            )
            .into());
        }
        leaves.push(recomputed);
    }

    let layers = build_merkle_layers(&leaves)?;
    let Some(root_layer) = layers.last() else {
        return Err("failed to compute Merkle root".into());
    };
    let Some(root_digest) = root_layer.first().copied() else {
        return Err("failed to compute Merkle root".into());
    };
    let computed_root_hex = hex::encode(root_digest);
    if !bundle
        .root_hex
        .eq_ignore_ascii_case(computed_root_hex.as_str())
    {
        return Err(format!(
            "bundle root {} does not match computed {}",
            bundle.root_hex, computed_root_hex
        )
        .into());
    }

    let mut norito_verified = false;
    if let Some(path) = options.norito_path.as_ref() {
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read Norito bundle `{}`: {err}", path.display()))?;
        let decoded: DenylistBundleV1 = decode_from_bytes(&bytes).map_err(|err| {
            format!(
                "failed to decode Norito denylist `{}`: {err}",
                path.display()
            )
        })?;
        if decoded != bundle {
            return Err(format!(
                "Norito bundle `{}` does not match JSON bundle {}",
                path.display(),
                options.bundle.display()
            )
            .into());
        }
        norito_verified = true;
    }

    let mut root_verified = false;
    if let Some(path) = options.root_path.as_ref() {
        let contents = fs::read_to_string(path)
            .map_err(|err| format!("failed to read root file `{}`: {err}", path.display()))?;
        verify_root_file(&bundle, &contents, path)?;
        root_verified = true;
    }

    let report = GatewayDenylistVerifyReport {
        bundle_path: options.bundle.display().to_string(),
        label: bundle.label.clone(),
        generated_at: bundle.generated_at.clone(),
        root_hex: bundle.root_hex.clone(),
        entry_count: bundle.entry_count,
        norito_verified,
        norito_path: options
            .norito_path
            .as_ref()
            .map(|path| path.display().to_string()),
        root_file_verified: root_verified,
        root_file_path: options
            .root_path
            .as_ref()
            .map(|path| path.display().to_string()),
    };
    Ok(report)
}

fn load_denylist_bundle(path: &Path) -> Result<DenylistBundleV1, Box<dyn Error>> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read denylist bundle `{}`: {err}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse denylist bundle `{}`: {err}",
            path.display()
        )
        .into()
    })
}

fn parse_digest_hex(hex_str: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex_str).map_err(|err| format!("invalid hex: {err}"))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes but found {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn verify_root_file(
    bundle: &DenylistBundleV1,
    contents: &str,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut lines = contents.lines().filter(|line| !line.trim().is_empty());
    let Some(root_line) = lines.next() else {
        return Err(format!("root file `{}` missing root digest line", path.display()).into());
    };
    if !bundle.root_hex.eq_ignore_ascii_case(root_line.trim()) {
        return Err(format!(
            "root file `{}` digest `{}` does not match bundle root `{}`",
            path.display(),
            root_line.trim(),
            bundle.root_hex
        )
        .into());
    }
    let Some(generated_line) = lines.next() else {
        return Err(format!("root file `{}` missing Generated line", path.display()).into());
    };
    let Some(timestamp) = generated_line.trim().strip_prefix("Generated:") else {
        return Err(format!(
            "root file `{}` second line must start with `Generated:`",
            path.display()
        )
        .into());
    };
    if timestamp.trim() != bundle.generated_at {
        return Err(format!(
            "root file `{}` generated timestamp `{}` does not match bundle `{}`",
            path.display(),
            timestamp.trim(),
            bundle.generated_at
        )
        .into());
    }
    Ok(())
}

fn index_bundle_entries(bundle: &DenylistBundleV1) -> BTreeMap<&str, &DenylistBundleEntry> {
    let mut map = BTreeMap::new();
    for entry in &bundle.entries {
        map.insert(entry.canonical_json.as_str(), entry);
    }
    map
}

fn diff_entry_from(entry: &DenylistBundleEntry) -> DenylistDiffEntry {
    DenylistDiffEntry {
        kind: entry.kind.clone(),
        digest_hex: entry.digest_hex.clone(),
        canonical_json: entry.canonical_json.clone(),
    }
}

fn gateway_cli_usage() {
    eprintln!("sorafs-gateway CLI usage:");
    eprintln!(
        "  cargo xtask sorafs-gateway tls renew --host <hostname>... [--hosts-from <san.json> ...] --out <dir> [--account-email <email>] [--directory-url <url>] [--dns-provider-id <id>] [--force]"
    );
    eprintln!(
        "  cargo xtask sorafs-gateway tls revoke --out <dir> [--archive-dir <dir>] [--reason <text>] [--force]"
    );
    eprintln!(
        "  cargo xtask sorafs-gateway key rotate --kind token-signing --out <path> [--public-out <path>] [--force]"
    );
    eprintln!(
        "  cargo xtask sorafs-gateway denylist pack --input <path> [--out <dir>] [--label <text>] [--force]"
    );
    eprintln!(
        "  cargo xtask sorafs-gateway denylist diff --old <bundle.json> --new <bundle.json> [--report-json <path>]"
    );
    eprintln!(
        "  cargo xtask sorafs-gateway denylist verify --bundle <bundle.json> [--norito <bundle.to>] [--root <root.txt>] [--report-json <path>]"
    );
    eprintln!(
        "  cargo xtask sorafs-gateway route plan --manifest-json <path> --hostname <host> [--alias <namespace:name>] [--route-label <label>] [--out <path>] [--headers-out <path>] [--rollback-manifest-json <path>] [--rollback-route-label <label>]"
    );
}

fn normalize_tls_host(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("hostname must not be empty".to_string());
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(format!("hostname `{trimmed}` must not contain whitespace"));
    }
    if trimmed.contains('/') {
        return Err(format!(
            "hostname `{trimmed}` must not include protocol/path separators"
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn load_tls_hosts_from_file(path: &Path) -> Result<Vec<String>, String> {
    let data = fs::read(path)
        .map_err(|err| format!("failed to read host fixture `{}`: {err}", path.display()))?;
    let value: JsonValue = serde_json::from_slice(&data)
        .map_err(|err| format!("failed to parse host fixture `{}`: {err}", path.display()))?;
    let hosts = if let Some(array) = value.as_array() {
        parse_host_array(path, array)?
    } else if let Some(object) = value.as_object() {
        if let Some(array) = object.get("san_hosts").and_then(JsonValue::as_array) {
            parse_host_array(path, array)?
        } else if let Some(array) = object.get("hosts").and_then(JsonValue::as_array) {
            parse_host_array(path, array)?
        } else {
            return Err(format!(
                "host fixture `{}` must contain a `san_hosts` array",
                path.display()
            ));
        }
    } else {
        return Err(format!(
            "host fixture `{}` must be a JSON array or object with `san_hosts`",
            path.display()
        ));
    };
    Ok(hosts)
}

fn parse_host_array(path: &Path, array: &[JsonValue]) -> Result<Vec<String>, String> {
    let mut hosts = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, entry) in array.iter().enumerate() {
        let Some(raw) = entry.as_str() else {
            return Err(format!(
                "host fixture `{}` entry #{index} is not a string",
                path.display()
            ));
        };
        let normalized = normalize_tls_host(raw).map_err(|err| {
            format!(
                "host fixture `{}` entry #{index} is invalid: {err}",
                path.display()
            )
        })?;
        if seen.insert(normalized.clone()) {
            hosts.push(normalized);
        }
    }
    Ok(hosts)
}

fn dedup_tls_hosts(hosts: &mut Vec<String>) {
    let mut seen = HashSet::new();
    hosts.retain(|host| seen.insert(host.clone()));
}

fn compute_self_signed_fingerprint(
    hosts: &[String],
    account_email: Option<&str>,
    directory_url: &str,
) -> String {
    let mut hasher = Blake3Hasher::new();
    for host in hosts {
        hasher.update(host.as_bytes());
    }

    if let Some(email) = account_email {
        hasher.update(email.as_bytes());
    }
    hasher.update(directory_url.as_bytes());

    hex::encode(hasher.finalize().as_bytes())
}

fn canonicalize_entry(value: &JsonValue, index: usize) -> Result<CanonicalEntry, String> {
    if !value.is_object() {
        return Err("denylist entry must be a JSON object".into());
    }
    let canonical = canonicalize_json_value(value);
    let kind = canonical
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "entry missing string `kind` field".to_string())?;
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|err| format!("failed to encode canonical entry #{index}: {err}"))?;
    let canonical_json = String::from_utf8(bytes.clone())
        .map_err(|err| format!("canonical JSON for entry #{index} is not UTF-8: {err}"))?;
    let digest = hash_leaf(&bytes);
    Ok(CanonicalEntry {
        kind: kind.to_owned(),
        canonical_json,
        digest,
    })
}

fn canonicalize_json_value(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let mut ordered = serde_json::Map::new();
            for key in keys {
                ordered.insert(key.clone(), canonicalize_json_value(&map[key]));
            }
            JsonValue::Object(ordered)
        }
        JsonValue::Array(items) => {
            JsonValue::Array(items.iter().map(canonicalize_json_value).collect())
        }
        _ => value.clone(),
    }
}

fn hash_leaf(payload: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"denylist-leaf:v1");
    hasher.update(payload);
    hash_to_array(hasher.finalize())
}

fn hash_internal(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"denylist-node:v1");
    hasher.update(left);
    hasher.update(right);
    hash_to_array(hasher.finalize())
}

fn hash_to_array(hash: blake3::Hash) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

fn build_merkle_layers(leaves: &[[u8; 32]]) -> Result<Vec<Vec<[u8; 32]>>, String> {
    if leaves.is_empty() {
        return Err("cannot build Merkle tree without leaves".into());
    }
    let mut layers = Vec::new();
    let mut current = leaves.to_vec();
    layers.push(current.clone());
    while current.len() > 1 {
        let mut next = Vec::with_capacity(current.len().div_ceil(2));
        for chunk in current.chunks(2) {
            let combined = if chunk.len() == 2 {
                hash_internal(&chunk[0], &chunk[1])
            } else {
                hash_internal(&chunk[0], &chunk[0])
            };
            next.push(combined);
        }
        layers.push(next.clone());
        current = next;
    }
    Ok(layers)
}

fn build_merkle_proof(layers: &[Vec<[u8; 32]>], mut index: usize) -> Vec<MerkleProofNode> {
    if layers.is_empty() {
        return Vec::new();
    }
    let mut proof = Vec::new();
    for nodes in layers.iter().take(layers.len().saturating_sub(1)) {
        if nodes.len() == 1 {
            index /= 2;
            continue;
        }
        let is_right = index % 2 == 1;
        let sibling_index = if is_right {
            index.saturating_sub(1)
        } else {
            index + 1
        };
        let sibling_digest = if sibling_index < nodes.len() {
            nodes[sibling_index]
        } else {
            nodes[index]
        };
        let direction = if is_right {
            MerkleSiblingPosition::Left
        } else {
            MerkleSiblingPosition::Right
        };
        proof.push(MerkleProofNode {
            direction,
            digest_hex: hex::encode(sibling_digest),
        });
        index /= 2;
    }
    proof
}

fn slugify_label(raw: &str) -> String {
    let mut slug = String::with_capacity(raw.len());
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if matches!(ch, ' ' | '_' | '-' | '.') && !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod reserve_matrix_tests {
    use serde_json::Value as SerdeJsonValue;

    use super::*;

    fn render_matrix_json(options: ReserveMatrixOptions) -> SerdeJsonValue {
        let value = reserve_matrix_report(options).expect("matrix report");
        let rendered =
            norito::json::to_json_pretty(&value).expect("matrix payload renders to JSON");
        serde_json::from_str(&rendered).expect("matrix payload parses")
    }

    #[test]
    fn reserve_matrix_includes_ledger_projection() {
        let options = ReserveMatrixOptions {
            capacities_gib: vec![10, 25],
            storage_classes: vec![StorageClass::Hot],
            tiers: vec![ReserveTier::TierA, ReserveTier::TierB],
            durations: vec![ReserveDuration::Monthly],
            reserve_balance: XorAmount::from_micro(5 * MICRO_XOR_PER_XOR),
            policy_json: None,
            policy_norito: None,
            label: Some("matrix-test".into()),
        };
        let json = render_matrix_json(options);
        assert_eq!(json["policy_version"].as_u64().unwrap(), 1);
        assert_eq!(
            json["policy_source"].as_str().unwrap(),
            "embedded default policy"
        );
        assert_eq!(json["matrix_entry_count"].as_u64().unwrap(), 4);
        assert_eq!(json["matrix"].as_array().unwrap().len(), 4);
        assert_eq!(json["label"].as_str().unwrap(), "matrix-test");
        assert_eq!(
            json["reserve_balance_micro_xor"].as_u64().unwrap(),
            5 * MICRO_XOR_PER_XOR as u64
        );
        assert_eq!(json["policy_sha256"].as_str().unwrap().len(), 64);
        let entry = &json["matrix"][0];
        assert_eq!(entry["storage_class"].as_str().unwrap(), "hot");
        assert_eq!(entry["tier"].as_str().unwrap(), "tier-a");
        assert_eq!(entry["inputs"]["storage_class"].as_str().unwrap(), "hot");
        assert_eq!(
            entry["inputs"]["reserve_balance"].as_u64().unwrap(),
            json["reserve_balance_micro_xor"].as_u64().unwrap()
        );
        let rent_due = entry["ledger_projection"]["rent_due"].as_u64().unwrap();
        let effective_rent = entry["quote"]["effective_rent"].as_u64().unwrap();
        assert_eq!(rent_due, effective_rent);
    }

    #[test]
    fn reserve_matrix_requires_capacity() {
        let options = ReserveMatrixOptions {
            capacities_gib: vec![],
            storage_classes: vec![StorageClass::Hot],
            tiers: vec![ReserveTier::TierA],
            durations: vec![ReserveDuration::Monthly],
            reserve_balance: XorAmount::zero(),
            policy_json: None,
            policy_norito: None,
            label: None,
        };
        let err =
            reserve_matrix_report(options).expect_err("matrix should fail without capacities");
        assert!(
            err.to_string()
                .contains("reserve matrix requires at least one --capacity")
        );
    }
}

#[cfg(test)]
mod rollout_tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn route_plan_generates_headers_and_plan() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(&manifest_path, r#"{ "root_cid": [0, 0, 0, 0, 0] }"#).expect("manifest");
        let plan_path = temp.path().join("route_plan.json");
        let headers_path = temp.path().join("route_headers.txt");
        let options = GatewayRoutePlanOptions {
            manifest_json: manifest_path.clone(),
            output_path: plan_path.clone(),
            headers_out: Some(headers_path.clone()),
            alias: Some("sora:docs".into()),
            hostname: Some("docs.sora.link".into()),
            route_label: Some("docs@canary".into()),
            proof_status: Some("ok".into()),
            release_tag: Some("v1.2.3".into()),
            cutover_window: Some("2026-03-21T15:00Z/2026-03-21T15:30Z".into()),
            rollback_manifest: None,
            rollback_headers_out: None,
            rollback_route_label: None,
            rollback_release_tag: None,
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        run_gateway_route_plan(options).expect("route plan generation succeeds");

        let rendered = fs::read_to_string(&plan_path).expect("plan contents");
        let plan_json: serde_json::Value =
            serde_json::from_str(&rendered).expect("json plan payload");
        assert_eq!(
            plan_json["content_cid"],
            serde_json::Value::String("baaaaaaaa".into())
        );
        assert_eq!(
            plan_json["route_binding"],
            serde_json::Value::String(
                "host=docs.sora.link;cid=baaaaaaaa;generated_at=1970-01-01T00:00:00Z;label=docs@canary"
                    .into()
            )
        );
        assert_eq!(
            plan_json["headers"]["Sora-Name"],
            serde_json::Value::String("sora:docs".into())
        );
        assert_eq!(
            plan_json["headers"]["Sora-Content-CID"],
            serde_json::Value::String("baaaaaaaa".into())
        );
        assert_eq!(
            plan_json["headers_path"],
            serde_json::Value::String(headers_path.display().to_string())
        );
        assert!(plan_json["rollback"].is_null());

        let template = fs::read_to_string(headers_path).expect("headers template");
        assert!(
            template.contains("Sora-Route-Binding: host=docs.sora.link;cid=baaaaaaaa;generated_at=1970-01-01T00:00:00Z;label=docs@canary"),
            "template missing route binding:\n{template}"
        );
        assert!(template.contains("Content-Security-Policy: default-src 'self'"));
    }

    #[test]
    fn route_plan_embeds_rollback_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        let rollback_manifest = temp.path().join("rollback_manifest.json");
        fs::write(&manifest_path, r#"{ "root_cids_hex": ["00"] }"#).expect("manifest");
        fs::write(&rollback_manifest, r#"{ "root_cid_hex": "ff" }"#).expect("rollback manifest");
        let plan_path = temp.path().join("route_plan.json");
        let rollback_headers = temp.path().join("rollback_headers.txt");
        let options = GatewayRoutePlanOptions {
            manifest_json: manifest_path,
            output_path: plan_path.clone(),
            headers_out: None,
            alias: Some("sora:docs".into()),
            hostname: Some("docs.sora.link".into()),
            route_label: None,
            proof_status: None,
            release_tag: Some("v1".into()),
            cutover_window: None,
            rollback_manifest: Some(rollback_manifest.clone()),
            rollback_headers_out: Some(rollback_headers.clone()),
            rollback_route_label: Some("previous".into()),
            rollback_release_tag: Some("v0".into()),
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        run_gateway_route_plan(options).expect("route plan generation succeeds");

        let rendered = fs::read_to_string(&plan_path).expect("plan contents");
        let plan_json: serde_json::Value =
            serde_json::from_str(&rendered).expect("json plan payload");
        let rollback = plan_json["rollback"].as_object().expect("rollback section");
        assert_eq!(
            rollback["manifest_json"],
            serde_json::Value::String(rollback_manifest.display().to_string())
        );
        assert_eq!(
            rollback["release_tag"],
            serde_json::Value::String("v0".into())
        );
        assert_eq!(
            rollback["route_binding"],
            serde_json::Value::String(
                "host=docs.sora.link;cid=b74;generated_at=1970-01-01T00:00:00Z;label=previous"
                    .into()
            )
        );
        assert_eq!(
            rollback["headers_path"],
            serde_json::Value::String(rollback_headers.display().to_string())
        );
        let template =
            fs::read_to_string(rollback_headers).expect("rollback headers template contents");
        assert!(template.contains("label=previous"));
    }

    #[test]
    fn denylist_pack_produces_merkle_bundle() {
        let tmp = tempdir().expect("tempdir");
        let input_path = tmp.path().join("denylist.json");
        let sample = serde_json::json!([
            {
                "kind": "provider",
                "provider_id_hex": "00ff",
                "reason": "test",
                "issued_at": "2025-01-01T00:00:00Z"
            },
            {
                "kind": "cid",
                "cid_b64": "Zm9vYmFy",
                "jurisdiction": "EU",
                "reason": "piracy",
                "issued_at": "2025-01-02T00:00:00Z"
            }
        ]);
        fs::write(&input_path, serde_json::to_vec_pretty(&sample).unwrap()).unwrap();
        let output_dir = tmp.path().join("out");

        let outcome = gateway_denylist_pack(GatewayDenylistPackOptions {
            input_path: input_path.clone(),
            output_dir: output_dir.clone(),
            label: Some("Example Bundle".into()),
            force: true,
        })
        .expect("pack denylist");

        assert_eq!(outcome.entry_count, 2);
        assert!(outcome.bundle_path.exists());
        assert!(outcome.norito_path.exists());
        assert!(outcome.root_path.exists());

        let bundle_bytes = fs::read(&outcome.bundle_path).unwrap();
        let bundle: DenylistBundleV1 = serde_json::from_slice(&bundle_bytes).unwrap();
        assert_eq!(bundle.root_hex, outcome.root_hex);
        assert_eq!(bundle.entries.len(), outcome.entry_count);

        let root_txt = fs::read_to_string(&outcome.root_path).unwrap();
        assert!(
            root_txt.contains(&outcome.root_hex),
            "root text should mention computed root"
        );
    }

    #[test]
    fn denylist_diff_detects_changes() {
        let tmp = tempdir().expect("tempdir");
        let v1_path = tmp.path().join("denylist_v1.json");
        fs::write(
            &v1_path,
            r#"[{"kind":"hash","hex":"00"},{"kind":"url","pattern":"https://one"}]"#,
        )
        .expect("write v1 denylist");
        let v2_path = tmp.path().join("denylist_v2.json");
        fs::write(
            &v2_path,
            r#"[{"kind":"url","pattern":"https://one"},{"kind":"hash","hex":"ff"}]"#,
        )
        .expect("write v2 denylist");

        let v1_out_dir = tmp.path().join("bundle_v1");
        let v2_out_dir = tmp.path().join("bundle_v2");
        let v1 = gateway_denylist_pack(GatewayDenylistPackOptions {
            input_path: v1_path,
            output_dir: v1_out_dir,
            label: Some("v1".into()),
            force: true,
        })
        .expect("pack v1 denylist");
        let v2 = gateway_denylist_pack(GatewayDenylistPackOptions {
            input_path: v2_path,
            output_dir: v2_out_dir,
            label: Some("v2".into()),
            force: true,
        })
        .expect("pack v2 denylist");

        let report = gateway_denylist_diff(&GatewayDenylistDiffOptions {
            old_bundle: v1.bundle_path,
            new_bundle: v2.bundle_path,
            report_path: None,
        })
        .expect("diff denylists");

        assert_eq!(report.added.len(), 1);
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.unchanged_entries, 1);
        assert_eq!(report.old_label.as_deref(), Some("v1"));
        assert_eq!(report.new_label.as_deref(), Some("v2"));
    }

    #[test]
    fn denylist_verify_confirms_valid_bundle() {
        let tmp = tempdir().expect("tempdir");
        let input_path = tmp.path().join("denylist.json");
        let sample = serde_json::json!([
            {"kind": "alias", "alias": "alpha.sora", "issued_at": "2025-01-01T00:00:00Z"},
            {"kind": "cid", "cid": "bafy-test", "issued_at": "2025-01-02T00:00:00Z"}
        ]);
        fs::write(&input_path, serde_json::to_vec_pretty(&sample).unwrap()).unwrap();
        let bundle_dir = tmp.path().join("bundle");
        let pack = gateway_denylist_pack(GatewayDenylistPackOptions {
            input_path: input_path.clone(),
            output_dir: bundle_dir,
            label: Some("Demo".into()),
            force: true,
        })
        .expect("pack denylist");
        let report = gateway_denylist_verify(&GatewayDenylistVerifyOptions {
            bundle: pack.bundle_path.clone(),
            norito_path: Some(pack.norito_path.clone()),
            root_path: Some(pack.root_path.clone()),
            report_path: None,
        })
        .expect("verify denylist");
        assert_eq!(report.entry_count, 2);
        assert_eq!(report.label.as_deref(), Some("Demo"));
        assert!(report.norito_verified);
        assert!(report.root_file_verified);
    }

    #[test]
    fn denylist_verify_rejects_modified_bundle() {
        let tmp = tempdir().expect("tempdir");
        let input_path = tmp.path().join("denylist.json");
        let sample = serde_json::json!([
            {"kind": "alias", "alias": "alpha.sora", "issued_at": "2025-01-01T00:00:00Z"}
        ]);
        fs::write(&input_path, serde_json::to_vec_pretty(&sample).unwrap()).unwrap();
        let bundle_dir = tmp.path().join("bundle");
        let pack = gateway_denylist_pack(GatewayDenylistPackOptions {
            input_path: input_path.clone(),
            output_dir: bundle_dir,
            label: None,
            force: true,
        })
        .expect("pack denylist");

        // Corrupt canonical_json to trigger a verification failure.
        let mut bundle_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&pack.bundle_path).unwrap()).unwrap();
        bundle_json["entries"][0]["canonical_json"] =
            serde_json::Value::String("{\"kind\":\"alias\",\"alias\":\"tampered\"}".into());
        fs::write(
            &pack.bundle_path,
            serde_json::to_vec_pretty(&bundle_json).unwrap(),
        )
        .unwrap();

        let err = gateway_denylist_verify(&GatewayDenylistVerifyOptions {
            bundle: pack.bundle_path.clone(),
            norito_path: None,
            root_path: None,
            report_path: None,
        })
        .expect_err("verification must fail");
        assert!(
            err.to_string().contains("canonical JSON is not normalized")
                || err.to_string().contains("digest mismatch"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn slugify_label_trims_invalid_chars() {
        assert_eq!(slugify_label("  My Bundle  "), "my-bundle");
        assert_eq!(slugify_label(""), "");
        assert_eq!(slugify_label("SORA_FS"), "sora-fs");
    }
}

fn write_file_with_mode(
    path: &Path,
    contents: &[u8],
    overwrite: bool,
    mode: u32,
) -> Result<(), Box<dyn Error>> {
    if path.exists() && !overwrite {
        return Err(format!(
            "{} already exists; pass --force to overwrite",
            path.display()
        )
        .into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
    }
    Ok(())
}

pub fn default_gateway_fixture_dir() -> PathBuf {
    self::gateway_fixture::default_output_dir()
}

fn default_denylist_pack_dir() -> PathBuf {
    workspace_root().join("artifacts/sorafs_gateway/denylist")
}

pub fn write_gateway_fixtures(output: &Path) -> Result<(), Box<dyn Error>> {
    let metadata = self::gateway_fixture::write_bundle(output)?;
    println!(
        "wrote SoraFS gateway fixtures {} to {} (fixtures_digest={})",
        metadata.version,
        output.display(),
        metadata.fixtures_digest_blake3_hex
    );
    println!(
        "  manifest_blake3={}\n  payload_blake3={}\n  car_blake3={}",
        metadata.manifest_blake3_hex, metadata.payload_blake3_hex, metadata.car_blake3_hex
    );
    Ok(())
}

pub fn verify_gateway_fixtures(target: &Path) -> Result<(), Box<dyn Error>> {
    let metadata = self::gateway_fixture::verify_bundle(target)?;
    println!(
        "verified SoraFS gateway fixtures {} at {} (fixtures_digest={})",
        metadata.version,
        target.display(),
        metadata.fixtures_digest_blake3_hex
    );
    println!(
        "  manifest_blake3={}\n  payload_blake3={}\n  car_blake3={}",
        metadata.manifest_blake3_hex, metadata.payload_blake3_hex, metadata.car_blake3_hex
    );
    Ok(())
}

impl FetchSource {
    pub fn parse(raw: &str) -> Result<Self, Box<dyn Error>> {
        if raw.starts_with("http://") || raw.starts_with("https://") {
            let url = Url::parse(raw).map_err(|err| format!("invalid URL {raw}: {err}"))?;
            return Ok(Self::Url(url));
        }
        if raw.starts_with("file://") {
            let url = Url::parse(raw).map_err(|err| format!("invalid file URL {raw}: {err}"))?;
            let path = url
                .to_file_path()
                .map_err(|_| format!("file URL {raw} could not be converted to a path"))?;
            return Ok(Self::File(path));
        }
        let path = Path::new(raw);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            crate::workspace_root().join(path)
        };
        Ok(Self::File(resolved))
    }

    fn resolve_relative(&self, value: &str) -> Result<Self, Box<dyn Error>> {
        if value.starts_with("http://")
            || value.starts_with("https://")
            || value.starts_with("file://")
        {
            return Self::parse(value);
        }
        match self {
            Self::Url(base) => {
                let joined = base.join(value).map_err(|err| {
                    format!("failed to resolve {value} relative to {base}: {err}")
                })?;
                Ok(Self::Url(joined))
            }
            Self::File(path) => {
                let parent = path.parent().ok_or_else(|| {
                    "signatures path has no parent directory; cannot resolve manifest location"
                        .to_owned()
                })?;
                let candidate = Path::new(value);
                let resolved = if candidate.is_absolute() {
                    candidate.to_path_buf()
                } else {
                    parent.join(candidate)
                };
                Ok(Self::File(resolved))
            }
        }
    }

    fn fetch_bytes(&self, client: &Client) -> Result<Vec<u8>, Box<dyn Error>> {
        match self {
            Self::Url(url) => {
                let response = client
                    .get(url.clone())
                    .send()
                    .map_err(|err| format!("failed to GET {url}: {err}"))?;
                if !response.status().is_success() {
                    return Err(format!("request to {url} returned {}", response.status()).into());
                }
                let bytes = response
                    .bytes()
                    .map_err(|err| format!("failed to read response body from {url}: {err}"))?;
                Ok(bytes.to_vec())
            }
            Self::File(path) => Ok(fs::read(path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?),
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Url(url) => url.as_str().to_owned(),
            Self::File(path) => path.display().to_string(),
        }
    }
}

pub fn fetch_fixture(options: FetchFixtureOptions) -> Result<(), Box<dyn Error>> {
    let FetchFixtureOptions {
        signatures_source,
        manifest_source,
        output_dir,
        profile_handle,
        allow_unsigned,
    } = options;

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;

    let profile_handle = if profile_handle.is_empty() {
        DEFAULT_PROFILE_HANDLE.to_owned()
    } else {
        profile_handle
    };

    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let expected_chunk_digest = vectors.sha3_digest_hex();

    let signatures_bytes = signatures_source.fetch_bytes(&client).map_err(|err| {
        format!(
            "failed to fetch manifest signatures from {}: {err}",
            signatures_source.describe()
        )
    })?;
    let signatures_value: Value = json::from_slice(&signatures_bytes)
        .map_err(|err| format!("failed to parse manifest signatures JSON: {err}"))?;

    ensure_profile_matches(&signatures_value, &profile_handle)?;
    ensure_aliases(&signatures_value, &profile_handle)?;

    let chunk_digest_from_signatures =
        extract_string(&signatures_value, "chunk_digest_sha3_256")?.to_ascii_lowercase();
    if chunk_digest_from_signatures != expected_chunk_digest {
        return Err(format!(
            "chunk digest mismatch: manifest_signatures.json reports {chunk_digest_from_signatures} \
             but the canonical fixture expects {expected_chunk_digest}"
        )
        .into());
    }

    let manifest_name = extract_string(&signatures_value, "manifest")?;
    let manifest_digest_expected =
        extract_string(&signatures_value, "manifest_blake3")?.to_ascii_lowercase();

    let manifest_source = match manifest_source {
        Some(source) => source,
        None => signatures_source
            .resolve_relative(manifest_name)
            .map_err(|err| format!("failed to resolve manifest path {manifest_name}: {err}"))?,
    };

    let manifest_bytes = manifest_source.fetch_bytes(&client).map_err(|err| {
        format!(
            "failed to fetch manifest from {}: {err}",
            manifest_source.describe()
        )
    })?;
    let manifest_digest_actual = blake3_hash(&manifest_bytes);
    let manifest_digest_actual_hex = to_hex(manifest_digest_actual.as_bytes());
    if manifest_digest_actual_hex != manifest_digest_expected {
        return Err(format!(
            "manifest digest mismatch: signatures expect {manifest_digest_expected} \
             but fetched manifest hashes to {manifest_digest_actual_hex}"
        )
        .into());
    }

    let manifest_value: Value = json::from_slice(&manifest_bytes)
        .map_err(|err| format!("failed to parse manifest JSON: {err}"))?;
    ensure_profile_matches(&manifest_value, &profile_handle)?;
    ensure_aliases(&manifest_value, &profile_handle)?;
    let manifest_chunk_digest =
        extract_string(&manifest_value, "chunk_digest_sha3_256")?.to_ascii_lowercase();
    if manifest_chunk_digest != expected_chunk_digest {
        return Err(format!(
            "manifest chunk digest mismatch: manifest reports {manifest_chunk_digest} \
             but canonical fixture expects {expected_chunk_digest}"
        )
        .into());
    }

    let signature_count = verify_manifest_signatures(
        &signatures_value,
        manifest_digest_actual.as_bytes(),
        allow_unsigned,
    )?;

    fs::create_dir_all(&output_dir)?;
    fs::write(
        output_dir.join("manifest_signatures.json"),
        &signatures_bytes,
    )?;
    fs::write(output_dir.join("manifest_blake3.json"), &manifest_bytes)?;

    println!(
        "Fetched SoraFS chunker manifest to {} (signatures: {}, digest: {})",
        output_dir.display(),
        signature_count,
        manifest_digest_actual_hex
    );

    Ok(())
}

pub fn generate_gateway_attestation(options: GatewayAttestOptions) -> Result<(), Box<dyn Error>> {
    let GatewayAttestOptions {
        output_dir,
        signing_key_path,
        signer_account,
        gateway_target,
    } = options;

    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("failed to create {}: {err}", output_dir.display()))?;

    let mut context = HarnessContext::new();
    if let Some(target) = gateway_target {
        context = context.with_gateway_target(target);
    }

    let suite = integration_tests::sorafs_gateway_conformance::run_suite(&context);
    if !suite.all_passed() {
        return Err("gateway conformance suite failed; refusing to issue attestation".into());
    }

    let key_text = fs::read_to_string(&signing_key_path).map_err(|err| {
        format!(
            "failed to read signing key {}: {err}",
            signing_key_path.display()
        )
    })?;
    let private_key_hex = key_text.trim();
    if private_key_hex.is_empty() {
        return Err("signing key file is empty".into());
    }
    let private_key =
        iroha_crypto::PrivateKey::from_hex(iroha_crypto::Algorithm::Ed25519, private_key_hex)
            .map_err(|err| {
                format!(
                    "failed to parse signing key {}: {err}",
                    signing_key_path.display()
                )
            })?;
    let key_pair =
        KeyPair::from_private_key(private_key).map_err(|err| format!("invalid key pair: {err}"))?;

    let signer_literal = signer_account.trim();
    let signer = AccountAddress::parse_any(signer_literal, None)
        .map(|(address, _)| address)
        .map_err(|err| format!("invalid signer account `{}`: {err}", signer_literal))?;

    let bundle = integration_tests::sorafs_gateway_conformance::generate_attestation(
        &suite,
        &key_pair,
        &signer,
        SystemTime::now(),
    )
    .map_err(|err| format!("failed to generate attestation: {err}"))?;

    let report_path = output_dir.join("sorafs_gateway_report.json");
    fs::write(&report_path, bundle.report_json)
        .map_err(|err| format!("failed to write report {}: {err}", report_path.display()))?;

    let envelope_path = output_dir.join("sorafs_gateway_attestation.to");
    fs::write(&envelope_path, bundle.envelope_bytes).map_err(|err| {
        format!(
            "failed to write attestation envelope {}: {err}",
            envelope_path.display()
        )
    })?;

    let summary_path = output_dir.join("sorafs_gateway_attestation.txt");
    fs::write(&summary_path, bundle.summary_text).map_err(|err| {
        format!(
            "failed to write attestation summary {}: {err}",
            summary_path.display()
        )
    })?;

    println!(
        "Wrote SoraFS gateway conformance attestation:\n  report: {}\n  envelope: {}\n  summary: {}",
        report_path.display(),
        envelope_path.display(),
        summary_path.display()
    );

    Ok(())
}

const DEFAULT_CACHE_MAX_AGE: u64 = 600;
const DEFAULT_CACHE_STALE_WHILE_REVALIDATE: u64 = 120;
const DEFAULT_CACHE_HARD_EXPIRY: u64 = 900;
const DEFAULT_CACHE_NEGATIVE_TTL: u64 = 60;
const DEFAULT_CACHE_REVOCATION_TTL: u64 = 300;
const DEFAULT_CACHE_ROTATION_MAX_AGE: u64 = 21_600;
const DEFAULT_SUCCESSOR_GRACE: u64 = 300;
const DEFAULT_GOVERNANCE_GRACE: u64 = 0;
const DRILL_LOG_HEADER: &str = r#"---
title: SoraFS Chaos Drill Log
summary: Registry of executed chaos drills and incident rehearsals.
---

| Date | Scenario | Status | Incident Commander | Scribe | Start (UTC) | End (UTC) | Notes | Follow-up / Incident Link |
|------|----------|--------|--------------------|--------|-------------|-----------|-------|---------------------------|
"#;
struct ProbeRunSummary<'a> {
    started_at: OffsetDateTime,
    ended_at: OffsetDateTime,
    success: bool,
    source_description: String,
    target_url: Option<String>,
    target_host: Option<String>,
    gar_path: PathBuf,
    findings: &'a [ProbeFinding],
}

impl<'a> ProbeRunSummary<'a> {
    fn failure_count(&self) -> usize {
        self.findings.iter().filter(|finding| !finding.ok).count()
    }

    fn target_label(&self) -> String {
        self.target_url
            .clone()
            .or_else(|| self.target_host.clone())
            .unwrap_or_else(|| self.source_description.clone())
    }
}

pub fn run_gateway_probe(options: GatewayProbeOptions) -> Result<(), Box<dyn Error>> {
    let report_target = options.report_target.clone();
    let log_to_stderr = matches!(report_target.as_ref(), Some(JsonTarget::Stdout));
    let started_at = OffsetDateTime::now_utc();
    let now_secs = options.now_override.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_secs()
    });
    let gar_record = load_and_verify_gar(&options, now_secs)?;
    let response = if let Some(request) = &options.request {
        probe_headers_via_http(request)?
    } else {
        let path = options
            .headers_path
            .as_ref()
            .expect("headers_path required when no request is present");
        probe_headers_from_file(path)?
    };

    let mut findings = Vec::new();

    let status_ok = (200..=299).contains(&response.status);
    record_finding(
        &mut findings,
        status_ok,
        "HTTP status",
        format!("{} {}", response.status, response.source.describe()),
    );

    let host = resolve_probe_host(&options, &response)?;
    let host_matches = gar_record.matches_host(&host);
    record_finding(
        &mut findings,
        host_matches,
        "GAR host pattern",
        if host_matches {
            format!("host `{host}` authorised by GAR")
        } else {
            format!("host `{host}` missing from GAR host_patterns")
        },
    );

    let cache_control = expect_header(&response.headers, "Cache-Control", &mut findings);
    let sora_name = expect_header(&response.headers, "Sora-Name", &mut findings);
    let sora_content_cid = expect_header(&response.headers, "Sora-Content-CID", &mut findings);
    let sora_proof_header = expect_header(&response.headers, "Sora-Proof", &mut findings);
    let sora_proof_status = expect_header(&response.headers, "Sora-Proof-Status", &mut findings);

    if options.require_tls_state {
        match read_header(&response.headers, "X-Sora-TLS-State") {
            Ok(Some(value)) => {
                let detail = if value.contains("expiry=") {
                    format!("X-Sora-TLS-State `{value}`")
                } else {
                    format!("header `{value}` missing expiry= annotation")
                };
                record_finding(
                    &mut findings,
                    value.contains("expiry="),
                    "X-Sora-TLS-State",
                    detail,
                );
            }
            Ok(None) => {
                record_finding(
                    &mut findings,
                    false,
                    "X-Sora-TLS-State",
                    "header missing from response",
                );
            }
            Err(err) => {
                record_finding(&mut findings, false, "X-Sora-TLS-State", err);
            }
        }
    }

    if let Some(template) = gar_record.csp_template() {
        match read_header(&response.headers, "Content-Security-Policy") {
            Ok(Some(value)) => {
                let matches = value.trim() == template.trim();
                record_finding(
                    &mut findings,
                    matches,
                    "Content-Security-Policy",
                    if matches {
                        "CSP matches GAR template".to_string()
                    } else {
                        format!("expected `{template}`, observed `{value}`")
                    },
                );
            }
            Ok(None) => {
                record_finding(
                    &mut findings,
                    false,
                    "Content-Security-Policy",
                    "header missing but GAR requires it",
                );
            }
            Err(err) => record_finding(&mut findings, false, "Content-Security-Policy", err),
        }
    }

    if let Some(template) = gar_record.hsts_template() {
        match read_header(&response.headers, "Strict-Transport-Security") {
            Ok(Some(value)) => {
                let matches = value.trim() == template.trim();
                record_finding(
                    &mut findings,
                    matches,
                    "Strict-Transport-Security",
                    if matches {
                        "HSTS matches GAR template".to_string()
                    } else {
                        format!("expected `{template}`, observed `{value}`")
                    },
                );
            }
            Ok(None) => {
                record_finding(
                    &mut findings,
                    false,
                    "Strict-Transport-Security",
                    "header missing but GAR requires it",
                );
            }
            Err(err) => record_finding(&mut findings, false, "Strict-Transport-Security", err),
        }
    }

    if let Some(header) = cache_control {
        let directives = parse_cache_directives(&header);
        let expected_max_age = options.cache_max_age.unwrap_or(DEFAULT_CACHE_MAX_AGE);
        let expected_swr = options
            .cache_swr
            .unwrap_or(DEFAULT_CACHE_STALE_WHILE_REVALIDATE);

        let parsed_max_age = directives
            .get("max-age")
            .and_then(|value| value.parse::<u64>().ok());
        let parsed_swr = directives
            .get("stale-while-revalidate")
            .and_then(|value| value.parse::<u64>().ok());

        let ttl_ok = parsed_max_age == Some(expected_max_age) && parsed_swr == Some(expected_swr);

        let detail = format!(
            "Cache-Control max-age={:?} (expected {expected_max_age}), stale-while-revalidate={:?} (expected {expected_swr})",
            parsed_max_age, parsed_swr
        );

        record_finding(&mut findings, ttl_ok, "Cache-Control policy", detail);
    }

    let mut proof_bundle: Option<AliasProofBundleV1> = None;
    if let Some(proof_b64) = sora_proof_header {
        match BASE64_STD.decode(proof_b64.as_bytes()) {
            Ok(bytes) => match decode_alias_proof(&bytes) {
                Ok(bundle) => {
                    proof_bundle = Some(bundle);
                }
                Err(err) => record_finding(
                    &mut findings,
                    false,
                    "Sora-Proof decode",
                    format!("invalid alias proof bundle: {err}"),
                ),
            },
            Err(err) => {
                record_finding(
                    &mut findings,
                    false,
                    "Sora-Proof decode",
                    format!("base64 error: {err}"),
                );
            }
        }
    }

    if let (Some(alias), Some(bundle)) = (sora_name.as_ref(), proof_bundle.as_ref()) {
        let matches = alias == &bundle.binding.alias;
        record_finding(
            &mut findings,
            matches,
            "Sora-Name vs alias proof",
            if matches {
                format!("alias `{alias}` matches proof bundle")
            } else {
                format!(
                    "Sora-Name `{alias}` disagrees with proof bundle alias `{}`",
                    bundle.binding.alias
                )
            },
        );
    }

    if let Some(bundle) = proof_bundle.as_ref() {
        match alias_manifest_id(bundle) {
            Ok(proof_cid) => {
                if let Some(content_cid) = sora_content_cid.as_ref() {
                    let matches = proof_cid == *content_cid;
                    record_finding(
                        &mut findings,
                        matches,
                        "Alias proof manifest",
                        if matches {
                            format!("alias proof manifest `{proof_cid}` matches Sora-Content-CID")
                        } else {
                            format!(
                                "alias proof manifest `{proof_cid}` differs from Sora-Content-CID `{content_cid}`"
                            )
                        },
                    );
                }
            }
            Err(err) => record_finding(&mut findings, false, "Alias proof manifest", err),
        }
    }

    if let Some(content_cid) = sora_content_cid.as_ref() {
        let gar_cid = gar_record.manifest_cid().trim();
        let matches = content_cid.trim() == gar_cid;
        record_finding(
            &mut findings,
            matches,
            "GAR manifest CID",
            if matches {
                format!("Sora-Content-CID `{gar_cid}` matches GAR manifest")
            } else {
                format!("Sora-Content-CID `{content_cid}` differs from GAR manifest `{gar_cid}`")
            },
        );
    }

    if let (Some(bundle), Some(status_value)) = (proof_bundle.as_ref(), sora_proof_status.as_ref())
    {
        let policy = default_alias_policy();
        let evaluation = policy.evaluate(bundle, now_secs);
        let matches = proof_status_matches(&evaluation, status_value);
        let detail = format!(
            "status `{status_value}` vs evaluation `{}` (age {}s{})",
            evaluation.status_label(),
            evaluation.age.as_secs(),
            if evaluation.rotation_due {
                ", rotation due"
            } else {
                ""
            }
        );
        record_finding(&mut findings, matches, "Sora-Proof-Status", detail);
    }

    if log_to_stderr {
        eprintln!(
            "SoraFS gateway probe summary ({})",
            response.source.describe()
        );
    } else {
        println!(
            "SoraFS gateway probe summary ({})",
            response.source.describe()
        );
    }
    let mut has_failures = false;
    for finding in &findings {
        if finding.ok {
            if log_to_stderr {
                eprintln!("[ok] {} — {}", finding.name, finding.detail);
            } else {
                println!("[ok] {} — {}", finding.name, finding.detail);
            }
        } else {
            has_failures = true;
            if log_to_stderr {
                eprintln!("[fail] {} — {}", finding.name, finding.detail);
            } else {
                println!("[fail] {} — {}", finding.name, finding.detail);
            }
        }
    }

    if let Some(target) = report_target {
        let gar_info = GatewayProbeGarInfo::from_record(&options.gar_path, &gar_record);
        let report = build_probe_report_value(now_secs, &response, &host, &gar_info, &findings);
        write_json_output(&report, target)?;
    }

    let ended_at = OffsetDateTime::now_utc();
    let summary = ProbeRunSummary {
        started_at,
        ended_at,
        success: !has_failures,
        source_description: response.source.describe(),
        target_url: response.source.url().map(str::to_string),
        target_host: response.source.host().map(str::to_string),
        gar_path: options.gar_path.clone(),
        findings: &findings,
    };

    if let Some(path) = &options.summary_path {
        write_probe_summary(path, &summary)?;
    }
    if let Some(drill) = &options.drill {
        append_drill_log_entry(drill, &summary)?;
    }

    if has_failures {
        if let Some(config) = &options.pagerduty {
            emit_pagerduty_event(config, &summary)?;
        }
        return Err("one or more gateway probe checks failed".into());
    }

    if log_to_stderr {
        eprintln!("All SoraFS gateway probe checks passed.");
    } else {
        println!("All SoraFS gateway probe checks passed.");
    }
    Ok(())
}

fn load_and_verify_gar(
    options: &GatewayProbeOptions,
    now_secs: u64,
) -> Result<GatewayAuthorizationRecord, Box<dyn Error>> {
    let verifier = build_gar_verifier(&options.gar_keys)?;
    let raw = fs::read_to_string(&options.gar_path)
        .map_err(|err| format!("failed to read GAR {}: {err}", options.gar_path.display()))?;
    let jws = raw.trim();
    if jws.is_empty() {
        return Err(format!("GAR file {} was empty", options.gar_path.display()).into());
    }
    verifier
        .verify_at(jws, now_secs)
        .map_err(|err| format!("failed to verify GAR {}: {err}", options.gar_path.display()).into())
}

fn build_gar_verifier(
    entries: &[(String, Vec<u8>)],
) -> Result<GatewayAuthorizationVerifier, Box<dyn Error>> {
    let mut verifier = GatewayAuthorizationVerifier::default();
    for (kid, bytes) in entries {
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, bytes)
            .map_err(|err| format!("invalid GAR public key `{kid}`: {err}"))?;
        verifier.insert(kid.clone(), public_key);
    }
    Ok(verifier)
}

fn probe_headers_via_http(request: &GatewayProbeRequest) -> Result<ProbeResponse, Box<dyn Error>> {
    let method = request.method.trim().to_ascii_uppercase();
    let url = Url::parse(&request.url)
        .map_err(|err| format!("invalid gateway URL `{}`: {err}", request.url))?;
    let host = url.host_str().map(|h| h.to_ascii_lowercase());

    let mut builder = Client::builder();
    if let Some(timeout) = request.timeout_secs {
        builder = builder.timeout(Duration::from_secs(timeout));
    }
    let client = builder.build()?;

    let mut req = match method.as_str() {
        "GET" => client.get(url.clone()),
        "HEAD" => client.head(url.clone()),
        other => {
            return Err(format!("unsupported HTTP method `{other}`").into());
        }
    };

    for (name, value) in &request.extra_headers {
        let header_name = HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|err| format!("invalid header name `{name}`: {err}"))?;
        let header_value = HeaderValue::from_str(value.trim())
            .map_err(|err| format!("invalid value for header `{name}`: {err}"))?;
        req = req.header(header_name, header_value);
    }

    let response = req.send()?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    Ok(ProbeResponse {
        status,
        headers,
        source: ProbeSource::Http {
            url: request.url.clone(),
            host,
        },
    })
}

fn probe_headers_from_file(path: &Path) -> Result<ProbeResponse, Box<dyn Error>> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read header file {}: {err}", path.display()))?;
    let mut lines = raw.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| format!("header file {} is empty", path.display()))?;
    let status = parse_status_line(status_line)?;
    let mut headers = HeaderMap::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("HTTP/") {
            // Ignore intermediate responses in case the dump captured redirects.
            continue;
        }
        let Some((name, value)) = trimmed.split_once(':') else {
            return Err(format!("invalid header line `{trimmed}` in {}", path.display()).into());
        };
        let header_name = HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|err| format!("invalid header name `{name}` in {}: {err}", path.display()))?;
        let header_value = HeaderValue::from_str(value.trim()).map_err(|err| {
            format!(
                "invalid header value for `{name}` in {}: {err}",
                path.display()
            )
        })?;
        headers.append(header_name, header_value);
    }
    Ok(ProbeResponse {
        status,
        headers,
        source: ProbeSource::File {
            path: path.to_path_buf(),
        },
    })
}

fn parse_status_line(line: &str) -> Result<u16, Box<dyn Error>> {
    let mut parts = line.split_whitespace();
    let _http = parts
        .next()
        .ok_or("invalid status line (missing HTTP version)")?;
    let status = parts
        .next()
        .ok_or("invalid status line (missing status code)")?;
    status
        .parse::<u16>()
        .map_err(|err| format!("invalid status code `{status}`: {err}").into())
}

fn resolve_probe_host(
    options: &GatewayProbeOptions,
    response: &ProbeResponse,
) -> Result<String, Box<dyn Error>> {
    if let Some(host) = &options.host_override {
        return Ok(host.trim().to_ascii_lowercase());
    }
    if let Some(host) = response.source.host() {
        return Ok(host.to_ascii_lowercase());
    }
    Err("host is unknown; provide --host when parsing captured headers".into())
}

fn expect_header(
    headers: &HeaderMap,
    name: &str,
    findings: &mut Vec<ProbeFinding>,
) -> Option<String> {
    match read_header(headers, name) {
        Ok(Some(value)) => Some(value),
        Ok(None) => {
            record_finding(findings, false, format!("{name} header"), "header missing");
            None
        }
        Err(err) => {
            record_finding(findings, false, format!("{name} header"), err);
            None
        }
    }
}

fn read_header(headers: &HeaderMap, name: &str) -> Result<Option<String>, String> {
    match headers.get(name) {
        Some(value) => value
            .to_str()
            .map(|text| Some(text.trim().to_string()))
            .map_err(|err| format!("header `{name}` is not valid UTF-8: {err}")),
        None => Ok(None),
    }
}

fn parse_cache_directives(value: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for directive in value.split(',') {
        let trimmed = directive.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=') {
            map.insert(
                key.trim().to_ascii_lowercase(),
                val.trim().trim_matches('"').to_string(),
            );
        } else {
            map.insert(trimmed.to_ascii_lowercase(), String::new());
        }
    }
    map
}

fn findings_to_json(findings: &[ProbeFinding]) -> Vec<Value> {
    findings
        .iter()
        .map(|finding| {
            let mut map = Map::new();
            map.insert("ok".to_string(), Value::Bool(finding.ok));
            map.insert("name".to_string(), Value::String(finding.name.clone()));
            map.insert("detail".to_string(), Value::String(finding.detail.clone()));
            Value::Object(map)
        })
        .collect()
}

fn write_probe_summary(path: &Path, summary: &ProbeRunSummary) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create summary directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let started = summary
        .started_at
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format summary start timestamp: {err}"))?;
    let ended = summary
        .ended_at
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format summary end timestamp: {err}"))?;
    let duration = (summary.ended_at - summary.started_at).whole_seconds();
    let mut root = Map::new();
    root.insert("started_at".into(), Value::String(started));
    root.insert("ended_at".into(), Value::String(ended));
    root.insert(
        "duration_seconds".into(),
        Value::Number(Number::from(duration)),
    );
    root.insert("success".into(), Value::Bool(summary.success));
    root.insert(
        "source".into(),
        Value::String(summary.source_description.clone()),
    );
    match &summary.target_url {
        Some(url) => {
            root.insert("target_url".into(), Value::String(url.clone()));
        }
        None => {
            root.insert("target_url".into(), Value::Null);
        }
    }
    match &summary.target_host {
        Some(host) => {
            root.insert("target_host".into(), Value::String(host.clone()));
        }
        None => {
            root.insert("target_host".into(), Value::Null);
        }
    }
    root.insert(
        "gar_path".into(),
        Value::String(summary.gar_path.display().to_string()),
    );
    root.insert(
        "failure_count".into(),
        Value::Number(Number::from(summary.failure_count() as u64)),
    );
    root.insert(
        "findings".into(),
        Value::Array(findings_to_json(summary.findings)),
    );
    let encoded = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to encode summary JSON: {err}"))?;
    fs::write(path, encoded)
        .map_err(|err| format!("failed to write summary JSON {}: {err}", path.display()))?;
    Ok(())
}

fn append_drill_log_entry(
    config: &DrillLogConfig,
    summary: &ProbeRunSummary,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = config.log_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create drill log directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let needs_header = !config.log_path.exists()
        || fs::metadata(&config.log_path)
            .map(|meta| meta.len() == 0)
            .unwrap_or(true);
    if needs_header {
        fs::write(&config.log_path, DRILL_LOG_HEADER).map_err(|err| {
            format!(
                "failed to initialise drill log {}: {err}",
                config.log_path.display()
            )
        })?;
    }
    let date = summary.started_at.date().to_string();
    let start = format!(
        "{:02}:{:02}Z",
        summary.started_at.hour(),
        summary.started_at.minute()
    );
    let end = format!(
        "{:02}:{:02}Z",
        summary.ended_at.hour(),
        summary.ended_at.minute()
    );
    let status = if summary.success { "pass" } else { "fail" };
    let scenario = sanitise_table_field(&config.scenario);
    let ic = config
        .ic
        .as_deref()
        .map(sanitise_table_field)
        .unwrap_or_else(|| "-".to_string());
    let scribe = config
        .scribe
        .as_deref()
        .map(sanitise_table_field)
        .unwrap_or_else(|| "-".to_string());
    let notes = config
        .notes
        .as_deref()
        .map(sanitise_table_field)
        .unwrap_or_else(|| "-".to_string());
    let link = config
        .link
        .as_deref()
        .map(sanitise_table_field)
        .unwrap_or_else(|| "-".to_string());
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&config.log_path)
        .map_err(|err| {
            format!(
                "failed to open drill log {}: {err}",
                config.log_path.display()
            )
        })?;
    writeln!(
        file,
        "| {date} | {scenario} | {status} | {ic} | {scribe} | {start} | {end} | {notes} | {link} |"
    )
    .map_err(|err| format!("failed to append drill log row: {err}"))?;
    Ok(())
}

fn sanitise_table_field(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        "-".to_string()
    } else {
        trimmed
            .replace('|', "&#124;")
            .replace('\n', "<br>")
            .replace('\r', "")
    }
}

fn emit_pagerduty_event(
    config: &PagerDutyConfig,
    summary: &ProbeRunSummary,
) -> Result<(), Box<dyn Error>> {
    let payload_value = pagerduty_payload_value(config, summary)?;
    let payload_text = to_string_pretty(&payload_value)
        .map_err(|err| format!("failed to encode PagerDuty payload: {err}"))?;
    if let Some(parent) = config.payload_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create PagerDuty payload directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&config.payload_path, payload_text.as_bytes()).map_err(|err| {
        format!(
            "failed to write PagerDuty payload {}: {err}",
            config.payload_path.display()
        )
    })?;
    if let Some(endpoint) = &config.endpoint_url {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|err| format!("failed to build PagerDuty client: {err}"))?;
        let response = client
            .post(endpoint.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload_text)
            .send()
            .map_err(|err| format!("failed to send PagerDuty request: {err}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "PagerDuty endpoint {} returned {}",
                endpoint,
                response.status()
            )
            .into());
        }
    }
    Ok(())
}

fn pagerduty_payload_value(
    config: &PagerDutyConfig,
    summary: &ProbeRunSummary,
) -> Result<Value, Box<dyn Error>> {
    let timestamp = summary
        .ended_at
        .format(&Rfc3339)
        .map_err(|err| format!("failed to format PagerDuty timestamp: {err}"))?;
    let mut custom_details = Map::new();
    if let Some(url) = &summary.target_url {
        custom_details.insert("target_url".into(), Value::String(url.clone()));
    }
    if let Some(host) = &summary.target_host {
        custom_details.insert("target_host".into(), Value::String(host.clone()));
    }
    custom_details.insert(
        "gar_path".into(),
        Value::String(summary.gar_path.display().to_string()),
    );
    let duration = (summary.ended_at - summary.started_at).whole_seconds();
    custom_details.insert(
        "duration_seconds".into(),
        Value::Number(Number::from(duration)),
    );
    custom_details.insert(
        "failure_count".into(),
        Value::Number(Number::from(summary.failure_count() as u64)),
    );
    custom_details.insert(
        "findings".into(),
        Value::Array(findings_to_json(summary.findings)),
    );

    let mut payload = Map::new();
    payload.insert(
        "summary".into(),
        Value::String(format!(
            "{} probe checks failed ({})",
            summary.failure_count(),
            summary.target_label()
        )),
    );
    payload.insert("severity".into(), Value::String(config.severity.clone()));
    payload.insert("source".into(), Value::String(config.source.clone()));
    if let Some(component) = &config.component {
        payload.insert("component".into(), Value::String(component.clone()));
    }
    if let Some(group) = &config.group {
        payload.insert("group".into(), Value::String(group.clone()));
    }
    if let Some(class_name) = &config.class_name {
        payload.insert("class".into(), Value::String(class_name.clone()));
    }
    payload.insert("timestamp".into(), Value::String(timestamp));
    payload.insert("custom_details".into(), Value::Object(custom_details));

    let mut event = Map::new();
    event.insert(
        "routing_key".into(),
        Value::String(config.routing_key.clone()),
    );
    event.insert("event_action".into(), Value::String("trigger".into()));
    if let Some(dedup) = &config.dedup_key {
        event.insert("dedup_key".into(), Value::String(dedup.clone()));
    }
    if !config.links.is_empty() {
        let mut link_values = Vec::with_capacity(config.links.len());
        for link in &config.links {
            let mut entry = Map::new();
            entry.insert("href".into(), Value::String(link.href.clone()));
            entry.insert("text".into(), Value::String(link.text.clone()));
            link_values.push(Value::Object(entry));
        }
        event.insert("links".into(), Value::Array(link_values));
    }
    event.insert("payload".into(), Value::Object(payload));
    Ok(Value::Object(event))
}

fn default_alias_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
        Duration::from_secs(DEFAULT_CACHE_MAX_AGE),
        Duration::from_secs(DEFAULT_CACHE_STALE_WHILE_REVALIDATE),
        Duration::from_secs(DEFAULT_CACHE_HARD_EXPIRY),
        Duration::from_secs(DEFAULT_CACHE_NEGATIVE_TTL),
        Duration::from_secs(DEFAULT_CACHE_REVOCATION_TTL),
        Duration::from_secs(DEFAULT_CACHE_ROTATION_MAX_AGE),
        Duration::from_secs(DEFAULT_SUCCESSOR_GRACE),
        Duration::from_secs(DEFAULT_GOVERNANCE_GRACE),
    )
}

fn proof_status_matches(evaluation: &AliasProofEvaluation, header: &str) -> bool {
    let normalized = header.trim().to_ascii_lowercase();
    match evaluation.state {
        AliasProofState::Fresh => {
            let acceptable: &[&str] = if evaluation.rotation_due {
                &["fresh-rotate", "fresh"][..]
            } else {
                &["fresh"][..]
            };
            acceptable.contains(&normalized.as_str())
        }
        AliasProofState::RefreshWindow => {
            let mut acceptable = vec!["refresh", "refresh-successor", "refresh-governance"];
            if evaluation.rotation_due {
                acceptable.push("refresh-rotate");
            }
            acceptable.contains(&normalized.as_str())
        }
        AliasProofState::Expired => normalized == "expired",
        AliasProofState::HardExpired => normalized == "hard-expired",
    }
}

fn alias_manifest_id(bundle: &AliasProofBundleV1) -> Result<String, String> {
    String::from_utf8(bundle.binding.manifest_cid.clone())
        .map(|text| text.trim().to_string())
        .map_err(|_| "manifest CID in alias proof is not valid UTF-8".to_string())
}

fn record_finding(
    findings: &mut Vec<ProbeFinding>,
    ok: bool,
    name: impl Into<String>,
    detail: impl Into<String>,
) {
    findings.push(ProbeFinding {
        ok,
        name: name.into(),
        detail: detail.into(),
    });
}

struct ProbeFinding {
    ok: bool,
    name: String,
    detail: String,
}

struct ProbeResponse {
    status: u16,
    headers: HeaderMap,
    source: ProbeSource,
}

enum ProbeSource {
    Http { url: String, host: Option<String> },
    File { path: PathBuf },
}

impl ProbeSource {
    fn describe(&self) -> String {
        match self {
            Self::Http { url, .. } => format!("HTTP probe via {url}"),
            Self::File { path } => format!("captured headers from {}", path.display()),
        }
    }

    fn host(&self) -> Option<&str> {
        match self {
            Self::Http { host, .. } => host.as_deref(),
            Self::File { .. } => None,
        }
    }

    fn url(&self) -> Option<&str> {
        match self {
            Self::Http { url, .. } => Some(url.as_str()),
            Self::File { .. } => None,
        }
    }
}

struct GatewayProbeGarInfo {
    path: String,
    name: String,
    record_version: u16,
    manifest_cid: String,
    valid_from_epoch: u64,
    valid_until_epoch: Option<u64>,
    host_patterns: Vec<String>,
}

impl GatewayProbeGarInfo {
    fn from_record(path: &Path, record: &GatewayAuthorizationRecord) -> Self {
        Self {
            path: path.display().to_string(),
            name: record.name().to_string(),
            record_version: record.record_version(),
            manifest_cid: record.manifest_cid().trim().to_string(),
            valid_from_epoch: record.valid_from_epoch(),
            valid_until_epoch: record.valid_until_epoch(),
            host_patterns: record
                .host_patterns()
                .iter()
                .map(|pattern| pattern.pattern().to_string())
                .collect(),
        }
    }
}

fn build_probe_report_value(
    timestamp: u64,
    response: &ProbeResponse,
    host: &str,
    gar_info: &GatewayProbeGarInfo,
    findings: &[ProbeFinding],
) -> Value {
    let failure_count = findings.iter().filter(|finding| !finding.ok).count() as u64;
    let mut root = Map::new();
    root.insert("ok".into(), Value::Bool(failure_count == 0));
    root.insert(
        "failure_count".into(),
        Value::Number(Number::from(failure_count)),
    );
    root.insert("timestamp".into(), Value::Number(Number::from(timestamp)));
    root.insert(
        "status".into(),
        Value::Number(Number::from(u64::from(response.status))),
    );
    root.insert("host".into(), Value::String(host.to_string()));
    root.insert("source".into(), probe_source_json(&response.source));
    root.insert("gar".into(), gar_info_to_json(gar_info));
    root.insert("findings".into(), Value::Array(findings_to_json(findings)));
    if failure_count > 0 {
        root.insert(
            "failures".into(),
            Value::Array(
                findings
                    .iter()
                    .filter(|finding| !finding.ok)
                    .map(|finding| Value::String(finding.name.clone()))
                    .collect(),
            ),
        );
    }
    Value::Object(root)
}

fn gar_info_to_json(gar_info: &GatewayProbeGarInfo) -> Value {
    let mut map = Map::new();
    map.insert("path".into(), Value::String(gar_info.path.clone()));
    map.insert("name".into(), Value::String(gar_info.name.clone()));
    map.insert(
        "record_version".into(),
        Value::Number(Number::from(u64::from(gar_info.record_version))),
    );
    map.insert(
        "manifest_cid".into(),
        Value::String(gar_info.manifest_cid.clone()),
    );
    map.insert(
        "valid_from".into(),
        Value::Number(Number::from(gar_info.valid_from_epoch)),
    );
    match gar_info.valid_until_epoch {
        Some(value) => {
            map.insert("valid_until".into(), Value::Number(Number::from(value)));
        }
        None => {
            map.insert("valid_until".into(), Value::Null);
        }
    }
    map.insert(
        "host_patterns".into(),
        Value::Array(
            gar_info
                .host_patterns
                .iter()
                .map(|pattern| Value::String(pattern.clone()))
                .collect(),
        ),
    );
    Value::Object(map)
}

fn probe_source_json(source: &ProbeSource) -> Value {
    match source {
        ProbeSource::Http { url, host } => {
            let mut map = Map::new();
            map.insert("type".into(), Value::String("http".to_string()));
            map.insert("url".into(), Value::String(url.clone()));
            if let Some(host) = host {
                map.insert("host".into(), Value::String(host.clone()));
            }
            Value::Object(map)
        }
        ProbeSource::File { path } => {
            let mut map = Map::new();
            map.insert("type".into(), Value::String("headers-file".to_string()));
            map.insert("path".into(), Value::String(path.display().to_string()));
            Value::Object(map)
        }
    }
}

fn ensure_profile_matches(root: &Value, expected: &str) -> Result<(), Box<dyn Error>> {
    let profile = extract_string(root, "profile")?;
    if profile != expected {
        return Err(format!("expected profile {expected} but found {profile}").into());
    }
    Ok(())
}

fn ensure_aliases(root: &Value, canonical: &str) -> Result<(), Box<dyn Error>> {
    let aliases = root
        .get("profile_aliases")
        .and_then(Value::as_array)
        .ok_or_else(|| "profile_aliases array missing".to_owned())?;
    let mut seen = HashSet::with_capacity(aliases.len());
    for entry in aliases {
        if let Some(text) = entry.as_str() {
            seen.insert(text.to_owned());
        }
    }
    if !seen.contains(canonical) {
        return Err(format!("profile_aliases missing canonical handle {canonical}").into());
    }
    Ok(())
}

fn verify_manifest_signatures(
    root: &Value,
    manifest_digest: &[u8],
    allow_unsigned: bool,
) -> Result<usize, Box<dyn Error>> {
    let entries = root
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest signatures missing signatures array".to_owned())?;
    if entries.is_empty() {
        if allow_unsigned {
            eprintln!(
                "warning: manifest signatures array empty; continuing due to --allow-unsigned"
            );
            return Ok(0);
        }
        return Err("manifest signatures array empty".into());
    }

    for entry in entries {
        let map = entry
            .as_object()
            .ok_or_else(|| "signature entry must be an object".to_owned())?;
        let algorithm = map
            .get("algorithm")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing algorithm".to_owned())?;
        if algorithm != "ed25519" {
            return Err(format!("unsupported signature algorithm {algorithm}").into());
        }
        let signer_hex = map
            .get("signer")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing signer".to_owned())?;
        let signature_hex = map
            .get("signature")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing signature".to_owned())?;

        let signer_bytes = decode_hex(signer_hex)?;
        let signature_bytes = decode_hex(signature_hex)?;

        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes)
            .map_err(|err| format!("invalid signer public key: {err}"))?;

        let public_key_hex = public_key.to_string();
        if let Some(multihash) = map.get("signer_multihash").and_then(Value::as_str)
            && multihash != public_key_hex
        {
            return Err("signer_multihash does not match encoded public key".into());
        }

        let signature = Signature::from_bytes(&signature_bytes);
        signature
            .verify(&public_key, manifest_digest)
            .map_err(|err| format!("signature verification failed: {err}"))?;
    }

    Ok(entries.len())
}

fn extract_string<'a>(root: &'a Value, key: &str) -> Result<&'a str, Box<dyn Error>> {
    root.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} field missing or not a string").into())
}

fn decode_hex(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    hex::decode(input.trim())
        .map_err(|err| format!("failed to decode hex value {input}: {err}").into())
}

pub fn write_admission_fixtures(target_dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(target_dir)?;

    let descriptor = chunker_registry::lookup_by_handle("sorafs.sf1@1.0.0")
        .ok_or("chunker profile sorafs.sf1@1.0.0 is not registered in the chunker registry")?;

    let canonical_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let profile_aliases = canonical_profile_aliases(descriptor);

    let provider_seed =
        decode_hex_array::<32>("505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f")?;
    let provider_pair = KeyPair::from_seed(provider_seed.to_vec(), Algorithm::Ed25519);
    let (provider_algo, provider_public_bytes) = provider_pair.public_key().to_bytes();
    debug_assert_eq!(provider_algo, Algorithm::Ed25519);
    let provider_public_vec = provider_public_bytes.to_vec();
    let provider_public: [u8; 32] = TryInto::<[u8; 32]>::try_into(provider_public_bytes)
        .expect("ed25519 public key must be 32 bytes");

    let provider_id =
        decode_hex_array::<32>("11223344556677889900aabbccddeeff00112233445566778899aabbccddeeff")?;
    let stake_pool_id =
        decode_hex_array::<32>("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100")?;

    let range_capability = ProviderCapabilityRangeV1 {
        max_chunk_span: 32,
        min_granularity: 8,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    }
    .to_bytes()
    .expect("encode range capability");

    let capabilities = vec![
        CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        },
        CapabilityTlv {
            cap_type: CapabilityType::QuicNoise,
            payload: Vec::new(),
        },
        CapabilityTlv {
            cap_type: CapabilityType::ChunkRangeFetch,
            payload: range_capability,
        },
    ];

    let torii_endpoint = AdvertEndpoint {
        kind: EndpointKind::Torii,
        host_pattern: "storage.alpha.svc".to_owned(),
        metadata: Vec::new(),
    };
    let torii_attestation = EndpointAttestationV1 {
        version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
        kind: EndpointAttestationKind::Mtls,
        attested_at: 1_700_592_000,
        expires_at: 1_703_198_400,
        leaf_certificate: decode_hex_vec("3081deadbeef")?,
        intermediate_certificates: vec![decode_hex_vec("aa55cc33")?],
        alpn_ids: vec!["h2".to_owned()],
        report: decode_hex_vec("9091")?,
    };
    let quic_endpoint = AdvertEndpoint {
        kind: EndpointKind::Quic,
        host_pattern: "quic.alpha.svc".to_owned(),
        metadata: Vec::new(),
    };
    let quic_attestation = EndpointAttestationV1 {
        version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
        kind: EndpointAttestationKind::Quic,
        attested_at: 1_700_595_600,
        expires_at: 1_703_202_000,
        leaf_certificate: decode_hex_vec("3045feedface")?,
        intermediate_certificates: Vec::new(),
        alpn_ids: vec!["h3".to_owned()],
        report: decode_hex_vec("a1b2c3")?,
    };

    let endpoints = vec![
        EndpointAdmissionV1 {
            endpoint: torii_endpoint.clone(),
            attestation: torii_attestation,
        },
        EndpointAdmissionV1 {
            endpoint: quic_endpoint.clone(),
            attestation: quic_attestation,
        },
    ];

    let stake_pointer = StakePointer {
        pool_id: stake_pool_id,
        stake_amount: 7_500,
    };

    let proposal = ProviderAdmissionProposalV1 {
        version: sorafs_manifest::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
        provider_id,
        profile_id: canonical_handle.clone(),
        profile_aliases: Some(profile_aliases.clone()),
        stake: stake_pointer,
        capabilities: capabilities.clone(),
        endpoints: endpoints.clone(),
        advert_key: provider_public,
        jurisdiction_code: "JP".to_owned(),
        contact_uri: Some("https://alpha.example/ops".to_owned()),
        stream_budget: Some(StreamBudgetV1 {
            max_in_flight: 10,
            max_bytes_per_sec: 12_000_000,
            burst_bytes: Some(6_000_000),
        }),
        transport_hints: Some(vec![
            TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 0,
            },
            TransportHintV1 {
                protocol: TransportProtocol::QuicStream,
                priority: 1,
            },
        ]),
    };
    proposal
        .validate()
        .map_err(|err| format!("proposal validation failed: {err}"))?;

    let proposal_bytes = to_bytes(&proposal)?;
    let proposal_digest = compute_proposal_digest(&proposal)?;

    let advert_body = ProviderAdvertBodyV1 {
        provider_id,
        profile_id: canonical_handle.clone(),
        profile_aliases: Some(profile_aliases.clone()),
        stake: stake_pointer,
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 1_200,
            max_concurrent_streams: 32,
        },
        capabilities: capabilities.clone(),
        endpoints: vec![torii_endpoint, quic_endpoint],
        rendezvous_topics: vec![
            RendezvousTopic {
                topic: "sorafs.sf1.primary".to_owned(),
                region: "global".to_owned(),
            },
            RendezvousTopic {
                topic: "sorafs.sf1.apac".to_owned(),
                region: "JP".to_owned(),
            },
        ],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: Some("Fixture provider for CI verification".to_owned()),
        stream_budget: proposal.stream_budget,
        transport_hints: proposal.transport_hints.clone(),
    };
    advert_body
        .validate()
        .map_err(|err| format!("advert body validation failed: {err}"))?;

    let advert_body_bytes = to_bytes(&advert_body)?;
    let advert_signature =
        Signature::new(provider_pair.private_key(), advert_body_bytes.as_slice());
    let advert_signature = advert_signature.payload().to_vec();

    let issued_at = 1_700_592_000;
    let expires_at = issued_at + 3_600;

    let advert = ProviderAdvertV1 {
        version: sorafs_manifest::PROVIDER_ADVERT_VERSION_V1,
        issued_at,
        expires_at,
        body: advert_body.clone(),
        signature: sorafs_manifest::AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: provider_public_vec.clone(),
            signature: advert_signature,
        },
        signature_strict: true,
        allow_unknown_capabilities: false,
    };
    advert
        .validate_with_body(issued_at)
        .map_err(|err| format!("advert validation failed: {err}"))?;
    let advert_bytes = to_bytes(&advert)?;
    let advert_body_digest = compute_advert_body_digest(&advert_body)?;

    let council_seeds = [
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        "8899aabbccddeeff00112233445566778899aabbccddeeff0011223344556677",
    ];
    let mut council_signatures = Vec::new();
    for seed in &council_seeds {
        let sk_bytes = decode_hex_array::<32>(seed)?;
        let council_pair = KeyPair::from_seed(sk_bytes.to_vec(), Algorithm::Ed25519);
        let (algo, signer_bytes) = council_pair.public_key().to_bytes();
        debug_assert_eq!(algo, Algorithm::Ed25519);
        let signer: [u8; 32] = TryInto::<[u8; 32]>::try_into(signer_bytes)
            .expect("ed25519 public key must be 32 bytes");
        let signature = Signature::new(council_pair.private_key(), proposal_digest.as_slice());
        council_signatures.push(CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        });
    }

    let retention_epoch = issued_at + 86_400 * 90;
    let envelope = ProviderAdmissionEnvelopeV1 {
        version: sorafs_manifest::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
        proposal: proposal.clone(),
        proposal_digest,
        advert_body: advert_body.clone(),
        advert_body_digest,
        issued_at,
        retention_epoch,
        council_signatures: council_signatures.clone(),
        notes: Some("Fixture council approval for provider alpha".to_owned()),
    };
    let record = AdmissionRecord::new(envelope.clone())
        .map_err(|err| format!("envelope validation failed: {err}"))?;
    verify_advert_against_record(&advert, &record)
        .map_err(|err| format!("fixture advert mismatched envelope: {err}"))?;

    let envelope_bytes = to_bytes(&envelope)?;
    let envelope_digest = compute_envelope_digest(&envelope)?;

    write_binary(
        target_dir.join("provider_alpha_proposal.to"),
        &proposal_bytes,
    )?;
    write_binary(
        target_dir.join("provider_alpha_advert_body.to"),
        &advert_body_bytes,
    )?;
    write_binary(target_dir.join("provider_alpha_advert.to"), &advert_bytes)?;
    write_binary(
        target_dir.join("provider_alpha_envelope.to"),
        &envelope_bytes,
    )?;

    write_json_file(
        target_dir.join("provider_alpha_proposal.json"),
        build_proposal_summary(
            &proposal,
            &proposal_bytes,
            &proposal_digest,
            &profile_aliases,
        ),
    )?;
    write_json_file(
        target_dir.join("provider_alpha_advert_body.json"),
        build_advert_body_summary(&advert_body, &capabilities),
    )?;
    write_json_file(
        target_dir.join("provider_alpha_advert.json"),
        build_advert_summary(&advert, &advert_bytes, &advert_body_digest),
    )?;
    write_json_file(
        target_dir.join("provider_alpha_envelope.json"),
        build_envelope_summary(
            &envelope,
            &council_signatures,
            &proposal_digest,
            &advert_body_digest,
            &envelope_digest,
        ),
    )?;
    write_json_file(
        target_dir.join("provider_alpha_metadata.json"),
        build_metadata_summary(
            &proposal_digest,
            &advert_body_digest,
            record.envelope_digest(),
            &council_signatures,
        ),
    )?;

    write_readme(target_dir)?;

    Ok(())
}

fn canonical_profile_aliases(descriptor: &ChunkerProfileDescriptor) -> Vec<String> {
    let canonical_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let mut aliases = Vec::with_capacity(descriptor.aliases.len() + 1);
    aliases.push(canonical_handle.clone());
    aliases.extend(descriptor.aliases.iter().map(|alias| alias.to_string()));
    let mut seen = HashSet::new();
    aliases.retain(|alias| seen.insert(alias.clone()));
    aliases
}

fn build_proposal_summary(
    proposal: &ProviderAdmissionProposalV1,
    proposal_bytes: &[u8],
    proposal_digest: &[u8; 32],
    aliases: &[String],
) -> Map {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(proposal.version as u64));
    map.insert(
        "provider_id_hex".into(),
        Value::from(hex_lower(proposal.provider_id)),
    );
    map.insert(
        "profile_id".into(),
        Value::from(proposal.profile_id.clone()),
    );
    map.insert(
        "profile_aliases".into(),
        Value::Array(
            aliases
                .iter()
                .map(|alias| Value::from(alias.clone()))
                .collect(),
        ),
    );
    map.insert(
        "stake_pool_id_hex".into(),
        Value::from(hex_lower(proposal.stake.pool_id)),
    );
    map.insert(
        "stake_amount".into(),
        Value::from(proposal.stake.stake_amount.to_string()),
    );
    map.insert(
        "jurisdiction_code".into(),
        Value::from(proposal.jurisdiction_code.clone()),
    );
    if let Some(contact) = &proposal.contact_uri {
        map.insert("contact_uri".into(), Value::from(contact.clone()));
    }
    map.insert(
        "endpoint_count".into(),
        Value::from(proposal.endpoints.len() as u64),
    );
    map.insert(
        "stream_budget".into(),
        match proposal.stream_budget.as_ref() {
            Some(budget) => stream_budget_to_value(budget),
            None => Value::Null,
        },
    );
    map.insert(
        "transport_hints".into(),
        match proposal.transport_hints.as_ref() {
            Some(hints) => transport_hints_to_value(hints),
            None => Value::Null,
        },
    );
    map.insert(
        "capability_types".into(),
        Value::Array(
            proposal
                .capabilities
                .iter()
                .map(|cap| Value::from(capability_label(cap.cap_type)))
                .collect(),
        ),
    );
    map.insert(
        "proposal_len".into(),
        Value::from(proposal_bytes.len() as u64),
    );
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(hex_lower(proposal_digest)),
    );
    map
}

fn build_advert_body_summary(
    advert_body: &ProviderAdvertBodyV1,
    capabilities: &[CapabilityTlv],
) -> Map {
    let mut map = Map::new();
    map.insert(
        "provider_id_hex".into(),
        Value::from(hex_lower(advert_body.provider_id)),
    );
    map.insert(
        "profile_id".into(),
        Value::from(advert_body.profile_id.clone()),
    );
    map.insert(
        "profile_aliases".into(),
        Value::Array(
            advert_body
                .profile_aliases
                .as_ref()
                .map(|aliases| {
                    aliases
                        .iter()
                        .map(|alias| Value::from(alias.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        ),
    );
    map.insert(
        "stake_pool_id_hex".into(),
        Value::from(hex_lower(advert_body.stake.pool_id)),
    );
    map.insert(
        "stake_amount".into(),
        Value::from(advert_body.stake.stake_amount.to_string()),
    );
    map.insert(
        "availability".into(),
        Value::from(match advert_body.qos.availability {
            AvailabilityTier::Hot => "hot",
            AvailabilityTier::Warm => "warm",
            AvailabilityTier::Cold => "cold",
        }),
    );
    map.insert(
        "max_retrieval_latency_ms".into(),
        Value::from(advert_body.qos.max_retrieval_latency_ms as u64),
    );
    map.insert(
        "max_concurrent_streams".into(),
        Value::from(advert_body.qos.max_concurrent_streams as u64),
    );
    map.insert(
        "capability_types".into(),
        Value::Array(
            capabilities
                .iter()
                .map(|cap| Value::from(capability_label(cap.cap_type)))
                .collect(),
        ),
    );
    map.insert(
        "endpoint_count".into(),
        Value::from(advert_body.endpoints.len() as u64),
    );
    map.insert(
        "stream_budget".into(),
        match advert_body.stream_budget.as_ref() {
            Some(budget) => stream_budget_to_value(budget),
            None => Value::Null,
        },
    );
    map.insert(
        "transport_hints".into(),
        match advert_body.transport_hints.as_ref() {
            Some(hints) => transport_hints_to_value(hints),
            None => Value::Null,
        },
    );
    map.insert(
        "rendezvous_topics".into(),
        Value::Array(
            advert_body
                .rendezvous_topics
                .iter()
                .map(|topic| {
                    let mut entry = Map::new();
                    entry.insert("topic".into(), Value::from(topic.topic.clone()));
                    entry.insert("region".into(), Value::from(topic.region.clone()));
                    Value::Object(entry)
                })
                .collect(),
        ),
    );
    map
}

fn build_advert_summary(
    advert: &ProviderAdvertV1,
    advert_bytes: &[u8],
    advert_body_digest: &[u8; 32],
) -> Map {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(advert.version as u64));
    map.insert("issued_at".into(), Value::from(advert.issued_at));
    map.insert("expires_at".into(), Value::from(advert.expires_at));
    map.insert(
        "signature_alg".into(),
        Value::from(match advert.signature.algorithm {
            SignatureAlgorithm::Ed25519 => "ed25519",
            SignatureAlgorithm::MultiSig => "multi_sig",
        }),
    );
    map.insert(
        "public_key_hex".into(),
        Value::from(hex_lower(&advert.signature.public_key)),
    );
    map.insert(
        "signature_hex".into(),
        Value::from(hex_lower(&advert.signature.signature)),
    );
    map.insert("advert_len".into(), Value::from(advert_bytes.len() as u64));
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(hex_lower(advert_body_digest)),
    );
    map.insert(
        "stream_budget".into(),
        match advert.body.stream_budget.as_ref() {
            Some(budget) => stream_budget_to_value(budget),
            None => Value::Null,
        },
    );
    map.insert(
        "transport_hints".into(),
        match advert.body.transport_hints.as_ref() {
            Some(hints) => transport_hints_to_value(hints),
            None => Value::Null,
        },
    );
    map
}

fn build_envelope_summary(
    envelope: &ProviderAdmissionEnvelopeV1,
    signatures: &[CouncilSignature],
    proposal_digest: &[u8; 32],
    advert_body_digest: &[u8; 32],
    envelope_digest: &[u8; 32],
) -> Map {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(envelope.version as u64));
    map.insert("issued_at".into(), Value::from(envelope.issued_at));
    map.insert(
        "retention_epoch".into(),
        Value::from(envelope.retention_epoch),
    );
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(hex_lower(proposal_digest)),
    );
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(hex_lower(advert_body_digest)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(hex_lower(envelope_digest)),
    );
    map.insert(
        "council_signature_count".into(),
        Value::from(signatures.len() as u64),
    );
    map.insert(
        "council_signers".into(),
        Value::Array(
            signatures
                .iter()
                .map(|sig| Value::from(hex_lower(sig.signer)))
                .collect(),
        ),
    );
    if let Some(notes) = &envelope.notes {
        map.insert("notes".into(), Value::from(notes.clone()));
    }
    map
}

fn build_metadata_summary(
    proposal_digest: &[u8; 32],
    advert_body_digest: &[u8; 32],
    envelope_digest: &[u8; 32],
    signatures: &[CouncilSignature],
) -> Map {
    let mut map = Map::new();
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(hex_lower(proposal_digest)),
    );
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(hex_lower(advert_body_digest)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(hex_lower(envelope_digest)),
    );
    map.insert(
        "council_signatures".into(),
        Value::Array(
            signatures
                .iter()
                .map(|sig| {
                    let mut entry = Map::new();
                    entry.insert("signer".into(), Value::from(hex_lower(sig.signer)));
                    entry.insert(
                        "signature_hex".into(),
                        Value::from(hex_lower(&sig.signature)),
                    );
                    Value::Object(entry)
                })
                .collect(),
        ),
    );
    map
}

fn write_binary(path: PathBuf, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn write_json_file(path: PathBuf, map: Map) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = to_string_pretty(&Value::Object(map))?;
    text.push('\n');
    fs::write(path, text)?;
    Ok(())
}

fn write_readme(target_dir: &Path) -> Result<(), Box<dyn Error>> {
    let readme_path = target_dir.join("README.md");
    let mut content = String::from(
        "# SoraFS Provider Admission Fixtures\n\n\
These fixtures capture the deterministic admission bundle used across CLI and Torii tests.\n\n\
- `provider_alpha_proposal.to` — Norito-encoded `ProviderAdmissionProposalV1`\n\
- `provider_alpha_advert_body.to` — Norito-encoded `ProviderAdvertBodyV1`\n\
- `provider_alpha_advert.to` — Signed `ProviderAdvertV1` payload\n\
- `provider_alpha_envelope.to` — Governance envelope binding the proposal, advert, and council signatures\n\
- `provider_alpha_metadata.json` — Digest and signer summary for quick verification\n\n\
Regenerate the fixtures with:\n\n```\n\
cargo xtask sorafs-admission-fixtures\n\
```\n\n\
The generator keeps the canonical chunker aliases, advert capabilities, and council signatures\n\
stable so CI can detect drift or accidental edits.\n",
    );
    content.push('\n');
    fs::write(readme_path, content)?;
    Ok(())
}

fn decode_hex_array<const N: usize>(input: &str) -> Result<[u8; N], Box<dyn Error>> {
    let bytes = hex::decode(input).map_err(|err| format!("invalid hex `{input}`: {err}"))?;
    if bytes.len() != N {
        return Err(format!(
            "expected {N} bytes from hex `{input}`, found {}",
            bytes.len()
        )
        .into());
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn decode_hex_vec(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    hex::decode(input).map_err(|err| format!("invalid hex `{input}`: {err}").into())
}

fn hex_lower<T: AsRef<[u8]>>(bytes: T) -> String {
    hex::encode(bytes)
}

fn stream_budget_to_value(budget: &StreamBudgetV1) -> Value {
    let mut map = Map::new();
    map.insert(
        "max_in_flight".into(),
        Value::from(budget.max_in_flight as u64),
    );
    map.insert(
        "max_bytes_per_sec".into(),
        Value::from(budget.max_bytes_per_sec),
    );
    map.insert(
        "burst_bytes".into(),
        match budget.burst_bytes {
            Some(burst) => Value::from(burst),
            None => Value::Null,
        },
    );
    Value::Object(map)
}

fn transport_hints_to_value(hints: &[TransportHintV1]) -> Value {
    Value::Array(
        hints
            .iter()
            .map(|hint| {
                let mut map = Map::new();
                map.insert(
                    "protocol".into(),
                    Value::from(transport_protocol_label(hint.protocol)),
                );
                map.insert("priority".into(), Value::from(hint.priority as u64));
                Value::Object(map)
            })
            .collect(),
    )
}

fn transport_protocol_label(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::ToriiHttpRange => "torii_http_range",
        TransportProtocol::QuicStream => "quic_stream",
        TransportProtocol::SoraNetRelay => "soranet_relay",
        TransportProtocol::VendorReserved => "vendor_reserved",
    }
}

fn capability_label(cap: CapabilityType) -> &'static str {
    match cap {
        CapabilityType::ToriiGateway => "torii_gateway",
        CapabilityType::QuicNoise => "quic_noise",
        CapabilityType::SoraNetHybridPq => "soranet_pq",
        CapabilityType::ChunkRangeFetch => "chunk_range_fetch",
        CapabilityType::VendorReserved => "vendor_reserved",
    }
}

pub fn write_pin_registry_fixture(output: PathBuf) -> Result<(), Box<dyn Error>> {
    let state = pin_fixture_make_state();
    let mut block = state.block(pin_fixture_default_block_header());
    let mut tx = block.transaction();

    let digest = pin_fixture_default_digest();
    let chunk_digest = pin_fixture_default_chunk_digest();
    let council_keys = pin_fixture_council_keypair();

    pin_fixture_register_and_approve(&mut tx, digest, chunk_digest, &council_keys)?;

    let alias_binding =
        pin_fixture_alias_binding_for(digest, "sora", "docs", 12, 36, &council_keys)?;
    BindManifestAlias {
        digest,
        binding: alias_binding.clone(),
        bound_epoch: 12,
        expiry_epoch: 36,
    }
    .execute(&pin_fixture_alice(), &mut tx)
    .map_err(|err| format!("failed to bind alias: {err}"))?;

    let providers = [
        ProviderId::new([0x51; 32]),
        ProviderId::new([0x52; 32]),
        ProviderId::new([0x53; 32]),
    ];
    let order_id = ReplicationOrderId::new([0x44; 32]);
    let order_struct = pin_fixture_replication_order(order_id, digest, &providers, 3);
    let order_payload = norito::to_bytes(&order_struct)?;
    IssueReplicationOrder {
        order_id,
        order_payload,
        issued_epoch: 20,
        deadline_epoch: 28,
    }
    .execute(&pin_fixture_alice(), &mut tx)
    .map_err(|err| format!("failed to issue replication order: {err}"))?;

    CompleteReplicationOrder {
        order_id,
        completion_epoch: 25,
    }
    .execute(&pin_fixture_alice(), &mut tx)
    .map_err(|err| format!("failed to complete replication order: {err}"))?;

    tx.apply();
    block
        .commit()
        .map_err(|err| format!("failed to commit block: {err}"))?;

    let view = state.view();
    let world = view.world();

    let manifest = world
        .pin_manifests()
        .get(&digest)
        .cloned()
        .ok_or("manifest missing after execution")?;
    let alias_id = ManifestAliasId::from(&alias_binding);
    let alias_record = world
        .manifest_aliases()
        .get(&alias_id)
        .cloned()
        .ok_or("alias missing after execution")?;
    let order_record = world
        .replication_orders()
        .get(&order_id)
        .cloned()
        .ok_or("replication order missing after execution")?;

    let snapshot = pin_fixture_snapshot_json(&manifest, &alias_record, &order_record)?;
    let pretty = to_string_pretty(&snapshot)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, format!("{pretty}\n"))?;
    println!("wrote {}", output.display());
    Ok(())
}

fn pin_fixture_make_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let live = LiveQueryStore::start_test();
    State::new_for_testing(World::new(), kura, live)
}

fn pin_fixture_default_block_header() -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    )
}

fn pin_fixture_register_and_approve(
    tx: &mut iroha_core::state::StateTransaction<'_, '_>,
    digest: ManifestDigest,
    chunk_digest: [u8; 32],
    council_keys: &KeyPair,
) -> Result<(), Box<dyn Error>> {
    RegisterPinManifest {
        digest,
        chunker: pin_fixture_default_chunker(),
        chunk_digest_sha3_256: chunk_digest,
        policy: pin_fixture_default_policy(),
        submitted_epoch: 5,
        alias: None,
        successor_of: None,
    }
    .execute(&pin_fixture_alice(), tx)
    .map_err(|err| format!("failed to register manifest: {err}"))?;

    let stored = tx
        .world()
        .pin_manifests()
        .get(&digest)
        .cloned()
        .ok_or("manifest missing after registration")?;
    let envelope = pin_fixture_build_envelope(&stored, council_keys)?;

    ApprovePinManifest {
        digest,
        approved_epoch: 7,
        council_envelope: Some(envelope),
        council_envelope_digest: None,
    }
    .execute(&pin_fixture_alice(), tx)
    .map_err(|err| format!("failed to approve manifest: {err}"))?;
    Ok(())
}

fn pin_fixture_default_digest() -> ManifestDigest {
    ManifestDigest::new([0xAA; 32])
}

fn pin_fixture_default_chunk_digest() -> [u8; 32] {
    [0xCD; 32]
}

fn pin_fixture_default_chunker() -> ChunkerProfileHandle {
    let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
    ChunkerProfileHandle {
        profile_id: descriptor.id.0,
        namespace: descriptor.namespace.to_owned(),
        name: descriptor.name.to_owned(),
        semver: descriptor.semver.to_owned(),
        multihash_code: descriptor.multihash_code,
    }
}

fn pin_fixture_default_policy() -> PinPolicy {
    PinPolicy {
        min_replicas: 3,
        storage_class: StorageClass::Hot,
        retention_epoch: 42,
    }
}

fn pin_fixture_replication_order(
    order_id: ReplicationOrderId,
    manifest: ManifestDigest,
    providers: &[ProviderId],
    target_replicas: u16,
) -> ReplicationOrderV1 {
    let assignments = providers
        .iter()
        .map(|provider| ReplicationAssignmentV1 {
            provider_id: *provider.as_bytes(),
            slice_gib: 512,
            lane: None,
        })
        .collect();
    ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: *order_id.as_bytes(),
        manifest_cid: manifest.as_bytes().to_vec(),
        manifest_digest: *manifest.as_bytes(),
        chunking_profile: format!(
            "{}.{}@{}",
            pin_fixture_default_chunker().namespace,
            pin_fixture_default_chunker().name,
            pin_fixture_default_chunker().semver
        ),
        target_replicas,
        assignments,
        issued_at: 1_700_000_000,
        deadline_at: 1_700_086_400,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 86_400,
            min_availability_percent_milli: 99_500,
            min_por_success_percent_milli: 98_000,
        },
        metadata: Vec::new(),
    }
}

fn pin_fixture_alias_binding_for(
    digest: ManifestDigest,
    namespace: &str,
    name: &str,
    bound_at: u64,
    expiry_epoch: u64,
    council_keys: &KeyPair,
) -> Result<ManifestAliasBinding, Box<dyn Error>> {
    let binding_payload = AliasBindingV1 {
        alias: format!("{namespace}/{name}"),
        manifest_cid: digest.as_bytes().to_vec(),
        bound_at,
        expiry_epoch,
    };

    let merkle_path: Vec<[u8; 32]> = Vec::new();
    let registry_root =
        alias_merkle_root(&binding_payload, &merkle_path).map_err(|err| format!("{err}"))?;

    let generated_at_unix = 1_700_000_000;
    let expires_at_unix = generated_at_unix + 86_400;

    let mut bundle = AliasProofBundleV1 {
        binding: binding_payload,
        registry_root,
        registry_height: bound_at,
        generated_at_unix,
        expires_at_unix,
        merkle_path,
        council_signatures: Vec::new(),
    };

    let digest = alias_proof_signature_digest(&bundle);
    let signature = Signature::new(council_keys.private_key(), digest.as_ref());
    let (_, public_bytes) = council_keys.public_key().to_bytes();
    let signer: [u8; 32] = public_bytes
        .try_into()
        .map_err(|_| "expected ed25519 public key to be 32 bytes")?;

    bundle.council_signatures.push(CouncilSignature {
        signer,
        signature: signature.payload().to_vec(),
    });

    let proof = to_bytes(&bundle)?;
    Ok(ManifestAliasBinding {
        name: name.to_owned(),
        namespace: namespace.to_owned(),
        proof,
    })
}

fn pin_fixture_council_keypair() -> KeyPair {
    let secret_bytes = [0x11; 32];
    let private =
        PrivateKey::from_bytes(Algorithm::Ed25519, &secret_bytes).expect("private key from bytes");
    KeyPair::from_private_key(private).expect("derive keypair")
}

fn pin_fixture_build_envelope(
    record: &PinManifestRecord,
    keypair: &KeyPair,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut sig_entry = json::Map::new();
    let signature = Signature::new(keypair.private_key(), record.digest.as_bytes());
    let public_bytes_hex = hex::encode(keypair.public_key().to_bytes().1);
    sig_entry.insert("algorithm".into(), Value::from("ed25519"));
    sig_entry.insert("signer".into(), Value::from(public_bytes_hex));
    sig_entry.insert(
        "signature".into(),
        Value::from(hex::encode(signature.payload())),
    );
    sig_entry.insert(
        "signer_multihash".into(),
        Value::from(keypair.public_key().to_string()),
    );

    let mut envelope = json::Map::new();
    envelope.insert(
        "chunk_digest_sha3_256".into(),
        Value::from(hex::encode(record.chunk_digest_sha3_256)),
    );
    envelope.insert(
        "manifest_blake3".into(),
        Value::from(hex::encode(record.digest.as_bytes())),
    );
    envelope.insert("profile".into(), Value::from(record.chunker.to_handle()));
    envelope.insert(
        "signatures".into(),
        Value::Array(vec![Value::Object(sig_entry)]),
    );

    let mut serialized = json::to_vec_pretty(&Value::Object(envelope))?;
    serialized.push(b'\n');
    Ok(serialized)
}

fn pin_fixture_snapshot_json(
    manifest: &PinManifestRecord,
    alias: &ManifestAliasRecord,
    order: &ReplicationOrderRecord,
) -> Result<Value, Box<dyn Error>> {
    let manifest_obj = pin_fixture_manifest_snapshot(manifest)?;
    let alias_obj = pin_fixture_alias_snapshot(alias)?;
    let order_obj = pin_fixture_order_snapshot(order)?;

    let mut root = json::Map::new();
    root.insert(
        "manifests".into(),
        Value::Array(vec![Value::Object(manifest_obj)]),
    );
    root.insert(
        "aliases".into(),
        Value::Array(vec![Value::Object(alias_obj)]),
    );
    root.insert(
        "replication_orders".into(),
        Value::Array(vec![Value::Object(order_obj)]),
    );
    Ok(Value::Object(root))
}

fn pin_fixture_manifest_snapshot(
    manifest: &PinManifestRecord,
) -> Result<json::Map, Box<dyn Error>> {
    let mut manifest_obj = json::Map::new();
    manifest_obj.insert(
        "digest_hex".into(),
        Value::String(hex::encode(manifest.digest.as_bytes())),
    );
    let (status_label, status_epoch) = match manifest.status {
        PinStatus::Pending => ("pending", None),
        PinStatus::Approved(epoch) => ("approved", Some(epoch)),
        PinStatus::Retired(epoch) => ("retired", Some(epoch)),
    };
    manifest_obj.insert("status".into(), Value::String(status_label.into()));
    manifest_obj.insert(
        "status_epoch".into(),
        status_epoch.map_or(Value::Null, Value::from),
    );
    manifest_obj.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::String(hex::encode(manifest.chunk_digest_sha3_256)),
    );
    manifest_obj.insert(
        "chunker_handle".into(),
        Value::String(manifest.chunker.to_handle()),
    );
    let mut policy_obj = json::Map::new();
    policy_obj.insert(
        "min_replicas".into(),
        Value::from(manifest.policy.min_replicas),
    );
    policy_obj.insert(
        "storage_class".into(),
        Value::String(
            match manifest.policy.storage_class {
                StorageClass::Hot => "hot",
                StorageClass::Warm => "warm",
                StorageClass::Cold => "cold",
            }
            .into(),
        ),
    );
    policy_obj.insert(
        "retention_epoch".into(),
        Value::from(manifest.policy.retention_epoch),
    );
    manifest_obj.insert("policy".into(), Value::Object(policy_obj));
    manifest_obj.insert(
        "submitted_by".into(),
        Value::String(manifest.submitted_by.to_string()),
    );
    manifest_obj.insert(
        "submitted_epoch".into(),
        Value::from(manifest.submitted_epoch),
    );
    manifest_obj.insert(
        "alias_label".into(),
        manifest.alias.as_ref().map_or(Value::Null, |binding| {
            Value::String(format!("{}/{}", binding.namespace, binding.name))
        }),
    );
    manifest_obj.insert(
        "council_envelope_digest_hex".into(),
        manifest
            .council_envelope_digest
            .map_or(Value::Null, |digest| Value::String(hex::encode(digest))),
    );
    Ok(manifest_obj)
}

fn pin_fixture_alias_snapshot(alias: &ManifestAliasRecord) -> Result<json::Map, Box<dyn Error>> {
    let mut alias_obj = json::Map::new();
    alias_obj.insert(
        "alias_label".into(),
        Value::String(alias.alias_id().as_label()),
    );
    alias_obj.insert(
        "namespace".into(),
        Value::String(alias.binding.namespace.clone()),
    );
    alias_obj.insert("name".into(), Value::String(alias.binding.name.clone()));
    alias_obj.insert(
        "manifest_digest_hex".into(),
        Value::String(hex::encode(alias.manifest.as_bytes())),
    );
    alias_obj.insert("bound_by".into(), Value::String(alias.bound_by.to_string()));
    alias_obj.insert("bound_epoch".into(), Value::from(alias.bound_epoch));
    alias_obj.insert("expiry_epoch".into(), Value::from(alias.expiry_epoch));
    alias_obj.insert(
        "proof_b64".into(),
        Value::String(BASE64_STD.encode(&alias.binding.proof)),
    );
    Ok(alias_obj)
}

fn pin_fixture_order_snapshot(order: &ReplicationOrderRecord) -> Result<json::Map, Box<dyn Error>> {
    let order_payload: ReplicationOrderV1 =
        norito::decode_from_bytes(&order.canonical_order).map_err(|err| format!("{err}"))?;
    let mut order_obj = json::Map::new();
    order_obj.insert(
        "order_id_hex".into(),
        Value::String(hex::encode(order.order_id.as_bytes())),
    );
    order_obj.insert(
        "manifest_digest_hex".into(),
        Value::String(hex::encode(order.manifest_digest.as_bytes())),
    );
    order_obj.insert(
        "issued_by".into(),
        Value::String(order.issued_by.to_string()),
    );
    order_obj.insert("issued_epoch".into(), Value::from(order.issued_epoch));
    order_obj.insert("deadline_epoch".into(), Value::from(order.deadline_epoch));
    let (status_label, status_epoch) = match order.status {
        ReplicationOrderStatus::Pending => ("pending", None),
        ReplicationOrderStatus::Completed(epoch) => ("completed", Some(epoch)),
        ReplicationOrderStatus::Expired(epoch) => ("expired", Some(epoch)),
    };
    order_obj.insert("status".into(), Value::String(status_label.into()));
    order_obj.insert(
        "status_epoch".into(),
        status_epoch.map_or(Value::Null, Value::from),
    );
    order_obj.insert(
        "target_replicas".into(),
        Value::from(order_payload.target_replicas as u64),
    );
    order_obj.insert(
        "canonical_order_b64".into(),
        Value::String(BASE64_STD.encode(&order.canonical_order)),
    );
    order_obj.insert(
        "sla_ingest_deadline_secs".into(),
        Value::from(order_payload.sla.ingest_deadline_secs),
    );
    order_obj.insert(
        "sla_min_availability_percent_milli".into(),
        Value::from(order_payload.sla.min_availability_percent_milli as u64),
    );
    order_obj.insert(
        "sla_min_por_success_percent_milli".into(),
        Value::from(order_payload.sla.min_por_success_percent_milli as u64),
    );
    let assignments = order_payload
        .assignments
        .iter()
        .map(|assignment| {
            let mut map = json::Map::new();
            map.insert(
                "provider_id_hex".into(),
                Value::String(hex::encode(assignment.provider_id)),
            );
            map.insert("slice_gib".into(), Value::from(assignment.slice_gib));
            Value::Object(map)
        })
        .collect();
    order_obj.insert("assignments".into(), Value::Array(assignments));
    Ok(order_obj)
}

fn pin_fixture_alice() -> AccountId {
    let domain: DomainId = "sora".parse().expect("valid domain");
    let public_key: PublicKey =
        "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
            .parse()
            .expect("valid public key");
    AccountId::new(domain, public_key)
}

pub fn run_adoption_check(
    options: AdoptionCheckOptions,
) -> Result<AdoptionCheckReport, Box<dyn Error>> {
    let scoreboard_paths = if options.scoreboard_paths.is_empty() {
        vec![default_adoption_scoreboard_path()]
    } else {
        options.scoreboard_paths.clone()
    };
    if scoreboard_paths.is_empty() {
        return Err("no scoreboards supplied for adoption check".into());
    }

    let summary_paths = if options.summary_paths.is_empty() {
        scoreboard_paths
            .iter()
            .map(|path| default_summary_path_for_scoreboard(path.as_path()))
            .collect()
    } else if options.summary_paths.len() == scoreboard_paths.len() {
        options.summary_paths.clone()
    } else {
        return Err(format!(
            "provided {} --summary path(s) but {scoreboard_count} scoreboard(s); counts must match",
            options.summary_paths.len(),
            scoreboard_count = scoreboard_paths.len()
        )
        .into());
    };

    let mut evaluated = 0usize;
    let mut single_source_override_used = false;
    let mut implicit_metadata_override_used = false;
    let mut reports = Vec::new();
    for (path, summary_path) in scoreboard_paths.iter().zip(summary_paths.iter()) {
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read scoreboard `{}`: {err}", path.display()))?;
        let value: Value = json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse scoreboard JSON `{}`: {err}",
                path.display()
            )
        })?;
        let root = value
            .as_object()
            .ok_or_else(|| format!("scoreboard `{}` must be a JSON object", path.display()))?;
        let metadata = match root.get("metadata") {
            Some(value) => parse_scoreboard_metadata(value, path)?,
            None => {
                return Err(format!(
                    "scoreboard `{}` missing `metadata` object; rerun the capture with --use-scoreboard so adoption evidence records provider totals, mix labels, and transport overrides",
                    path.display()
                )
                .into());
            }
        };
        let meta = metadata.as_ref().ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata block is null or empty; rerun the capture with --use-scoreboard so provider totals and mix labels are recorded",
                path.display()
            )
        })?;
        let mut expected_gateway_manifest: Option<(String, String)> = None;
        let scoreboard_telemetry_label = non_empty_trimmed(meta.telemetry_source.as_deref());

        let direct = meta.provider_count.ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `provider_count`; rerun the capture with the latest CLI/SDK so adoption evidence records provider totals",
                path.display()
            )
        })?;
        let gateway = meta.gateway_provider_count.ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `gateway_provider_count`; rerun the capture with the latest CLI/SDK so adoption evidence records provider totals",
                path.display()
            )
        })?;
        let provider_mix = meta.provider_mix.as_deref().ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `provider_mix`; rerun the capture with the latest CLI/SDK so adoption evidence records `provider_count`, `gateway_provider_count`, and the derived mix label",
                path.display()
            )
        })?;
        let expected_provider_mix = provider_mix_label_from_counts(direct, gateway);
        if provider_mix != expected_provider_mix {
            return Err(format!(
                "scoreboard `{}` metadata.provider_mix=`{provider_mix}` does not match the derived `{expected_provider_mix}` label for provider_count={} and gateway_provider_count={}; regenerate the capture or fix the metadata",
                path.display(),
                direct,
                gateway
            )
            .into());
        }
        if meta.use_scoreboard == Some(false) {
            return Err(format!(
                "scoreboard `{}` metadata.use_scoreboard=false; multi-source adoption requires scoreboard mode",
                path.display()
            )
            .into());
        }
        let stray_gateway_manifest_metadata = meta.gateway_manifest_provided == Some(true)
            || non_empty_trimmed(meta.gateway_manifest_id.as_deref()).is_some()
            || non_empty_trimmed(meta.gateway_manifest_cid.as_deref()).is_some();
        if gateway == 0 && stray_gateway_manifest_metadata {
            return Err(format!(
                "scoreboard `{}` metadata includes gateway manifest fields but records no gateway providers; rerun without gateway manifest flags or capture gateway fetch evidence",
                path.display()
            )
            .into());
        }
        if meta.allow_implicit_metadata == Some(true) {
            if options.allow_implicit_metadata {
                implicit_metadata_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` metadata.allow_implicit_metadata=true; rerun the capture with live provider adverts or pass --allow-implicit-metadata when the baked-in capability metadata is intentionally used",
                    path.display()
                )
                .into());
            }
        }
        if let Some(label) = metadata_direct_transport_label(meta) {
            if options.allow_single_source_fallback {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` metadata transport policy `{label}` indicates a direct-only fallback; rerun with --allow-single-source when downgrades are intentional",
                    path.display()
                )
                .into());
            }
        }
        if let Some((field, value)) = metadata_single_source_hint(meta) {
            if options.allow_single_source_fallback {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` metadata {field}={value} indicates single-source fallback; rerun with --allow-single-source when downgrades are intentional",
                    path.display()
                )
                .into());
            }
        }
        if gateway > 0 {
            if meta.gateway_manifest_provided != Some(true) {
                return Err(format!(
                    "scoreboard `{}` recorded {gateway} gateway provider(s) but metadata.gateway_manifest_provided=false; capture evidence with --manifest-envelope so gateway fetches include the signed manifest",
                    path.display()
                )
                .into());
            }
            let manifest_id = non_empty_trimmed(meta.gateway_manifest_id.as_deref()).ok_or_else(|| {
                format!(
                    "scoreboard `{}` recorded gateway providers but metadata.gateway_manifest_id is missing; rerun the capture with --manifest-envelope so manifest digests are persisted",
                    path.display()
                )
            })?;
            let manifest_cid = non_empty_trimmed(meta.gateway_manifest_cid.as_deref()).ok_or_else(|| {
                format!(
                    "scoreboard `{}` recorded gateway providers but metadata.gateway_manifest_cid is missing; rerun the capture with --manifest-envelope so manifest digests are persisted",
                    path.display()
                )
            })?;
            expected_gateway_manifest = Some((manifest_id, manifest_cid));
        }
        let entries = root
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("scoreboard `{}` missing `entries` array", path.display()))?;
        let mut eligible = 0usize;
        let mut zero_weight: Vec<String> = Vec::new();
        let mut weight_sum = 0.0;
        let mut scoreboard_providers: HashSet<String> = HashSet::new();
        for (index, entry) in entries.iter().enumerate() {
            let provider = entry
                .get("provider_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "scoreboard `{}` entry {index} missing `provider_id` string",
                        path.display()
                    )
                })?;
            scoreboard_providers.insert(provider.to_string());
            if scoreboard_entry_is_eligible(entry) {
                eligible += 1;
                let weight = entry
                    .get("normalised_weight")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                weight_sum += weight;
                if options.require_positive_weight && weight <= 0.0 {
                    zero_weight.push(provider.to_string());
                }
                continue;
            }
        }
        if eligible < options.min_eligible_providers {
            if options.allow_single_source_fallback && eligible == 1 {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "scoreboard `{}` has {eligible} eligible provider(s); require at least {}",
                    path.display(),
                    options.min_eligible_providers
                )
                .into());
            }
        }
        if !zero_weight.is_empty() {
            return Err(format!(
                "scoreboard `{}` reported zero normalised weight for providers: {}",
                path.display(),
                zero_weight.join(", ")
            )
            .into());
        }
        const WEIGHT_SUM_TOLERANCE: f64 = 1e-3;
        if (weight_sum - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
            return Err(format!(
                "scoreboard `{}` normalised weights sum to {:.6}; expected 1.0 (±{WEIGHT_SUM_TOLERANCE})",
                path.display(),
                weight_sum
            )
            .into());
        }
        let declared_total = direct + gateway;
        if declared_total > 0
            && declared_total != u64::try_from(scoreboard_providers.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "scoreboard `{}` metadata reports {} provider(s) (local + gateway) but the scoreboard lists {}; regenerate the capture so counts stay in sync",
                path.display(),
                declared_total,
                scoreboard_providers.len()
            )
            .into());
        }

        let summary_bytes = fs::read(summary_path)
            .map_err(|err| format!("failed to read summary `{}`: {err}", summary_path.display()))?;
        let summary_value: Value = json::from_slice(&summary_bytes).map_err(|err| {
            format!(
                "failed to parse summary JSON `{}`: {err}",
                summary_path.display()
            )
        })?;
        let stats = collect_summary_stats(&summary_value, summary_path)?;
        if options.require_telemetry_source {
            let scoreboard_label = scoreboard_telemetry_label
                .as_deref()
                .ok_or_else(|| {
                    format!(
                        "scoreboard `{}` metadata missing `telemetry_source`; rerun the capture with --telemetry-json so adoption evidence records the OTLP source (or omit --require-telemetry-source)",
                        path.display()
                    )
                })?;
            let summary_label = stats
                .telemetry_source
                .as_deref()
                .ok_or_else(|| {
                    format!(
                        "summary `{}` missing `telemetry_source`; rerun the capture so scoreboard `{}` and summary reference the same OTLP source",
                        summary_path.display(),
                        path.display()
                    )
                })?;
            if summary_label != scoreboard_label {
                return Err(format!(
                    "summary `{}` telemetry_source=`{summary_label}` does not match scoreboard `{}` metadata `{scoreboard_label}`; rerun the capture with matching telemetry labels",
                    summary_path.display(),
                    path.display()
                )
                .into());
            }
        } else if let Some(summary_label) = stats.telemetry_source.as_deref() {
            match scoreboard_telemetry_label.as_deref() {
                Some(scoreboard_label) if summary_label != scoreboard_label => {
                    return Err(format!(
                        "summary `{}` telemetry_source=`{summary_label}` does not match scoreboard `{}` metadata `{scoreboard_label}`; rerun the capture with matching telemetry labels",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
                Some(_) => {}
                None => {
                    return Err(format!(
                        "summary `{}` records telemetry_source=`{summary_label}` but scoreboard `{}` metadata.telemetry_source is missing; rerun the capture so both artefacts include the same OTLP label",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
            }
        }
        let scoreboard_region = meta
            .telemetry_region
            .as_deref()
            .and_then(|value| non_empty_trimmed(Some(value)));
        if options.require_telemetry_region && scoreboard_region.is_none() {
            return Err(format!(
                "scoreboard `{}` metadata missing `telemetry_region`; rerun the capture with --telemetry-region=<label> (or omit --require-telemetry-region)",
                path.display()
            )
            .into());
        }
        if let Some(region) = scoreboard_region.as_deref() {
            match stats.telemetry_region.as_deref() {
                Some(summary_region) if summary_region == region => {}
                Some(summary_region) => {
                    return Err(format!(
                        "summary `{}` telemetry_region=`{summary_region}` does not match scoreboard `{}` metadata `{region}`; rerun the capture so the region labels stay in sync",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
                None => {
                    return Err(format!(
                        "summary `{}` is missing `telemetry_region` but scoreboard `{}` metadata records `{region}`; rerun the capture so both artefacts advertise the region label",
                        summary_path.display(),
                        path.display()
                    )
                    .into());
                }
            }
        } else if stats.telemetry_region.is_some() {
            return Err(format!(
                "summary `{}` records telemetry_region but scoreboard `{}` metadata.telemetry_region is missing; rerun the capture so both artefacts include the same region label",
                summary_path.display(),
                path.display()
            )
            .into());
        }
        if options.require_telemetry_region && stats.telemetry_region.is_none() {
            return Err(format!(
                "summary `{}` missing `telemetry_region`; rerun the capture with --telemetry-region=<label> so scoreboard `{}` and summary align (or omit --require-telemetry-region)",
                summary_path.display(),
                path.display()
            )
            .into());
        }
        let mut summary_override_label: Option<&str> = None;
        if stats.transport_policy_override {
            let override_label = stats.transport_policy_override_label.as_deref().ok_or_else(
                || {
                    format!(
                        "summary `{}` sets transport_policy_override=true but transport_policy_override_label is missing; rerun the capture with the latest CLI/SDK so override evidence is preserved",
                        summary_path.display()
                    )
                },
            )?;
            if override_label != stats.transport_policy {
                return Err(format!(
                    "summary `{}` transport_policy_override_label=`{}` does not match transport_policy=`{}`; rerun the capture so override evidence stays consistent",
                    summary_path.display(),
                    override_label,
                    stats.transport_policy
                )
                .into());
            }
            summary_override_label = Some(override_label);
        }
        let scoreboard_transport_policy =
            meta.transport_policy.as_deref().ok_or_else(|| {
                format!(
                    "scoreboard `{}` metadata missing `transport_policy`; rerun the capture with --use-scoreboard from the latest CLI/SDK so policy evidence is persisted",
                    path.display()
                )
            })?;
        if scoreboard_transport_policy != stats.transport_policy {
            return Err(format!(
                "scoreboard `{}` metadata.transport_policy=`{}` does not match summary transport_policy=`{}`; rerun the capture so policy evidence stays consistent",
                path.display(),
                scoreboard_transport_policy,
                stats.transport_policy
            )
            .into());
        }
        let scoreboard_override = meta.transport_policy_override.ok_or_else(|| {
            format!(
                "scoreboard `{}` metadata missing `transport_policy_override`; rerun the capture with the latest CLI/SDK so override evidence is preserved",
                path.display()
            )
        })?;
        if scoreboard_override != stats.transport_policy_override {
            return Err(format!(
                "scoreboard `{}` metadata.transport_policy_override={} does not match summary transport_policy_override={}; rerun the capture so policy evidence stays consistent",
                path.display(),
                scoreboard_override,
                stats.transport_policy_override
            )
            .into());
        }
        if scoreboard_override {
            let scoreboard_label = meta
                .transport_policy_override_label
                .as_deref()
                .ok_or_else(|| {
                    format!(
                        "scoreboard `{}` metadata.transport_policy_override_label missing; rerun the capture so override evidence stays consistent",
                        path.display()
                    )
                })?;
            let summary_label = summary_override_label.ok_or_else(|| {
                format!(
                    "summary `{}` missing transport_policy_override_label despite override flag; rerun the capture with the latest CLI/SDK",
                    summary_path.display()
                )
            })?;
            if scoreboard_label != summary_label {
                return Err(format!(
                    "scoreboard `{}` metadata.transport_policy_override_label=`{}` does not match summary override label `{}`; rerun the capture so override evidence stays consistent",
                    path.display(),
                    scoreboard_label,
                    summary_label
                )
                .into());
            }
        }
        let transport_is_direct_only = is_direct_only_label(&stats.transport_policy);
        if options.require_direct_only && !transport_is_direct_only {
            return Err(format!(
                "summary `{}` transport_policy=`{}` but --require-direct-only was supplied; rerun the capture with --transport-policy=direct-only (or omit the flag when running the standard SoraNet-first posture)",
                summary_path.display(),
                stats.transport_policy
            )
            .into());
        }
        if transport_is_direct_only {
            if options.allow_single_source_fallback {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "summary `{}` transport_policy=`{}` indicates a direct-only fallback; rerun with --allow-single-source when downgrades are intentional",
                    summary_path.display(),
                    stats.transport_policy
                )
                .into());
            }
        }
        if direct != stats.provider_count {
            return Err(format!(
                "summary `{}` recorded provider_count={} but scoreboard metadata reported {}; rerun the capture so counts stay in sync",
                summary_path.display(),
                stats.provider_count,
                direct
            )
            .into());
        }
        if gateway != stats.gateway_provider_count {
            return Err(format!(
                "summary `{}` recorded gateway_provider_count={} but scoreboard metadata reported {}; rerun the capture so counts stay in sync",
                summary_path.display(),
                stats.gateway_provider_count,
                gateway
            )
            .into());
        }
        if provider_mix != stats.provider_mix {
            return Err(format!(
                "summary `{}` recorded provider_mix=`{}` but scoreboard metadata reported `{provider_mix}`; rerun the capture so mix labels remain consistent",
                summary_path.display(),
                stats.provider_mix
            )
            .into());
        }
        if let Some((expected_manifest_id, expected_manifest_cid)) =
            expected_gateway_manifest.as_ref()
        {
            let summary_manifest_id = non_empty_trimmed(stats.manifest_id.as_deref()).ok_or_else(|| {
                format!(
                    "summary `{}` missing `manifest_id` while gateway providers were captured; rerun the capture with the latest CLI/SDK so manifest digests are included",
                    summary_path.display()
                )
            })?;
            let summary_manifest_cid = non_empty_trimmed(stats.manifest_cid.as_deref()).ok_or_else(|| {
                format!(
                    "summary `{}` missing `manifest_cid` while gateway providers were captured; rerun the capture with the latest CLI/SDK so manifest digests are included",
                    summary_path.display()
                )
            })?;
            if &summary_manifest_id != expected_manifest_id {
                return Err(format!(
                    "summary `{}` manifest_id=`{}` does not match scoreboard metadata `{}`; ensure the same manifest envelope is referenced in both artefacts",
                    summary_path.display(),
                    summary_manifest_id,
                    expected_manifest_id
                )
                .into());
            }
            if &summary_manifest_cid != expected_manifest_cid {
                return Err(format!(
                    "summary `{}` manifest_cid=`{}` does not match scoreboard metadata `{}`; ensure the same manifest envelope is referenced in both artefacts",
                    summary_path.display(),
                    summary_manifest_cid,
                    expected_manifest_cid
                )
                .into());
            }
            if !stats.gateway_manifest_provided {
                return Err(format!(
                    "summary `{}` recorded gateway providers but gateway_manifest_provided=false; capture evidence with --gateway-manifest-envelope so manifest presence is reflected in the adoption artefacts",
                    summary_path.display()
                )
                .into());
            }
        } else if stats.gateway_manifest_provided {
            return Err(format!(
                "summary `{}` sets gateway_manifest_provided=true without gateway metadata; rerun the capture without gateway manifest flags or include gateway provider evidence",
                summary_path.display()
            )
            .into());
        }
        let active = stats.active_providers.len();
        if active < options.min_eligible_providers {
            if options.allow_single_source_fallback && active == 1 {
                single_source_override_used = true;
            } else {
                return Err(format!(
                    "summary `{}` recorded {active} provider(s) with successful chunk fetches; require at least {}",
                    summary_path.display(),
                    options.min_eligible_providers
                )
                .into());
            }
        }
        let mut referenced_providers = stats.chunk_receipt_providers.clone();
        referenced_providers.extend(stats.provider_report_providers.iter().cloned());
        let mut missing: Vec<String> = referenced_providers
            .into_iter()
            .filter(|provider| !scoreboard_providers.contains(provider))
            .collect();
        if !missing.is_empty() {
            missing.sort();
            return Err(format!(
                "summary `{}` references provider(s) not present in scoreboard `{}`: {}",
                summary_path.display(),
                path.display(),
                missing.join(", ")
            )
            .into());
        }

        evaluated += 1;
        reports.push(ScoreboardReport {
            scoreboard_path: path.display().to_string(),
            summary_path: summary_path.display().to_string(),
            provider_count: scoreboard_providers.len(),
            eligible_providers: eligible,
            active_providers: active,
            min_required: options.min_eligible_providers,
            weight_sum,
            summary_provider_count: stats.provider_count,
            summary_gateway_provider_count: stats.gateway_provider_count,
            summary_provider_mix: stats.provider_mix.clone(),
            summary_transport_policy: stats.transport_policy.clone(),
            summary_transport_policy_override: stats.transport_policy_override,
            summary_transport_policy_override_label: stats.transport_policy_override_label.clone(),
            metadata,
        });
    }

    if evaluated == 0 {
        return Err("no scoreboards evaluated; provide --scoreboard <path>".into());
    }

    Ok(AdoptionCheckReport {
        scoreboard_reports: reports,
        total_evaluated: evaluated,
        min_providers_required: options.min_eligible_providers,
        single_source_override_used,
        implicit_metadata_override_used,
    })
}

fn default_adoption_scoreboard_path() -> PathBuf {
    crate::workspace_root()
        .join("artifacts")
        .join("sorafs_orchestrator")
        .join("latest")
        .join("scoreboard.json")
}

const WEIGHT_EPSILON: f64 = 1e-9;

pub fn run_scoreboard_diff(
    options: ScoreboardDiffOptions,
) -> Result<ScoreboardDiffReport, Box<dyn Error>> {
    if options.threshold_percent < 0.0 {
        return Err("threshold-percent must be non-negative".into());
    }

    let threshold_fraction = (options.threshold_percent / 100.0).abs();
    let previous = load_scoreboard_weights(&options.previous_scoreboard)?;
    let current = load_scoreboard_weights(&options.current_scoreboard)?;
    let mut providers: BTreeSet<String> = BTreeSet::new();
    providers.extend(previous.keys().cloned());
    providers.extend(current.keys().cloned());

    let mut deltas: Vec<ProviderWeightDelta> = Vec::new();
    for provider in &providers {
        let previous_weight = *previous.get(provider).unwrap_or(&0.0);
        let current_weight = *current.get(provider).unwrap_or(&0.0);
        if approx_equal(previous_weight, current_weight) {
            continue;
        }
        let delta = current_weight - previous_weight;
        deltas.push(ProviderWeightDelta {
            provider_id: provider.clone(),
            previous_weight,
            current_weight,
            delta,
            delta_percent: delta * 100.0,
            was_added: approx_equal(previous_weight, 0.0) && !approx_equal(current_weight, 0.0),
            was_removed: approx_equal(current_weight, 0.0) && !approx_equal(previous_weight, 0.0),
            exceeds_threshold: delta.abs() > threshold_fraction + WEIGHT_EPSILON,
        });
    }

    deltas.sort_by(|left, right| {
        right
            .delta
            .abs()
            .partial_cmp(&left.delta.abs())
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.provider_id.cmp(&right.provider_id))
    });

    Ok(ScoreboardDiffReport {
        previous_scoreboard: options.previous_scoreboard.display().to_string(),
        current_scoreboard: options.current_scoreboard.display().to_string(),
        threshold_percent: options.threshold_percent,
        total_providers: providers.len(),
        changed_providers: deltas,
    })
}

pub fn print_scoreboard_diff(report: &ScoreboardDiffReport) {
    println!("sorafs scoreboard diff:");
    println!("  previous: {}", report.previous_scoreboard);
    println!("  current : {}", report.current_scoreboard);
    if report.changed_providers.is_empty() {
        println!(
            "  no eligible provider weight changes detected across {} provider(s)",
            report.total_providers
        );
        return;
    }
    let exceed_count = report
        .changed_providers
        .iter()
        .filter(|entry| entry.exceeds_threshold)
        .count();
    println!(
        "  {} provider(s) changed; {} exceed the {:.2}% threshold ({} compared)",
        report.changed_providers.len(),
        exceed_count,
        report.threshold_percent,
        report.total_providers
    );
    let displayed = report.changed_providers.len().min(10);
    println!("  Provider weight deltas (showing top {displayed} by |Δ|):");
    println!(
        "  {:<24} {:>11} {:>11} {:>11} {:>12}",
        "Provider", "Previous", "Current", "Δ", "Flags"
    );
    for entry in report.changed_providers.iter().take(displayed) {
        let mut flags: Vec<&str> = Vec::new();
        if entry.was_added {
            flags.push("added");
        }
        if entry.was_removed {
            flags.push("removed");
        }
        if entry.exceeds_threshold {
            flags.push("exceeds");
        }
        let flag_text = if flags.is_empty() {
            "-".to_string()
        } else {
            flags.join("|")
        };
        println!(
            "  {:<24} {:>11.6} {:>11.6} {:>11.6} {:>12}",
            entry.provider_id, entry.previous_weight, entry.current_weight, entry.delta, flag_text
        );
    }
}

fn load_scoreboard_weights(path: &Path) -> Result<HashMap<String, f64>, Box<dyn Error>> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read scoreboard `{}`: {err}", path.display()))?;
    let value: Value = json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse scoreboard JSON `{}`: {err}",
            path.display()
        )
    })?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("scoreboard `{}` missing `entries` array", path.display()))?;

    let mut weights = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if !scoreboard_entry_is_eligible(entry) {
            continue;
        }
        let provider = entry
            .get("provider_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "scoreboard `{}` entry {index} missing `provider_id` string",
                    path.display()
                )
            })?;
        let weight = entry
            .get("normalised_weight")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                format!(
                    "scoreboard `{}` entry {index} missing `normalised_weight` number",
                    path.display()
                )
            })?;
        weights.insert(provider.to_string(), weight);
    }

    if weights.is_empty() {
        return Err(format!(
            "scoreboard `{}` has no eligible providers to compare",
            path.display()
        )
        .into());
    }
    Ok(weights)
}

fn approx_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= WEIGHT_EPSILON
}

fn default_summary_path_for_scoreboard(scoreboard_path: &Path) -> PathBuf {
    scoreboard_path
        .parent()
        .map(|parent| parent.join("summary.json"))
        .unwrap_or_else(|| PathBuf::from("summary.json"))
}

fn scoreboard_entry_is_eligible(entry: &Value) -> bool {
    match entry.get("eligibility") {
        Some(Value::String(label)) => label == "eligible",
        Some(Value::Object(obj)) => obj.get("status").and_then(Value::as_str) == Some("eligible"),
        _ => false,
    }
}

struct SummaryStats {
    active_providers: HashSet<String>,
    chunk_receipt_providers: HashSet<String>,
    provider_report_providers: HashSet<String>,
    manifest_id: Option<String>,
    manifest_cid: Option<String>,
    gateway_manifest_provided: bool,
    provider_count: u64,
    gateway_provider_count: u64,
    provider_mix: String,
    transport_policy: String,
    transport_policy_override: bool,
    transport_policy_override_label: Option<String>,
    telemetry_source: Option<String>,
    telemetry_region: Option<String>,
}

fn collect_summary_stats(summary: &Value, path: &Path) -> Result<SummaryStats, String> {
    let chunk_count = summary
        .get("chunk_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("summary `{}` missing `chunk_count` number", path.display()))?;
    let manifest_id = summary
        .get("manifest_id")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let manifest_cid = summary
        .get("manifest_cid")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let receipts = summary
        .get("chunk_receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `chunk_receipts` array",
                path.display()
            )
        })?;
    if receipts.len() != chunk_count as usize {
        return Err(format!(
            "summary `{}` reports chunk_count={} but chunk_receipts has {} entries",
            path.display(),
            chunk_count,
            receipts.len()
        ));
    }
    let mut receipt_providers = HashSet::new();
    for (idx, receipt) in receipts.iter().enumerate() {
        let provider = receipt
            .get("provider")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "summary `{}` chunk_receipts[{idx}] missing `provider` string",
                    path.display()
                )
            })?;
        receipt_providers.insert(provider.to_string());
    }

    let reports = summary
        .get("provider_reports")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `provider_reports` array",
                path.display()
            )
        })?;
    let mut provider_report_providers = HashSet::new();
    let mut provider_success_sum = 0u64;
    let mut active_providers = HashSet::new();
    for (idx, report) in reports.iter().enumerate() {
        let provider = report
            .get("provider")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "summary `{}` provider_reports[{idx}] missing `provider` string",
                    path.display()
                )
            })?;
        provider_report_providers.insert(provider.to_string());
        let successes = report
            .get("successes")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "summary `{}` provider_reports[{idx}] missing `successes` number",
                    path.display()
                )
            })?;
        provider_success_sum = provider_success_sum.checked_add(successes).ok_or_else(|| {
            format!(
                "summary `{}` provider success count overflowed u64",
                path.display()
            )
        })?;
        if successes > 0 {
            active_providers.insert(provider.to_string());
        }
    }

    let provider_success_total = summary
        .get("provider_success_total")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `provider_success_total` number",
                path.display()
            )
        })?;
    if provider_success_total != provider_success_sum {
        return Err(format!(
            "summary `{}` reports provider_success_total={} but provider_reports sum to {}",
            path.display(),
            provider_success_total,
            provider_success_sum
        ));
    }
    if provider_success_total != chunk_count {
        return Err(format!(
            "summary `{}` reports provider_success_total={} but chunk_count={}",
            path.display(),
            provider_success_total,
            chunk_count
        ));
    }

    let gateway_manifest_provided = summary
        .get("gateway_manifest_provided")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `gateway_manifest_provided` boolean",
                path.display()
            )
        })?;
    let provider_count = summary
        .get("provider_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `provider_count` number",
                path.display()
            )
        })?;
    let gateway_provider_count = summary
        .get("gateway_provider_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `gateway_provider_count` number",
                path.display()
            )
        })?;
    let provider_mix = summary
        .get("provider_mix")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("summary `{}` missing `provider_mix` string", path.display()))?;
    let expected_mix = provider_mix_label_from_counts(provider_count, gateway_provider_count);
    if provider_mix != expected_mix {
        return Err(format!(
            "summary `{}` provider_mix=`{provider_mix}` does not match derived `{expected_mix}` for provider_count={} gateway_provider_count={}",
            path.display(),
            provider_count,
            gateway_provider_count
        ));
    }
    let transport_policy = summary
        .get("transport_policy")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `transport_policy` string",
                path.display()
            )
        })?
        .to_string();
    let transport_policy_override = summary
        .get("transport_policy_override")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "summary `{}` missing `transport_policy_override` boolean",
                path.display()
            )
        })?;
    let transport_policy_override_label = summary
        .get("transport_policy_override_label")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let telemetry_source = summary
        .get("telemetry_source")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let telemetry_region = summary
        .get("telemetry_region")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(SummaryStats {
        active_providers,
        chunk_receipt_providers: receipt_providers,
        provider_report_providers,
        manifest_id,
        manifest_cid,
        gateway_manifest_provided,
        provider_count,
        gateway_provider_count,
        provider_mix: provider_mix.to_string(),
        transport_policy,
        transport_policy_override,
        transport_policy_override_label,
        telemetry_source,
        telemetry_region,
    })
}

fn parse_scoreboard_metadata(
    value: &Value,
    path: &Path,
) -> Result<Option<ScoreboardMetadata>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let obj = value.as_object().ok_or_else(|| {
        format!(
            "scoreboard `{}` metadata must be a JSON object",
            path.display()
        )
    })?;
    let metadata = ScoreboardMetadata {
        orchestrator_version: value_as_string(obj.get("version")),
        use_scoreboard: value_as_bool(obj.get("use_scoreboard")),
        allow_implicit_metadata: value_as_bool(obj.get("allow_implicit_metadata")),
        provider_count: value_as_u64(obj.get("provider_count")),
        gateway_provider_count: value_as_u64(obj.get("gateway_provider_count")),
        provider_mix: value_as_string(obj.get("provider_mix")),
        max_parallel: value_as_u64(obj.get("max_parallel")),
        max_peers: value_as_u64(obj.get("max_peers")),
        retry_budget: value_as_u64(obj.get("retry_budget")),
        provider_failure_threshold: value_as_u64(obj.get("provider_failure_threshold")),
        assume_now: value_as_u64(obj.get("assume_now")),
        telemetry_source: value_as_string(obj.get("telemetry_source")),
        telemetry_region: value_as_string(obj.get("telemetry_region")),
        gateway_manifest_id: value_as_string(obj.get("gateway_manifest_id")),
        gateway_manifest_cid: value_as_string(obj.get("gateway_manifest_cid")),
        gateway_manifest_provided: value_as_bool(obj.get("gateway_manifest_provided")),
        transport_policy: value_as_string(obj.get("transport_policy")),
        transport_policy_override: value_as_bool(obj.get("transport_policy_override")),
        transport_policy_override_label: value_as_string(
            obj.get("transport_policy_override_label"),
        ),
        anonymity_policy: value_as_string(obj.get("anonymity_policy")),
        anonymity_policy_override: value_as_bool(obj.get("anonymity_policy_override")),
        anonymity_policy_override_label: value_as_string(
            obj.get("anonymity_policy_override_label"),
        ),
    };
    Ok(Some(metadata))
}

fn value_as_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|v| match v {
        Value::Number(num) => num
            .as_u64()
            .or_else(|| num.as_i64().and_then(|signed| u64::try_from(signed).ok())),
        _ => None,
    })
}

fn value_as_bool(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_bool)
}

fn value_as_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(|s| s.to_string())
}

fn non_empty_trimmed(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn metadata_direct_transport_label(metadata: &ScoreboardMetadata) -> Option<&str> {
    metadata
        .transport_policy
        .as_deref()
        .filter(|label| is_direct_only_label(label))
        .or_else(|| {
            metadata
                .transport_policy_override_label
                .as_deref()
                .filter(|label| is_direct_only_label(label))
        })
}

fn is_direct_only_label(label: &str) -> bool {
    let normalised = label.trim().to_ascii_lowercase();
    normalised == "direct-only" || normalised == "direct_only"
}

fn provider_mix_label_from_counts(direct: u64, gateway: u64) -> &'static str {
    match (direct > 0, gateway > 0) {
        (true, true) => "mixed",
        (true, false) => "direct-only",
        (false, true) => "gateway-only",
        (false, false) => "none",
    }
}

fn metadata_single_source_hint(metadata: &ScoreboardMetadata) -> Option<(&'static str, u64)> {
    if let Some(value) = metadata.max_parallel
        && value <= 1
    {
        return Some(("max_parallel", value));
    }
    if let Some(value) = metadata.max_peers
        && value <= 1
    {
        return Some(("max_peers", value));
    }
    let direct = metadata.provider_count.unwrap_or(0);
    let gateway = metadata.gateway_provider_count.unwrap_or(0);
    if direct == 0 && gateway == 0 {
        return None;
    }
    let total = direct.saturating_add(gateway);
    if total <= 1 {
        if direct > 0 && gateway == 0 {
            return Some(("provider_count", direct));
        }
        if gateway > 0 && direct == 0 {
            return Some(("gateway_provider_count", gateway));
        }
        return Some(("total_provider_count", total));
    }
    None
}

pub fn run_burn_in_check(options: BurnInCheckOptions) -> Result<BurnInSummary, Box<dyn Error>> {
    if options.log_paths.is_empty() {
        return Err("sorafs-burn-in-check requires at least one --log <path>".into());
    }

    let mut accumulator = BurnInAccumulator::default();
    for path in &options.log_paths {
        let contents = fs::read_to_string(path)?;
        for line in contents.lines() {
            if let Some(event) = parse_telemetry_line(line) {
                accumulator.observe(event);
            }
        }
    }

    let summary = accumulator.finalize(&options)?;
    Ok(summary)
}

pub fn run_taikai_cache_bundle(options: TaikaiCacheBundleOptions) -> Result<(), Box<dyn Error>> {
    let mut profile_paths = if options.profile_paths.is_empty() {
        discover_taikai_cache_profiles(default_taikai_cache_profile_dir())?
    } else {
        options.profile_paths
    };
    profile_paths.sort();
    profile_paths.dedup();
    if profile_paths.is_empty() {
        return Err("no Taikai cache profiles were specified or discovered".into());
    }

    fs::create_dir_all(&options.output_root).map_err(|err| {
        format!(
            "failed to create Taikai cache output directory `{}`: {err}",
            options.output_root.display()
        )
    })?;

    let mut index_entries: Vec<TaikaiCacheIndexEntry> = Vec::new();
    for profile_path in profile_paths {
        let document = TaikaiCacheProfileDocument::load(&profile_path)?;
        let profile = document.into_profile()?;
        profile.validate().map_err(|err| {
            format!(
                "Taikai cache profile `{}` failed validation: {err}",
                profile.profile_id
            )
        })?;
        ensure_safe_profile_id(&profile.profile_id)?;

        let profile_dir = options.output_root.join(&profile.profile_id);
        fs::create_dir_all(&profile_dir).map_err(|err| {
            format!(
                "failed to create profile output directory `{}`: {err}",
                profile_dir.display()
            )
        })?;

        let profile_json_bytes = json::to_vec_pretty(&profile)?;
        let profile_json_path = profile_dir.join("profile.json");
        fs::write(&profile_json_path, &profile_json_bytes).map_err(|err| {
            format!(
                "failed to write profile JSON `{}`: {err}",
                profile_json_path.display()
            )
        })?;

        let cache_json_bytes = json::to_vec_pretty(&profile.config)?;
        let cache_json_path = profile_dir.join("cache_config.json");
        fs::write(&cache_json_path, &cache_json_bytes).map_err(|err| {
            format!(
                "failed to write cache JSON `{}`: {err}",
                cache_json_path.display()
            )
        })?;

        let artifact_bytes = to_bytes(&profile)?;
        let artifact_path = profile_dir.join("profile.taikai_cache_profile.to");
        fs::write(&artifact_path, &artifact_bytes).map_err(|err| {
            format!(
                "failed to write Norito profile `{}`: {err}",
                artifact_path.display()
            )
        })?;

        let artifact_digest = ArtifactDigest::from_bytes(&artifact_bytes);
        let profile_json_digest = ArtifactDigest::from_bytes(&profile_json_bytes);
        let cache_json_digest = ArtifactDigest::from_bytes(&cache_json_bytes);

        let manifest_artifact = BundleArtifact::with_path(
            rel_posix_path(&profile_dir, &artifact_path)?,
            &artifact_digest,
        );
        let manifest_profile_json = BundleArtifact::with_path(
            rel_posix_path(&profile_dir, &profile_json_path)?,
            &profile_json_digest,
        );
        let manifest_cache_json = BundleArtifact::with_path(
            rel_posix_path(&profile_dir, &cache_json_path)?,
            &cache_json_digest,
        );

        let manifest = TaikaiCacheBundleManifest {
            version: 1,
            profile_id: profile.profile_id.clone(),
            rollout_stage: profile.rollout_stage.as_str().to_string(),
            generated_unix_ms: unix_timestamp_ms(),
            artifact: manifest_artifact,
            profile_json: manifest_profile_json,
            cache_json: manifest_cache_json,
            annotations: profile.annotations.clone(),
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        let manifest_path = profile_dir.join("profile.manifest.json");
        fs::write(&manifest_path, &manifest_bytes).map_err(|err| {
            format!(
                "failed to write manifest `{}`: {err}",
                manifest_path.display()
            )
        })?;

        index_entries.push(TaikaiCacheIndexEntry {
            profile_id: profile.profile_id.clone(),
            rollout_stage: profile.rollout_stage.as_str().to_string(),
            artifact: BundleArtifact::with_path(
                rel_posix_path(&options.output_root, &artifact_path)?,
                &artifact_digest,
            ),
            profile_json: BundleArtifact::with_path(
                rel_posix_path(&options.output_root, &profile_json_path)?,
                &profile_json_digest,
            ),
            cache_json: BundleArtifact::with_path(
                rel_posix_path(&options.output_root, &cache_json_path)?,
                &cache_json_digest,
            ),
            manifest_path: rel_posix_path(&options.output_root, &manifest_path)?,
            annotations: profile.annotations.clone(),
        });
    }

    let index = TaikaiCacheBundleIndex {
        generated_unix_ms: unix_timestamp_ms(),
        profiles: index_entries,
    };
    let index_bytes = serde_json::to_vec_pretty(&index)?;
    let index_path = options.output_root.join("index.json");
    fs::write(&index_path, &index_bytes).map_err(|err| {
        format!(
            "failed to write Taikai cache index `{}`: {err}",
            index_path.display()
        )
    })?;

    println!(
        "[taikai-cache] bundled {} profile(s) into {}",
        index.profiles.len(),
        options.output_root.display()
    );

    Ok(())
}

pub fn resolve_taikai_cache_profile_path(spec: &str) -> Result<PathBuf, Box<dyn Error>> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err("profile label must not be empty".into());
    }
    let looks_like_path = trimmed.contains(['/', '\\']) || trimmed.ends_with(".json");
    if looks_like_path {
        return normalize_path(Path::new(trimmed));
    }
    Ok(default_taikai_cache_profile_dir().join(format!("{trimmed}.json")))
}

pub fn default_taikai_cache_output_root() -> PathBuf {
    workspace_root().join("artifacts/taikai_cache")
}

fn default_taikai_cache_profile_dir() -> PathBuf {
    workspace_root().join("configs/taikai_cache/profiles")
}

fn discover_taikai_cache_profiles(dir: PathBuf) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&dir).map_err(|err| {
        format!(
            "failed to read Taikai cache profile directory `{}`: {err}",
            dir.display()
        )
    })?;
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            entries.push(path);
        }
    }
    if entries.is_empty() {
        return Err(format!(
            "no Taikai cache profiles found under `{}`; add JSON files or pass --profile",
            dir.display()
        )
        .into());
    }
    Ok(entries)
}

fn ensure_safe_profile_id(id: &str) -> Result<(), Box<dyn Error>> {
    if id.trim().is_empty() {
        return Err("Taikai cache profile id must not be empty".into());
    }
    if id.contains('/') || id.contains('\\') {
        return Err(format!("profile id `{id}` must not contain path separators").into());
    }
    Ok(())
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn rel_posix_path(base: &Path, path: &Path) -> Result<String, Box<dyn Error>> {
    let rel = path.strip_prefix(base).map_err(|err| {
        format!(
            "path `{}` is not under `{}`: {err}",
            path.display(),
            base.display()
        )
    })?;
    let mut buffer = String::new();
    for (index, component) in rel.components().enumerate() {
        if index > 0 {
            buffer.push('/');
        }
        buffer.push_str(component.as_os_str().to_str().ok_or_else(|| {
            format!(
                "component `{component:?}` of `{}` is not valid UTF-8",
                rel.display()
            )
        })?);
    }
    Ok(buffer)
}

#[derive(Clone, Debug, Serialize)]
struct TaikaiCacheBundleManifest {
    version: u8,
    profile_id: String,
    rollout_stage: String,
    generated_unix_ms: u64,
    artifact: BundleArtifact,
    profile_json: BundleArtifact,
    cache_json: BundleArtifact,
    annotations: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
struct TaikaiCacheBundleIndex {
    generated_unix_ms: u64,
    profiles: Vec<TaikaiCacheIndexEntry>,
}

#[derive(Clone, Debug, Serialize)]
struct TaikaiCacheIndexEntry {
    profile_id: String,
    rollout_stage: String,
    artifact: BundleArtifact,
    profile_json: BundleArtifact,
    cache_json: BundleArtifact,
    manifest_path: String,
    annotations: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
struct BundleArtifact {
    path: String,
    bytes: u64,
    sha256_hex: String,
    blake3_hex: String,
}

impl BundleArtifact {
    fn with_path(path: String, digest: &ArtifactDigest) -> Self {
        Self {
            path,
            bytes: digest.bytes,
            sha256_hex: digest.sha256_hex.clone(),
            blake3_hex: digest.blake3_hex.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct ArtifactDigest {
    bytes: u64,
    sha256_hex: String,
    blake3_hex: String,
}

impl ArtifactDigest {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut sha = Sha256::new();
        sha.update(bytes);
        let sha = sha.finalize();
        let blake = blake3_hash(bytes);
        Self {
            bytes: bytes.len() as u64,
            sha256_hex: hex::encode(sha),
            blake3_hex: blake.to_hex().to_string(),
        }
    }
}

#[derive(Debug, JsonDeserialize)]
struct TaikaiCacheProfileDocument {
    #[norito(default)]
    profile_id: Option<String>,
    rollout_stage: String,
    description: String,
    #[norito(default)]
    annotations: BTreeMap<String, String>,
    #[norito(rename = "taikai_cache")]
    config: TaikaiCacheConfigDocument,
    #[norito(skip)]
    source_path: PathBuf,
}

impl TaikaiCacheProfileDocument {
    fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = fs::read(path).map_err(|err| {
            format!(
                "failed to read Taikai cache profile `{}`: {err}",
                path.display()
            )
        })?;
        let mut doc: TaikaiCacheProfileDocument = json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse Taikai cache profile `{}`: {err}",
                path.display()
            )
        })?;
        if doc
            .profile_id
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            let derived = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    format!(
                        "profile `{}` is missing profile_id and its filename is not valid UTF-8",
                        path.display()
                    )
                })?;
            doc.profile_id = Some(derived.to_string());
        }
        doc.source_path = path.to_path_buf();
        Ok(doc)
    }

    fn into_profile(self) -> Result<TaikaiCacheProfileV1, Box<dyn Error>> {
        let profile_id = self.profile_id.unwrap_or_default().trim().to_string();
        if profile_id.is_empty() {
            return Err(format!(
                "profile `{}` did not supply a valid profile_id",
                self.source_path.display()
            )
            .into());
        }
        let rollout_stage =
            TaikaiCacheRolloutStage::from_str(self.rollout_stage.trim()).map_err(|err| {
                format!(
                    "profile `{}` specifies an unknown rollout stage: {err}",
                    profile_id
                )
            })?;
        Ok(TaikaiCacheProfileV1 {
            profile_id,
            rollout_stage,
            description: self.description,
            annotations: self.annotations,
            config: self.config.into(),
        })
    }
}

#[derive(Debug, JsonDeserialize)]
struct TaikaiCacheConfigDocument {
    hot_capacity_bytes: u64,
    hot_retention_secs: u64,
    warm_capacity_bytes: u64,
    warm_retention_secs: u64,
    cold_capacity_bytes: u64,
    cold_retention_secs: u64,
    qos: TaikaiCacheQosDocument,
}

impl From<TaikaiCacheConfigDocument> for TaikaiCacheConfigV1 {
    fn from(value: TaikaiCacheConfigDocument) -> Self {
        Self {
            hot_capacity_bytes: value.hot_capacity_bytes,
            hot_retention_secs: value.hot_retention_secs,
            warm_capacity_bytes: value.warm_capacity_bytes,
            warm_retention_secs: value.warm_retention_secs,
            cold_capacity_bytes: value.cold_capacity_bytes,
            cold_retention_secs: value.cold_retention_secs,
            qos: value.qos.into(),
        }
    }
}

#[derive(Debug, JsonDeserialize)]
struct TaikaiCacheQosDocument {
    priority_rate_bps: u64,
    standard_rate_bps: u64,
    bulk_rate_bps: u64,
    burst_multiplier: u32,
}

impl From<TaikaiCacheQosDocument> for TaikaiCacheQosConfigV1 {
    fn from(value: TaikaiCacheQosDocument) -> Self {
        Self {
            priority_rate_bps: value.priority_rate_bps,
            standard_rate_bps: value.standard_rate_bps,
            bulk_rate_bps: value.bulk_rate_bps,
            burst_multiplier: value.burst_multiplier,
        }
    }
}

struct ParsedTelemetryLine {
    timestamp: OffsetDateTime,
    target: String,
    fields: HashMap<String, String>,
}

fn parse_telemetry_line(line: &str) -> Option<ParsedTelemetryLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let mut parts = trimmed.splitn(3, ' ');
    let timestamp_raw = parts.next()?;
    let target_raw = parts.next()?;
    let rest = parts.next().unwrap_or_default();

    if !target_raw.starts_with("telemetry::") {
        return None;
    }
    let timestamp = OffsetDateTime::parse(timestamp_raw, &Rfc3339).ok()?;
    let target = target_raw.trim_start_matches("telemetry::").to_string();
    let mut fields = HashMap::new();
    for token in tokenize_field_pairs(rest) {
        if let Some((key, value)) = token.split_once('=') {
            let mut value = value.trim().to_string();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            fields.insert(key.trim().to_string(), value);
        }
    }

    Some(ParsedTelemetryLine {
        timestamp,
        target,
        fields,
    })
}

fn tokenize_field_pairs(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

struct BurnInAccumulator {
    first_timestamp: Option<OffsetDateTime>,
    last_timestamp: Option<OffsetDateTime>,
    fetches_total: u64,
    successful_fetches: u64,
    brownout_fetches: u64,
    stall_events: u64,
    stall_chunks: u64,
    pq_ratio_min: f64,
    pq_ratio_sum: f64,
    pq_ratio_samples: u64,
    failure_reasons: HashMap<String, u64>,
}

impl Default for BurnInAccumulator {
    fn default() -> Self {
        Self {
            first_timestamp: None,
            last_timestamp: None,
            fetches_total: 0,
            successful_fetches: 0,
            brownout_fetches: 0,
            stall_events: 0,
            stall_chunks: 0,
            pq_ratio_min: f64::INFINITY,
            pq_ratio_sum: 0.0,
            pq_ratio_samples: 0,
            failure_reasons: HashMap::new(),
        }
    }
}

impl BurnInAccumulator {
    fn observe(&mut self, event: ParsedTelemetryLine) {
        self.update_span(event.timestamp);
        match event.target.as_str() {
            "sorafs.fetch.lifecycle" => self.observe_lifecycle(&event.fields),
            "sorafs.fetch.error" => {
                let reason = event
                    .fields
                    .get("reason")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                *self.failure_reasons.entry(reason).or_insert(0) += 1;
            }
            "sorafs.fetch.stall" => {
                self.stall_events += 1;
            }
            _ => {}
        }
    }

    fn update_span(&mut self, timestamp: OffsetDateTime) {
        match self.first_timestamp {
            Some(existing) if timestamp < existing => self.first_timestamp = Some(timestamp),
            None => self.first_timestamp = Some(timestamp),
            _ => {}
        }
        match self.last_timestamp {
            Some(existing) if timestamp > existing => self.last_timestamp = Some(timestamp),
            None => self.last_timestamp = Some(timestamp),
            _ => {}
        }
    }

    fn observe_lifecycle(&mut self, fields: &HashMap<String, String>) {
        if let Some("complete") = fields.get("event").map(String::as_str) {
            self.fetches_total += 1;
            if fields.get("status").map(String::as_str) == Some("success") {
                self.successful_fetches += 1;
            }
            if fields.get("anonymity_outcome").map(String::as_str) == Some("brownout") {
                self.brownout_fetches += 1;
            }
            if let Some(value) = fields
                .get("stall_count")
                .and_then(|raw| raw.parse::<u64>().ok())
            {
                self.stall_chunks += value;
            }
            if let Some(value) = fields
                .get("anonymity_ratio")
                .and_then(|raw| raw.parse::<f64>().ok())
                && !value.is_nan()
            {
                if self.pq_ratio_min.is_infinite() || value < self.pq_ratio_min {
                    self.pq_ratio_min = value;
                }
                self.pq_ratio_sum += value;
                self.pq_ratio_samples += 1;
            }
        }
    }

    fn finalize(self, options: &BurnInCheckOptions) -> Result<BurnInSummary, Box<dyn Error>> {
        let first = self
            .first_timestamp
            .ok_or("no telemetry lifecycle events were parsed")?;
        let last = self
            .last_timestamp
            .ok_or("no telemetry lifecycle events were parsed")?;
        if last < first {
            return Err("telemetry timestamps are out of order".into());
        }

        let span = last - first;
        let required_days = i64::try_from(options.required_window_days)
            .map_err(|_| "required_window_days exceeds supported range")?;
        let required_window = TimeDuration::days(required_days);
        if span < required_window {
            let coverage_days = span.as_seconds_f64() / 86_400.0;
            return Err(format!(
                "telemetry window {:.2} days shorter than required {} days",
                coverage_days, options.required_window_days
            )
            .into());
        }

        if self.fetches_total == 0 {
            return Err("telemetry log did not include any completed fetches".into());
        }
        if options.min_fetches > 0 && self.fetches_total < options.min_fetches {
            return Err(format!(
                "telemetry log captured {} fetch(es); require at least {} during the burn-in window",
                self.fetches_total, options.min_fetches
            )
            .into());
        }
        if self.pq_ratio_samples == 0 {
            return Err("no anonymity_ratio fields were found in lifecycle telemetry".into());
        }
        let min_ratio = if self.pq_ratio_min.is_infinite() {
            0.0
        } else {
            self.pq_ratio_min
        };
        if min_ratio + f64::EPSILON < options.min_pq_ratio {
            return Err(format!(
                "minimum anonymity_ratio {:.3} fell below the required {:.3}",
                min_ratio, options.min_pq_ratio
            )
            .into());
        }
        let no_provider_errors = self
            .failure_reasons
            .get("no_healthy_providers")
            .copied()
            .unwrap_or(0);
        if no_provider_errors > options.max_no_provider_errors {
            return Err(format!(
                "encountered {no_provider_errors} no_healthy_providers errors (max allowed: {})",
                options.max_no_provider_errors
            )
            .into());
        }

        let coverage_days = span.as_seconds_f64() / 86_400.0;
        let brownout_ratio = self.brownout_fetches as f64 / self.fetches_total as f64;
        if brownout_ratio > options.max_brownout_ratio + f64::EPSILON {
            return Err(format!(
                "brownout ratio {:.3}% exceeded allowed {:.3}%",
                brownout_ratio * 100.0,
                options.max_brownout_ratio * 100.0
            )
            .into());
        }
        let stall_event_ratio = self.stall_events as f64 / self.fetches_total as f64;
        let stall_chunk_ratio = self.stall_chunks as f64 / self.fetches_total as f64;
        let avg_ratio = self.pq_ratio_sum / self.pq_ratio_samples as f64;
        let failure_reasons = self.failure_reasons.into_iter().collect::<BTreeMap<_, _>>();
        let summary = BurnInSummary {
            sources: options
                .log_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            coverage_days,
            first_timestamp: first.format(&Rfc3339)?,
            last_timestamp: last.format(&Rfc3339)?,
            fetches_total: self.fetches_total,
            successful_fetches: self.successful_fetches,
            brownout_fetches: self.brownout_fetches,
            brownout_ratio,
            stall_event_ratio,
            stall_events: self.stall_events,
            stall_chunks: self.stall_chunks,
            stall_chunk_ratio,
            pq_ratio_min: min_ratio,
            pq_ratio_avg: avg_ratio,
            failure_reasons,
        };
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, path::Path, time::Duration};

    use tempfile::tempdir;

    use super::*;

    const TEST_MANIFEST_ID: &str = "fixture-manifest";
    const TEST_MANIFEST_CID: &str = "fixture-cid";

    fn summary_with_providers_value(providers: &[(&str, u64)]) -> Value {
        let mut receipts: Vec<Value> = Vec::new();
        let mut chunk_index = 0u64;
        let mut direct_providers = HashSet::new();
        let mut gateway_providers = HashSet::new();
        for (provider, successes) in providers {
            if provider.starts_with("gateway-") {
                gateway_providers.insert(provider.to_string());
            } else {
                direct_providers.insert(provider.to_string());
            }
            for _ in 0..*successes {
                receipts.push(norito::json!({
                    "provider": provider,
                    "chunk_index": chunk_index,
                    "attempts": 1u64,
                    "bytes": 1u64,
                    "latency_ms": 0.0
                }));
                chunk_index += 1;
            }
        }
        let reports: Vec<Value> = providers
            .iter()
            .map(|(provider, successes)| {
                norito::json!({
                    "provider": provider,
                    "successes": successes,
                    "failures": 0u64,
                    "disabled": false
                })
            })
            .collect();
        let total_successes: u64 = providers.iter().map(|(_, successes)| *successes).sum();
        let provider_count =
            u64::try_from(direct_providers.len()).expect("convert direct provider count");
        let gateway_provider_count =
            u64::try_from(gateway_providers.len()).expect("convert gateway provider count");
        let provider_mix = provider_mix_label_from_counts(provider_count, gateway_provider_count);
        let gateway_manifest_provided = gateway_provider_count > 0;
        norito::json!({
            "manifest_id": TEST_MANIFEST_ID,
            "manifest_cid": TEST_MANIFEST_CID,
            "chunk_count": total_successes,
            "chunk_attempt_total": total_successes,
            "chunk_receipts": receipts,
            "provider_reports": reports,
            "provider_success_total": total_successes,
            "provider_count": provider_count,
            "gateway_provider_count": gateway_provider_count,
            "provider_mix": provider_mix,
            "gateway_manifest_provided": gateway_manifest_provided,
            "transport_policy": "soranet-first",
            "transport_policy_override": false,
            "transport_policy_override_label": null,
        })
    }

    fn write_summary_with_providers(path: &Path, providers: &[(&str, u64)]) {
        let summary = summary_with_providers_value(providers);
        fs::write(path, to_string_pretty(&summary).expect("render summary"))
            .expect("write summary");
    }

    fn write_summary_with_telemetry(
        path: &Path,
        providers: &[(&str, u64)],
        telemetry: &str,
        telemetry_region: Option<&str>,
    ) {
        let mut summary = summary_with_providers_value(providers);
        summary
            .as_object_mut()
            .expect("summary object")
            .insert("telemetry_source".into(), Value::from(telemetry));
        if let Some(region) = telemetry_region {
            summary
                .as_object_mut()
                .expect("summary object")
                .insert("telemetry_region".into(), Value::from(region));
        }
        fs::write(path, to_string_pretty(&summary).expect("render summary"))
            .expect("write summary");
    }

    fn write_scoreboard_with_weights(path: &Path, entries: &[(&str, f64)]) {
        let rows: Vec<Value> = entries
            .iter()
            .map(|(provider, weight)| {
                norito::json!({
                    "provider_id": provider,
                    "eligibility": "eligible",
                    "normalised_weight": weight,
                })
            })
            .collect();
        let scoreboard = norito::json!({ "entries": rows });
        fs::write(
            path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
    }

    #[test]
    fn normalize_tls_host_guards_whitespace() {
        assert_eq!(
            normalize_tls_host("Docs.Sora.GW.Sora.Name").expect("normalize lowercase"),
            "docs.sora.gw.sora.name"
        );
        assert!(normalize_tls_host("invalid host").is_err());
    }

    #[test]
    fn load_tls_hosts_from_fixture_payload() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("hosts.json");
        let payload = serde_json::json!({
            "san_hosts": [
                "Docs.Sora.GW.Sora.Name ",
                "*.GW.SORA.ID",
                "docs.sora.gw.sora.name"
            ]
        });
        fs::write(&path, serde_json::to_vec(&payload).unwrap()).expect("write fixture");
        let hosts = load_tls_hosts_from_file(&path).expect("loaded hosts");
        assert_eq!(
            hosts,
            vec![
                "docs.sora.gw.sora.name".to_string(),
                "*.gw.sora.id".to_string()
            ]
        );
    }

    #[test]
    fn dedup_tls_hosts_preserves_first_occurrence() {
        let mut hosts = vec![
            "docs.sora.gw.sora.name".to_string(),
            "*.gw.sora.id".to_string(),
            "docs.sora.gw.sora.name".to_string(),
        ];
        dedup_tls_hosts(&mut hosts);
        assert_eq!(
            hosts,
            vec![
                "docs.sora.gw.sora.name".to_string(),
                "*.gw.sora.id".to_string()
            ]
        );
    }

    fn metadata_with_counts(direct: Option<u64>, gateway: Option<u64>) -> ScoreboardMetadata {
        ScoreboardMetadata {
            provider_count: direct,
            gateway_provider_count: gateway,
            ..Default::default()
        }
    }

    #[test]
    fn metadata_single_source_hint_allows_gateway_multi_source() {
        let meta = metadata_with_counts(Some(0), Some(2));
        assert!(
            metadata_single_source_hint(&meta).is_none(),
            "gateway multi-source should not trigger fallback"
        );
    }

    #[test]
    fn metadata_single_source_hint_allows_mixed_provider_classes() {
        let meta = metadata_with_counts(Some(1), Some(1));
        assert!(
            metadata_single_source_hint(&meta).is_none(),
            "mixed direct+gateway providers should pass"
        );
    }

    #[test]
    fn metadata_single_source_hint_flags_single_gateway_provider() {
        let meta = metadata_with_counts(Some(0), Some(1));
        assert_eq!(
            metadata_single_source_hint(&meta),
            Some(("gateway_provider_count", 1))
        );
    }

    #[test]
    fn burn_in_check_accepts_valid_window() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_path = temp.path().join("burnin.log");
        fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"a\" region=\"lab\" job_id=\"job-1\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"a\" region=\"lab\" job_id=\"job-1\" anonymity_ratio=0.98 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
",
        )
        .expect("write log");

        let summary = run_burn_in_check(BurnInCheckOptions {
            log_paths: vec![log_path],
            required_window_days: 30,
            min_pq_ratio: 0.95,
            max_no_provider_errors: 0,
            max_brownout_ratio: 0.05,
            min_fetches: 1,
        })
        .expect("burn-in check should succeed");

        assert_eq!(summary.fetches_total, 1);
        assert_eq!(summary.successful_fetches, 1);
        assert!(
            summary.coverage_days >= 35.0,
            "unexpected coverage: {} days",
            summary.coverage_days
        );
        assert!(
            (summary.pq_ratio_min - 0.98).abs() < f64::EPSILON,
            "pq ratio mismatch"
        );
        assert!(
            (summary.stall_event_ratio - 0.0).abs() < f64::EPSILON,
            "stall event ratio should be zero"
        );
        assert!(
            (summary.stall_chunk_ratio - 0.0).abs() < f64::EPSILON,
            "stall chunk ratio should be zero"
        );
    }

    #[test]
    fn scoreboard_diff_detects_added_and_changed_weights() {
        let temp = tempdir().expect("tempdir");
        let previous = temp.path().join("prev.scoreboard.json");
        let current = temp.path().join("curr.scoreboard.json");
        write_scoreboard_with_weights(&previous, &[("alpha", 0.7), ("beta", 0.3)]);
        write_scoreboard_with_weights(&current, &[("alpha", 0.5), ("beta", 0.3), ("gamma", 0.2)]);

        let report = run_scoreboard_diff(ScoreboardDiffOptions {
            previous_scoreboard: previous,
            current_scoreboard: current,
            threshold_percent: 5.0,
        })
        .expect("diff report");

        assert_eq!(report.total_providers, 3);
        assert_eq!(report.changed_providers.len(), 2);
        let alpha = report
            .changed_providers
            .iter()
            .find(|entry| entry.provider_id == "alpha")
            .expect("alpha delta");
        assert!(!alpha.was_added);
        assert!(!alpha.was_removed);
        assert!(alpha.exceeds_threshold);
        let gamma = report
            .changed_providers
            .iter()
            .find(|entry| entry.provider_id == "gamma")
            .expect("gamma delta");
        assert!(gamma.was_added);
        assert!(!gamma.was_removed);
        assert!(gamma.exceeds_threshold);
    }

    #[test]
    fn scoreboard_diff_marks_removed_providers() {
        let temp = tempdir().expect("tempdir");
        let previous = temp.path().join("prev.scoreboard.json");
        let current = temp.path().join("curr.scoreboard.json");
        write_scoreboard_with_weights(&previous, &[("alpha", 0.4), ("beta", 0.6)]);
        write_scoreboard_with_weights(&current, &[("beta", 1.0)]);

        let report = run_scoreboard_diff(ScoreboardDiffOptions {
            previous_scoreboard: previous,
            current_scoreboard: current,
            threshold_percent: 10.0,
        })
        .expect("diff report");

        assert_eq!(report.changed_providers.len(), 2);
        let alpha = report
            .changed_providers
            .iter()
            .find(|entry| entry.provider_id == "alpha")
            .expect("alpha removal");
        assert!(alpha.was_removed);
        assert!(!alpha.was_added);
        assert!(alpha.exceeds_threshold);
        let beta = report
            .changed_providers
            .iter()
            .find(|entry| entry.provider_id == "beta")
            .expect("beta delta");
        assert!(!beta.was_added);
        assert!(!beta.was_removed);
        assert!(beta.exceeds_threshold);
    }

    #[test]
    fn burn_in_check_rejects_low_pq_ratio() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_path = temp.path().join("burnin_low_ratio.log");
        fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"b\" region=\"lab\" job_id=\"job-low\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"b\" region=\"lab\" job_id=\"job-low\" anonymity_ratio=0.90 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
",
        )
        .expect("write log");

        let result = run_burn_in_check(BurnInCheckOptions {
            log_paths: vec![log_path],
            required_window_days: 30,
            min_pq_ratio: 0.95,
            max_no_provider_errors: 0,
            max_brownout_ratio: 0.05,
            min_fetches: 1,
        });

        assert!(result.is_err(), "expected pq ratio violation");
    }

    #[test]
    fn burn_in_check_rejects_no_provider_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_path = temp.path().join("burnin_errors.log");
        fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"c\" region=\"lab\" job_id=\"job-err\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"c\" region=\"lab\" job_id=\"job-err\" anonymity_ratio=0.98 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
2026-02-06T00:00:00Z telemetry::sorafs.fetch.error reason=\"no_healthy_providers\" manifest=\"c\" region=\"lab\"
",
        )
        .expect("write log");

        let result = run_burn_in_check(BurnInCheckOptions {
            log_paths: vec![log_path],
            required_window_days: 30,
            min_pq_ratio: 0.95,
            max_no_provider_errors: 0,
            max_brownout_ratio: 0.05,
            min_fetches: 1,
        });

        assert!(
            result.is_err(),
            "expected failure when no_healthy_providers errors are present"
        );
    }

    #[test]
    fn burn_in_check_rejects_brownout_ratio() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_path = temp.path().join("burnin_brownout.log");
        fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"d\" region=\"lab\" job_id=\"job-brownout\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-01-15T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"d\" region=\"lab\" job_id=\"job-brownout\" anonymity_ratio=0.99 anonymity_outcome=\"brownout\" anonymity_reason=\"guard-deficit\" stall_count=0
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"d\" region=\"lab\" job_id=\"job-brownout\" anonymity_ratio=0.99 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
",
        )
        .expect("write log");

        let result = run_burn_in_check(BurnInCheckOptions {
            log_paths: vec![log_path],
            required_window_days: 30,
            min_pq_ratio: 0.95,
            max_no_provider_errors: 0,
            max_brownout_ratio: 0.10,
            min_fetches: 1,
        });

        assert!(
            result.is_err(),
            "expected failure when brownout ratio exceeds threshold"
        );
    }

    #[test]
    fn burn_in_check_reports_stall_ratios() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_path = temp.path().join("burnin_stalls.log");
        fs::write(
            &log_path,
            "\
2026-02-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"stall\" region=\"lab\" job_id=\"job-stall\" anonymity_ratio=0.97 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=4
2026-02-02T12:00:00Z telemetry::sorafs.fetch.stall manifest=\"stall\" region=\"lab\" job_id=\"job-stall\" reason=\"timeout\"
2026-02-03T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"stall\" region=\"lab\" job_id=\"job-stall\" anonymity_ratio=0.99 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=2
",
        )
        .expect("write log");

        let summary = run_burn_in_check(BurnInCheckOptions {
            log_paths: vec![log_path],
            required_window_days: 1,
            min_pq_ratio: 0.90,
            max_no_provider_errors: 0,
            max_brownout_ratio: 1.0,
            min_fetches: 1,
        })
        .expect("burn-in summary should succeed");

        assert_eq!(summary.fetches_total, 2);
        assert_eq!(summary.stall_events, 1);
        assert_eq!(summary.stall_chunks, 6);
        assert!(
            (summary.stall_event_ratio - 0.5).abs() < f64::EPSILON,
            "expected stall event ratio for one stall event across two fetches"
        );
        assert!(
            (summary.stall_chunk_ratio - 3.0).abs() < f64::EPSILON,
            "expected stall chunk ratio for six stalled chunks across two fetches"
        );
    }

    #[test]
    fn telemetry_parser_extracts_fields() {
        let line = "2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" anonymity_ratio=0.98 stall_count=0";
        let parsed =
            super::parse_telemetry_line(line).expect("parser should accept lifecycle line");
        assert_eq!(parsed.target, "sorafs.fetch.lifecycle");
        assert_eq!(
            parsed.fields.get("event").map(String::as_str),
            Some("complete")
        );
        assert_eq!(
            parsed.fields.get("anonymity_ratio").map(String::as_str),
            Some("0.98")
        );
        assert_eq!(
            parsed.fields.get("stall_count").map(String::as_str),
            Some("0")
        );
    }

    #[test]
    fn burn_in_accumulator_tracks_complete_events() {
        let line = "2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" anonymity_ratio=0.98 stall_count=0";
        let event = super::parse_telemetry_line(line).expect("parser should accept lifecycle line");
        let mut accumulator = super::BurnInAccumulator::default();
        accumulator.observe(event);
        assert_eq!(accumulator.fetches_total, 1);
        assert_eq!(accumulator.successful_fetches, 1);
        assert_eq!(accumulator.pq_ratio_samples, 1);
        assert!(
            (accumulator.pq_ratio_min - 0.98).abs() < f64::EPSILON,
            "expected ratio to be recorded (min={}, sum={}, samples={})",
            accumulator.pq_ratio_min,
            accumulator.pq_ratio_sum,
            accumulator.pq_ratio_samples
        );
    }

    #[test]
    fn adoption_check_accepts_multi_provider_scoreboard() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("scoreboard.json");
        let summary_path = temp.path().join("summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "fixture-a",
                    "normalised_weight": 0.6,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "fixture-b",
                    "normalised_weight": 0.4,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(
            &path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("fixture-a", 2), ("fixture-b", 3)]);
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("scoreboard should pass adoption check");
        assert_eq!(report.total_evaluated, 1);
        assert_eq!(
            report
                .scoreboard_reports
                .first()
                .expect("report entry")
                .eligible_providers,
            2
        );
    }

    #[test]
    fn adoption_check_rejects_missing_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing_metadata_scoreboard.json");
        let summary_path = temp.path().join("missing_metadata_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "metadata must be present even when not explicitly required"
        );
    }

    #[test]
    fn adoption_check_rejects_null_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("null_metadata_scoreboard.json");
        let summary_path = temp.path().join("null_metadata_summary.json");
        let scoreboard = norito::json!({
            "entries": [{
                "provider_id": "solo",
                "normalised_weight": 1.0,
                "raw_score": 1.0,
                "eligibility": "eligible"
            }],
            "metadata": null
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("solo", 1)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 1,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "null metadata block should fail the adoption gate"
        );
    }

    #[test]
    fn adoption_check_rejects_missing_provider_totals() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing_counts_scoreboard.json");
        let summary_path = temp.path().join("missing_counts_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/missing_counts.json",
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 1)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing provider totals should fail adoption check");
        assert!(
            err.to_string().contains("provider_count"),
            "error should mention missing provider_count: {}",
            err
        );
    }

    #[test]
    fn adoption_check_rejects_missing_provider_mix_metadata_without_counts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing_mix_scoreboard.json");
        let summary_path = temp.path().join("missing_mix_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.7,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.3,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/missing_mix.json",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing provider metadata should fail adoption check");
        assert!(
            err.to_string().contains("provider_count"),
            "error should mention missing provider totals: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_provider_count_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("provider_count_mismatch_scoreboard.json");
        let summary_path = temp.path().join("provider_count_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/fixture.json",
                "provider_count": 3u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "metadata/provider count mismatch should fail the adoption gate"
        );
    }

    #[test]
    fn adoption_check_rejects_summary_provider_count_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("summary_provider_count_mismatch_scoreboard.json");
        let summary_path = temp
            .path()
            .join("summary_provider_count_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/fixture.json",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_value: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for mismatch"))
                .expect("parse summary");
        summary_value
            .as_object_mut()
            .expect("summary object")
            .insert("provider_count".into(), Value::from(1u64));
        fs::write(
            &summary_path,
            to_string_pretty(&summary_value).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.unwrap_err().to_string().contains("provider_count"),
            "mismatched summary provider_count should fail the adoption gate",
        );
    }

    #[test]
    fn adoption_check_rejects_summary_gateway_count_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("summary_gateway_count_mismatch_scoreboard.json");
        let summary_path = temp
            .path()
            .join("summary_gateway_count_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 0.5,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/fixture.json",
                "provider_count": 1u64,
                "gateway_provider_count": 1u64,
                "provider_mix": "mixed",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "gateway_manifest_id": "feedface",
                "gateway_manifest_cid": "c0ffee",
                "gateway_manifest_provided": true
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 1), ("gateway-a", 1)]);
        let mut summary_value: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for mismatch"))
                .expect("parse summary");
        let summary_obj = summary_value
            .as_object_mut()
            .expect("summary object for mismatch");
        summary_obj.insert("gateway_provider_count".into(), Value::from(2u64));
        summary_obj.insert(
            "provider_mix".into(),
            Value::from(provider_mix_label_from_counts(1, 2)),
        );
        fs::write(
            &summary_path,
            to_string_pretty(&summary_value).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("gateway_provider_count"),
            "mismatched summary gateway_provider_count should fail the adoption gate",
        );
    }

    #[test]
    fn adoption_check_rejects_missing_summary_provider_mix() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("summary_missing_provider_mix_scoreboard.json");
        let summary_path = temp
            .path()
            .join("summary_missing_provider_mix_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/fixture.json",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_value: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary"))
                .expect("parse summary");
        summary_value
            .as_object_mut()
            .expect("summary object")
            .remove("provider_mix");
        fs::write(
            &summary_path,
            to_string_pretty(&summary_value).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.unwrap_err().to_string().contains("provider_mix"),
            "missing summary provider_mix should fail the adoption gate",
        );
    }

    #[test]
    fn adoption_check_rejects_gateway_runs_without_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("gateway_missing_metadata_scoreboard.json");
        let summary_path = temp.path().join("gateway_missing_metadata_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("gateway-a", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 1,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("gateway captures without metadata must fail");
        let err_text = err.to_string();
        assert!(
            err_text.contains("metadata"),
            "error should mention missing metadata for gateway captures: {err_text}"
        );
    }

    #[test]
    fn adoption_check_accepts_matching_provider_counts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("provider_count_match_scoreboard.json");
        let summary_path = temp.path().join("provider_count_match_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/fixture.json",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("matching counts should pass the adoption gate");
        assert_eq!(report.total_evaluated, 1);
    }

    #[test]
    fn adoption_check_rejects_missing_provider_mix_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing_provider_mix_scoreboard.json");
        let summary_path = temp.path().join("missing_provider_mix_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/missing_mix.json",
                "provider_count": 1u64,
                "gateway_provider_count": 1u64,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing provider_mix metadata must fail the adoption gate");
        assert!(
            err.to_string().contains("provider_mix"),
            "error should mention provider_mix requirement"
        );
    }

    #[test]
    fn adoption_check_rejects_metadata_missing_transport_policy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing_transport_policy_scoreboard.json");
        let summary_path = temp.path().join("missing_transport_policy_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing transport_policy should fail the adoption gate");
        assert!(
            err.to_string().contains("transport_policy"),
            "error should mention missing transport policy metadata: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_metadata_transport_policy_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("transport_policy_mismatch_scoreboard.json");
        let summary_path = temp.path().join("transport_policy_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-strict",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("transport policy mismatch must fail");
        assert!(
            err.to_string().contains("metadata.transport_policy"),
            "error should highlight transport policy mismatch: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_metadata_transport_override_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("transport_override_mismatch_scoreboard.json");
        let summary_path = temp.path().join("transport_override_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": true,
                "transport_policy_override_label": "soranet-strict"
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("unexpected override should fail");
        assert!(
            err.to_string().contains("transport_policy_override"),
            "error should mention override mismatch: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_metadata_missing_override_label() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("transport_override_missing_label_scoreboard.json");
        let summary_path = temp
            .path()
            .join("transport_override_missing_label_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": true
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        let mut summary = summary_with_providers_value(&[("alpha", 2), ("beta", 2)]);
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("transport_policy_override".into(), Value::Bool(true));
            obj.insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary).expect("render summary"),
        )
        .expect("write summary");
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing override label must fail");
        assert!(
            err.to_string().contains("transport_policy_override_label"),
            "error should mention missing override label: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_provider_mix_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("provider_mix_mismatch_scoreboard.json");
        let summary_path = temp.path().join("provider_mix_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "telemetry_source": "file:///tmp/mix_mismatch.json",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "mixed",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("provider_mix mismatch must fail the adoption gate");
        assert!(
            err.to_string().contains("metadata.provider_mix"),
            "error should flag the provider_mix mismatch"
        );
    }

    #[test]
    fn adoption_check_rejects_gateway_runs_without_manifest_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("gateway_missing_manifest_scoreboard.json");
        let summary_path = temp.path().join("gateway_missing_manifest_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "gateway-b",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 0u64,
                "gateway_provider_count": 2u64,
                "gateway_manifest_provided": false,
                "use_scoreboard": true,
                "provider_mix": "gateway-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(
            &path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("gateway-a", 2), ("gateway-b", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing gateway manifest metadata should fail adoption check");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("gateway provider(s)"),
            "expected error mentioning gateway manifest requirement, got: {err_msg}"
        );
    }

    #[test]
    fn adoption_check_rejects_gateway_manifest_metadata_without_gateways() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("gateway_manifest_without_gateways_scoreboard.json");
        let summary_path = temp
            .path()
            .join("gateway_manifest_without_gateways_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "direct-a",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "direct-b",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "gateway_manifest_provided": true,
                "gateway_manifest_id": TEST_MANIFEST_ID,
                "gateway_manifest_cid": TEST_MANIFEST_CID,
                "use_scoreboard": true,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(
            &path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("direct-a", 2), ("direct-b", 3)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("direct-only runs should not carry gateway manifest metadata");
        assert!(
            err.to_string().contains("no gateway providers"),
            "error should mention gateway manifest metadata mismatch"
        );
    }

    #[test]
    fn adoption_check_accepts_gateway_runs_with_manifest_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("gateway_manifest_scoreboard.json");
        let summary_path = temp.path().join("gateway_manifest_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 0.55,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "gateway-b",
                    "normalised_weight": 0.45,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 0u64,
                "gateway_provider_count": 2u64,
                "gateway_manifest_id": TEST_MANIFEST_ID,
                "gateway_manifest_cid": TEST_MANIFEST_CID,
                "gateway_manifest_provided": true,
                "use_scoreboard": true,
                "provider_mix": "gateway-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(
            &path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("gateway-a", 3), ("gateway-b", 2)]);
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("gateway scoreboard with manifest metadata should pass");
        assert_eq!(report.total_evaluated, 1);
        assert!(!report.single_source_override_used);
    }

    #[test]
    fn adoption_check_rejects_gateway_runs_without_manifest_identifiers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("gateway_missing_manifest_identifiers_scoreboard.json");
        let summary_path = temp
            .path()
            .join("gateway_missing_manifest_identifiers_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "gateway-b",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 0u64,
                "gateway_provider_count": 2u64,
                "gateway_manifest_provided": true,
                "use_scoreboard": true,
                "provider_mix": "gateway-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("gateway-a", 1), ("gateway-b", 1)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing manifest identifiers must fail adoption check");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("gateway_manifest_id"),
            "error should mention missing manifest id: {err_msg}"
        );
    }

    #[test]
    fn adoption_check_rejects_summary_manifest_flag_without_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("gateway_manifest_summary_flag_scoreboard.json");
        let summary_path = temp
            .path()
            .join("gateway_manifest_summary_flag_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "direct-a",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "direct-b",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "gateway_manifest_provided": false,
                "use_scoreboard": true,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("direct-a", 2), ("direct-b", 1)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for manifest flag"))
                .expect("parse summary json");
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert("gateway_manifest_provided".into(), Value::Bool(true));
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render mutated summary"),
        )
        .expect("rewrite summary");
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("summary manifest flag without gateway metadata must fail");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("gateway_manifest_provided"),
            "error should mention stray summary manifest flag: {err_msg}"
        );
    }

    #[test]
    fn adoption_check_rejects_gateway_runs_with_summary_manifest_gap() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("gateway_summary_manifest_gap_scoreboard.json");
        let summary_path = temp
            .path()
            .join("gateway_summary_manifest_gap_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "gateway-b",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 0u64,
                "gateway_provider_count": 2u64,
                "gateway_manifest_id": TEST_MANIFEST_ID,
                "gateway_manifest_cid": TEST_MANIFEST_CID,
                "gateway_manifest_provided": true,
                "use_scoreboard": true,
                "provider_mix": "gateway-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("gateway-a", 2), ("gateway-b", 1)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for manifest gap"))
                .expect("parse summary json");
        summary_json
            .as_object_mut()
            .expect("summary object")
            .remove("manifest_id");
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render mutated summary"),
        )
        .expect("rewrite summary");
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing summary manifest should fail adoption check");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("manifest_id"),
            "error should mention missing summary manifest field: {err_msg}"
        );
    }

    #[test]
    fn adoption_check_rejects_gateway_runs_with_manifest_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("gateway_manifest_mismatch_scoreboard.json");
        let summary_path = temp.path().join("gateway_manifest_mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "gateway-a",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "gateway-b",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "provider_count": 0u64,
                "gateway_provider_count": 2u64,
                "gateway_manifest_id": TEST_MANIFEST_ID,
                "gateway_manifest_cid": TEST_MANIFEST_CID,
                "gateway_manifest_provided": true,
                "use_scoreboard": true,
                "provider_mix": "gateway-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("gateway-a", 2), ("gateway-b", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for mismatch"))
                .expect("parse summary json");
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert("manifest_cid".into(), Value::from("different-cid"));
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render mutated summary"),
        )
        .expect("rewrite summary");
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("manifest mismatch should fail adoption check");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("manifest_cid"),
            "error should mention manifest mismatch: {err_msg}"
        );
    }

    #[test]
    fn adoption_check_rejects_single_provider_scoreboard() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("single_scoreboard.json");
        let summary_path = temp.path().join("single_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "solo",
                    "normalised_weight": 1.0,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 1u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("solo", 5), ("backup", 1)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(result.is_err(), "single-provider scoreboard should fail");
    }

    #[test]
    fn adoption_check_rejects_metadata_single_source_max_parallel() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_single_source.json");
        let summary_path = temp.path().join("metadata_single_source_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "max_parallel": 1,
                "use_scoreboard": true,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "max_parallel": 1,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "metadata with max_parallel=1 should be rejected without override"
        );
    }

    #[test]
    fn adoption_check_allows_metadata_single_source_with_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_single_source_override.json");
        let summary_path = temp
            .path()
            .join("metadata_single_source_override_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "max_parallel": 1,
                "transport_policy": "direct-only",
                "transport_policy_override": true,
                "transport_policy_override_label": "direct-only"
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for policy edit"))
                .expect("parse summary json");
        if let Some(obj) = summary_json.as_object_mut() {
            obj.insert("transport_policy".into(), Value::from("direct-only"));
            obj.insert("transport_policy_override".into(), Value::from(true));
            obj.insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should allow single-source metadata");
        assert!(
            report.single_source_override_used,
            "override flag should be recorded when metadata trips single-source hint"
        );
    }

    #[test]
    fn adoption_check_rejects_direct_only_transport_policy_without_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("direct_only_policy.json");
        let summary_path = temp.path().join("direct_only_policy_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "transport_policy": "direct-only",
                "transport_policy_override": true,
                "transport_policy_override_label": "direct-only",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 0.9,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "direct-only transport policy should be rejected without override"
        );
    }

    #[test]
    fn adoption_check_reports_override_for_direct_only_transport_policy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("direct_only_policy_override.json");
        let summary_path = temp.path().join("direct_only_policy_override_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "transport_policy": "direct-only",
                "transport_policy_override": true,
                "transport_policy_override_label": "direct-only",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 0.9,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for override edit"))
                .expect("parse summary json");
        if let Some(obj) = summary_json.as_object_mut() {
            obj.insert("transport_policy".into(), Value::from("direct-only"));
            obj.insert("transport_policy_override".into(), Value::from(true));
            obj.insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should allow direct-only metadata");
        assert!(
            report.single_source_override_used,
            "direct-only override should be recorded"
        );
    }

    #[test]
    fn adoption_check_requires_direct_only_when_flag_is_set() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("require_direct_only_scoreboard.json");
        let summary_path = temp.path().join("require_direct_only_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: true,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "runs that set --require-direct-only must fail when the summary advertises the SoraNet-first posture"
        );
    }

    #[test]
    fn adoption_check_accepts_direct_only_when_required_flag_is_set() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("require_direct_only_success_scoreboard.json");
        let summary_path = temp.path().join("require_direct_only_success_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "transport_policy": "direct-only",
                "transport_policy_override": true,
                "transport_policy_override_label": "direct-only",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.55,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.45,
                    "raw_score": 0.9,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("summary")).expect("parse summary");
        if let Some(obj) = summary_json.as_object_mut() {
            obj.insert("transport_policy".into(), Value::from("direct-only"));
            obj.insert("transport_policy_override".into(), Value::from(true));
            obj.insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: true,
            require_telemetry_region: false,
        })
        .expect("direct-only runs must pass when the flag is supplied");
        assert!(
            report.single_source_override_used,
            "direct-only override should still be recorded in the adoption report"
        );
    }

    #[test]
    fn adoption_check_rejects_direct_only_summary_transport_without_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("direct_only_summary_scoreboard.json");
        let summary_path = temp.path().join("direct_only_summary_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for transport edit"))
                .expect("parse summary json");
        if let Some(obj) = summary_json.as_object_mut() {
            obj.insert("transport_policy".into(), Value::from("direct-only"));
            obj.insert("transport_policy_override".into(), Value::from(true));
            obj.insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        }
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert("transport_policy_override".into(), Value::from(true));
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "summary transport policy downgrades should be rejected without an override"
        );
    }

    #[test]
    fn adoption_check_reports_override_for_direct_only_summary_transport() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("direct_only_summary_override_scoreboard.json");
        let summary_path = temp
            .path()
            .join("direct_only_summary_override_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "direct-only",
                "transport_policy_override": true,
                "transport_policy_override_label": "direct-only"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary for override edit"))
                .expect("parse summary json");
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert("transport_policy".into(), Value::from("direct-only"));
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert("transport_policy_override".into(), Value::from(true));
        summary_json
            .as_object_mut()
            .expect("summary object")
            .insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should permit direct-only summary captures");
        assert!(
            report.single_source_override_used,
            "direct-only override should be recorded when surfaced via summary"
        );
    }

    #[test]
    fn adoption_check_allows_single_provider_with_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("single_scoreboard_override.json");
        let summary_path = temp.path().join("single_summary_override.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "solo",
                    "normalised_weight": 1.0,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 1u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("solo", 4)]);
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("explicit override should allow single-provider fallback");
        assert!(report.single_source_override_used);
    }

    #[test]
    fn adoption_check_rejects_scoreboard_metadata_opt_out() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_scoreboard.json");
        let summary_path = temp.path().join("metadata_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "fixture-provider",
                    "normalised_weight": 1.0,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "version": "test",
                "use_scoreboard": false,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("fixture-provider", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 1,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "metadata that disables scoreboard mode should fail the adoption gate"
        );
    }

    #[test]
    fn adoption_check_rejects_single_source_metadata_without_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_single_source_scoreboard.json");
        let summary_path = temp.path().join("metadata_single_source_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "max_peers": 1,
                "gateway_provider_count": 2,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 3)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "single-source metadata must fail when overrides are not allowed"
        );
    }

    #[test]
    fn adoption_check_reports_override_for_single_source_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_override_scoreboard.json");
        let summary_path = temp.path().join("metadata_override_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "max_parallel": 1,
                "transport_policy": "direct-only",
                "transport_policy_override": true,
                "transport_policy_override_label": "direct-only"
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let mut summary_json: Value =
            json::from_slice(&fs::read(&summary_path).expect("summary json"))
                .expect("parse summary");
        if let Some(obj) = summary_json.as_object_mut() {
            obj.insert("transport_policy".into(), Value::from("direct-only"));
            obj.insert("transport_policy_override".into(), Value::from(true));
            obj.insert(
                "transport_policy_override_label".into(),
                Value::from("direct-only"),
            );
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary_json).expect("render updated summary"),
        )
        .expect("rewrite summary");
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should allow single-source metadata hints");
        assert!(
            report.single_source_override_used,
            "override flag should be surfaced in the adoption report"
        );
    }

    #[test]
    fn adoption_check_rejects_implicit_metadata_without_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("implicit_scoreboard.json");
        let summary_path = temp.path().join("implicit_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "allow_implicit_metadata": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 1), ("beta", 1)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "implicit metadata should fail without an explicit override"
        );
    }

    #[test]
    fn adoption_check_reports_override_for_implicit_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("implicit_override_scoreboard.json");
        let summary_path = temp.path().join("implicit_override_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "allow_implicit_metadata": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 1), ("beta", 1)]);
        let report = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: true,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should permit implicit metadata fixtures");
        assert!(
            report.implicit_metadata_override_used,
            "implicit metadata override usage should be tracked"
        );
    }

    #[test]
    fn adoption_check_requires_telemetry_source_when_requested() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_missing_telemetry.json");
        let summary_path = temp.path().join("metadata_missing_telemetry_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "max_parallel": 4,
                "use_scoreboard": true,
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.3,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 3), ("beta", 2)],
            "file:/tmp/telemetry.json",
            None,
        );
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: true,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "requiring telemetry should fail when metadata.telemetry_source is missing"
        );
    }

    #[test]
    fn adoption_check_accepts_present_telemetry_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("metadata_with_telemetry.json");
        let summary_path = temp.path().join("metadata_with_telemetry_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "max_parallel": 4,
                "use_scoreboard": true,
                "telemetry_source": "file:/tmp/telemetry.json",
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.3,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 3), ("beta", 2)],
            "file:/tmp/telemetry.json",
            None,
        );
        run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: true,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("telemetry metadata should satisfy the new requirement");
    }

    #[test]
    fn adoption_check_requires_summary_telemetry_label() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("scoreboard.json");
        let summary_path = temp.path().join("summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "telemetry_source": "otel::ci"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 1,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: true,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing summary telemetry label should fail the adoption gate");
        assert!(
            err.to_string().contains("missing `telemetry_source`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_mismatched_summary_telemetry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("scoreboard.json");
        let summary_path = temp.path().join("summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "telemetry_source": "otel::primary"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 2), ("beta", 2)],
            "otel::other",
            None,
        );
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("telemetry mismatch should fail the adoption gate");
        assert!(
            err.to_string().contains("telemetry_source=`otel::other`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn adoption_check_requires_summary_telemetry_region_when_metadata_present() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("scoreboard.json");
        let summary_path = temp.path().join("summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "telemetry_source": "otel::ci",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "telemetry_region": "iad-prod"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        // Summary lacks telemetry_region.
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 2), ("beta", 2)],
            "otel::ci",
            None,
        );
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("missing summary telemetry_region should fail when metadata is present");
        assert!(
            err.to_string().contains("telemetry_region"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn adoption_check_rejects_mismatched_telemetry_region() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("scoreboard.json");
        let summary_path = temp.path().join("summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "telemetry_source": "otel::ci",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "telemetry_region": "iad-prod"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 3), ("beta", 1)],
            "otel::ci",
            Some("sea-primary"),
        );
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect_err("telemetry_region mismatch should fail adoption gate");
        assert!(
            err.to_string().contains("telemetry_region=`sea-primary`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn adoption_check_accepts_matching_telemetry_region() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("scoreboard.json");
        let summary_path = temp.path().join("summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "telemetry_source": "otel::ci",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null,
                "telemetry_region": "iad-prod"
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 2), ("beta", 2)],
            "otel::ci",
            Some("iad-prod"),
        );
        run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            require_telemetry_region: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
        })
        .expect("matching telemetry_region labels should pass");
    }

    #[test]
    fn adoption_check_allows_missing_telemetry_region_without_flag() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("no_region_scoreboard.json");
        let summary_path = temp.path().join("no_region_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "telemetry_source": "otel::ci",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 2), ("beta", 2)],
            "otel::ci",
            None,
        );
        run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            require_telemetry_region: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
        })
        .expect("missing telemetry_region should be allowed without the flag");
    }

    #[test]
    fn adoption_check_requires_telemetry_region_when_requested() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("require_region_scoreboard.json");
        let summary_path = temp.path().join("require_region_summary.json");
        let scoreboard = norito::json!({
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "telemetry_source": "otel::ci",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            },
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_telemetry(
            &summary_path,
            &[("alpha", 2), ("beta", 2)],
            "otel::ci",
            None,
        );
        let err = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            require_telemetry_region: true,
            allow_implicit_metadata: false,
            require_direct_only: false,
        })
        .expect_err("requiring telemetry_region should fail when metadata is missing");
        assert!(
            err.to_string().contains("telemetry_region"),
            "missing telemetry_region error should mention telemetry_region"
        );
    }

    #[test]
    fn adoption_check_rejects_zero_weight_provider_when_required() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("zero_weight_scoreboard.json");
        let summary_path = temp.path().join("zero_weight_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "zero-weight",
                    "normalised_weight": 0.0,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "healthy",
                    "normalised_weight": 1.0,
                    "raw_score": 1.4,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("zero-weight", 1), ("healthy", 3)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "zero-weight provider should fail strict check"
        );
    }

    #[test]
    fn adoption_check_allows_zero_weight_when_override_enabled() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("zero_weight_allowed.json");
        let summary_path = temp.path().join("zero_weight_allowed_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "zero-weight",
                    "normalised_weight": 0.0,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "backup",
                    "normalised_weight": 1.0,
                    "raw_score": 1.3,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("zero-weight", 1), ("backup", 2)]);
        run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: false,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should allow zero-weight entries");
    }

    #[test]
    fn adoption_check_rejects_weight_sum_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("weight_sum_bad.json");
        let summary_path = temp.path().join("weight_sum_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.4,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 0.9,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(&path, to_string_pretty(&scoreboard).expect("render")).expect("write");
        write_summary_with_providers(&summary_path, &[("alpha", 2), ("beta", 2)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "weights that do not sum to unity should fail adoption check"
        );
    }

    #[test]
    fn adoption_check_rejects_single_active_provider_in_summary() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("summary_single_provider_scoreboard.json");
        let summary_path = temp.path().join("summary_single_provider.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.2,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 5), ("beta", 0)]);
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "single active provider in summary should fail adoption check"
        );
    }

    #[test]
    fn adoption_check_allows_single_active_provider_in_summary_with_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp
            .path()
            .join("summary_single_provider_override_scoreboard.json");
        let summary_path = temp.path().join("summary_single_provider_override.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.3,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ],
            "metadata": {
                "use_scoreboard": true,
                "provider_count": 2u64,
                "gateway_provider_count": 0u64,
                "provider_mix": "direct-only",
                "transport_policy": "soranet-first",
                "transport_policy_override": false,
                "transport_policy_override_label": null
            }
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 4), ("beta", 0)]);
        run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: true,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        })
        .expect("override should allow single active provider when downgrade is intentional");
    }

    #[test]
    fn adoption_check_rejects_chunk_count_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("mismatch_scoreboard.json");
        let summary_path = temp.path().join("mismatch_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.5,
                    "raw_score": 1.1,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.5,
                    "raw_score": 1.0,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 1), ("beta", 1)]);
        let mut summary: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary")).expect("parse");
        if let Some(map) = summary.as_object_mut() {
            map.insert("chunk_count".into(), norito::json!(999u64));
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary).expect("render summary"),
        )
        .expect("write summary");
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "chunk_count mismatch should fail adoption check"
        );
    }

    #[test]
    fn adoption_check_rejects_unknown_provider_in_summary() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scoreboard_path = temp.path().join("unknown_scoreboard.json");
        let summary_path = temp.path().join("unknown_summary.json");
        let scoreboard = norito::json!({
            "entries": [
                {
                    "provider_id": "alpha",
                    "normalised_weight": 0.6,
                    "raw_score": 1.25,
                    "eligibility": "eligible"
                },
                {
                    "provider_id": "beta",
                    "normalised_weight": 0.4,
                    "raw_score": 0.95,
                    "eligibility": "eligible"
                }
            ]
        });
        fs::write(
            &scoreboard_path,
            to_string_pretty(&scoreboard).expect("render scoreboard"),
        )
        .expect("write scoreboard");
        write_summary_with_providers(&summary_path, &[("alpha", 1), ("beta", 1)]);
        let mut summary: Value =
            json::from_slice(&fs::read(&summary_path).expect("read summary")).expect("parse");
        if let Some(obj) = summary
            .get_mut("chunk_receipts")
            .and_then(Value::as_array_mut)
            .and_then(|receipts| receipts.first_mut())
            .and_then(Value::as_object_mut)
        {
            obj.insert("provider".into(), norito::json!("intruder"));
        }
        if let Some(obj) = summary
            .get_mut("provider_reports")
            .and_then(Value::as_array_mut)
            .and_then(|reports| reports.first_mut())
            .and_then(Value::as_object_mut)
        {
            obj.insert("provider".into(), norito::json!("intruder"));
        }
        fs::write(
            &summary_path,
            to_string_pretty(&summary).expect("render summary"),
        )
        .expect("write modified summary");
        let result = run_adoption_check(AdoptionCheckOptions {
            scoreboard_paths: vec![scoreboard_path],
            summary_paths: vec![summary_path],
            min_eligible_providers: 2,
            require_positive_weight: true,
            allow_single_source_fallback: false,
            require_telemetry_source: false,
            allow_implicit_metadata: false,
            require_direct_only: false,
            require_telemetry_region: false,
        });
        assert!(
            result.is_err(),
            "summary referencing providers outside scoreboard should fail"
        );
    }

    #[test]
    fn cache_directive_parser_extracts_expected_values() {
        let directives = parse_cache_directives("max-age=600, stale-while-revalidate=120, public");
        assert_eq!(directives.get("max-age").map(String::as_str), Some("600"));
        assert_eq!(
            directives.get("stale-while-revalidate").map(String::as_str),
            Some("120")
        );
        assert!(directives.contains_key("public"));
    }

    #[test]
    fn proof_status_matching_handles_rotate_suffix() {
        let evaluation = AliasProofEvaluation {
            state: AliasProofState::Fresh,
            rotation_due: true,
            age: Duration::from_secs(42),
            generated_at_unix: 0,
            expires_at_unix: 0,
            expires_in: None,
        };
        assert!(proof_status_matches(&evaluation, "fresh-rotate"));
        assert!(proof_status_matches(&evaluation, "fresh"));
        assert!(!proof_status_matches(&evaluation, "refresh"));
    }

    #[test]
    fn tls_renew_writes_bundle() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let bundle_dir = temp.path().join("tls");
        let options = GatewayTlsRenewOptions {
            hostnames: vec!["gw.example.com".to_string()],
            account_email: Some("ops@example.com".to_string()),
            directory_url: "https://acme.invalid/directory".to_string(),
            dns_provider_id: None,
            output_dir: bundle_dir.clone(),
            force: false,
        };

        let outcome = gateway_tls_renew(options).expect("tls renew");
        assert_eq!(outcome.hostnames, vec!["gw.example.com".to_string()]);
        assert!(outcome.certificate_path.exists());
        assert!(outcome.private_key_path.exists());
        assert!(outcome.ech_config_path.exists());
        let ech_contents = fs::read_to_string(outcome.ech_config_path).expect("read ech.json");
        assert!(
            ech_contents.trim() == "null" || ech_contents.contains("ech_config_b64"),
            "unexpected ech.json contents: {ech_contents}"
        );
    }

    #[test]
    fn tls_revoke_archives_bundle() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let bundle_dir = temp.path().join("bundle");
        fs::create_dir_all(&bundle_dir).expect("bundle dir");
        fs::write(bundle_dir.join("fullchain.pem"), "CERT").expect("seed cert");
        fs::write(bundle_dir.join("privkey.pem"), "KEY").expect("seed key");
        fs::write(bundle_dir.join("ech.json"), "null\n").expect("seed ech");

        let outcome = gateway_tls_revoke(GatewayTlsRevokeOptions {
            bundle_dir: bundle_dir.clone(),
            archive_dir: None,
            reason: Some("test".into()),
            force: false,
        })
        .expect("revoke");

        assert!(
            !bundle_dir.join("fullchain.pem").exists(),
            "certificate should be moved"
        );
        assert_eq!(outcome.archived_files.len(), 4);
        for path in outcome.archived_files {
            assert!(path.exists(), "{path:?} should exist");
        }
    }

    #[test]
    fn key_rotate_generates_material() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let private_path = temp.path().join("token_signing_sk");
        let public_path = temp.path().join("token_signing_pk.json");

        let outcome = gateway_key_rotate(GatewayKeyRotateOptions {
            kind: "token-signing".into(),
            output_path: private_path.clone(),
            public_out: Some(public_path.clone()),
            force: true,
        })
        .expect("rotate");

        let private_contents = fs::read_to_string(&private_path).expect("read private key");
        assert_eq!(private_contents.trim().len(), 64);
        assert!(public_path.exists());
        assert_eq!(outcome.public_key_hex.len(), 64);
        assert!(
            outcome.public_key_prefixed.starts_with("ed"),
            "expected ed-prefixed multihash"
        );
    }

    #[test]
    fn gateway_probe_report_includes_failure_summary() {
        let gar_info = GatewayProbeGarInfo {
            path: "gar/sample.jws".into(),
            name: "gateway-alpha".into(),
            record_version: 1,
            manifest_cid: "bafy-example".into(),
            valid_from_epoch: 1_700_000_000,
            valid_until_epoch: Some(1_700_086_400),
            host_patterns: vec!["gw.example.com".into(), "*.gw.example.com".into()],
        };
        let response = ProbeResponse {
            status: 503,
            headers: HeaderMap::new(),
            source: ProbeSource::File {
                path: PathBuf::from("headers.txt"),
            },
        };
        let findings = vec![
            ProbeFinding {
                ok: false,
                name: "HTTP status".into(),
                detail: "503 HTTP probe via capture".into(),
            },
            ProbeFinding {
                ok: true,
                name: "GAR host pattern".into(),
                detail: "host `gw.example.com` authorised by GAR".into(),
            },
        ];

        let report = build_probe_report_value(
            1_700_123_456,
            &response,
            "gw.example.com",
            &gar_info,
            &findings,
        );

        assert_eq!(report["ok"], norito::json!(false));
        assert_eq!(report["failure_count"], norito::json!(1u64));
        assert_eq!(report["status"], norito::json!(503u64));
        assert_eq!(report["host"], norito::json!("gw.example.com"));
        assert_eq!(report["source"]["type"], norito::json!("headers-file"));
        assert_eq!(
            report["gar"]["host_patterns"],
            norito::json!(["gw.example.com", "*.gw.example.com"])
        );
        assert_eq!(report["failures"], norito::json!(["HTTP status"]));
        let findings_array = report["findings"].as_array().expect("findings array");
        assert_eq!(findings_array.len(), 2);
        assert_eq!(
            findings_array[0]["detail"],
            norito::json!("503 HTTP probe via capture")
        );
    }

    #[test]
    fn drill_log_helper_appends_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_path = temp.path().join("drill-log.md");
        let gar_path = temp.path().join("gar.jws");
        let findings = vec![ProbeFinding {
            ok: false,
            name: "HTTP status".into(),
            detail: "500 HTTP probe via https://gw.example".into(),
        }];
        let summary = ProbeRunSummary {
            started_at: OffsetDateTime::UNIX_EPOCH,
            ended_at: OffsetDateTime::UNIX_EPOCH,
            success: false,
            source_description: "HTTP probe via https://gw.example".into(),
            target_url: Some("https://gw.example".into()),
            target_host: Some("gw.example".into()),
            gar_path,
            findings: &findings,
        };
        let config = DrillLogConfig {
            log_path: log_path.clone(),
            scenario: "tls-drill".into(),
            ic: Some("Alice".into()),
            scribe: Some("Bob".into()),
            notes: Some("note|test".into()),
            link: Some("https://example.com".into()),
        };
        append_drill_log_entry(&config, &summary).expect("append drill log");
        let contents = fs::read_to_string(&log_path).expect("read log");
        assert!(
            contents.contains(
                "| 1970-01-01 | tls-drill | fail | Alice | Bob | 00:00Z | 00:00Z | note&#124;test | https://example.com |"
            ),
            "unexpected log contents:\n{contents}"
        );
    }

    #[test]
    fn pagerduty_payload_includes_details() {
        let temp = tempfile::tempdir().expect("tempdir");
        let payload_path = temp.path().join("pd.json");
        let gar_path = temp.path().join("gar.jws");
        let findings = vec![
            ProbeFinding {
                ok: false,
                name: "HTTP status".into(),
                detail: "500".into(),
            },
            ProbeFinding {
                ok: true,
                name: "Cache-Control".into(),
                detail: "ok".into(),
            },
        ];
        let summary = ProbeRunSummary {
            started_at: OffsetDateTime::UNIX_EPOCH,
            ended_at: OffsetDateTime::UNIX_EPOCH,
            success: false,
            source_description: "HTTP probe via https://gw.example".into(),
            target_url: Some("https://gw.example".into()),
            target_host: Some("gw.example".into()),
            gar_path,
            findings: &findings,
        };
        let config = PagerDutyConfig {
            payload_path,
            routing_key: "rk".into(),
            severity: "critical".into(),
            source: "unit-test".into(),
            component: Some("gateway".into()),
            group: Some("tls".into()),
            class_name: Some("drill".into()),
            dedup_key: Some("dedup".into()),
            links: vec![PagerDutyLink {
                text: "drill-log".into(),
                href: "https://example.com/drill".into(),
            }],
            endpoint_url: None,
        };
        let value = pagerduty_payload_value(&config, &summary).expect("payload");
        assert_eq!(value["routing_key"], norito::json!("rk"));
        assert_eq!(value["payload"]["severity"], norito::json!("critical"));
        assert_eq!(
            value["payload"]["custom_details"]["failure_count"],
            norito::json!(1)
        );
        assert_eq!(
            value["links"][0]["href"],
            norito::json!("https://example.com/drill")
        );
    }

    #[test]
    fn taikai_cache_bundle_writes_artifacts() {
        let temp_profiles = tempfile::tempdir().expect("tempdir");
        let profile_path = temp_profiles.path().join("bundle.json");
        let profile_json = r#"
{
  "profile_id": "bundle",
  "rollout_stage": "canary",
  "description": "fixture profile",
  "taikai_cache": {
    "hot_capacity_bytes": 1048576,
    "hot_retention_secs": 30,
    "warm_capacity_bytes": 2097152,
    "warm_retention_secs": 120,
    "cold_capacity_bytes": 4194304,
    "cold_retention_secs": 600,
    "qos": {
      "priority_rate_bps": 10485760,
      "standard_rate_bps": 5242880,
      "bulk_rate_bps": 1048576,
      "burst_multiplier": 2
    }
  }
}
"#;
        fs::write(&profile_path, profile_json).expect("write profile");
        let output_dir = tempfile::tempdir().expect("tempdir");
        run_taikai_cache_bundle(TaikaiCacheBundleOptions {
            profile_paths: vec![profile_path],
            output_root: output_dir.path().to_path_buf(),
        })
        .expect("bundle generation succeeds");

        let profile_dir = output_dir.path().join("bundle");
        assert!(profile_dir.join("profile.json").exists());
        assert!(profile_dir.join("cache_config.json").exists());
        assert!(profile_dir.join("profile.taikai_cache_profile.to").exists());
        assert!(profile_dir.join("profile.manifest.json").exists());

        let manifest_bytes =
            fs::read(profile_dir.join("profile.manifest.json")).expect("read manifest");
        let manifest: Value = json::from_slice(&manifest_bytes).expect("parse manifest json");
        assert_eq!(manifest["profile_id"], Value::String("bundle".to_string()));
        assert_eq!(
            manifest["artifact"]["path"],
            Value::String("profile.taikai_cache_profile.to".to_string())
        );

        let index_path = output_dir.path().join("index.json");
        assert!(index_path.exists());
        let index: Value =
            json::from_slice(&fs::read(index_path).expect("read index")).expect("parse index");
        assert_eq!(index["profiles"].as_array().map(|arr| arr.len()), Some(1));
    }
}
