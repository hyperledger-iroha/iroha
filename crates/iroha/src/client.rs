//! Contains the end-point querying logic.  This is where you need to
//! add any custom end-point related logic.

use std::{
    collections::HashMap,
    fmt,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::{Duration, Instant},
};

use base64::Engine as _;
use derive_more::Display;
use eyre::{Result, WrapErr, eyre};
use futures_util::{Stream, StreamExt, stream};
use http_default::{AsyncWebSocketStream, WebSocketStream};
pub use iroha_config::client_api::{
    ConfidentialGas as ConfidentialGasDTO, ConfigGetDTO, ConfigUpdateDTO, Logger as LoggerDTO,
};
use iroha_config::parameters::actual::SorafsRolloutPhase;
use iroha_crypto::Hash;
use iroha_data_model::{
    block::consensus::{
        EvidenceRecord, SumeragiDaGateReason, SumeragiDaGateSatisfaction, SumeragiQcEntry,
        SumeragiQcSnapshot, SumeragiStatusWire,
    },
    da::{
        ingest::{DaIngestReceipt, DaIngestRequest},
        manifest::DaManifestV1,
        types::{BlobDigest, ExtraMetadata},
    },
    nexus::AssetPermissionManifest,
};
use iroha_logger::prelude::*;
pub use iroha_telemetry::metrics::{Status, TxGossipSnapshot, Uptime};
use iroha_torii_shared::uri as torii_uri;
use iroha_version::{DecodeAll, codec::EncodeVersioned};
use norito::{
    codec::Decode,
    decode_from_bytes,
    derive::{JsonDeserialize, JsonSerialize},
    json::{Map as JsonMap, Value as JsonValue},
    to_bytes,
};
use rand::Rng;
use sorafs_manifest::{
    alias_cache::{decode_alias_proof, unix_now_secs},
    pdp::PdpCommitmentV1,
};
use sorafs_orchestrator::{
    AnonymityPolicy, OrchestratorConfig, PolicyOverride, RolloutPhase, TransportPolicy,
    WriteModeHint, fetch_via_gateway as orchestrator_fetch_via_gateway,
    prelude::{
        CarBuildPlan, FetchSession as SorafsFetchOutcome,
        GatewayFetchConfig as SorafsGatewayFetchConfig,
        GatewayProviderInput as SorafsGatewayProviderInput, GuardSet, RelayDirectory,
    },
};
use thiserror::Error;
use url::Url;

use self::{blocks_api::AsyncBlockStream, events_api::AsyncEventStream};
pub use crate::query::QueryError;
use crate::{
    config::Config,
    crypto::{HashOf, KeyPair},
    da::{
        DaIngestParams, DaManifestBundle, DaManifestPersistedPaths, DaProofArtifactMetadata,
        DaProofConfig, PDP_COMMITMENT_HEADER, build_car_plan_from_manifest, build_da_request,
        decode_pdp_commitment_header, generate_da_proof_artifact, generate_da_proof_summary,
    },
    data_model::{
        ChainId,
        block::{BlockHeader, SignedBlock},
        consensus::ExecutionQcRecord,
        events::pipeline::{
            BlockEventFilter, BlockStatus, PipelineEventBox, PipelineEventFilterBox,
            TransactionEventFilter, TransactionStatus,
        },
        prelude::*,
        transaction::{
            TransactionBuilder, TransactionEntrypoint, error::TransactionRejectionReason,
        },
    },
    http::{Method as HttpMethod, RequestBuilder, Response, StatusCode},
    http_default::{self, DefaultRequest, DefaultRequestBuilder, WebSocketError, WebSocketMessage},
    nexus::{CrossLaneTransferProof, verify_lane_relay_envelopes},
};
// (No query imports needed here)

const APPLICATION_JSON: &str = "application/json";
const HEADER_API_VERSION: &str = iroha_torii_shared::HEADER_API_VERSION;
const HEADER_SORA_CLIENT: &str = "x-sorafs-client";
const HEADER_SORA_NONCE: &str = "x-sorafs-nonce";
pub(crate) const APPLICATION_NORITO: &str = "application/x-norito";
// Integration scenarios involving DA/RBC can legitimately spend a few seconds
// in the mempool before proposal assembly starts; keep the queue grace period
// generous enough to avoid spurious timeouts.
const DEFAULT_MAX_QUEUED_DURATION: Duration = Duration::from_secs(60);
const HEADER_SORA_PROOF: &str = "sora-proof";
const HEADER_SORA_NAME: &str = "sora-name";
const HEADER_SORA_PROOF_STATUS: &str = "sora-proof-status";

/// `Result` with [`QueryError`] as an error
pub type QueryResult<T> = core::result::Result<T, QueryError>;

/// Filters for `/v1/zk/prover/reports` listing/counting/deletion endpoints.
#[derive(Debug, Default, Clone)]
pub struct ZkProverReportsFilter<'a> {
    /// Only successful reports
    pub ok_only: Option<bool>,
    /// Only failed reports
    pub failed_only: Option<bool>,
    /// Only error messages (alias for failed-only in some endpoints)
    pub errors_only: Option<bool>,
    /// Exact id filter
    pub id: Option<&'a str>,
    /// Content-type substring filter
    pub content_type: Option<&'a str>,
    /// Require a particular ZK1 tag (e.g., PROF, IPAK)
    pub has_tag: Option<&'a str>,
    /// Limit number of results
    pub limit: Option<u32>,
    /// Lower bound for `processed_ms`
    pub since_ms: Option<u64>,
    /// Upper bound for `processed_ms`
    pub before_ms: Option<u64>,
    /// Return only ids
    pub ids_only: Option<bool>,
    /// Sort order (`asc`/`desc`)
    pub order: Option<&'a str>,
    /// Offset for pagination
    pub offset: Option<u32>,
    /// Return only the latest matching report
    pub latest: Option<bool>,
    /// Return only messages payloads
    pub messages_only: Option<bool>,
}

impl ZkProverReportsFilter<'_> {
    fn apply(&self, mut req: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(true) = self.ok_only {
            req = req.param("ok_only", &"true");
        }
        if let Some(true) = self.failed_only {
            req = req.param("failed_only", &"true");
        }
        if let Some(true) = self.errors_only {
            req = req.param("errors_only", &"true");
        }
        if let Some(v) = self.id {
            req = req.param("id", &v);
        }
        if let Some(v) = self.content_type {
            req = req.param("content_type", &v);
        }
        if let Some(v) = self.has_tag {
            req = req.param("has_tag", &v);
        }
        if let Some(v) = self.limit {
            req = req.param("limit", &v);
        }
        if let Some(v) = self.since_ms {
            req = req.param("since_ms", &v);
        }
        if let Some(v) = self.before_ms {
            req = req.param("before_ms", &v);
        }
        if let Some(true) = self.ids_only {
            req = req.param("ids_only", &"true");
        }
        if let Some(v) = self.order {
            req = req.param("order", &v);
        }
        if let Some(v) = self.offset {
            req = req.param("offset", &v);
        }
        if let Some(true) = self.latest {
            req = req.param("latest", &"true");
        }
        if let Some(true) = self.messages_only {
            req = req.param("messages_only", &"true");
        }
        req
    }
}

/// Filters for `/v1/zk/proofs` list/count endpoints.
#[derive(Debug, Default, Clone)]
pub struct ZkProofsFilter<'a> {
    /// Exact backend (e.g., `halo2/ipa`).
    pub backend: Option<&'a str>,
    /// Status filter (`Submitted`, `Verified`, `Rejected`).
    pub status: Option<&'a str>,
    /// Require a specific ZK1 TLV tag (4 ASCII characters).
    pub has_tag: Option<&'a str>,
    /// Minimum `verified_at_height` (inclusive).
    pub verified_from_height: Option<u64>,
    /// Maximum `verified_at_height` (inclusive).
    pub verified_until_height: Option<u64>,
    /// Maximum number of results (server caps at 1000).
    pub limit: Option<u32>,
    /// Offset for server-side pagination.
    pub offset: Option<u32>,
    /// Sort order (`asc` or `desc`).
    pub order: Option<&'a str>,
    /// Return only `{ backend, hash }` objects when true.
    pub ids_only: Option<bool>,
}

impl ZkProofsFilter<'_> {
    fn apply(&self, mut req: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(v) = self.backend {
            req = req.param("backend", &v);
        }
        if let Some(v) = self.status {
            req = req.param("status", &v);
        }
        if let Some(v) = self.has_tag {
            req = req.param("has_tag", &v);
        }
        if let Some(v) = self.verified_from_height {
            req = req.param("verified_from_height", &v);
        }
        if let Some(v) = self.verified_until_height {
            req = req.param("verified_until_height", &v);
        }
        if let Some(v) = self.limit {
            req = req.param("limit", &v);
        }
        if let Some(v) = self.offset {
            req = req.param("offset", &v);
        }
        if let Some(v) = self.order {
            req = req.param("order", &v);
        }
        if let Some(true) = self.ids_only {
            req = req.param("ids_only", &"true");
        }
        req
    }
}

/// Filters for `/v1/sorafs/pin` listing endpoint.
#[derive(Debug, Default, Clone)]
pub struct SorafsPinListFilter<'a> {
    /// Maximum number of manifests to return.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
    /// Optional status filter (`pending`, `approved`, `retired`).
    pub status: Option<&'a str>,
}

impl SorafsPinListFilter<'_> {
    fn apply(&self, mut req: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(limit) = self.limit {
            req = req.param("limit", &limit);
        }
        if let Some(offset) = self.offset {
            req = req.param("offset", &offset);
        }
        if let Some(status) = self.status {
            req = req.param("status", &status);
        }
        req
    }
}

/// Filters for `/v1/sorafs/aliases` listing endpoint.
#[derive(Debug, Default, Clone)]
pub struct SorafsAliasListFilter<'a> {
    /// Maximum number of aliases to return.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
    /// Optional namespace filter.
    pub namespace: Option<&'a str>,
    /// Optional manifest digest filter (hex-encoded).
    pub manifest_digest: Option<&'a str>,
}

impl SorafsAliasListFilter<'_> {
    fn apply(&self, mut req: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(limit) = self.limit {
            req = req.param("limit", &limit);
        }
        if let Some(offset) = self.offset {
            req = req.param("offset", &offset);
        }
        if let Some(namespace) = self.namespace {
            req = req.param("namespace", &namespace);
        }
        if let Some(digest) = self.manifest_digest {
            req = req.param("manifest_digest", &digest);
        }
        req
    }
}

/// Filters for `/v1/sorafs/replication` listing endpoint.
#[derive(Debug, Default, Clone)]
pub struct SorafsReplicationListFilter<'a> {
    /// Maximum number of replication orders to return.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
    /// Optional status filter (`pending`, `completed`, `expired`).
    pub status: Option<&'a str>,
    /// Optional manifest digest filter (hex-encoded).
    pub manifest_digest: Option<&'a str>,
}

impl SorafsReplicationListFilter<'_> {
    fn apply(&self, mut req: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(limit) = self.limit {
            req = req.param("limit", &limit);
        }
        if let Some(offset) = self.offset {
            req = req.param("offset", &offset);
        }
        if let Some(status) = self.status {
            req = req.param("status", &status);
        }
        if let Some(digest) = self.manifest_digest {
            req = req.param("manifest_digest", &digest);
        }
        req
    }
}

/// Optional overrides supplied when requesting a `SoraFS` stream token.
#[derive(Debug, Default, Clone, Copy)]
pub struct SorafsTokenOverrides {
    /// Override the default TTL (seconds) configured on the gateway.
    pub ttl_secs: Option<u64>,
    /// Override the maximum concurrent stream budget.
    pub max_streams: Option<u16>,
    /// Override the sustained throughput limit (bytes per second).
    pub rate_limit_bytes: Option<u64>,
    /// Override the allowed requests per minute.
    pub requests_per_minute: Option<u32>,
}

/// Optional tuning knobs applied when orchestrating `SoraFS` gateway fetches.
#[derive(Debug, Default, Clone)]
pub struct SorafsGatewayFetchOptions {
    /// Maximum retry attempts per chunk before aborting the session.
    pub retry_budget: Option<usize>,
    /// Hard cap on the number of providers used in a session.
    pub max_peers: Option<usize>,
    /// Override the telemetry region label emitted with orchestrator metrics.
    pub telemetry_region: Option<String>,
    /// Override the default `soranet-first` transport policy. Use
    /// [`TransportPolicy::DirectOnly`] only when staging a downgrade or when a compliance policy
    /// temporarily forbids relay use; pass [`TransportPolicy::SoranetStrict`] to require PQ relays.
    pub transport_policy: Option<TransportPolicy>,
    /// Override the staged anonymity policy applied to `SoraNet` providers (defaults to Stage A / `anon-guard-pq`).
    pub anonymity_policy: Option<AnonymityPolicy>,
    /// Optional guard cache describing pinned `SoraNet` relays.
    pub guard_set: Option<GuardSet>,
    /// Optional `SoraNet` directory describing the available relays.
    pub relay_directory: Option<RelayDirectory>,
    /// Optional write-mode hint to tighten PQ requirements (e.g., uploads).
    pub write_mode_hint: Option<WriteModeHint>,
    /// Overrides forcing a specific transport/anonymity stage for this request.
    pub policy_override: PolicyOverride,
    /// Optional scoreboard controls used for adoption evidence.
    pub scoreboard: Option<SorafsGatewayScoreboardOptions>,
    /// Expected cache/denylist version advertised by gateway responses.
    pub expected_cache_version: Option<String>,
    /// Base64-encoded moderation proof key for validating denylist tokens.
    pub moderation_token_key_b64: Option<String>,
}

/// Scoreboard persistence and evaluation overrides for gateway fetches.
#[derive(Debug, Default, Clone)]
pub struct SorafsGatewayScoreboardOptions {
    /// Persist the scoreboard JSON artefact to the provided path.
    pub persist_path: Option<PathBuf>,
    /// Override the Unix timestamp (seconds) used when evaluating adverts.
    pub now_unix_secs: Option<u64>,
    /// Optional metadata blob (serialized as JSON) persisted alongside the scoreboard entries.
    pub metadata: Option<JsonValue>,
    /// Human-readable label describing the telemetry stream that produced the scoreboard snapshot.
    pub telemetry_source_label: Option<String>,
}

/// Response returned after submitting a DA ingest request via [`Client::submit_da_blob`].
#[derive(Debug, Clone)]
pub struct DaIngestSubmitResult {
    /// Status string reported by Torii (typically `accepted`).
    pub status: String,
    /// Whether the submission reused an existing manifest/sequence pair.
    pub duplicate: bool,
    /// Parsed ingest receipt, when the server returns one.
    pub receipt: Option<DaIngestReceipt>,
    /// Rendered JSON form of the receipt for logging/persistence.
    pub receipt_json: Option<String>,
    /// Optional PDP commitment advertised via the `sora-pdp-commitment` header.
    pub pdp_commitment: Option<PdpCommitmentV1>,
    /// Canonical client blob identifier bound to the payload.
    pub client_blob_id: BlobDigest,
    /// Raw payload length in bytes.
    pub payload_len: usize,
}

/// File-system artefacts emitted when persisting DA ingest requests/results.
#[derive(Debug, Clone)]
pub struct DaIngestPersistedPaths {
    /// Path to the Norito-encoded ingest request.
    pub request_raw: PathBuf,
    /// Path to the rendered ingest request JSON.
    pub request_json: PathBuf,
    /// Path to the Norito-encoded receipt, when present.
    pub receipt_raw: Option<PathBuf>,
    /// Path to the rendered receipt JSON, when present.
    pub receipt_json: Option<PathBuf>,
    /// Path to the response header JSON (e.g., PDP commitment), when present.
    pub response_headers_json: Option<PathBuf>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct DaIngestResponsePayload {
    status: String,
    duplicate: bool,
    receipt: Option<DaIngestReceipt>,
}

/// Aggregated artefacts returned by [`Client::prove_da_availability`].
#[derive(Debug, Clone)]
pub struct DaAvailabilityProof {
    /// Manifest/chunk plan bundle used for verification.
    pub manifest: DaManifestBundle,
    /// Detailed orchestrator session, including chunk receipts and provider telemetry.
    pub fetch_session: SorafsFetchOutcome,
    /// `PoR` summary mirroring `iroha da prove --json-out`.
    pub proof_summary: JsonValue,
    /// Sampling configuration used to derive the proof summary.
    pub proof_config: DaProofConfig,
}

/// File system artefacts produced when persisting a DA availability proof.
#[derive(Debug, Clone)]
pub struct DaAvailabilityProofPersistedPaths {
    /// Paths to the manifest artefacts written to disk.
    pub manifest: DaManifestPersistedPaths,
    /// Path to the assembled payload fetched from the gateway.
    pub payload_path: PathBuf,
    /// Path to the rendered proof summary JSON.
    pub proof_summary_path: PathBuf,
    /// Path to the persisted scoreboard JSON, when enabled.
    pub scoreboard_path: Option<PathBuf>,
}

fn derive_scoreboard_telemetry_label(
    explicit: Option<&str>,
    options: &SorafsGatewayFetchOptions,
    chain_id: &ChainId,
) -> String {
    if let Some(label) = explicit {
        let trimmed = label.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(region) = options.telemetry_region.as_deref() {
        let trimmed = region.trim();
        if !trimmed.is_empty() {
            return format!("region:{trimmed}");
        }
    }
    format!("chain:{chain_id}")
}

fn base_scoreboard_metadata() -> JsonMap {
    let mut map = JsonMap::new();
    map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));
    map.insert("use_scoreboard".into(), JsonValue::from(true));
    map.insert("allow_implicit_metadata".into(), JsonValue::from(false));
    map.insert("gateway_manifest_provided".into(), JsonValue::Null);
    map
}

fn ensure_scoreboard_metadata(
    metadata: Option<JsonValue>,
    telemetry_label: Option<&str>,
    assume_now: u64,
) -> JsonValue {
    let mut map = match metadata {
        Some(JsonValue::Object(map)) => map,
        Some(other) => return other,
        None => base_scoreboard_metadata(),
    };
    let insert_if_absent = |map: &mut JsonMap, key: &str, value: JsonValue| match map.get(key) {
        Some(existing) if !existing.is_null() => {}
        _ => {
            map.insert(key.into(), value);
        }
    };
    insert_if_absent(&mut map, "assume_now", JsonValue::from(assume_now));
    if let Some(label) = telemetry_label
        && !label.is_empty()
    {
        insert_if_absent(&mut map, "telemetry_source", JsonValue::from(label));
    }
    JsonValue::Object(map)
}

fn annotate_scoreboard_with_gateway_context(
    metadata: &mut JsonValue,
    gateway_config: &SorafsGatewayFetchConfig,
) {
    let JsonValue::Object(map) = metadata else {
        return;
    };

    let manifest_id = gateway_config.manifest_id_hex.trim().to_ascii_lowercase();
    map.insert("gateway_manifest_id".into(), JsonValue::from(manifest_id));

    let manifest_cid_value =
        gateway_config
            .expected_manifest_cid_hex
            .as_deref()
            .map_or(JsonValue::Null, |cid| {
                let trimmed = cid.trim();
                if trimmed.is_empty() {
                    JsonValue::Null
                } else {
                    JsonValue::from(trimmed.to_ascii_lowercase())
                }
            });
    map.insert("gateway_manifest_cid".into(), manifest_cid_value);

    map.insert(
        "gateway_manifest_provided".into(),
        JsonValue::from(gateway_config.manifest_envelope_b64.is_some()),
    );
}

fn normalize_storage_ticket_hex(value: &str) -> Result<String> {
    let trimmed = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.len() != 64 {
        return Err(eyre!(
            "storage ticket must contain 64 hexadecimal characters (got {})",
            trimmed.len()
        ));
    }
    hex::decode(trimmed).map_err(|err| eyre!("invalid storage ticket hex: {err}"))?;
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_block_hash_hex(value: &str) -> Result<String> {
    let trimmed = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.len() != 64 {
        return Err(eyre!(
            "block hash must contain 64 hexadecimal characters (got {})",
            trimmed.len()
        ));
    }
    Hash::from_str(trimmed).map_err(|err| eyre!("invalid block hash: {err}"))?;
    Ok(trimmed.to_ascii_lowercase())
}

/// Aggregated portfolio totals returned by the UAID portfolio endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UaidPortfolioTotals {
    /// Number of ledger accounts referencing the UAID.
    pub accounts: u64,
    /// Total count of non-zero asset positions across those accounts.
    pub positions: u64,
}

/// Asset position grouped under a UAID-backed account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidPortfolioAsset {
    /// Fully-qualified asset identifier (`asset#domain::account@domain`).
    pub asset_id: String,
    /// Asset definition identifier.
    pub asset_definition_id: String,
    /// Norito numeric quantity (string encoded to preserve precision).
    pub quantity: String,
}

/// Account entry returned by the UAID portfolio endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidPortfolioAccount {
    /// Ledger account identifier.
    pub account_id: String,
    /// Optional human-readable label attached to the account.
    pub label: Option<String>,
    /// Sorted list of asset positions held by the account.
    pub assets: Vec<UaidPortfolioAsset>,
}

/// Dataspace slice aggregating UAID holdings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidPortfolioDataspace {
    /// Numeric dataspace identifier.
    pub dataspace_id: u64,
    /// Optional dataspace alias resolved from the catalog.
    pub dataspace_alias: Option<String>,
    /// Accounts bound to the UAID inside this dataspace.
    pub accounts: Vec<UaidPortfolioAccount>,
}

/// Response returned by `/v1/accounts/{uaid}/portfolio`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidPortfolioResponse {
    /// Canonical UAID literal (`uaid:<64-hex>`).
    pub uaid: String,
    /// Aggregated totals spanning every dataspace.
    pub totals: UaidPortfolioTotals,
    /// Dataspace-specific holdings.
    pub dataspaces: Vec<UaidPortfolioDataspace>,
}

/// Dataspace bindings entry returned by `/v1/space-directory/uaids/{uaid}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidBindingsDataspace {
    /// Dataspace identifier.
    pub dataspace_id: u64,
    /// Optional alias assigned to the dataspace.
    pub dataspace_alias: Option<String>,
    /// Accounts within the dataspace that reference the UAID.
    pub accounts: Vec<String>,
}

/// Response returned by `/v1/space-directory/uaids/{uaid}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidBindingsResponse {
    /// Canonical UAID literal (`uaid:<64-hex>`).
    pub uaid: String,
    /// Dataspace bindings tracked by the Space Directory.
    pub dataspaces: Vec<UaidBindingsDataspace>,
}

/// Preferred textual representation for account identifiers in Torii responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFormat {
    /// IH58 literal (`ih…@domain`).
    Ih58,
    /// Compressed literal (`snx1…@domain`).
    Compressed,
}

impl AddressFormat {
    fn as_query_str(self) -> &'static str {
        match self {
            Self::Ih58 => "ih58",
            Self::Compressed => "compressed",
        }
    }
}

/// Optional knobs accepted by the explorer QR endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExplorerAccountQrOptions {
    /// Preferred address literal encoding (`ih58` by default).
    pub address_format: Option<AddressFormat>,
}

impl ExplorerAccountQrOptions {
    fn apply(self, builder: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(format) = self.address_format {
            builder.param("address_format", format.as_query_str())
        } else {
            builder
        }
    }
}

/// Snapshot returned by `/v1/explorer/accounts/{account_id}/qr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAccountQrSnapshot {
    /// Canonical account identifier (`ih58@domain`).
    pub canonical_id: String,
    /// Rendered literal using the requested address format.
    pub literal: String,
    /// Address format preferred by the caller.
    pub address_format: AddressFormat,
    /// IH58 prefix for the network.
    pub network_prefix: u16,
    /// QR error-correction label (`L`, `M`, `Q`, `H`).
    pub error_correction: String,
    /// Module width/height in pixels.
    pub modules: u32,
    /// QR version number reported by the renderer.
    pub qr_version: u8,
    /// Inline SVG payload for direct embedding.
    pub svg: String,
}

impl ExplorerAccountQrSnapshot {
    fn from_value(value: JsonValue) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("explorer account qr response must be an object"));
        };
        let canonical_id = require_string(
            map.get("canonical_id"),
            "explorer account qr response.canonical_id",
        )?
        .to_owned();
        let literal =
            require_string(map.get("literal"), "explorer account qr response.literal")?.to_owned();
        let address_format_raw = require_string(
            map.get("address_format"),
            "explorer account qr response.address_format",
        )?;
        let address_format = match address_format_raw.to_ascii_lowercase().as_str() {
            "ih58" => AddressFormat::Ih58,
            "compressed" => AddressFormat::Compressed,
            other => {
                return Err(eyre!(
                    "explorer account qr response.address_format must be `ih58` or `compressed` \
                     (got `{other}`)"
                ));
            }
        };
        let network_prefix = parse_required_u64(
            map.get("network_prefix"),
            "explorer account qr response.network_prefix",
        )?;
        let network_prefix = u16::try_from(network_prefix).map_err(|_| {
            eyre!(
                "explorer account qr response.network_prefix must fit in a u16 (got {network_prefix})"
            )
        })?;
        let modules =
            parse_required_u64(map.get("modules"), "explorer account qr response.modules")?;
        let modules = u32::try_from(modules).map_err(|_| {
            eyre!("explorer account qr response.modules must fit in a u32 (got {modules})")
        })?;
        let qr_version = parse_required_u64(
            map.get("qr_version"),
            "explorer account qr response.qr_version",
        )?;
        let qr_version = u8::try_from(qr_version).map_err(|_| {
            eyre!("explorer account qr response.qr_version must fit in a u8 (got {qr_version})")
        })?;
        let error_correction = require_string(
            map.get("error_correction"),
            "explorer account qr response.error_correction",
        )?
        .to_owned();
        let svg = require_string(map.get("svg"), "explorer account qr response.svg")?.to_owned();
        Ok(Self {
            canonical_id,
            literal,
            address_format,
            network_prefix,
            error_correction,
            modules,
            qr_version,
            svg,
        })
    }
}

/// Query parameters accepted by the UAID bindings endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UaidBindingsQuery {
    /// Optional address format for the `accounts` arrays.
    pub address_format: Option<AddressFormat>,
}

impl UaidBindingsQuery {
    fn apply(self, mut builder: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(format) = self.address_format {
            builder = builder.param("address_format", format.as_query_str());
        }
        builder
    }
}

/// Revocation metadata attached to a manifest lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidManifestRevocation {
    /// Epoch when the manifest was revoked.
    pub epoch: u64,
    /// Optional reason string supplied by governance.
    pub reason: Option<String>,
}

/// Lifecycle summary attached to a UAID manifest record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidManifestLifecycle {
    /// Epoch when the manifest became active.
    pub activated_epoch: Option<u64>,
    /// Epoch when the manifest expired (if scheduled).
    pub expired_epoch: Option<u64>,
    /// Revocation metadata when the manifest was explicitly denied.
    pub revocation: Option<UaidManifestRevocation>,
}

/// Manifest status reported by `/v1/space-directory/uaids/{uaid}/manifests`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UaidManifestStatus {
    /// Manifest is pending activation.
    Pending,
    /// Manifest is currently active.
    Active,
    /// Manifest reached its expiry epoch.
    Expired,
    /// Manifest was revoked before expiry (deny wins).
    Revoked,
}

impl UaidManifestStatus {
    fn from_str(raw: &str, context: &str) -> Result<Self> {
        match raw {
            "Pending" => Ok(Self::Pending),
            "Active" => Ok(Self::Active),
            "Expired" => Ok(Self::Expired),
            "Revoked" => Ok(Self::Revoked),
            _ => Err(eyre!(
                "{context} must be Pending, Active, Expired, or Revoked"
            )),
        }
    }
}

/// Filter for `/v1/space-directory/uaids/{uaid}/manifests`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UaidManifestStatusFilter {
    /// Return only active manifests.
    Active,
    /// Return pending/expired/revoked manifests.
    Inactive,
    /// Return every manifest regardless of lifecycle.
    All,
}

impl UaidManifestStatusFilter {
    fn as_query_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::All => "all",
        }
    }
}

/// Query parameters accepted by the UAID manifest endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UaidManifestQuery {
    /// Optional dataspace filter.
    pub dataspace_id: Option<u64>,
    /// Optional lifecycle filter.
    pub status: Option<UaidManifestStatusFilter>,
    /// Maximum number of manifests to return.
    pub limit: Option<u32>,
    /// Number of manifests to skip before collecting results.
    pub offset: Option<u32>,
    /// Optional address format for the embedded account lists.
    pub address_format: Option<AddressFormat>,
}

impl UaidManifestQuery {
    fn apply(&self, mut builder: DefaultRequestBuilder) -> DefaultRequestBuilder {
        if let Some(id) = self.dataspace_id {
            builder = builder.param("dataspace", &id);
        }
        if let Some(status) = self.status {
            builder = builder.param("status", status.as_query_str());
        }
        if let Some(limit) = self.limit {
            builder = builder.param("limit", &limit);
        }
        if let Some(offset) = self.offset {
            builder = builder.param("offset", &offset);
        }
        if let Some(format) = self.address_format {
            builder = builder.param("address_format", format.as_query_str());
        }
        builder
    }
}

/// UAID manifest entry returned by `/v1/space-directory/uaids/{uaid}/manifests`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidManifestRecord {
    /// Dataspace identifier.
    pub dataspace_id: u64,
    /// Optional dataspace alias resolved from the catalog.
    pub dataspace_alias: Option<String>,
    /// Canonical manifest digest (hex string).
    pub manifest_hash: String,
    /// Lifecycle status.
    pub status: UaidManifestStatus,
    /// Lifecycle metadata emitted by the Space Directory.
    pub lifecycle: UaidManifestLifecycle,
    /// Accounts bound to the manifest.
    pub accounts: Vec<String>,
    /// Canonical manifest payload.
    pub manifest: AssetPermissionManifest,
}

/// Response returned by `/v1/space-directory/uaids/{uaid}/manifests`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaidManifestsResponse {
    /// Canonical UAID literal (`uaid:<64-hex>`).
    pub uaid: String,
    /// Optional total count returned by Torii when pagination is enabled.
    pub total: Option<u64>,
    /// Manifest entries visible to the caller.
    pub manifests: Vec<UaidManifestRecord>,
}

impl UaidPortfolioResponse {
    fn from_value(value: JsonValue) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("uaid portfolio response must be an object"));
        };
        let uaid_raw = require_string(map.get("uaid"), "uaid portfolio response.uaid")?;
        let uaid = canonicalize_uaid_literal(uaid_raw, "uaid portfolio response.uaid")?;

        let totals = match map.get("totals") {
            Some(JsonValue::Object(obj)) => {
                let accounts = parse_optional_u64(
                    obj.get("accounts"),
                    "uaid portfolio response.totals.accounts",
                )?
                .unwrap_or(0);
                let positions = parse_optional_u64(
                    obj.get("positions"),
                    "uaid portfolio response.totals.positions",
                )?
                .unwrap_or(0);
                UaidPortfolioTotals {
                    accounts,
                    positions,
                }
            }
            Some(JsonValue::Null) | None => UaidPortfolioTotals {
                accounts: 0,
                positions: 0,
            },
            Some(_) => {
                return Err(eyre!(
                    "uaid portfolio response.totals must be an object when present"
                ));
            }
        };

        let dataspace_entries =
            owned_array(map.get("dataspaces"), "uaid portfolio response.dataspaces")?;
        let dataspaces = dataspace_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                UaidPortfolioDataspace::from_value(
                    entry,
                    &format!("uaid portfolio response.dataspaces[{index}]"),
                )
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            uaid,
            totals,
            dataspaces,
        })
    }
}

impl UaidPortfolioDataspace {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let dataspace_id =
            parse_required_u64(map.get("dataspace_id"), &format!("{context}.dataspace_id"))?;
        let dataspace_alias = parse_optional_string(
            map.get("dataspace_alias"),
            &format!("{context}.dataspace_alias"),
        )?;
        let account_entries = owned_array(map.get("accounts"), &format!("{context}.accounts"))?;
        let accounts = account_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                UaidPortfolioAccount::from_value(entry, &format!("{context}.accounts[{index}]"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            dataspace_id,
            dataspace_alias,
            accounts,
        })
    }
}

impl UaidPortfolioAccount {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let account_id =
            require_string(map.get("account_id"), &format!("{context}.account_id"))?.to_owned();
        let label = parse_optional_string(map.get("label"), &format!("{context}.label"))?;
        let asset_entries = owned_array(map.get("assets"), &format!("{context}.assets"))?;
        let assets = asset_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                UaidPortfolioAsset::from_value(entry, &format!("{context}.assets[{index}]"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            account_id,
            label,
            assets,
        })
    }
}

impl UaidPortfolioAsset {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let asset_id =
            require_string(map.get("asset_id"), &format!("{context}.asset_id"))?.to_owned();
        let asset_definition_id = require_string(
            map.get("asset_definition_id"),
            &format!("{context}.asset_definition_id"),
        )?
        .to_owned();
        let quantity =
            require_string(map.get("quantity"), &format!("{context}.quantity"))?.to_owned();
        Ok(Self {
            asset_id,
            asset_definition_id,
            quantity,
        })
    }
}

impl UaidBindingsResponse {
    fn from_value(value: JsonValue) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("uaid bindings response must be an object"));
        };
        let uaid_raw = require_string(map.get("uaid"), "uaid bindings response.uaid")?;
        let uaid = canonicalize_uaid_literal(uaid_raw, "uaid bindings response.uaid")?;
        let dataspace_entries =
            owned_array(map.get("dataspaces"), "uaid bindings response.dataspaces")?;
        let dataspaces = dataspace_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                UaidBindingsDataspace::from_value(
                    entry,
                    &format!("uaid bindings response.dataspaces[{index}]"),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { uaid, dataspaces })
    }
}

impl UaidBindingsDataspace {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let dataspace_id =
            parse_required_u64(map.get("dataspace_id"), &format!("{context}.dataspace_id"))?;
        let dataspace_alias = parse_optional_string(
            map.get("dataspace_alias"),
            &format!("{context}.dataspace_alias"),
        )?;
        let accounts = string_array(map.get("accounts"), &format!("{context}.accounts"))?;
        Ok(Self {
            dataspace_id,
            dataspace_alias,
            accounts,
        })
    }
}

impl UaidManifestsResponse {
    fn from_value(value: JsonValue) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("uaid manifests response must be an object"));
        };
        let uaid_raw = require_string(map.get("uaid"), "uaid manifests response.uaid")?;
        let uaid = canonicalize_uaid_literal(uaid_raw, "uaid manifests response.uaid")?;
        let total = parse_optional_u64(map.get("total"), "uaid manifests response.total")?;
        let manifest_entries =
            owned_array(map.get("manifests"), "uaid manifests response.manifests")?;
        let manifests = manifest_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                UaidManifestRecord::from_value(
                    entry,
                    &format!("uaid manifests response.manifests[{index}]"),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            uaid,
            total,
            manifests,
        })
    }
}

impl UaidManifestRecord {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let dataspace_id =
            parse_required_u64(map.get("dataspace_id"), &format!("{context}.dataspace_id"))?;
        let dataspace_alias = parse_optional_string(
            map.get("dataspace_alias"),
            &format!("{context}.dataspace_alias"),
        )?;
        let manifest_hash_raw = require_string(
            map.get("manifest_hash"),
            &format!("{context}.manifest_hash"),
        )?;
        let manifest_hash =
            canonicalize_hex32_literal(manifest_hash_raw, &format!("{context}.manifest_hash"))?;
        let status_raw = require_string(map.get("status"), &format!("{context}.status"))?;
        let status = UaidManifestStatus::from_str(status_raw, &format!("{context}.status"))?;
        let lifecycle_value = map
            .get("lifecycle")
            .cloned()
            .unwrap_or_else(|| JsonValue::Object(JsonMap::new()));
        let lifecycle =
            UaidManifestLifecycle::from_value(lifecycle_value, &format!("{context}.lifecycle"))?;
        let accounts = string_array(map.get("accounts"), &format!("{context}.accounts"))?;
        let manifest_value = map
            .get("manifest")
            .ok_or_else(|| eyre!("{context}.manifest is missing"))?;
        let manifest = parse_manifest(manifest_value, &format!("{context}.manifest"))?;
        Ok(Self {
            dataspace_id,
            dataspace_alias,
            manifest_hash,
            status,
            lifecycle,
            accounts,
            manifest,
        })
    }
}

impl UaidManifestLifecycle {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let activated_epoch = parse_optional_u64(
            map.get("activated_epoch"),
            &format!("{context}.activated_epoch"),
        )?;
        let expired_epoch = parse_optional_u64(
            map.get("expired_epoch"),
            &format!("{context}.expired_epoch"),
        )?;
        let revocation = match map.get("revocation") {
            Some(JsonValue::Null) | None => None,
            Some(other) => Some(UaidManifestRevocation::from_value(
                other.clone(),
                &format!("{context}.revocation"),
            )?),
        };
        Ok(Self {
            activated_epoch,
            expired_epoch,
            revocation,
        })
    }
}

impl UaidManifestRevocation {
    fn from_value(value: JsonValue, context: &str) -> Result<Self> {
        let JsonValue::Object(map) = value else {
            return Err(eyre!("{context} must be an object"));
        };
        let epoch = parse_required_u64(map.get("epoch"), &format!("{context}.epoch"))?;
        let reason = parse_optional_string(map.get("reason"), &format!("{context}.reason"))?;
        Ok(Self { epoch, reason })
    }
}

fn canonicalize_uaid_literal(literal: &str, context: &str) -> Result<String> {
    let trimmed = literal.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{context} must be a non-empty UAID literal"));
    }
    let hex_portion = trimmed
        .strip_prefix("uaid:")
        .or_else(|| trimmed.strip_prefix("UAID:"))
        .unwrap_or(trimmed);
    if hex_portion.len() != 64 || !hex_portion.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre!(
            "{context} must contain exactly 64 hexadecimal characters"
        ));
    }
    Ok(format!("uaid:{}", hex_portion.to_ascii_lowercase()))
}

fn canonicalize_hex32_literal(literal: &str, context: &str) -> Result<String> {
    let trimmed = literal.trim();
    if trimmed.len() != 64 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre!(
            "{context} must contain exactly 64 hexadecimal characters"
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn require_string<'a>(value: Option<&'a JsonValue>, context: &str) -> Result<&'a str> {
    match value {
        Some(JsonValue::String(text)) => Ok(text),
        Some(other) => Err(eyre!("{context} must be a string (got {other:?})")),
        None => Err(eyre!("{context} is missing")),
    }
}

fn parse_optional_string(value: Option<&JsonValue>, context: &str) -> Result<Option<String>> {
    match value {
        Some(JsonValue::String(text)) => Ok(Some(text.to_owned())),
        Some(JsonValue::Null) | None => Ok(None),
        Some(other) => Err(eyre!(
            "{context} must be a string when present (got {other:?})"
        )),
    }
}

fn parse_required_u64(value: Option<&JsonValue>, context: &str) -> Result<u64> {
    let Some(raw) = value else {
        return Err(eyre!("{context} is missing"));
    };
    parse_u64(raw, context)
}

fn parse_optional_u64(value: Option<&JsonValue>, context: &str) -> Result<Option<u64>> {
    match value {
        Some(JsonValue::Null) | None => Ok(None),
        Some(other) => parse_u64(other, context).map(Some),
    }
}

fn parse_u64(value: &JsonValue, context: &str) -> Result<u64> {
    match value {
        JsonValue::Number(number) => number
            .as_u64()
            .ok_or_else(|| eyre!("{context} must be a non-negative integer")),
        JsonValue::String(text) => text
            .parse::<u64>()
            .wrap_err_with(|| format!("{context} must be a non-negative integer string")),
        other => Err(eyre!(
            "{context} must be a non-negative integer (got {other:?})"
        )),
    }
}

fn owned_array(value: Option<&JsonValue>, context: &str) -> Result<Vec<JsonValue>> {
    match value {
        Some(JsonValue::Array(items)) => Ok(items.clone()),
        Some(JsonValue::Null) | None => Ok(Vec::new()),
        Some(other) => Err(eyre!("{context} must be an array (got {other:?})")),
    }
}

fn string_array(value: Option<&JsonValue>, context: &str) -> Result<Vec<String>> {
    let entries = owned_array(value, context)?;
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| match entry {
            JsonValue::String(text) => Ok(text),
            other => Err(eyre!("{context}[{index}] must be a string (got {other:?})")),
        })
        .collect()
}

fn parse_manifest(value: &JsonValue, context: &str) -> Result<AssetPermissionManifest> {
    norito::json::from_value(value.clone())
        .wrap_err_with(|| format!("{context} must be a valid AssetPermissionManifest"))
}

/// Alias binding supplied when registering a `SoraFS` pin manifest.
#[derive(Debug, Clone, Copy)]
pub struct SorafsPinAlias<'a> {
    /// Namespace portion of the alias (for example `sora`).
    pub namespace: &'a str,
    /// Local name bound within the namespace (for example `docs`).
    pub name: &'a str,
    /// Alias proof payload supplied as raw bytes (the caller is responsible for base64 encoding).
    pub proof: &'a [u8],
}

/// Arguments required to register a `SoraFS` manifest with the pin registry.
#[derive(Clone, Copy)]
pub struct SorafsPinRegisterArgs<'a> {
    /// Account submitting the manifest registration request.
    pub authority: &'a iroha_data_model::account::AccountId,
    /// Private key used to sign the registration transaction.
    pub private_key: &'a iroha_crypto::PrivateKey,
    /// Manifest describing the chunk layout and governance proofs to register.
    pub manifest: &'a sorafs_manifest::ManifestV1,
    /// SHA3-256 digest of the manifest chunk referenced for registration.
    pub chunk_digest_sha3_256: [u8; 32],
    /// Epoch at which the registration was submitted.
    pub submitted_epoch: u64,
    /// Optional alias binding to attach to the manifest entry.
    pub alias: Option<SorafsPinAlias<'a>>,
    /// Optional predecessor manifest hash when rotating registrations.
    pub successor_of: Option<[u8; 32]>,
}

/// Errors returned when executing a `SoraFS` orchestrated fetch.
#[derive(Debug, Error)]
pub enum SorafsFetchError {
    /// Wrapper around gateway/orchestrator failures.
    #[error(transparent)]
    Orchestrator(#[from] sorafs_orchestrator::GatewayOrchestratorError),
}

/// Filters for `/v1/sumeragi/evidence` listing endpoint.
#[derive(Debug, Default, Clone)]
pub struct SumeragiEvidenceListFilter<'a> {
    /// Maximum number of entries to return (server caps at 1000).
    pub limit: Option<u32>,
    /// Offset into the persisted evidence list.
    pub offset: Option<u32>,
    /// Optional filter by evidence kind (`DoublePrepare`, `InvalidCommitCertificate`, etc.).
    pub kind: Option<&'a str>,
}

impl SumeragiEvidenceListFilter<'_> {
    fn apply(&self, mut req: DefaultRequestBuilder) -> DefaultRequestBuilder {
        for (key, value) in self.param_entries() {
            req = req.param(key, &value);
        }
        req
    }

    fn param_entries(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::with_capacity(3);
        if let Some(limit) = self.limit {
            out.push(("limit", limit.to_string()));
        }
        if let Some(offset) = self.offset {
            out.push(("offset", offset.to_string()));
        }
        if let Some(kind) = self.kind {
            out.push(("kind", kind.to_string()));
        }
        out
    }
}

/// Phantom struct that handles Transaction API HTTP response
#[derive(Clone, Copy)]
struct TransactionResponseHandler;

impl TransactionResponseHandler {
    fn handle(resp: &Response<Vec<u8>>) -> Result<()> {
        if matches!(resp.status(), StatusCode::OK | StatusCode::ACCEPTED) {
            Ok(())
        } else {
            Err(
                ResponseReport::with_msg("Unexpected transaction response", resp)
                    .unwrap_or_else(core::convert::identity)
                    .into(),
            )
        }
    }
}

/// Decode a `/status` response body, preferring Norito and falling back to JSON.
fn decode_status_response(resp: &Response<Vec<u8>>) -> Result<Status> {
    if resp.status() != StatusCode::OK {
        return Err(ResponseReport::with_msg("Unexpected status response", resp)
            .unwrap_or_else(core::convert::identity)
            .into());
    }

    let body = resp.body();
    let is_json = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));

    if is_json {
        norito::json::from_slice(body).map_err(Into::into)
    } else {
        decode_from_bytes::<Status>(body)
            .map_err(|err| eyre!("failed to decode status Norito payload: {err}"))
    }
}

fn decode_parameters_response(
    resp: &Response<Vec<u8>>,
) -> Result<iroha_data_model::parameter::Parameters> {
    if resp.status() != StatusCode::OK {
        return Err(
            ResponseReport::with_msg("Unexpected parameters response", resp)
                .unwrap_or_else(core::convert::identity)
                .into(),
        );
    }

    norito::json::from_slice(resp.body()).map_err(Into::into)
}

#[cfg(test)]
fn decode_parameters_for_test(
    resp: &Response<Vec<u8>>,
) -> Result<iroha_data_model::parameter::Parameters> {
    decode_parameters_response(resp)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

fn exec_qc_json_payload(
    hash_hex: &str,
    record_opt: Option<ExecutionQcRecord>,
) -> norito::json::Value {
    use norito::json::{Map, Value};
    if let Some(record) = record_opt {
        let mut map = Map::new();
        map.insert(
            "subject_block_hash".into(),
            Value::from(hash_hex.to_string()),
        );
        map.insert(
            "post_state_root".into(),
            Value::from(format!("{}", record.post_state_root)),
        );
        map.insert("height".into(), Value::from(record.height));
        map.insert("view".into(), Value::from(record.view));
        map.insert("epoch".into(), Value::from(record.epoch));
        map.insert(
            "signers_bitmap".into(),
            Value::from(bytes_to_hex(&record.signers_bitmap)),
        );
        map.insert(
            "bls_aggregate_signature".into(),
            Value::from(bytes_to_hex(&record.bls_aggregate_signature)),
        );
        Value::Object(map)
    } else {
        let mut map = Map::new();
        map.insert(
            "subject_block_hash".into(),
            Value::from(hash_hex.to_string()),
        );
        map.insert("exec_qc".into(), Value::Null);
        Value::Object(map)
    }
}

#[allow(clippy::too_many_lines)]
fn sumeragi_status_json_payload(wire: &SumeragiStatusWire) -> norito::json::Value {
    use iroha_data_model::block::consensus::{
        SumeragiWorkerQueueDepths, SumeragiWorkerQueueTotals,
    };
    use norito::json::{Map, Value};

    let mut highest = Map::new();
    highest.insert("height".into(), Value::from(wire.highest_qc_height));
    highest.insert("view".into(), Value::from(wire.highest_qc_view));
    highest.insert(
        "subject_block_hash".into(),
        wire.highest_qc_subject
            .as_ref()
            .map_or(Value::Null, |h| Value::from(format!("{h}"))),
    );

    let mut locked = Map::new();
    locked.insert("height".into(), Value::from(wire.locked_qc_height));
    locked.insert("view".into(), Value::from(wire.locked_qc_view));
    locked.insert(
        "subject_block_hash".into(),
        wire.locked_qc_subject
            .as_ref()
            .map_or(Value::Null, |h| Value::from(format!("{h}"))),
    );
    let mut view_change_causes = Map::new();
    view_change_causes.insert(
        "commit_failure_total".into(),
        Value::from(wire.view_change_causes.commit_failure_total),
    );
    view_change_causes.insert(
        "quorum_timeout_total".into(),
        Value::from(wire.view_change_causes.quorum_timeout_total),
    );
    view_change_causes.insert(
        "da_gate_total".into(),
        Value::from(wire.view_change_causes.da_gate_total),
    );
    view_change_causes.insert(
        "missing_payload_total".into(),
        Value::from(wire.view_change_causes.missing_payload_total),
    );
    view_change_causes.insert(
        "missing_qc_total".into(),
        Value::from(wire.view_change_causes.missing_qc_total),
    );
    view_change_causes.insert(
        "validation_reject_total".into(),
        Value::from(wire.view_change_causes.validation_reject_total),
    );
    view_change_causes.insert(
        "last_cause".into(),
        wire.view_change_causes
            .last_cause
            .as_ref()
            .map_or(Value::Null, |cause| Value::from(cause.clone())),
    );
    view_change_causes.insert(
        "last_cause_timestamp_ms".into(),
        Value::from(wire.view_change_causes.last_cause_timestamp_ms),
    );

    let mut validation_rejects = Map::new();
    validation_rejects.insert("total".into(), Value::from(wire.validation_rejects.total));
    validation_rejects.insert(
        "stateless_total".into(),
        Value::from(wire.validation_rejects.stateless_total),
    );
    validation_rejects.insert(
        "execution_total".into(),
        Value::from(wire.validation_rejects.execution_total),
    );
    validation_rejects.insert(
        "prev_hash_total".into(),
        Value::from(wire.validation_rejects.prev_hash_total),
    );
    validation_rejects.insert(
        "prev_height_total".into(),
        Value::from(wire.validation_rejects.prev_height_total),
    );
    validation_rejects.insert(
        "topology_total".into(),
        Value::from(wire.validation_rejects.topology_total),
    );
    validation_rejects.insert(
        "last_reason".into(),
        wire.validation_rejects
            .last_reason
            .as_ref()
            .map_or(Value::Null, |reason| Value::from(reason.clone())),
    );
    validation_rejects.insert(
        "last_height".into(),
        wire.validation_rejects
            .last_height
            .map_or(Value::Null, Value::from),
    );
    validation_rejects.insert(
        "last_view".into(),
        wire.validation_rejects
            .last_view
            .map_or(Value::Null, Value::from),
    );
    validation_rejects.insert(
        "last_block".into(),
        wire.validation_rejects
            .last_block
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(format!("{hash}"))),
    );
    validation_rejects.insert(
        "last_timestamp_ms".into(),
        Value::from(wire.validation_rejects.last_timestamp_ms),
    );

    let mut tx_queue = Map::new();
    tx_queue.insert("depth".into(), Value::from(wire.tx_queue_depth));
    tx_queue.insert("capacity".into(), Value::from(wire.tx_queue_capacity));
    tx_queue.insert("saturated".into(), Value::from(wire.tx_queue_saturated));

    let queue_depths_value = |depths: &SumeragiWorkerQueueDepths| {
        let mut map = Map::new();
        map.insert("vote_rx".into(), Value::from(depths.vote_rx));
        map.insert(
            "block_payload_rx".into(),
            Value::from(depths.block_payload_rx),
        );
        map.insert("rbc_chunk_rx".into(), Value::from(depths.rbc_chunk_rx));
        map.insert("block_rx".into(), Value::from(depths.block_rx));
        map.insert("consensus_rx".into(), Value::from(depths.consensus_rx));
        map.insert("lane_relay_rx".into(), Value::from(depths.lane_relay_rx));
        map.insert("background_rx".into(), Value::from(depths.background_rx));
        map
    };
    let queue_totals_value = |totals: &SumeragiWorkerQueueTotals| {
        let mut map = Map::new();
        map.insert("vote_rx".into(), Value::from(totals.vote_rx));
        map.insert(
            "block_payload_rx".into(),
            Value::from(totals.block_payload_rx),
        );
        map.insert("rbc_chunk_rx".into(), Value::from(totals.rbc_chunk_rx));
        map.insert("block_rx".into(), Value::from(totals.block_rx));
        map.insert("consensus_rx".into(), Value::from(totals.consensus_rx));
        map.insert("lane_relay_rx".into(), Value::from(totals.lane_relay_rx));
        map.insert("background_rx".into(), Value::from(totals.background_rx));
        map
    };

    let worker_queue_depths = queue_depths_value(&wire.worker_loop.queue_depths);
    let worker_queue_diagnostics = {
        let mut map = Map::new();
        map.insert(
            "blocked_total".into(),
            Value::Object(queue_totals_value(
                &wire.worker_loop.queue_diagnostics.blocked_total,
            )),
        );
        map.insert(
            "blocked_ms_total".into(),
            Value::Object(queue_totals_value(
                &wire.worker_loop.queue_diagnostics.blocked_ms_total,
            )),
        );
        map.insert(
            "blocked_max_ms".into(),
            Value::Object(queue_totals_value(
                &wire.worker_loop.queue_diagnostics.blocked_max_ms,
            )),
        );
        map.insert(
            "dropped_total".into(),
            Value::Object(queue_totals_value(
                &wire.worker_loop.queue_diagnostics.dropped_total,
            )),
        );
        map
    };

    let mut worker_loop = Map::new();
    worker_loop.insert("stage".into(), Value::from(wire.worker_loop.stage.clone()));
    worker_loop.insert(
        "stage_started_ms".into(),
        Value::from(wire.worker_loop.stage_started_ms),
    );
    worker_loop.insert(
        "last_iteration_ms".into(),
        Value::from(wire.worker_loop.last_iteration_ms),
    );
    worker_loop.insert("queue_depths".into(), Value::Object(worker_queue_depths));
    worker_loop.insert(
        "queue_diagnostics".into(),
        Value::Object(worker_queue_diagnostics),
    );

    let commit_pause_queue_depths = queue_depths_value(&wire.commit_inflight.pause_queue_depths);
    let commit_resume_queue_depths = queue_depths_value(&wire.commit_inflight.resume_queue_depths);
    let mut commit_inflight = Map::new();
    commit_inflight.insert("active".into(), Value::from(wire.commit_inflight.active));
    commit_inflight.insert("id".into(), Value::from(wire.commit_inflight.id));
    commit_inflight.insert("height".into(), Value::from(wire.commit_inflight.height));
    commit_inflight.insert("view".into(), Value::from(wire.commit_inflight.view));
    commit_inflight.insert(
        "block_hash".into(),
        wire.commit_inflight
            .block_hash
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(format!("{hash}"))),
    );
    commit_inflight.insert(
        "started_ms".into(),
        Value::from(wire.commit_inflight.started_ms),
    );
    commit_inflight.insert(
        "elapsed_ms".into(),
        Value::from(wire.commit_inflight.elapsed_ms),
    );
    commit_inflight.insert(
        "timeout_ms".into(),
        Value::from(wire.commit_inflight.timeout_ms),
    );
    commit_inflight.insert(
        "timeout_total".into(),
        Value::from(wire.commit_inflight.timeout_total),
    );
    commit_inflight.insert(
        "last_timeout_timestamp_ms".into(),
        Value::from(wire.commit_inflight.last_timeout_timestamp_ms),
    );
    commit_inflight.insert(
        "last_timeout_elapsed_ms".into(),
        Value::from(wire.commit_inflight.last_timeout_elapsed_ms),
    );
    commit_inflight.insert(
        "last_timeout_height".into(),
        Value::from(wire.commit_inflight.last_timeout_height),
    );
    commit_inflight.insert(
        "last_timeout_view".into(),
        Value::from(wire.commit_inflight.last_timeout_view),
    );
    commit_inflight.insert(
        "last_timeout_block_hash".into(),
        wire.commit_inflight
            .last_timeout_block_hash
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(format!("{hash}"))),
    );
    commit_inflight.insert(
        "pause_total".into(),
        Value::from(wire.commit_inflight.pause_total),
    );
    commit_inflight.insert(
        "resume_total".into(),
        Value::from(wire.commit_inflight.resume_total),
    );
    commit_inflight.insert(
        "paused_since_ms".into(),
        Value::from(wire.commit_inflight.paused_since_ms),
    );
    commit_inflight.insert(
        "pause_queue_depths".into(),
        Value::Object(commit_pause_queue_depths),
    );
    commit_inflight.insert(
        "resume_queue_depths".into(),
        Value::Object(commit_resume_queue_depths),
    );

    let mut missing_block_fetch = Map::new();
    missing_block_fetch.insert("total".into(), Value::from(wire.missing_block_fetch.total));
    missing_block_fetch.insert(
        "last_targets".into(),
        Value::from(wire.missing_block_fetch.last_targets),
    );
    missing_block_fetch.insert(
        "last_dwell_ms".into(),
        Value::from(wire.missing_block_fetch.last_dwell_ms),
    );

    let mut da_gate = Map::new();
    da_gate.insert(
        "reason".into(),
        Value::from(match wire.da_gate.reason {
            SumeragiDaGateReason::MissingLocalData => "missing_local_data",
            SumeragiDaGateReason::ManifestMissing => "manifest_missing",
            SumeragiDaGateReason::ManifestHashMismatch => "manifest_hash_mismatch",
            SumeragiDaGateReason::ManifestReadFailed => "manifest_read_failed",
            SumeragiDaGateReason::ManifestSpoolScan => "manifest_spool_scan",
            SumeragiDaGateReason::None => "none",
        }),
    );
    da_gate.insert(
        "last_satisfied".into(),
        Value::from(match wire.da_gate.last_satisfied {
            SumeragiDaGateSatisfaction::MissingDataRecovered => "missing_data_recovered",
            SumeragiDaGateSatisfaction::None => "none",
        }),
    );
    da_gate.insert(
        "missing_local_data_total".into(),
        Value::from(wire.da_gate.missing_local_data_total),
    );
    da_gate.insert(
        "manifest_guard_total".into(),
        Value::from(wire.da_gate.manifest_guard_total),
    );

    let mut kura_store = Map::new();
    kura_store.insert(
        "failures_total".into(),
        Value::from(wire.kura_store.failures_total),
    );
    kura_store.insert(
        "abort_total".into(),
        Value::from(wire.kura_store.abort_total),
    );
    kura_store.insert(
        "last_retry_attempt".into(),
        Value::from(wire.kura_store.last_retry_attempt),
    );
    kura_store.insert(
        "last_retry_backoff_ms".into(),
        Value::from(wire.kura_store.last_retry_backoff_ms),
    );
    kura_store.insert(
        "last_height".into(),
        Value::from(wire.kura_store.last_height),
    );
    kura_store.insert("last_view".into(), Value::from(wire.kura_store.last_view));
    kura_store.insert(
        "last_hash".into(),
        wire.kura_store
            .last_hash
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(format!("{hash}"))),
    );

    let recent_rbc_evictions: Vec<Value> = wire
        .rbc_store
        .recent_evictions
        .iter()
        .map(|entry| {
            let mut map = Map::new();
            map.insert(
                "block_hash".into(),
                Value::from(format!("{}", entry.block_hash)),
            );
            map.insert("height".into(), Value::from(entry.height));
            map.insert("view".into(), Value::from(entry.view));
            Value::Object(map)
        })
        .collect();
    let mut rbc_store = Map::new();
    rbc_store.insert("sessions".into(), Value::from(wire.rbc_store.sessions));
    rbc_store.insert("bytes".into(), Value::from(wire.rbc_store.bytes));
    rbc_store.insert(
        "pressure_level".into(),
        Value::from(wire.rbc_store.pressure_level),
    );
    rbc_store.insert(
        "backpressure_deferrals_total".into(),
        Value::from(wire.rbc_store.backpressure_deferrals_total),
    );
    rbc_store.insert(
        "evictions_total".into(),
        Value::from(wire.rbc_store.evictions_total),
    );
    rbc_store.insert(
        "recent_evictions".into(),
        Value::Array(recent_rbc_evictions),
    );

    let mut prf = Map::new();
    prf.insert("height".into(), Value::from(wire.prf_height));
    prf.insert("view".into(), Value::from(wire.prf_view));
    prf.insert(
        "epoch_seed".into(),
        wire.prf_epoch_seed
            .as_ref()
            .map_or(Value::Null, |seed| Value::from(bytes_to_hex(seed))),
    );

    let mut membership = Map::new();
    membership.insert("height".into(), Value::from(wire.membership.height));
    membership.insert("view".into(), Value::from(wire.membership.view));
    membership.insert("epoch".into(), Value::from(wire.membership.epoch));
    membership.insert(
        "view_hash".into(),
        wire.membership
            .view_hash
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(bytes_to_hex(hash))),
    );

    let mut membership_mismatch = Map::new();
    membership_mismatch.insert(
        "active_peers".into(),
        Value::Array(
            wire.membership_mismatch
                .active_peers
                .iter()
                .map(|peer| Value::from(peer.to_string()))
                .collect(),
        ),
    );
    membership_mismatch.insert(
        "last_peer".into(),
        wire.membership_mismatch
            .last_peer
            .as_ref()
            .map_or(Value::Null, |peer| Value::from(peer.to_string())),
    );
    membership_mismatch.insert(
        "last_height".into(),
        Value::from(wire.membership_mismatch.last_height),
    );
    membership_mismatch.insert(
        "last_view".into(),
        Value::from(wire.membership_mismatch.last_view),
    );
    membership_mismatch.insert(
        "last_epoch".into(),
        Value::from(wire.membership_mismatch.last_epoch),
    );
    membership_mismatch.insert(
        "last_local_hash".into(),
        wire.membership_mismatch
            .last_local_hash
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(bytes_to_hex(hash))),
    );
    membership_mismatch.insert(
        "last_remote_hash".into(),
        wire.membership_mismatch
            .last_remote_hash
            .as_ref()
            .map_or(Value::Null, |hash| Value::from(bytes_to_hex(hash))),
    );
    membership_mismatch.insert(
        "last_timestamp_ms".into(),
        Value::from(wire.membership_mismatch.last_timestamp_ms),
    );

    let lane_commitments: Vec<Value> = wire
        .lane_commitments
        .iter()
        .map(|entry| {
            let mut map = Map::new();
            map.insert("block_height".into(), Value::from(entry.block_height));
            map.insert(
                "lane_id".into(),
                Value::from(u64::from(entry.lane_id.as_u32())),
            );
            map.insert("tx_count".into(), Value::from(entry.tx_count));
            map.insert("total_chunks".into(), Value::from(entry.total_chunks));
            map.insert("rbc_bytes_total".into(), Value::from(entry.rbc_bytes_total));
            map.insert("teu_total".into(), Value::from(entry.teu_total));
            map.insert(
                "block_hash".into(),
                Value::from(format!("{}", entry.block_hash)),
            );
            Value::Object(map)
        })
        .collect();

    let dataspace_commitments: Vec<Value> = wire
        .dataspace_commitments
        .iter()
        .map(|entry| {
            let mut map = Map::new();
            map.insert("block_height".into(), Value::from(entry.block_height));
            map.insert(
                "lane_id".into(),
                Value::from(u64::from(entry.lane_id.as_u32())),
            );
            map.insert(
                "dataspace_id".into(),
                Value::from(entry.dataspace_id.as_u64()),
            );
            map.insert("tx_count".into(), Value::from(entry.tx_count));
            map.insert("total_chunks".into(), Value::from(entry.total_chunks));
            map.insert("rbc_bytes_total".into(), Value::from(entry.rbc_bytes_total));
            map.insert("teu_total".into(), Value::from(entry.teu_total));
            map.insert(
                "block_hash".into(),
                Value::from(format!("{}", entry.block_hash)),
            );
            Value::Object(map)
        })
        .collect();
    let lane_settlement_commitments: Vec<Value> = wire
        .lane_settlement_commitments
        .iter()
        .map(|commitment| {
            norito::json::to_value(commitment)
                .expect("serialize lane settlement commitment to JSON")
        })
        .collect();
    let lane_relay_envelopes: Vec<Value> = wire
        .lane_relay_envelopes
        .iter()
        .map(|envelope| {
            norito::json::to_value(envelope).expect("serialize lane relay envelope to JSON")
        })
        .collect();
    let lane_governance: Vec<Value> = wire
        .lane_governance
        .iter()
        .map(|entry| {
            let mut map = Map::new();
            map.insert(
                "lane_id".into(),
                Value::from(u64::from(entry.lane_id.as_u32())),
            );
            map.insert("alias".into(), Value::from(entry.alias.clone()));
            map.insert(
                "governance".into(),
                entry
                    .governance
                    .as_ref()
                    .map_or(Value::Null, |g| Value::from(g.clone())),
            );
            map.insert(
                "manifest_required".into(),
                Value::from(entry.manifest_required),
            );
            map.insert("manifest_ready".into(), Value::from(entry.manifest_ready));
            map.insert(
                "manifest_path".into(),
                entry
                    .manifest_path
                    .as_ref()
                    .map_or(Value::Null, |p| Value::from(p.clone())),
            );
            map.insert(
                "validator_ids".into(),
                Value::Array(
                    entry
                        .validator_ids
                        .iter()
                        .cloned()
                        .map(Value::from)
                        .collect(),
                ),
            );
            map.insert(
                "quorum".into(),
                entry
                    .quorum
                    .map_or(Value::Null, |q| Value::from(u64::from(q))),
            );
            map.insert(
                "protected_namespaces".into(),
                Value::Array(
                    entry
                        .protected_namespaces
                        .iter()
                        .cloned()
                        .map(Value::from)
                        .collect(),
                ),
            );
            let runtime_upgrade = entry.runtime_upgrade.as_ref().map(|hook| {
                let mut hook_map = Map::new();
                hook_map.insert("allow".into(), Value::from(hook.allow));
                hook_map.insert(
                    "require_metadata".into(),
                    Value::from(hook.require_metadata),
                );
                hook_map.insert(
                    "metadata_key".into(),
                    hook.metadata_key
                        .as_ref()
                        .map_or(Value::Null, |key| Value::from(key.clone())),
                );
                hook_map.insert(
                    "allowed_ids".into(),
                    Value::Array(hook.allowed_ids.iter().cloned().map(Value::from).collect()),
                );
                Value::Object(hook_map)
            });
            map.insert(
                "runtime_upgrade".into(),
                runtime_upgrade.unwrap_or(Value::Null),
            );
            Value::Object(map)
        })
        .collect();
    let mut root = Map::new();
    root.insert("leader_index".into(), Value::from(wire.leader_index));
    root.insert("highest_qc".into(), Value::Object(highest));
    root.insert("locked_qc".into(), Value::Object(locked));
    root.insert("tx_queue".into(), Value::Object(tx_queue));
    root.insert("worker_loop".into(), Value::Object(worker_loop));
    root.insert("commit_inflight".into(), Value::Object(commit_inflight));
    root.insert(
        "missing_block_fetch".into(),
        Value::Object(missing_block_fetch),
    );
    root.insert("kura_store".into(), Value::Object(kura_store));
    root.insert("epoch".into(), {
        let mut map = Map::new();
        map.insert(
            "length_blocks".into(),
            Value::from(wire.epoch_length_blocks),
        );
        map.insert(
            "commit_deadline_offset".into(),
            Value::from(wire.epoch_commit_deadline_offset),
        );
        map.insert(
            "reveal_deadline_offset".into(),
            Value::from(wire.epoch_reveal_deadline_offset),
        );
        Value::Object(map)
    });
    root.insert(
        "view_change_proof_accepted_total".into(),
        Value::from(wire.view_change_proof_accepted_total),
    );
    root.insert(
        "view_change_proof_stale_total".into(),
        Value::from(wire.view_change_proof_stale_total),
    );
    root.insert(
        "view_change_proof_rejected_total".into(),
        Value::from(wire.view_change_proof_rejected_total),
    );
    root.insert(
        "view_change_suggest_total".into(),
        Value::from(wire.view_change_suggest_total),
    );
    root.insert(
        "view_change_install_total".into(),
        Value::from(wire.view_change_install_total),
    );
    root.insert(
        "view_change_causes".into(),
        Value::Object(view_change_causes),
    );
    root.insert(
        "gossip_fallback_total".into(),
        Value::from(wire.gossip_fallback_total),
    );
    root.insert(
        "block_created_dropped_by_lock_total".into(),
        Value::from(wire.block_created_dropped_by_lock_total),
    );
    root.insert(
        "block_created_hint_mismatch_total".into(),
        Value::from(wire.block_created_hint_mismatch_total),
    );
    root.insert(
        "block_created_proposal_mismatch_total".into(),
        Value::from(wire.block_created_proposal_mismatch_total),
    );
    root.insert(
        "validation_reject_total".into(),
        Value::from(wire.validation_reject_total),
    );
    root.insert(
        "validation_reject_reason".into(),
        wire.validation_reject_reason
            .as_ref()
            .map_or(Value::Null, |reason| Value::from(reason.clone())),
    );
    root.insert(
        "validation_rejects".into(),
        Value::Object(validation_rejects),
    );
    root.insert("da_gate".into(), Value::Object(da_gate));
    root.insert(
        "pacemaker_backpressure_deferrals_total".into(),
        Value::from(wire.pacemaker_backpressure_deferrals_total),
    );
    root.insert(
        "commit_pipeline_tick_total".into(),
        Value::from(wire.commit_pipeline_tick_total),
    );
    root.insert(
        "da_reschedule_total".into(),
        Value::from(wire.da_reschedule_total),
    );
    root.insert("rbc_store".into(), Value::Object(rbc_store));
    root.insert("prf".into(), Value::Object(prf));
    root.insert("membership".into(), Value::Object(membership));
    root.insert(
        "membership_mismatch".into(),
        Value::Object(membership_mismatch),
    );
    root.insert("lane_commitments".into(), Value::Array(lane_commitments));
    root.insert(
        "dataspace_commitments".into(),
        Value::Array(dataspace_commitments),
    );
    root.insert(
        "lane_settlement_commitments".into(),
        Value::Array(lane_settlement_commitments),
    );
    root.insert(
        "lane_relay_envelopes".into(),
        Value::Array(lane_relay_envelopes),
    );
    root.insert(
        "lane_governance_sealed_total".into(),
        Value::from(u64::from(wire.lane_governance_sealed_total)),
    );
    root.insert(
        "lane_governance_sealed_aliases".into(),
        Value::Array(
            wire.lane_governance_sealed_aliases
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
    root.insert("lane_governance".into(), Value::Array(lane_governance));
    root.insert(
        "vrf_penalty_epoch".into(),
        Value::from(wire.vrf_penalty_epoch),
    );
    root.insert(
        "vrf_committed_no_reveal_total".into(),
        Value::from(wire.vrf_committed_no_reveal_total),
    );
    root.insert(
        "vrf_no_participation_total".into(),
        Value::from(wire.vrf_no_participation_total),
    );
    root.insert(
        "vrf_late_reveals_total".into(),
        Value::from(wire.vrf_late_reveals_total),
    );
    norito::json::Value::Object(root)
}

fn sumeragi_qc_json_payload(snapshot: SumeragiQcSnapshot) -> norito::json::Value {
    use norito::json::{Map, Value};

    fn entry_to_value(entry: &SumeragiQcEntry) -> Value {
        let mut map = Map::new();
        map.insert("height".into(), Value::from(entry.height));
        map.insert("view".into(), Value::from(entry.view));
        map.insert(
            "subject_block_hash".into(),
            entry
                .subject_block_hash
                .map_or(Value::Null, |h| Value::from(format!("{h}"))),
        );
        Value::Object(map)
    }

    let mut root = Map::new();
    root.insert("highest_qc".into(), entry_to_value(&snapshot.highest_qc));
    root.insert("locked_qc".into(), entry_to_value(&snapshot.locked_qc));
    Value::Object(root)
}

#[derive(Debug, Decode)]
struct ExecRootWire {
    block_hash: HashOf<BlockHeader>,
    exec_root: Option<iroha_crypto::Hash>,
}

impl Client {
    fn parse_json_ok_response(
        response: &Response<Vec<u8>>,
        context: &'static str,
    ) -> Result<norito::json::Value> {
        if response.status() != StatusCode::OK {
            return Err(eyre!(
                "{context}: {} {}",
                response.status(),
                std::str::from_utf8(response.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(response.body())?)
    }

    fn build_evidence_request_body(evidence_hex: &str) -> Result<Vec<u8>> {
        let mut payload = norito::json::Map::new();
        payload.insert(
            "evidence_hex".to_owned(),
            norito::json::Value::from(evidence_hex),
        );
        Ok(norito::json::to_vec(&norito::json::Value::from(payload))?)
    }

    fn evidence_status_payload(status: StatusCode) -> norito::json::Value {
        let mut status_map = norito::json::Map::new();
        status_map.insert(
            "status".to_owned(),
            norito::json::Value::from(status.as_u16()),
        );
        norito::json::Value::from(status_map)
    }

    fn parse_evidence_post_response(response: &Response<Vec<u8>>) -> Result<norito::json::Value> {
        if !matches!(response.status(), StatusCode::OK | StatusCode::ACCEPTED) {
            return Err(eyre!(
                "Failed to submit sumeragi evidence: {} {}",
                response.status(),
                std::str::from_utf8(response.body()).unwrap_or("")
            ));
        }
        if response.body().is_empty() {
            return Ok(Self::evidence_status_payload(response.status()));
        }
        Ok(norito::json::from_slice(response.body())?)
    }

    /// GET `/v1/sumeragi/status` — consensus status snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_status(&self) -> Result<SumeragiStatusWire> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/status");
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_NORITO),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi status: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if content_type.starts_with(APPLICATION_NORITO) {
            return decode_from_bytes::<SumeragiStatusWire>(resp.body())
                .map_err(|err| eyre!("Failed to decode sumeragi status Norito payload: {err}"));
        }
        norito::json::from_slice(resp.body())
            .map_err(|e| eyre!("Failed to decode sumeragi status JSON payload: {e}"))
    }

    /// GET `/v1/sumeragi/status` — consensus status snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_status_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/status");
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_NORITO),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi status: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if content_type.starts_with(APPLICATION_NORITO) {
            let wire = decode_from_bytes::<SumeragiStatusWire>(resp.body())
                .map_err(|err| eyre!("Failed to decode sumeragi status Norito payload: {err}"))?;
            return Ok(sumeragi_status_json_payload(&wire));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/status` with typed decoding and lane relay validation.
    ///
    /// This helper decodes the status payload into [`SumeragiStatusWire`] and rejects responses that
    /// contain invalid lane relay envelopes (e.g., mismatched settlement hashes or DA/QC bindings).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, decoding fails, or any
    /// lane relay envelope fails verification.
    pub fn get_sumeragi_status_wire(&self) -> Result<SumeragiStatusWire> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/status");
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_NORITO),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi status: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        let wire = if content_type.starts_with(APPLICATION_NORITO) {
            decode_from_bytes::<SumeragiStatusWire>(resp.body())
                .map_err(|err| eyre!("Failed to decode sumeragi status Norito payload: {err}"))?
        } else {
            norito::json::from_slice(resp.body())?
        };
        for envelope in &wire.lane_relay_envelopes {
            envelope
                .verify()
                .map_err(|err| eyre!("Invalid lane relay envelope in status payload: {err}"))?;
        }
        Ok(wire)
    }

    /// GET `/v1/sumeragi/status` and return verified cross-lane transfer proofs.
    ///
    /// This helper enforces lane relay envelope validation (settlement hash, DA hash, QC subject)
    /// and rejects duplicate `(lane_id, dataspace_id, block_height)` tuples before returning the
    /// wrapped proof objects.
    ///
    /// # Errors
    /// Returns an error if the status request fails or if relay envelopes fail validation or deduplication.
    pub fn get_cross_lane_transfer_proofs(&self) -> Result<Vec<CrossLaneTransferProof>> {
        let status = self.get_sumeragi_status_wire()?;
        verify_lane_relay_envelopes(&status.lane_relay_envelopes)?;
        Ok(status
            .lane_relay_envelopes
            .into_iter()
            .map(CrossLaneTransferProof::new)
            .collect())
    }

    /// GET `/v1/nexus/public_lanes/{lane}/validators` — lifecycle snapshot for public-lane validators.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_public_lane_validators(
        &self,
        lane_id: LaneId,
        address_format: Option<AddressFormat>,
    ) -> Result<JsonValue> {
        let url = join_torii_url(
            &self.torii_url,
            &format!("v1/nexus/public_lanes/{}/validators", lane_id.as_u32()),
        );
        let mut req = self.default_request(HttpMethod::GET, url);
        if let Some(format) = address_format {
            req = req.param("address_format", format.as_query_str());
        }
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get public lane validators: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        norito::json::from_slice(resp.body()).map_err(Into::into)
    }

    /// GET `/v1/nexus/public_lanes/{lane}/stake` — bonded stake per validator/staker.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_public_lane_stake(
        &self,
        lane_id: LaneId,
        validator: Option<&str>,
        address_format: Option<AddressFormat>,
    ) -> Result<JsonValue> {
        let url = join_torii_url(
            &self.torii_url,
            &format!("v1/nexus/public_lanes/{}/stake", lane_id.as_u32()),
        );
        let mut req = self.default_request(HttpMethod::GET, url);
        if let Some(format) = address_format {
            req = req.param("address_format", format.as_query_str());
        }
        if let Some(value) = validator {
            if value.is_empty() {
                return Err(eyre!("validator filter must not be empty"));
            }
            req = req.param("validator", value);
        }
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get public lane stake: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        norito::json::from_slice(resp.body()).map_err(Into::into)
    }

    /// GET `/v1/nexus/public_lanes/{lane}/rewards/pending` — pending rewards for an account.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_public_lane_pending_rewards(
        &self,
        lane_id: LaneId,
        account: &str,
        upto_epoch: Option<u64>,
        address_format: Option<AddressFormat>,
    ) -> Result<JsonValue> {
        if account.is_empty() {
            return Err(eyre!("account filter must not be empty"));
        }
        let url = join_torii_url(
            &self.torii_url,
            &format!("v1/nexus/public_lanes/{}/rewards/pending", lane_id.as_u32()),
        );
        let mut req = self
            .default_request(HttpMethod::GET, url)
            .param("account", account);
        if let Some(epoch) = upto_epoch {
            req = req.param("upto_epoch", &epoch);
        }
        if let Some(format) = address_format {
            req = req.param("address_format", format.as_query_str());
        }
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get public lane pending rewards: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        norito::json::from_slice(resp.body()).map_err(Into::into)
    }

    /// GET `/v1/sumeragi/exec_root/:hash` — execution post-state root for a parent block hash.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_exec_root_json(&self, hash_hex: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(
            &self.torii_url,
            &format!("v1/sumeragi/exec_root/{hash_hex}"),
        );
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_NORITO),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi exec_root: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if content_type.starts_with(APPLICATION_NORITO) {
            use norito::json::{Map, Value};

            let wire = decode_from_bytes::<ExecRootWire>(resp.body())
                .map_err(|err| eyre!("Failed to decode exec_root Norito payload: {err}"))?;
            let mut map = Map::new();
            map.insert(
                "block_hash".into(),
                Value::from(format!("{}", wire.block_hash)),
            );
            match wire.exec_root {
                Some(root) => {
                    map.insert("exec_root".into(), Value::from(format!("{root}")));
                }
                None => {
                    map.insert("exec_root".into(), Value::Null);
                }
            }
            return Ok(Value::Object(map));
        }
        norito::json::from_slice(resp.body()).map_err(Into::into)
    }

    /// GET `/v1/sumeragi/exec_qc/:hash` — full `ExecutionQC` record for a parent block hash.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_exec_qc_json(&self, hash_hex: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, &format!("v1/sumeragi/exec_qc/{hash_hex}"));
        let resp = self
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_NORITO)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi exec_qc: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if content_type.starts_with(APPLICATION_NORITO) {
            let record_opt = decode_from_bytes::<Option<ExecutionQcRecord>>(resp.body())
                .map_err(|err| eyre!("Failed to decode exec_qc Norito payload: {err}"))?;
            return Ok(exec_qc_json_payload(hash_hex, record_opt));
        }
        norito::json::from_slice(resp.body()).map_err(Into::into)
    }

    /// GET `/v1/sumeragi/vrf/penalties/:epoch` — VRF penalties snapshot for the given epoch.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_vrf_penalties_json(&self, epoch: u64) -> Result<norito::json::Value> {
        let url = join_torii_url(
            &self.torii_url,
            &format!("v1/sumeragi/vrf/penalties/{epoch}"),
        );
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi vrf penalties: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/vrf/epoch/:epoch` — VRF epoch snapshot (participants, randomness state).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_vrf_epoch_json(&self, epoch: u64) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, &format!("v1/sumeragi/vrf/epoch/{epoch}"));
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi vrf epoch: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/leader` — leader index snapshot with optional PRF context.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_leader_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/leader");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi leader: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/params` — on-chain Sumeragi parameters snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_params_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/params");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi params: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/parameters` — full parameter snapshot (system + custom).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_parameters(&self) -> Result<iroha_data_model::parameter::Parameters> {
        let url = join_torii_url(&self.torii_url, "v1/parameters");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        decode_parameters_response(&resp)
    }

    /// GET `/v1/sumeragi/collectors` — current collector indices and peer IDs.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_collectors_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/collectors");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi collectors: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/qc` — `HighestQC`/`LockedQC` snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_qc_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/qc");
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_NORITO),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi qc: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if content_type.starts_with(APPLICATION_NORITO) {
            let wire = decode_from_bytes::<SumeragiQcSnapshot>(resp.body())
                .map_err(|e| eyre!("Failed to decode sumeragi qc Norito payload: {e}"))?;
            return Ok(sumeragi_qc_json_payload(wire));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/pacemaker` — pacemaker timers/config snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_pacemaker_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/pacemaker");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi pacemaker: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/phases` — latest per-phase latencies (ms).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_phases_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/phases");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi phases: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/telemetry` — aggregated telemetry snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_telemetry_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/telemetry");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi telemetry: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/rbc` — RBC session/throughput counters.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_rbc_status_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/rbc");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi rbc status: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/rbc/sessions` — RBC sessions snapshot.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_sumeragi_rbc_sessions_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/rbc/sessions");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi rbc sessions: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/sumeragi/evidence/count` — total persisted evidence entries.
    ///
    /// # Errors
    /// Returns an error if the request fails or the response is non-OK/invalid JSON.
    pub fn get_sumeragi_evidence_count_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/evidence/count");
        let response = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        let value =
            Self::parse_json_ok_response(&response, "Failed to get sumeragi evidence count")?;
        Ok(value)
    }

    /// GET `/v1/sumeragi/evidence` — list persisted evidence entries.
    ///
    /// # Errors
    /// Returns an error if the request fails or the response is non-OK/invalid JSON.
    pub fn get_sumeragi_evidence_list_json(
        &self,
        filter: &SumeragiEvidenceListFilter<'_>,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/evidence");
        let req = filter.apply(self.default_request(HttpMethod::GET, url));
        let response = self.send_builder(req)?;
        let value =
            Self::parse_json_ok_response(&response, "Failed to get sumeragi evidence list")?;
        Ok(value)
    }

    /// GET `/v1/sumeragi/evidence` — list persisted evidence entries (Norito wire).
    ///
    /// Sets `Accept: application/x-norito` and decodes the `(total, Vec<EvidenceRecord>)` payload.
    ///
    /// # Errors
    /// Returns an error if the request fails, the response is non-OK, or Norito decoding fails.
    pub fn get_sumeragi_evidence_list_wire(
        &self,
        filter: &SumeragiEvidenceListFilter<'_>,
    ) -> Result<(u64, Vec<EvidenceRecord>)> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/evidence");
        let req = filter
            .apply(self.default_request(HttpMethod::GET, url))
            .header("Accept", APPLICATION_NORITO);
        let response = self.send_builder(req)?;
        if response.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get sumeragi evidence list: {} {}",
                response.status(),
                std::str::from_utf8(response.body()).unwrap_or("")
            ));
        }
        let decoded: (u64, Vec<EvidenceRecord>) =
            decode_from_bytes(response.body()).map_err(|err| {
                eyre!("Failed to decode sumeragi evidence list Norito payload: {err}")
            })?;
        Ok(decoded)
    }

    /// POST `/v1/sumeragi/evidence` — submit Norito-framed evidence (hex string).
    ///
    /// # Errors
    /// Returns an error if the request fails, the response is unexpected, or JSON parsing fails.
    pub fn post_sumeragi_evidence_hex(&self, evidence_hex: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/sumeragi/evidence");
        let body = Self::build_evidence_request_body(evidence_hex)?;
        let response = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", APPLICATION_JSON)
                .body(body),
        )?;
        let value = Self::parse_evidence_post_response(&response)?;
        Ok(value)
    }
}

#[cfg(test)]
mod status_tests {
    use http::Response as HttpResponse;
    use iroha_telemetry::metrics::{
        CryptoStatus, GovernanceStatus, Halo2Status, StackStatus, SumeragiConsensusStatus,
    };

    use super::*;

    fn mk_response(
        status: StatusCode,
        body: Vec<u8>,
        content_type: Option<&str>,
    ) -> Response<Vec<u8>> {
        let mut builder = HttpResponse::builder().status(status);
        if let Some(ct) = content_type {
            builder = builder.header("content-type", ct);
        }
        builder.body(body).unwrap()
    }

    #[test]
    fn decode_status_prefers_norito_bare() {
        let s = Status {
            peers: 1,
            blocks: 2,
            blocks_non_empty: 1,
            commit_time_ms: 10,
            txs_approved: 3,
            txs_rejected: 1,
            uptime: Uptime(Duration::from_millis(1234)),
            view_changes: 0,
            queue_size: 7,
            da_reschedule_total: 0,
            tx_gossip: TxGossipSnapshot::default(),
            stack: StackStatus::default(),
            crypto: CryptoStatus {
                sm_helpers_available: true,
                sm_openssl_preview_enabled: true,
                halo2: Halo2Status::default(),
            },
            sumeragi: Some(SumeragiConsensusStatus::default()),
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let body = norito::to_bytes(&s).expect("encode status");
        let resp = mk_response(StatusCode::OK, body, Some("application/x-norito"));
        let got = decode_status_response(&resp).expect("decode");
        assert_eq!(got.peers, s.peers);
        assert_eq!(got.blocks, s.blocks);
        assert_eq!(got.blocks_non_empty, s.blocks_non_empty);
        assert_eq!(got.commit_time_ms, s.commit_time_ms);
        assert_eq!(got.txs_approved, s.txs_approved);
        assert_eq!(got.txs_rejected, s.txs_rejected);
        assert_eq!(got.view_changes, s.view_changes);
        assert_eq!(got.queue_size, s.queue_size);
        assert_eq!(got.uptime.0, s.uptime.0);
    }

    #[test]
    fn decode_status_falls_back_to_json() {
        let s = Status {
            peers: 5,
            blocks: 6,
            blocks_non_empty: 4,
            commit_time_ms: 200,
            txs_approved: 10,
            txs_rejected: 2,
            uptime: Uptime(Duration::from_millis(5678)),
            view_changes: 1,
            queue_size: 9,
            da_reschedule_total: 0,
            tx_gossip: TxGossipSnapshot::default(),
            stack: StackStatus::default(),
            crypto: CryptoStatus {
                sm_helpers_available: false,
                sm_openssl_preview_enabled: false,
                halo2: Halo2Status::default(),
            },
            sumeragi: Some(SumeragiConsensusStatus::default()),
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let body = norito::json::to_vec(&s).unwrap();
        let resp = mk_response(StatusCode::OK, body, Some("application/json"));
        let got = decode_status_response(&resp).expect("json decode");
        assert_eq!(got.blocks, s.blocks);
        assert_eq!(got.queue_size, s.queue_size);
    }
}

#[cfg(test)]
mod evidence_filter_tests {
    use super::*;

    #[test]
    fn evidence_filter_apply_sets_expected_params() {
        let filter = SumeragiEvidenceListFilter {
            limit: Some(25),
            offset: Some(10),
            kind: Some("InvalidCommitCertificate"),
        };
        let params = filter.param_entries();
        assert_eq!(
            params,
            vec![
                ("limit", "25".to_string()),
                ("offset", "10".to_string()),
                ("kind", "InvalidCommitCertificate".to_string())
            ]
        );
    }

    #[test]
    fn evidence_filter_apply_no_params() {
        let filter = SumeragiEvidenceListFilter::default();
        let params = filter.param_entries();
        assert!(params.is_empty());
    }
}

#[cfg(test)]
mod evidence_response_tests {
    use http::Response as HttpResponse;
    use norito::json::Value;

    use super::*;

    #[test]
    fn build_evidence_request_body_serializes_hex_field() {
        let body = Client::build_evidence_request_body("deadbeef").expect("body");
        let value: Value = norito::json::from_slice(&body).expect("valid json");
        let map = value.as_object().expect("payload encoded as JSON object");
        let field = map.get("evidence_hex").expect("field present");
        match field {
            Value::String(s) => assert_eq!(s, "deadbeef"),
            _ => panic!("expected string field for evidence_hex, got {field:?}"),
        }
    }

    #[test]
    fn parse_evidence_post_response_returns_status_map_for_empty_body() {
        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .body(Vec::new())
            .unwrap();
        let json = Client::parse_evidence_post_response(&response).expect("success");
        let map = json.as_object().expect("status payload is object");
        match map.get("status") {
            Some(Value::Number(num)) if num.as_u64() == Some(200) => {}
            other => panic!("unexpected status payload: {other:?}"),
        }
    }

    #[test]
    fn parse_evidence_post_response_decodes_json_body() {
        let mut payload = norito::json::Map::new();
        payload.insert("ok".to_owned(), Value::Bool(true));
        let body = norito::json::to_vec(&Value::from(payload.clone())).unwrap();
        let response = HttpResponse::builder()
            .status(StatusCode::ACCEPTED)
            .body(body)
            .unwrap();
        let json = Client::parse_evidence_post_response(&response).expect("success");
        assert_eq!(json.as_object(), Some(&payload));
    }

    #[test]
    fn parse_evidence_post_response_errors_on_unexpected_status() {
        let response = HttpResponse::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(b"nope".to_vec())
            .unwrap();
        assert!(Client::parse_evidence_post_response(&response).is_err());
    }

    #[test]
    fn parse_json_ok_response_returns_decoded_value() {
        let mut payload = norito::json::Map::new();
        payload.insert("count".to_owned(), Value::from(42u64));
        let body = norito::json::to_vec(&Value::from(payload.clone())).unwrap();
        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .body(body)
            .unwrap();
        let json = Client::parse_json_ok_response(&response, "context")
            .expect("expected successful parse");
        assert_eq!(json.as_object(), Some(&payload));
    }

    #[test]
    fn parse_json_ok_response_errors_on_non_ok_status() {
        let response = HttpResponse::builder()
            .status(StatusCode::FORBIDDEN)
            .body(b"denied".to_vec())
            .unwrap();
        assert!(Client::parse_json_ok_response(&response, "context").is_err());
    }
}

#[cfg(test)]
fn default_alias_policy() -> sorafs_manifest::alias_cache::AliasCachePolicy {
    sorafs_manifest::alias_cache::AliasCachePolicy::new(
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS,
        ),
        std::time::Duration::from_secs(
            iroha_config::parameters::defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS,
        ),
    )
}

#[cfg(test)]
mod evidence_http_tests {
    use std::{
        collections::HashMap,
        convert::TryInto,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{Arc, Mutex, OnceLock},
        time::Duration,
    };

    use http::StatusCode;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey, Signature};
    use iroha_data_model::block::consensus::{
        CertPhase as Phase, CommitVote as Vote, Evidence, EvidenceKind, EvidencePayload,
        EvidenceRecord,
    };
    use iroha_test_samples::gen_account_in;
    use norito::json::Value;
    use sorafs_manifest::{
        alias_cache::unix_now_secs,
        pin_registry::{
            AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
        },
    };

    use super::{default_alias_policy, *};
    use crate::{
        http::{Method as HttpMethod, Response as HttpResponse},
        http_default::{RequestSnapshot, with_send_hook},
    };

    pub(super) type SnapshotStore = Arc<Mutex<Vec<RequestSnapshot>>>;

    pub(super) fn client_with_base_url(url: Url) -> Client {
        let (account_id, key_pair) = gen_account_in("wonderland");
        let config = Config {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            key_pair,
            account: account_id,
            torii_api_url: url,
            torii_api_version: crate::config::default_torii_api_version(),
            torii_api_min_proof_version: crate::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                .to_string(),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            basic_auth: None,
            transaction_add_nonce: false,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(10),
            connect_queue_root: crate::config::default_connect_queue_root(),
            sorafs_alias_cache: default_alias_policy(),
            sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
            sorafs_rollout_phase: SorafsRolloutPhase::Canary,
        };
        Client::new(config)
    }

    pub(super) fn base_url() -> Url {
        Url::parse("http://mock.local/").expect("valid mock URL")
    }

    pub(super) fn json_response(status: StatusCode, body: &str) -> HttpResponse<Vec<u8>> {
        HttpResponse::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(body.as_bytes().to_vec())
            .expect("response build")
    }

    fn empty_response(status: StatusCode) -> HttpResponse<Vec<u8>> {
        HttpResponse::builder()
            .status(status)
            .body(Vec::new())
            .expect("response build")
    }

    fn norito_response<T: norito::core::NoritoSerialize>(
        status: StatusCode,
        value: &T,
    ) -> HttpResponse<Vec<u8>> {
        HttpResponse::builder()
            .status(status)
            .header("content-type", APPLICATION_NORITO)
            .body(norito::to_bytes(value).expect("encode norito response"))
            .expect("response build")
    }

    fn hook_mutex() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    pub(super) fn with_mock_http<R>(
        responder: impl Fn(RequestSnapshot) -> Result<HttpResponse<Vec<u8>>> + Send + Sync + 'static,
        f: impl FnOnce() -> R,
    ) -> R {
        with_send_hook(Arc::new(responder), f)
    }

    pub(super) fn respond_with(
        store: &SnapshotStore,
        response: HttpResponse<Vec<u8>>,
    ) -> impl Fn(RequestSnapshot) -> Result<HttpResponse<Vec<u8>>> + Send + Sync + 'static {
        let store = Arc::clone(store);
        move |snapshot| {
            store.lock().expect("lock snapshot store").push(snapshot);
            Ok(response.clone())
        }
    }

    type SorafsFetchHook = Arc<
        dyn Fn(
                &CarBuildPlan,
                &SorafsGatewayFetchConfig,
                &[SorafsGatewayProviderInput],
                &SorafsGatewayFetchOptions,
            ) -> Result<SorafsFetchOutcome, SorafsFetchError>
            + Send
            + Sync,
    >;

    fn sorafs_fetch_hook_slot() -> &'static Mutex<Option<SorafsFetchHook>> {
        static HOOK: OnceLock<Mutex<Option<SorafsFetchHook>>> = OnceLock::new();
        HOOK.get_or_init(|| Mutex::new(None))
    }

    /// Fetch the currently installed mocked gateway fetch hook, if any.
    pub(super) fn sorafs_fetch_hook() -> Option<SorafsFetchHook> {
        sorafs_fetch_hook_slot()
            .lock()
            .expect("lock fetch hook slot")
            .clone()
    }

    /// Install a mocked gateway fetch hook for the duration of the provided closure.
    pub(super) fn with_mock_sorafs_fetch<R>(
        hook: impl Fn(
            &CarBuildPlan,
            &SorafsGatewayFetchConfig,
            &[SorafsGatewayProviderInput],
            &SorafsGatewayFetchOptions,
        ) -> Result<SorafsFetchOutcome, SorafsFetchError>
        + Send
        + Sync
        + 'static,
        f: impl FnOnce() -> R,
    ) -> R {
        let _guard = hook_mutex().lock().expect("acquire hook mutex");
        *sorafs_fetch_hook_slot()
            .lock()
            .expect("lock fetch hook slot") = Some(Arc::new(hook));
        let outcome = catch_unwind(AssertUnwindSafe(f));
        *sorafs_fetch_hook_slot()
            .lock()
            .expect("lock fetch hook slot") = None;
        match outcome {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    fn alias_proof_bundle(generated: u64, expires: u64) -> AliasProofBundleV1 {
        let mut bundle = AliasProofBundleV1 {
            binding: AliasBindingV1 {
                alias: "docs/sora".to_owned(),
                manifest_cid: vec![0xAA, 0xBB],
                bound_at: 1,
                expiry_epoch: 100,
            },
            registry_root: [0u8; 32],
            registry_height: 1,
            generated_at_unix: generated,
            expires_at_unix: expires,
            merkle_path: Vec::new(),
            council_signatures: Vec::new(),
        };
        if bundle.expires_at_unix <= bundle.generated_at_unix {
            bundle.expires_at_unix = bundle.generated_at_unix.saturating_add(600);
        }
        let root = alias_merkle_root(&bundle.binding, &bundle.merkle_path)
            .expect("compute alias proof root");
        bundle.registry_root = root;
        let digest = alias_proof_signature_digest(&bundle);
        let keypair = KeyPair::from_private_key(
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x44; 32]).expect("seeded key"),
        )
        .expect("derive keypair");
        let signature = Signature::new(keypair.private_key(), digest.as_ref());
        let (_, signer_bytes) = keypair.public_key().to_bytes();
        let signer: [u8; 32] = signer_bytes
            .try_into()
            .expect("ed25519 public key must be 32 bytes");
        bundle
            .council_signatures
            .push(sorafs_manifest::CouncilSignature {
                signer,
                signature: signature.payload().to_vec(),
            });
        bundle
    }

    fn alias_proof_base64(bundle: &AliasProofBundleV1) -> String {
        let bytes = norito::to_bytes(bundle).expect("encode alias proof");
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn storage_token_request_sets_headers_and_body() {
        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = respond_with(&store, empty_response(StatusCode::OK));
        let manifest_hex = "aa".repeat(32);
        let provider_hex = "bb".repeat(32);
        let overrides = SorafsTokenOverrides {
            ttl_secs: Some(1_200),
            max_streams: Some(4),
            rate_limit_bytes: Some(512_000),
            requests_per_minute: Some(120),
        };

        with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            client
                .post_sorafs_storage_token(
                    &manifest_hex,
                    &provider_hex,
                    "gateway-alpha",
                    "nonce-abc",
                    &overrides,
                )
                .expect("token request");
        });

        let snapshots = store.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), 1);
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert_eq!(snapshot.url.path(), "/v1/sorafs/storage/token");
        assert!(
            snapshot
                .headers
                .iter()
                .any(|(name, value)| name == HEADER_SORA_CLIENT && value == "gateway-alpha"),
            "client header missing"
        );
        assert!(
            snapshot
                .headers
                .iter()
                .any(|(name, value)| name == HEADER_SORA_NONCE && value == "nonce-abc"),
            "nonce header missing"
        );
        assert!(
            snapshot
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("content-type")
                    && value == APPLICATION_JSON),
            "content-type header missing"
        );

        let body: Value =
            norito::json::from_slice(&snapshot.body).expect("token request body JSON");
        assert_eq!(
            body.get("manifest_id_hex").and_then(Value::as_str),
            Some(manifest_hex.as_str())
        );
        assert_eq!(
            body.get("provider_id_hex").and_then(Value::as_str),
            Some(provider_hex.as_str())
        );
        assert_eq!(
            body.get("ttl_secs").and_then(Value::as_u64),
            overrides.ttl_secs
        );
        assert_eq!(
            body.get("max_streams").and_then(Value::as_u64),
            overrides.max_streams.map(u64::from)
        );
        assert_eq!(
            body.get("rate_limit_bytes").and_then(Value::as_u64),
            overrides.rate_limit_bytes
        );
        assert_eq!(
            body.get("requests_per_minute").and_then(Value::as_u64),
            overrides.requests_per_minute.map(u64::from)
        );
    }

    #[test]
    fn get_pin_manifest_accepts_fresh_alias_proof() {
        let now = unix_now_secs();
        let bundle = alias_proof_bundle(now.saturating_sub(30), now + 600);
        let proof_b64 = alias_proof_base64(&bundle);
        let policy = default_alias_policy();
        let status_label = policy.evaluate(&bundle, now).status_label();

        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .header(HEADER_SORA_NAME, "docs/sora")
            .header(HEADER_SORA_PROOF, proof_b64)
            .header(HEADER_SORA_PROOF_STATUS, status_label)
            .body(b"{}".to_vec())
            .expect("response build");

        let store = Arc::new(Mutex::new(Vec::new()));
        with_mock_http(respond_with(&store, response), || {
            let client = client_with_base_url(base_url());
            let resp = client
                .get_sorafs_pin_manifest("deadbeef")
                .expect("fresh alias proof should be accepted");
            assert_eq!(resp.status(), StatusCode::OK);
        });
    }

    #[test]
    fn get_pin_manifest_rejects_stale_alias_proof() {
        let bundle = alias_proof_bundle(1, 5);
        let proof_b64 = alias_proof_base64(&bundle);
        let policy = default_alias_policy();
        let status_label = policy.evaluate(&bundle, unix_now_secs()).status_label();

        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .header(HEADER_SORA_NAME, "docs/sora")
            .header(HEADER_SORA_PROOF, proof_b64)
            .header(HEADER_SORA_PROOF_STATUS, status_label)
            .body(b"{}".to_vec())
            .expect("response build");

        let store = Arc::new(Mutex::new(Vec::new()));
        with_mock_http(respond_with(&store, response), || {
            let client = client_with_base_url(base_url());
            let err = client
                .get_sorafs_pin_manifest("deadbeef")
                .expect_err("stale alias proof should be rejected");
            assert!(err.to_string().contains("alias proof"));
        });
    }

    #[test]
    fn send_builder_enforces_alias_policy_for_stale_proof() {
        let now = unix_now_secs();
        let bundle = alias_proof_bundle(now.saturating_sub(900), now.saturating_sub(300));
        let proof_b64 = alias_proof_base64(&bundle);
        let status_label = default_alias_policy().evaluate(&bundle, now).status_label();

        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .header(HEADER_SORA_NAME, "docs/stale")
            .header(HEADER_SORA_PROOF, proof_b64)
            .header(HEADER_SORA_PROOF_STATUS, status_label)
            .body(b"{}".to_vec())
            .expect("response build");

        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        with_mock_http(respond_with(&store, response), || {
            let client = client_with_base_url(base_url());
            let builder = client.default_request(HttpMethod::GET, base_url());
            let err = client
                .send_builder(builder)
                .expect_err("stale alias proof should trigger send_builder error");
            assert!(
                err.to_string().contains("alias proof"),
                "expected alias proof rejection, got {err}"
            );
        });
    }

    #[test]
    fn build_register_manifest_payload_contains_expected_fields() {
        let (authority, key_pair) = gen_account_in("wonderland");
        let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
        let chunking_profile = sorafs_manifest::ChunkingProfileV1::from_descriptor(descriptor);
        let manifest = sorafs_manifest::ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(sorafs_manifest::DagCodecId(0x71))
            .chunking_profile(chunking_profile)
            .content_length(1_024)
            .car_digest([0xAB; 32])
            .car_size(2_048)
            .pin_policy(sorafs_manifest::PinPolicy {
                min_replicas: 3,
                storage_class: sorafs_manifest::StorageClass::Warm,
                retention_epoch: 77,
            })
            .build()
            .expect("manifest build");

        let manifest_digest_hex =
            hex::encode(manifest.digest().expect("manifest digest").as_bytes());
        let chunk_digest = [0xCD; 32];
        let chunk_digest_hex = hex::encode(chunk_digest);
        let alias_bytes = *b"alias-proof";
        let alias = SorafsPinAlias {
            namespace: "sora",
            name: "docs",
            proof: alias_bytes.as_ref(),
        };
        let expected_alias_b64 =
            base64::engine::general_purpose::STANDARD.encode(alias_bytes.as_ref());
        let successor = [0x11; 32];
        let successor_hex = hex::encode(successor);

        let payload = Client::build_sorafs_pin_register_payload(
            &authority,
            key_pair.private_key(),
            &manifest,
            chunk_digest,
            9,
            Some(alias),
            Some(successor),
        )
        .expect("payload build succeeds");

        let obj = payload
            .as_object()
            .expect("payload should serialize to a map");

        assert_manifest_core_fields(
            obj,
            &authority,
            manifest_digest_hex.as_str(),
            chunk_digest_hex.as_str(),
            descriptor,
        );

        let policy_map = obj
            .get("pin_policy")
            .and_then(norito::json::Value::as_object)
            .expect("pin policy map");
        assert_pin_policy_fields(policy_map);

        let alias_obj = obj
            .get("alias")
            .and_then(norito::json::Value::as_object)
            .expect("alias map");
        assert_alias_fields(alias_obj, expected_alias_b64.as_str());

        assert_successor_hex(obj, successor_hex.as_str());
    }

    fn assert_manifest_core_fields(
        obj: &norito::json::Map,
        authority: &iroha_data_model::account::AccountId,
        manifest_digest_hex: &str,
        chunk_digest_hex: &str,
        descriptor: &sorafs_manifest::chunker_registry::ChunkerProfileDescriptor,
    ) {
        let authority_str = authority.to_string();
        assert_eq!(
            obj.get("authority").and_then(norito::json::Value::as_str),
            Some(authority_str.as_str())
        );
        assert_eq!(
            obj.get("manifest_digest_hex")
                .and_then(norito::json::Value::as_str),
            Some(manifest_digest_hex)
        );
        assert_eq!(
            obj.get("chunk_digest_sha3_256_hex")
                .and_then(norito::json::Value::as_str),
            Some(chunk_digest_hex)
        );
        assert_eq!(
            obj.get("chunker_profile_id")
                .and_then(norito::json::Value::as_u64),
            Some(u64::from(descriptor.id.0))
        );
        assert_eq!(
            obj.get("chunker_namespace")
                .and_then(norito::json::Value::as_str),
            Some(descriptor.namespace)
        );
        assert_eq!(
            obj.get("chunker_name")
                .and_then(norito::json::Value::as_str),
            Some(descriptor.name)
        );
        assert_eq!(
            obj.get("chunker_semver")
                .and_then(norito::json::Value::as_str),
            Some(descriptor.semver)
        );
        assert_eq!(
            obj.get("chunker_multihash_code")
                .and_then(norito::json::Value::as_u64),
            Some(descriptor.multihash_code)
        );
    }

    fn assert_pin_policy_fields(policy_map: &norito::json::Map) {
        assert_eq!(
            policy_map
                .get("min_replicas")
                .and_then(norito::json::Value::as_u64),
            Some(3)
        );
        assert_eq!(
            policy_map
                .get("retention_epoch")
                .and_then(norito::json::Value::as_u64),
            Some(77)
        );
        let storage_class = policy_map
            .get("storage_class")
            .and_then(norito::json::Value::as_object)
            .and_then(|map| map.get("type"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(storage_class, Some("Warm"));
    }

    fn assert_alias_fields(alias_obj: &norito::json::Map, expected_alias_b64: &str) {
        assert_eq!(
            alias_obj
                .get("namespace")
                .and_then(norito::json::Value::as_str),
            Some("sora")
        );
        assert_eq!(
            alias_obj.get("name").and_then(norito::json::Value::as_str),
            Some("docs")
        );
        assert_eq!(
            alias_obj
                .get("proof_base64")
                .and_then(norito::json::Value::as_str),
            Some(expected_alias_b64)
        );
    }

    fn assert_successor_hex(obj: &norito::json::Map, successor_hex: &str) {
        assert_eq!(
            obj.get("successor_of_hex")
                .and_then(norito::json::Value::as_str),
            Some(successor_hex)
        );
    }

    #[test]
    fn post_evidence_sends_json_body_and_headers() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());

        let json = with_mock_http(
            respond_with(
                &snapshots,
                json_response(StatusCode::ACCEPTED, r#"{"ok":true}"#),
            ),
            || client.post_sumeragi_evidence_hex("deadbeef"),
        )
        .expect("call succeeds");
        assert_eq!(
            json.as_object()
                .and_then(|map| map.get("ok"))
                .and_then(Value::as_bool),
            Some(true)
        );

        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert_eq!(snapshot.url.path(), "/v1/sumeragi/evidence");
        let headers: HashMap<_, _> = snapshot
            .headers
            .iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();
        assert_eq!(
            headers.get("content-type"),
            Some(&"application/json".to_owned())
        );
        let payload: norito::json::Value =
            norito::json::from_slice(&snapshot.body).expect("json body");
        let map = payload.as_object().expect("payload object");
        assert_eq!(
            map.get("evidence_hex"),
            Some(&norito::json::Value::String("deadbeef".to_owned()))
        );
    }

    #[test]
    fn post_evidence_empty_body_returns_status_map() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());

        let json = with_mock_http(
            respond_with(&snapshots, empty_response(StatusCode::OK)),
            || client.post_sumeragi_evidence_hex("abcd"),
        )
        .expect("call succeeds");
        let status_value = json
            .as_object()
            .and_then(|map| map.get("status"))
            .expect("status field present");
        match status_value {
            norito::json::Value::Number(number) => {
                assert_eq!(number.as_u64(), Some(200));
            }
            other => panic!("unexpected status payload: {other:?}"),
        }

        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert_eq!(snapshot.url.path(), "/v1/sumeragi/evidence");
    }

    #[test]
    fn post_evidence_propagates_error_status() {
        let client = client_with_base_url(base_url());
        let err = with_mock_http(
            respond_with(
                &Arc::new(Mutex::new(Vec::new())),
                json_response(StatusCode::BAD_REQUEST, r#"{"error":"invalid"}"#),
            ),
            || client.post_sumeragi_evidence_hex("bad"),
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("Failed to submit sumeragi evidence"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn get_evidence_count_fetches_json() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());

        let json = with_mock_http(
            respond_with(&snapshots, json_response(StatusCode::OK, r#"{"count":7}"#)),
            || client.get_sumeragi_evidence_count_json(),
        )
        .expect("count request");
        let count_value = json
            .as_object()
            .and_then(|map| map.get("count"))
            .expect("count field");
        match count_value {
            norito::json::Value::Number(number) => {
                assert_eq!(number.as_u64(), Some(7));
            }
            other => panic!("unexpected count payload: {other:?}"),
        }

        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(snapshot.url.path(), "/v1/sumeragi/evidence/count");
    }

    #[test]
    fn get_evidence_list_includes_query_params() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let filter = SumeragiEvidenceListFilter {
            limit: Some(5),
            offset: Some(2),
            kind: Some("InvalidCommitCertificate"),
        };

        let json = with_mock_http(
            respond_with(&snapshots, json_response(StatusCode::OK, r#"{"items":[]}"#)),
            || client.get_sumeragi_evidence_list_json(&filter),
        )
        .expect("list request");
        assert!(
            json.as_object().is_some(),
            "list response should decode as object"
        );

        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(snapshot.url.path(), "/v1/sumeragi/evidence");
        let params: HashMap<_, _> = snapshot
            .url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(params.get("limit"), Some(&"5".to_string()));
        assert_eq!(params.get("offset"), Some(&"2".to_string()));
        assert_eq!(
            params.get("kind"),
            Some(&"InvalidCommitCertificate".to_string())
        );
    }

    #[test]
    fn get_evidence_count_propagates_error() {
        let client = client_with_base_url(base_url());
        let err = with_mock_http(
            respond_with(
                &Arc::new(Mutex::new(Vec::new())),
                json_response(StatusCode::INTERNAL_SERVER_ERROR, r#"{"error":1}"#),
            ),
            || client.get_sumeragi_evidence_count_json(),
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("Failed to get sumeragi evidence count"),
            "unexpected error: {err}"
        );
    }

    fn make_vote(height: u64, view: u64, seed: u8) -> Vote {
        let hash = Hash::prehashed([seed; 32]);
        Vote {
            phase: Phase::Prepare,
            block_hash: HashOf::from_untyped_unchecked(hash),
            height,
            view,
            epoch: 0,
            highest_cert: None,
            signer: 0,
            bls_sig: Vec::new(),
        }
    }

    fn sample_record() -> EvidenceRecord {
        let v1 = make_vote(10, 3, 0x55);
        let v2 = make_vote(10, 3, 0x66);
        EvidenceRecord {
            evidence: Evidence {
                kind: EvidenceKind::DoublePrepare,
                payload: EvidencePayload::DoubleVote { v1, v2 },
            },
            recorded_at_height: 42,
            recorded_at_view: 5,
            recorded_at_ms: 123_456,
            penalty_applied: false,
            penalty_applied_at_height: None,
        }
    }

    #[test]
    fn get_evidence_list_wire_decodes_norito_payload() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let sample = sample_record();
        let payload = (7_u64, vec![sample.clone()]);

        let (total, items) = with_mock_http(
            respond_with(&snapshots, norito_response(StatusCode::OK, &payload)),
            || client.get_sumeragi_evidence_list_wire(&SumeragiEvidenceListFilter::default()),
        )
        .expect("wire request");

        assert_eq!(total, 7);
        assert_eq!(items, vec![sample]);

        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(snapshot.url.path(), "/v1/sumeragi/evidence");
        let headers: HashMap<_, _> = snapshot
            .headers
            .iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();
        assert_eq!(
            headers.get("accept").map(String::as_str),
            Some(APPLICATION_NORITO)
        );
    }

    #[test]
    fn get_evidence_list_wire_propagates_errors() {
        let client = client_with_base_url(base_url());
        let err = with_mock_http(
            respond_with(
                &Arc::new(Mutex::new(Vec::new())),
                norito_response(
                    StatusCode::BAD_REQUEST,
                    &(0_u64, Vec::<EvidenceRecord>::new()),
                ),
            ),
            || client.get_sumeragi_evidence_list_wire(&SumeragiEvidenceListFilter::default()),
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("Failed to get sumeragi evidence list"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn pipeline_status_404_falls_back_to_committed_query() {
        use iroha_data_model::query::{
            QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse,
        };

        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let store = Arc::clone(&store);
            move |snapshot: RequestSnapshot| {
                let path = snapshot.url.path().to_string();
                store.lock().expect("lock snapshot store").push(snapshot);
                match path.as_str() {
                    "/v1/pipeline/transactions/status" => Ok(empty_response(StatusCode::NOT_FOUND)),
                    "/query" => {
                        let response = QueryResponse::Iterable(QueryOutput {
                            batch: QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::CommittedTransaction(Vec::new())],
                            },
                            remaining_items: 0,
                            continue_cursor: None,
                        });
                        Ok(norito_response(StatusCode::OK, &response))
                    }
                    path => Err(eyre::eyre!("unexpected request path: {path}")),
                }
            }
        };

        let result = with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            let hash =
                HashOf::<crate::data_model::transaction::SignedTransaction>::from_untyped_unchecked(
                    Hash::prehashed([0x11; Hash::LENGTH]),
                );
            let entry_hash: HashOf<crate::data_model::transaction::TransactionEntrypoint> =
                HashOf::from_untyped_unchecked(Hash::prehashed([0x11; Hash::LENGTH]));
            client.transaction_pipeline_status(hash, entry_hash)
        });

        let status = result.expect("pipeline status query");
        assert!(status.is_none());

        let snapshots = store.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].url.path(), "/v1/pipeline/transactions/status");
        assert_eq!(snapshots[1].url.path(), "/query");
    }

    #[test]
    fn pipeline_status_empty_body_falls_back_to_committed_query() {
        use iroha_data_model::query::{
            QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse,
        };

        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let store = Arc::clone(&store);
            move |snapshot: RequestSnapshot| {
                let path = snapshot.url.path().to_string();
                store.lock().expect("lock snapshot store").push(snapshot);
                match path.as_str() {
                    "/v1/pipeline/transactions/status" => Ok(empty_response(StatusCode::OK)),
                    "/query" => {
                        let response = QueryResponse::Iterable(QueryOutput {
                            batch: QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::CommittedTransaction(Vec::new())],
                            },
                            remaining_items: 0,
                            continue_cursor: None,
                        });
                        Ok(norito_response(StatusCode::OK, &response))
                    }
                    path => Err(eyre::eyre!("unexpected request path: {path}")),
                }
            }
        };

        let result = with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            let hash =
                HashOf::<crate::data_model::transaction::SignedTransaction>::from_untyped_unchecked(
                    Hash::prehashed([0x22; Hash::LENGTH]),
                );
            let entry_hash: HashOf<crate::data_model::transaction::TransactionEntrypoint> =
                HashOf::from_untyped_unchecked(Hash::prehashed([0x22; Hash::LENGTH]));
            client.transaction_confirmation_status(hash, entry_hash)
        });

        let status = result.expect("confirmation status query");
        assert!(status.is_none());

        let snapshots = store.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].url.path(), "/v1/pipeline/transactions/status");
        assert_eq!(snapshots[1].url.path(), "/query");
    }

    #[test]
    fn pipeline_status_queued_skips_committed_query() {
        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let payload = norito::json!({
            "content": { "status": { "kind": "Queued" } },
        });
        let status_body = norito::json::to_string(&payload).expect("status payload");
        let responder = {
            let store = Arc::clone(&store);
            move |snapshot: RequestSnapshot| {
                let path = snapshot.url.path().to_string();
                store.lock().expect("lock snapshot store").push(snapshot);
                match path.as_str() {
                    "/v1/pipeline/transactions/status" => {
                        Ok(json_response(StatusCode::OK, &status_body))
                    }
                    path => Err(eyre::eyre!("unexpected request path: {path}")),
                }
            }
        };

        let result = with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            let hash =
                HashOf::<crate::data_model::transaction::SignedTransaction>::from_untyped_unchecked(
                    Hash::prehashed([0x23; Hash::LENGTH]),
                );
            let entry_hash: HashOf<crate::data_model::transaction::TransactionEntrypoint> =
                HashOf::from_untyped_unchecked(Hash::prehashed([0x23; Hash::LENGTH]));
            client.transaction_confirmation_status(hash, entry_hash)
        });

        assert_eq!(
            result.expect("confirmation status query"),
            Some(super::TxConfirmationStatus::Queued)
        );
        let snapshots = store.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].url.path(), "/v1/pipeline/transactions/status");
    }

    #[test]
    fn pipeline_status_approved_with_height_skips_committed_query() {
        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let payload = norito::json!({
            "content": { "status": { "kind": "Approved", "block_height": 9 } },
        });
        let status_body = norito::json::to_string(&payload).expect("status payload");
        let responder = {
            let store = Arc::clone(&store);
            move |snapshot: RequestSnapshot| {
                let path = snapshot.url.path().to_string();
                store.lock().expect("lock snapshot store").push(snapshot);
                match path.as_str() {
                    "/v1/pipeline/transactions/status" => {
                        Ok(json_response(StatusCode::OK, &status_body))
                    }
                    path => Err(eyre::eyre!("unexpected request path: {path}")),
                }
            }
        };

        let result = with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            let hash =
                HashOf::<crate::data_model::transaction::SignedTransaction>::from_untyped_unchecked(
                    Hash::prehashed([0x24; Hash::LENGTH]),
                );
            let entry_hash: HashOf<crate::data_model::transaction::TransactionEntrypoint> =
                HashOf::from_untyped_unchecked(Hash::prehashed([0x24; Hash::LENGTH]));
            client.transaction_confirmation_status(hash, entry_hash)
        });

        assert_eq!(
            result.expect("confirmation status query"),
            Some(super::TxConfirmationStatus::Approved(
                std::num::NonZeroU64::new(9)
            ))
        );
        let snapshots = store.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].url.path(), "/v1/pipeline/transactions/status");
    }

    #[test]
    fn pipeline_status_approved_without_height_falls_back_to_committed_query() {
        use iroha_data_model::query::{
            QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse,
        };

        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let payload = norito::json!({
            "content": { "status": { "kind": "Approved" } },
        });
        let status_body = norito::json::to_string(&payload).expect("status payload");
        let responder = {
            let store = Arc::clone(&store);
            move |snapshot: RequestSnapshot| {
                let path = snapshot.url.path().to_string();
                store.lock().expect("lock snapshot store").push(snapshot);
                match path.as_str() {
                    "/v1/pipeline/transactions/status" => {
                        Ok(json_response(StatusCode::OK, &status_body))
                    }
                    "/query" => {
                        let response = QueryResponse::Iterable(QueryOutput {
                            batch: QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::CommittedTransaction(Vec::new())],
                            },
                            remaining_items: 0,
                            continue_cursor: None,
                        });
                        Ok(norito_response(StatusCode::OK, &response))
                    }
                    path => Err(eyre::eyre!("unexpected request path: {path}")),
                }
            }
        };

        let result = with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            let hash =
                HashOf::<crate::data_model::transaction::SignedTransaction>::from_untyped_unchecked(
                    Hash::prehashed([0x24; Hash::LENGTH]),
                );
            let entry_hash: HashOf<crate::data_model::transaction::TransactionEntrypoint> =
                HashOf::from_untyped_unchecked(Hash::prehashed([0x24; Hash::LENGTH]));
            client.transaction_confirmation_status(hash, entry_hash)
        });

        assert_eq!(
            result.expect("confirmation status query"),
            Some(super::TxConfirmationStatus::Approved(None))
        );
        let snapshots = store.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].url.path(), "/v1/pipeline/transactions/status");
        assert_eq!(snapshots[1].url.path(), "/query");
    }

    #[test]
    fn pipeline_status_rejection_without_reason_uses_committed_query() {
        use iroha_data_model::query::{
            QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse,
        };

        use crate::{
            crypto::{MerkleProof, PrivateKey, PublicKey},
            data_model::{
                ValidationFail,
                prelude::{AccountId, ChainId, DomainId, TransactionBuilder},
                query::CommittedTransaction,
                transaction::{
                    TransactionEntrypoint, TransactionResult, error::TransactionRejectionReason,
                },
            },
        };

        let chain: ChainId = "hash-chain".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(domain, public_key);
        let private_key: PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let tx = TransactionBuilder::new(chain, authority.clone()).sign(&private_key);
        let hash = tx.hash();
        let entry = TransactionEntrypoint::External(tx);
        let entry_hash = entry.hash();
        let reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "nope".to_string(),
        ));
        let result = TransactionResult(Err(reason.clone()));
        let committed = CommittedTransaction {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0x33; Hash::LENGTH])),
            entrypoint_hash: entry_hash,
            entrypoint_proof: MerkleProof::from_audit_path(0, Vec::new()),
            entrypoint: entry,
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, Vec::new()),
            result,
        };
        let status_payload = norito::json!({
            "kind": "Transaction",
            "content": {
                "hash": "deadbeef",
                "status": { "kind": "Rejected" },
            },
        });
        let status_body = norito::json::to_string(&status_payload).expect("status payload");

        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = {
            let store = Arc::clone(&store);
            move |snapshot: RequestSnapshot| {
                let path = snapshot.url.path().to_string();
                store.lock().expect("lock snapshot store").push(snapshot);
                match path.as_str() {
                    "/v1/pipeline/transactions/status" => {
                        Ok(json_response(StatusCode::OK, &status_body))
                    }
                    "/query" => {
                        let response = QueryResponse::Iterable(QueryOutput {
                            batch: QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::CommittedTransaction(vec![
                                    committed.clone(),
                                ])],
                            },
                            remaining_items: 0,
                            continue_cursor: None,
                        });
                        Ok(norito_response(StatusCode::OK, &response))
                    }
                    path => Err(eyre::eyre!("unexpected request path: {path}")),
                }
            }
        };

        let result = with_mock_http(responder, || {
            let client = client_with_base_url(base_url());
            client.transaction_confirmation_status(hash, entry_hash)
        });

        assert_eq!(
            result.expect("confirmation status query"),
            Some(super::TxConfirmationStatus::Rejected(Some(reason)))
        );
    }
}

/// Private structure to incapsulate error reporting for HTTP response.
pub(crate) struct ResponseReport(pub(crate) eyre::Report);

impl ResponseReport {
    /// Constructs report with provided message
    ///
    /// # Errors
    /// If response body isn't a valid utf-8 string
    pub(crate) fn with_msg<S: AsRef<str>>(
        msg: S,
        response: &Response<Vec<u8>>,
    ) -> Result<Self, Self> {
        let status = response.status();
        let reject_suffix = response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .map_or_else(String::new, |code| format!("; reject code: {code}"));
        let body = std::str::from_utf8(response.body());
        let msg = msg.as_ref();

        body.map_err(|_| {
            Self(eyre!(
                "{msg}; status: {status}{reject_suffix}; body isn't a valid utf-8 string"
            ))
        })
        .map(|body| {
            Self(eyre!(
                "{msg}; status: {status}{reject_suffix}; response body: {body}"
            ))
        })
    }
}

impl From<ResponseReport> for eyre::Report {
    #[inline]
    fn from(report: ResponseReport) -> Self {
        report.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TxConfirmationStatus {
    Queued,
    Approved(Option<NonZeroU64>),
    Committed,
    Applied,
    Rejected(Option<TransactionRejectionReason>),
    Expired,
}

#[derive(Debug)]
struct TxConfirmationFinalError {
    report: eyre::Report,
}

impl TxConfirmationFinalError {
    fn new(report: eyre::Report) -> Self {
        Self { report }
    }
}

impl fmt::Display for TxConfirmationFinalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.report)
    }
}

impl std::error::Error for TxConfirmationFinalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.report.as_ref())
    }
}

fn tx_confirmation_final_report(report: eyre::Report) -> eyre::Report {
    TxConfirmationFinalError::new(report).into()
}

fn is_final_tx_confirmation_error(err: &eyre::Report) -> bool {
    err.chain()
        .any(|cause| cause.downcast_ref::<TxConfirmationFinalError>().is_some())
}

fn should_fallback_after_confirmation_error(err: &eyre::Report) -> bool {
    !is_final_tx_confirmation_error(err)
}

/// Iroha client
#[derive(Clone, Display)]
#[display("{}@{torii_url}", key_pair.public_key())]
/// Main entry point used by external applications to communicate with an Iroha
/// peer.  The client is lightweight and holds only configuration data needed
/// to build and sign transactions.
pub struct Client {
    /// Unique id of the blockchain. Used for simple replay attack protection.
    pub chain: ChainId,
    /// Url for accessing Iroha node
    pub torii_url: Url,
    /// Accounts keypair
    pub key_pair: KeyPair,
    /// Transaction time to live in milliseconds
    pub transaction_ttl: Option<Duration>,
    /// Transaction status timeout
    pub transaction_status_timeout: Duration,
    /// Timeout for Torii HTTP requests.
    pub torii_request_timeout: Duration,
    /// Current account
    pub account: AccountId,
    /// Http headers which will be appended to each request
    pub headers: HashMap<String, String>,
    /// If `true` add nonce, which makes different hashes for
    /// transactions which occur repeatedly and/or simultaneously
    pub add_transaction_nonce: bool,
    /// Alias cache policy enforced when validating `SoraFS` alias proofs.
    pub alias_cache_policy: sorafs_manifest::alias_cache::AliasCachePolicy,
    /// Default `SoraNet` anonymity policy stage applied to gateway fetches.
    pub default_anonymity_policy: AnonymityPolicy,
    /// Rollout phase controlling the default anonymity policy.
    pub rollout_phase: SorafsRolloutPhase,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("chain", &self.chain)
            .field("torii_url", &self.torii_url)
            .field("public_key", &self.key_pair.public_key())
            .field("transaction_ttl", &self.transaction_ttl)
            .field(
                "transaction_status_timeout",
                &self.transaction_status_timeout,
            )
            .field("torii_request_timeout", &self.torii_request_timeout)
            .field("account", &self.account)
            .field("headers", &self.headers)
            .field("add_transaction_nonce", &self.add_transaction_nonce)
            .field("alias_cache_policy", &self.alias_cache_policy)
            .field(
                "default_anonymity_policy",
                &self.default_anonymity_policy.label(),
            )
            .field("rollout_phase", &self.rollout_phase.label())
            .finish()
    }
}

/// Representation of `Iroha` client.
impl Client {
    /// Constructor for client from configuration
    #[inline]
    pub fn new(configuration: Config) -> Self {
        Self::with_headers(configuration, HashMap::new())
    }

    /// Constructor for client from configuration and headers
    ///
    /// *Authorization* header will be added if `basic_auth` is presented
    #[inline]
    pub fn with_headers(
        Config {
            chain,
            account,
            torii_api_url,
            torii_api_version,
            torii_api_min_proof_version: _torii_api_min_proof_version,
            torii_request_timeout,
            key_pair,
            basic_auth,
            transaction_add_nonce,
            transaction_ttl,
            transaction_status_timeout,
            connect_queue_root: _connect_queue_root,
            sorafs_alias_cache,
            sorafs_anonymity_policy,
            sorafs_rollout_phase,
        }: Config,
        mut headers: HashMap<String, String>,
    ) -> Self {
        headers
            .entry(HEADER_API_VERSION.to_string())
            .or_insert_with(|| torii_api_version.clone());
        if let Some(basic_auth) = basic_auth {
            let credentials = format!(
                "{}:{}",
                basic_auth.web_login,
                basic_auth.password.expose_secret()
            );
            let engine = base64::engine::general_purpose::STANDARD;
            let encoded = base64::engine::Engine::encode(&engine, credentials);
            headers.insert(String::from("Authorization"), format!("Basic {encoded}"));
        }

        Self {
            chain,
            torii_url: torii_api_url,
            key_pair,
            transaction_ttl: Some(transaction_ttl),
            transaction_status_timeout,
            torii_request_timeout,
            account,
            headers,
            add_transaction_nonce: transaction_add_nonce,
            alias_cache_policy: sorafs_alias_cache,
            default_anonymity_policy: sorafs_anonymity_policy,
            rollout_phase: sorafs_rollout_phase,
        }
    }

    pub(crate) fn default_request(&self, method: HttpMethod, url: Url) -> DefaultRequestBuilder {
        let mut builder = DefaultRequestBuilder::new(method, url)
            .headers(&self.headers)
            .header(http::header::ACCEPT, APPLICATION_JSON);
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        builder
    }

    fn send_builder(&self, builder: DefaultRequestBuilder) -> Result<Response<Vec<u8>>> {
        let request = builder.build()?;
        self.send_prepared_request(request)
    }

    fn send_prepared_request(&self, request: DefaultRequest) -> Result<Response<Vec<u8>>> {
        let response = request.send()?;
        self.enforce_alias_policy(&response)?;
        Ok(response)
    }

    fn build_sorafs_gateway_fetch_config(
        &self,
        options: &SorafsGatewayFetchOptions,
    ) -> (OrchestratorConfig, Option<usize>) {
        let telemetry_region = options
            .telemetry_region
            .clone()
            .or_else(|| Some(self.chain.to_string()));
        let mut config = OrchestratorConfig {
            telemetry_region,
            ..OrchestratorConfig::default()
        };
        let rollout_phase = match self.rollout_phase {
            SorafsRolloutPhase::Canary => RolloutPhase::Canary,
            SorafsRolloutPhase::Ramp => RolloutPhase::Ramp,
            SorafsRolloutPhase::Default => RolloutPhase::Default,
        };
        config = config.with_rollout_phase(rollout_phase);
        let phase_default_policy = config.anonymity_policy;
        if self.default_anonymity_policy != phase_default_policy {
            config.anonymity_policy = self.default_anonymity_policy;
            config.anonymity_policy_override = Some(self.default_anonymity_policy);
        }
        config.write_mode = options.write_mode_hint.unwrap_or(WriteModeHint::ReadOnly);
        if let Some(budget) = options.retry_budget {
            config.fetch.per_chunk_retry_limit = if budget == 0 { None } else { Some(budget) };
        }
        let max_peers = options
            .max_peers
            .and_then(|value| if value == 0 { None } else { Some(value) });
        let mut explicit_transport = options.transport_policy.is_some_and(|policy| {
            config.transport_policy = policy;
            true
        });
        let mut explicit_anonymity = options.anonymity_policy.is_some_and(|policy| {
            config.anonymity_policy = policy;
            config.anonymity_policy_override = Some(policy);
            true
        });
        config.policy_override = options.policy_override.clone();
        if config.policy_override.transport_policy.is_some() {
            explicit_transport = true;
        }
        if config.policy_override.anonymity_policy.is_some() {
            explicit_anonymity = true;
        }
        if let Some(guard_set) = options.guard_set.clone() {
            config.guard_set = Some(guard_set);
        }
        if let Some(directory) = options.relay_directory.clone() {
            config.relay_directory = Some(directory);
        }
        if let Some(scoreboard) = &options.scoreboard {
            if let Some(path) = scoreboard.persist_path.as_ref() {
                config.scoreboard.persist_path = Some(path.clone());
            }
            if let Some(now) = scoreboard.now_unix_secs {
                config.scoreboard.now_unix_secs = now;
            }
            let telemetry_label = derive_scoreboard_telemetry_label(
                scoreboard.telemetry_source_label.as_deref(),
                options,
                &self.chain,
            );
            config.scoreboard.persist_metadata = Some(ensure_scoreboard_metadata(
                scoreboard.metadata.clone(),
                Some(telemetry_label.as_str()),
                config.scoreboard.now_unix_secs,
            ));
        }
        if config.write_mode.enforces_pq_only() {
            if !explicit_anonymity {
                config.anonymity_policy = AnonymityPolicy::StrictPq;
                config.anonymity_policy_override = Some(AnonymityPolicy::StrictPq);
            }
            if !explicit_transport {
                config.transport_policy = TransportPolicy::SoranetStrict;
            }
        }
        (config, max_peers)
    }

    fn enforce_alias_policy(&self, response: &Response<Vec<u8>>) -> Result<()> {
        if response.status() != StatusCode::OK {
            return Ok(());
        }

        let headers = response.headers();
        let proof_header = match headers.get(HEADER_SORA_PROOF) {
            Some(value) => value,
            None => return Ok(()),
        };

        let proof_b64 = proof_header
            .to_str()
            .wrap_err("Sora-Proof header must contain valid ASCII")?;
        let proof_bytes = base64::engine::general_purpose::STANDARD
            .decode(proof_b64.as_bytes())
            .wrap_err("failed to decode Sora-Proof header as base64")?;
        let bundle = decode_alias_proof(&proof_bytes)
            .wrap_err("invalid alias proof bundle in Sora-Proof header")?;

        let evaluation = self.alias_cache_policy.evaluate(&bundle, unix_now_secs());

        if evaluation.state.is_servable() {
            return Ok(());
        }

        let alias_label = headers
            .get(HEADER_SORA_NAME)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("<unknown>");
        let status_header = headers
            .get(HEADER_SORA_PROOF_STATUS)
            .and_then(|value| value.to_str().ok());

        let status_label = evaluation.status_label();
        let status_hint = status_header
            .map(|hdr| format!("; header reported {hdr}"))
            .unwrap_or_default();

        Err(eyre!(
            "alias proof for `{alias_label}` rejected: state {status_label}{status_hint} (age {} seconds)",
            evaluation.age.as_secs()
        ))
    }

    /// Builds transaction out of supplied instructions or IVM bytecode.
    ///
    /// # Errors
    /// Fails if signing transaction fails
    pub fn build_transaction<Exec: Into<Executable>>(
        &self,
        instructions: Exec,
        metadata: Metadata,
    ) -> SignedTransaction {
        let tx_builder = TransactionBuilder::new(self.chain.clone(), self.account.clone());

        let mut tx_builder = match instructions.into() {
            Executable::Instructions(instructions) => tx_builder.with_instructions(instructions),
            Executable::Ivm(bytecode) => tx_builder.with_bytecode(bytecode),
        };

        if let Some(transaction_ttl) = self.transaction_ttl {
            tx_builder.set_ttl(transaction_ttl);
        }
        if self.add_transaction_nonce {
            let mut rng = rand::rng();
            let nonce: NonZeroU32 = rng.random();
            tx_builder.set_nonce(nonce);
        }

        tx_builder
            .with_metadata(metadata)
            .sign(self.key_pair.private_key())
    }

    /// Builds a transaction from a collection of items convertible into `InstructionBox`.
    ///
    /// This avoids re-boxing already boxed instructions.
    pub fn build_transaction_from_items<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
        metadata: Metadata,
    ) -> SignedTransaction
    where
        I: Into<InstructionBox>,
    {
        let mut tx_builder = TransactionBuilder::new(self.chain.clone(), self.account.clone())
            .with_instructions(instructions);

        if let Some(transaction_ttl) = self.transaction_ttl {
            tx_builder.set_ttl(transaction_ttl);
        }
        if self.add_transaction_nonce {
            let mut rng = rand::rng();
            let nonce: NonZeroU32 = rng.random();
            tx_builder.set_nonce(nonce);
        }

        tx_builder
            .with_metadata(metadata)
            .sign(self.key_pair.private_key())
    }

    /// Signs transaction
    ///
    /// # Errors
    /// Fails if signature generation fails
    pub fn sign_transaction(&self, transaction: TransactionBuilder) -> SignedTransaction {
        transaction.sign(self.key_pair.private_key())
    }

    /// Instructions API entry point. Submits one Iroha Special Instruction to `Iroha` peers.
    /// Returns submitted transaction's hash or error string.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit<I>(&self, isi: I) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all([isi])
    }

    /// Instructions API entry point. Submits several Iroha Special Instructions to `Iroha` peers.
    /// Returns submitted transaction's hash or error string.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_all<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_with_metadata(instructions, Metadata::default())
    }

    /// Instructions API entry point. Submits one Iroha Special Instruction to `Iroha` peers.
    /// Allows to specify [`Metadata`] of [`TransactionBuilder`].
    /// Returns submitted transaction's hash or error string.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_with_metadata<I>(
        &self,
        instruction: I,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_with_metadata([instruction], metadata)
    }

    /// Instructions API entry point. Submits several Iroha Special Instructions to `Iroha` peers.
    /// Allows to specify [`Metadata`] of [`TransactionBuilder`].
    /// Returns submitted transaction's hash or error string.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_all_with_metadata<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_transaction(&self.build_transaction_from_items(instructions, metadata))
    }

    /// Submit a prebuilt transaction.
    /// Returns submitted transaction's hash or error string.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_transaction(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<HashOf<SignedTransaction>> {
        iroha_logger::trace!(tx=?transaction, "Submitting");
        let (req, hash) = self.prepare_transaction_request::<DefaultRequestBuilder>(transaction);
        let response = req
            .build()?
            .send()
            .wrap_err_with(|| format!("Failed to send transaction with hash {hash:?}"))?;
        TransactionResponseHandler::handle(&response)?;
        Ok(hash)
    }

    /// Submit the prebuilt transaction and wait until it is either rejected or committed.
    /// If rejected, return the rejection reason.
    /// Note: `Committed` is emitted after Kura persistence (before WSV apply).
    /// Wait for `Applied` if you require WSV apply semantics.
    ///
    /// # Errors
    /// Fails if sending a transaction to a peer fails or there is an error in the response
    pub fn submit_transaction_blocking(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<HashOf<SignedTransaction>> {
        let (init_sender, init_receiver) = tokio::sync::oneshot::channel();
        let hash = transaction.hash();
        let entry_hash = transaction.hash_as_entrypoint();
        tracing::debug!(%hash, ?transaction, "Submitting transaction");

        thread::scope(|spawner| {
            let submitter_handle = spawner.spawn(move || -> Result<()> {
                // Wait for the listener connection attempt so we don't miss early
                // events, but still proceed even if setup fails.
                let _ = init_receiver.blocking_recv();
                self.submit_transaction(transaction)?;
                Ok(())
            });

            let confirmation_res = self.listen_for_tx_confirmation(init_sender, hash, entry_hash);

            match submitter_handle.join() {
                Ok(Ok(())) => confirmation_res,
                Ok(Err(e)) => Err(e).wrap_err("Transaction submitter thread exited with error"),
                Err(_) => Err(eyre!("Transaction submitter thread panicked")),
            }
        })
    }

    fn tx_confirmation_filters(hash: HashOf<SignedTransaction>) -> Vec<PipelineEventFilterBox> {
        vec![
            TransactionEventFilter::default().for_hash(hash).into(),
            PipelineEventFilterBox::from(
                BlockEventFilter::default().for_status(BlockStatus::Applied),
            ),
            PipelineEventFilterBox::from(
                BlockEventFilter::default().for_status(BlockStatus::Committed),
            ),
        ]
    }

    #[allow(clippy::too_many_lines)]
    fn listen_for_tx_confirmation(
        &self,
        init_sender: tokio::sync::oneshot::Sender<bool>,
        hash: HashOf<SignedTransaction>,
        entry_hash: HashOf<TransactionEntrypoint>,
    ) -> Result<HashOf<SignedTransaction>> {
        debug!(
            %hash,
            timeout_ms = %self.transaction_status_timeout.as_millis(),
            "starting tx confirmation listener"
        );
        let deadline = tokio::time::Instant::now() + self.transaction_status_timeout;
        thread::scope(|scope| {
            let client = self;
            scope
                .spawn(move || -> Result<HashOf<SignedTransaction>> {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    rt.block_on(async {
                        let timeout_err = || Self::tx_confirmation_timeout_report(client.transaction_status_timeout);
                        let filters = Self::tx_confirmation_filters(hash);
                        let connect_timeout =
                            Self::tx_confirmation_connect_timeout(client.transaction_status_timeout);
                        let connect_deadline = tokio::time::Instant::now() + connect_timeout;
                        let event_iterator_result = tokio::time::timeout_at(
                            connect_deadline,
                            client.listen_for_events_async(filters),
                        )
                        .await
                        .map_err(Into::into)
                        .and_then(std::convert::identity)
                        .wrap_err("Failed to establish event listener connection");
                        // Signal the submitter thread after attempting to connect so we
                        // avoid missing early events when the stream is available.
                        let _ = init_sender.send(true);
                        let mut event_iterator = match event_iterator_result {
                            Ok(iter) => Some(iter),
                            Err(err) => {
                                warn!(
                                    %hash,
                                    ?err,
                                    timeout_ms = %connect_timeout.as_millis(),
                                    "tx confirmation listener setup failed; continuing with status polling"
                                );
                                None
                            }
                        };
                        let max_queued_duration = if client.transaction_status_timeout
                            == Duration::ZERO
                        {
                            DEFAULT_MAX_QUEUED_DURATION
                        } else {
                            client.transaction_status_timeout
                        };
                        let poll_interval =
                            Self::tx_confirmation_poll_interval(client.transaction_status_timeout);
                        let hash_for_check = hash;
                        let entry_hash_for_check = entry_hash;
                        let result = if let Some(ref mut iterator) = event_iterator {
                            tokio::time::timeout_at(
                                deadline,
                                Self::listen_for_tx_confirmation_loop(
                                    iterator,
                                    hash,
                                    max_queued_duration,
                                    poll_interval,
                                    || {
                                        client.transaction_confirmation_status(
                                            hash_for_check,
                                            entry_hash_for_check,
                                        )
                                    },
                                ),
                            )
                            .await
                        } else {
                            let mut empty_stream = stream::empty::<Result<EventBox>>();
                            tokio::time::timeout_at(
                                deadline,
                                listen_for_tx_confirmation_stream_with_status_check(
                                    &mut empty_stream,
                                    hash,
                                    max_queued_duration,
                                    poll_interval,
                                    || {
                                        client.transaction_confirmation_status(
                                            hash_for_check,
                                            entry_hash_for_check,
                                        )
                                    },
                                ),
                            )
                            .await
                        };
                        if let Some(iterator) = event_iterator {
                            iterator.close().await;
                        }
                        match result {
                            Ok(inner) => match inner {
                                Ok(ok) => Ok(ok),
                                Err(err) => {
                                    if !should_fallback_after_confirmation_error(&err) {
                                        return Err(err);
                                    }
                                    let err = err.wrap_err(timeout_err());
                                    warn!(
                                        %hash,
                                        ?err,
                                        "tx confirmation stream returned error; falling back to pipeline status query"
                                    );
                                    Self::resolve_committed_fallback(
                                        || client.transaction_confirmation_status(hash, entry_hash),
                                        hash,
                                        Duration::from_millis(200),
                                        3,
                                        "transaction confirmation stream failed; fallback status check failed",
                                        err,
                                    )
                                    .await
                                }
                            },
                            Err(_elapsed) => {
                                warn!(
                                    %hash,
                                    "tx confirmation timed out; falling back to pipeline status query"
                                );
                                Self::resolve_committed_fallback(
                                    || client.transaction_confirmation_status(hash, entry_hash),
                                    hash,
                                    Duration::from_millis(200),
                                    3,
                                    "transaction confirmation timed out; fallback status check failed",
                                    timeout_err(),
                                )
                                .await
                            }
                        }
                    })
                })
                .join()
                .unwrap_or_else(|_| Err(eyre!("tx confirmation listener thread panicked")))
        })
    }

    async fn listen_for_tx_confirmation_loop<F>(
        event_iterator: &mut AsyncEventStream,
        hash: HashOf<SignedTransaction>,
        max_queued_duration: Duration,
        poll_interval: Duration,
        status_check: F,
    ) -> Result<HashOf<SignedTransaction>>
    where
        F: FnMut() -> Result<Option<TxConfirmationStatus>>,
    {
        listen_for_tx_confirmation_stream_with_status_check(
            event_iterator,
            hash,
            max_queued_duration,
            poll_interval,
            status_check,
        )
        .await
    }

    fn tx_confirmation_timeout_report(timeout: Duration) -> eyre::Report {
        eyre!(
            "haven't got tx confirmation within {:?} (configured with `transaction.status_timeout_ms`)",
            timeout
        )
    }

    fn tx_confirmation_connect_timeout(timeout: Duration) -> Duration {
        if timeout == Duration::ZERO {
            return Duration::from_millis(500);
        }
        let min = Duration::from_millis(200);
        let max = Duration::from_secs(5);
        let candidate = timeout / 10;
        let bounded = if candidate < min {
            min
        } else if candidate > max {
            max
        } else {
            candidate
        };
        bounded.min(timeout)
    }

    fn tx_confirmation_poll_interval(timeout: Duration) -> Duration {
        if timeout == Duration::ZERO {
            return Duration::from_millis(500);
        }
        let min = Duration::from_millis(200);
        let max = Duration::from_secs(2);
        let candidate = timeout / 40;
        if candidate < min {
            min
        } else if candidate > max {
            max
        } else {
            candidate
        }
    }

    async fn resolve_committed_fallback<F>(
        check: F,
        hash: HashOf<SignedTransaction>,
        delay: Duration,
        retries: usize,
        context: &'static str,
        fallback_err: eyre::Report,
    ) -> Result<HashOf<SignedTransaction>>
    where
        F: FnMut() -> Result<Option<TxConfirmationStatus>>,
    {
        let outcome = Self::retry_transaction_committed(check, delay, retries)
            .await
            .wrap_err(context)?;
        outcome.map_or_else(
            || Err(fallback_err),
            |result| match result {
                TxConfirmationStatus::Committed | TxConfirmationStatus::Applied => Ok(hash),
                TxConfirmationStatus::Rejected(Some(reason)) => {
                    Err(tx_rejection_to_report(&reason))
                }
                TxConfirmationStatus::Rejected(None) => Err(eyre!("Transaction rejected")),
                TxConfirmationStatus::Expired => Err(eyre!("Transaction expired")),
                TxConfirmationStatus::Queued | TxConfirmationStatus::Approved(_) => {
                    Err(eyre!("Transaction status not finalized"))
                }
            },
        )
    }

    async fn retry_transaction_committed<F>(
        mut check: F,
        delay: Duration,
        retries: usize,
    ) -> Result<Option<TxConfirmationStatus>>
    where
        F: FnMut() -> Result<Option<TxConfirmationStatus>>,
    {
        let mut last_err: Option<eyre::Report> = None;
        for attempt in 0..=retries {
            match check() {
                Ok(Some(outcome)) => match outcome {
                    status @ (TxConfirmationStatus::Queued | TxConfirmationStatus::Approved(_)) => {
                        debug!(
                            attempt,
                            ?delay,
                            status = ?status,
                            "tx confirmation status not final; retrying"
                        );
                        if attempt == retries {
                            return Ok(None);
                        }
                        tokio::time::sleep(delay).await;
                    }
                    other => return Ok(Some(other)),
                },
                Ok(None) => {
                    debug!(
                        attempt,
                        ?delay,
                        "tx confirmation status not ready; retrying"
                    );
                    if attempt == retries {
                        return Ok(None);
                    }
                    tokio::time::sleep(delay).await;
                }
                Err(err) => {
                    debug!(attempt, ?delay, ?err, "tx confirmation status check failed");
                    last_err = Some(err);
                    if attempt == retries {
                        break;
                    }
                    tokio::time::sleep(delay).await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| eyre!("retry_transaction_committed exhausted")))
    }

    fn committed_transaction_matches_hash(
        tx: &crate::data_model::query::CommittedTransaction,
        target: HashOf<SignedTransaction>,
        entry_hash: HashOf<TransactionEntrypoint>,
    ) -> bool {
        if tx.entrypoint_hash().as_ref() == entry_hash.as_ref() {
            return true;
        }
        if hashes_match(&target, tx.entrypoint_hash().as_ref()) {
            return true;
        }
        match tx.entrypoint() {
            crate::data_model::transaction::TransactionEntrypoint::External(entry) => {
                entry.hash() == target
            }
            crate::data_model::transaction::TransactionEntrypoint::Time(_) => false,
        }
    }

    fn transaction_confirmation_status(
        &self,
        hash: HashOf<SignedTransaction>,
        entry_hash: HashOf<TransactionEntrypoint>,
    ) -> Result<Option<TxConfirmationStatus>> {
        match self.transaction_pipeline_status(hash, entry_hash) {
            Ok(Some(status)) => match status {
                TxConfirmationStatus::Rejected(None) | TxConfirmationStatus::Approved(None) => {
                    let committed = self.transaction_committed(hash, entry_hash)?;
                    Ok(committed.or(Some(status)))
                }
                _ => Ok(Some(status)),
            },
            Ok(None) => self.transaction_committed(hash, entry_hash),
            Err(err) => {
                warn!(
                    %hash,
                    ?err,
                    "pipeline status query failed; falling back to committed query"
                );
                self.transaction_committed(hash, entry_hash)
            }
        }
    }

    fn transaction_pipeline_status(
        &self,
        hash: HashOf<SignedTransaction>,
        entry_hash: HashOf<TransactionEntrypoint>,
    ) -> Result<Option<TxConfirmationStatus>> {
        let hash_hex = bytes_to_hex(hash.as_ref());
        let url = join_torii_url(&self.torii_url, "v1/pipeline/transactions/status");
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .param("hash", &hash_hex),
        )?;
        match resp.status() {
            StatusCode::OK | StatusCode::ACCEPTED => {
                if resp.body().is_empty() {
                    return Ok(None);
                }
                let payload: JsonValue = norito::json::from_slice(resp.body())?;
                Ok(tx_confirmation_status_from_pipeline_payload(&payload))
            }
            StatusCode::NO_CONTENT => Ok(None),
            StatusCode::NOT_FOUND => {
                debug!(
                    %hash,
                    "pipeline status query returned 404; falling back to committed query"
                );
                self.transaction_committed(hash, entry_hash)
            }
            status => Err(eyre!(
                "Failed to get pipeline transaction status: {} {}",
                status,
                std::str::from_utf8(resp.body()).unwrap_or("")
            )),
        }
    }

    fn transaction_committed(
        &self,
        hash: HashOf<SignedTransaction>,
        entry_hash: HashOf<TransactionEntrypoint>,
    ) -> Result<Option<TxConfirmationStatus>> {
        use crate::data_model::query::transaction::prelude::FindTransactions;

        let snapshot = self.query(FindTransactions::new()).execute_all()?;
        let outcome = snapshot
            .iter()
            .find(|tx| Self::committed_transaction_matches_hash(tx, hash, entry_hash))
            .map(|tx| tx_confirmation_status_from_committed_result(tx.result()));
        debug!(
            %hash,
            snapshot_len = snapshot.len(),
            found = outcome.is_some(),
            "transaction_committed snapshot check"
        );
        Ok(outcome)
    }

    /// Lower-level Instructions API entry point.
    ///
    /// Returns a tuple with a provided request builder, a hash of the transaction, and a response handler.
    /// Despite the fact that response handling can be implemented just by asserting that status code is 200,
    /// it is better to use a response handler anyway. It allows to abstract from implementation details.
    ///
    /// For general usage example see [`Client::prepare_query_request`].
    fn prepare_transaction_request<B: RequestBuilder>(
        &self,
        transaction: &SignedTransaction,
    ) -> (B, HashOf<SignedTransaction>) {
        let transaction_bytes: Vec<u8> = transaction.encode_versioned();
        let mut headers = self.headers.clone();
        headers.retain(|name, _| !name.eq_ignore_ascii_case("content-type"));

        (
            B::new(
                HttpMethod::POST,
                join_torii_url(&self.torii_url, torii_uri::TRANSACTION),
            )
            .headers(headers)
            .header("Content-Type", APPLICATION_NORITO)
            .body(transaction_bytes),
            transaction.hash(),
        )
    }

    /// Submits and waits until the transaction is either rejected or committed.
    /// Returns rejection reason if transaction was rejected.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_blocking<I>(&self, instruction: I) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_blocking(core::iter::once(instruction))
    }

    /// Submits and waits until the transaction is either rejected or committed.
    /// Returns rejection reason if transaction was rejected.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_all_blocking<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_blocking_with_metadata(instructions, Metadata::default())
    }

    /// Submits and waits until the transaction is either rejected or committed.
    /// Allows to specify [`Metadata`] of [`TransactionBuilder`].
    /// Returns rejection reason if transaction was rejected.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_blocking_with_metadata<I>(
        &self,
        instruction: I,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_blocking_with_metadata(core::iter::once(instruction), metadata)
    }

    /// Submits and waits until the transaction is either rejected or committed.
    /// Allows to specify [`Metadata`] of [`TransactionBuilder`].
    /// Returns rejection reason if transaction was rejected.
    ///
    /// # Errors
    /// Fails if sending transaction to peer fails or if it response with error
    pub fn submit_all_blocking_with_metadata<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";
        let instructions: Vec<InstructionBox> = instructions.into_iter().map(Into::into).collect();

        // Guard against reserved multisig role names being registered by regular clients.
        // Server-side executors should enforce this, but we also short-circuit here to
        // avoid committing an invalid transaction when the executor is missing the check.
        for instr in &instructions {
            if let Some(RegisterBox::Role(register_role)) =
                instr.as_any().downcast_ref::<RegisterBox>()
                && register_role
                    .object()
                    .id()
                    .name()
                    .as_ref()
                    .starts_with(MULTISIG_SIGNATORY)
            {
                return Err(eyre!(
                    "reserved multisig role names may not be registered by clients"
                ));
            }
        }

        let transaction = self.build_transaction_from_items(instructions, metadata);
        self.submit_transaction_blocking(&transaction)
    }

    /// Connect (through `WebSocket`) to listen for `Iroha` `pipeline` and `data` events.
    /// Pipeline events are emitted from the commit worker thread, so ordering with data events
    /// is not single-threaded.
    ///
    /// # Errors
    /// - Forwards from [`Self::events_handler`]
    /// - Forwards from `events_api::EventIterator::new`
    pub fn listen_for_events(
        &self,
        event_filters: impl IntoIterator<Item = impl Into<EventFilterBox>>,
    ) -> Result<impl Iterator<Item = Result<EventBox>>> {
        events_api::EventIterator::new(self.events_handler(event_filters)?)
    }

    /// Connect asynchronously (through `WebSocket`) to listen for `Iroha` `pipeline` and `data` events.
    /// Pipeline events are emitted from the commit worker thread, so ordering with data events
    /// is not single-threaded.
    ///
    /// # Errors
    /// - Forwards from [`Self::events_handler`]
    /// - Forwards from `events_api::AsyncEventStream::new`
    pub async fn listen_for_events_async(
        &self,
        event_filters: impl IntoIterator<Item = impl Into<EventFilterBox>> + Send,
    ) -> Result<AsyncEventStream> {
        events_api::AsyncEventStream::new(self.events_handler(event_filters)?).await
    }

    /// Constructs an Events API handler. With it, you can use any WS client you want.
    ///
    /// # Errors
    /// Fails if handler construction fails
    #[inline]
    pub fn events_handler(
        &self,
        event_filters: impl IntoIterator<Item = impl Into<EventFilterBox>>,
    ) -> Result<events_api::flow::Init> {
        events_api::flow::Init::new(
            event_filters.into_iter().map(Into::into).collect(),
            self.headers.clone(),
            join_torii_url(&self.torii_url, torii_uri::SUBSCRIPTION),
        )
    }

    /// Connect (through `WebSocket`) to listen for `Iroha` blocks
    ///
    /// # Errors
    /// - Forwards from [`Self::events_handler`]
    /// - Forwards from `blocks_api::BlockIterator::new`
    pub fn listen_for_blocks(
        &self,
        height: NonZeroU64,
    ) -> Result<impl Iterator<Item = Result<SignedBlock>>> {
        blocks_api::BlockIterator::new(self.blocks_handler(height)?)
    }

    /// Connect asynchronously (through `WebSocket`) to listen for `Iroha` blocks
    ///
    /// # Errors
    /// - Forwards from [`Self::events_handler`]
    /// - Forwards from `blocks_api::BlockIterator::new`
    pub async fn listen_for_blocks_async(&self, height: NonZeroU64) -> Result<AsyncBlockStream> {
        blocks_api::AsyncBlockStream::new(self.blocks_handler(height)?).await
    }

    /// Construct a handler for Blocks API. With this handler you can use any WS client you want.
    ///
    /// # Errors
    /// - if handler construction fails
    #[inline]
    pub fn blocks_handler(&self, height: NonZeroU64) -> Result<blocks_api::flow::Init> {
        blocks_api::flow::Init::new(
            height,
            self.headers.clone(),
            join_torii_url(&self.torii_url, torii_uri::BLOCKS_STREAM),
        )
    }

    /// Get value of config on peer
    ///
    /// # Errors
    /// Fails if sending request or decoding fails
    /// Fetch node configuration via `/v1/config`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, response is non-OK, or decoding fails.
    pub fn get_config(&self) -> Result<ConfigGetDTO> {
        let resp = self
            .default_request(
                HttpMethod::GET,
                join_torii_url(&self.torii_url, torii_uri::CONFIGURATION),
            )
            .header(http::header::CONTENT_TYPE, APPLICATION_JSON)
            .build()?
            .send()?;

        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get configuration with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or(""),
            ));
        }
        let s = std::str::from_utf8(resp.body()).wrap_err("Invalid UTF-8")?;
        norito::json::from_json_fast_smart::<ConfigGetDTO>(s).map_err(|e| eyre!("{e}"))
    }

    /// Convenience helper returning only the confidential gas schedule.
    ///
    /// # Errors
    /// Returns an error if fetching the configuration fails or the payload cannot be decoded.
    pub fn get_confidential_gas_schedule(&self) -> Result<ConfidentialGasDTO> {
        self.get_config().map(|cfg| cfg.confidential_gas)
    }

    /// Send a request to change the configuration of a specified field.
    ///
    /// # Errors
    /// If sending request or decoding fails
    /// Update node configuration via `/v1/config`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or response is non-OK.
    pub fn set_config(&self, dto: &ConfigUpdateDTO) -> Result<()> {
        // Prefer Norito's fast JSON writer when available
        let body = norito::json::to_json(dto)
            .map(std::string::String::into_bytes)
            .wrap_err(format!("Failed to serialize {dto:?}"))?;
        let url = join_torii_url(&self.torii_url, torii_uri::CONFIGURATION);
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header(http::header::CONTENT_TYPE, APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;

        if resp.status() != StatusCode::ACCEPTED {
            return Err(eyre!(
                "Failed to post configuration with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or(""),
            ));
        }

        Ok(())
    }

    /// Gets network status seen from the peer
    ///
    /// Tries Norito first (preferred, compact and stable), then gracefully falls back
    /// to JSON if the server responds with a JSON payload.
    ///
    /// # Errors
    /// Fails if sending request or decoding fails for both Norito and JSON formats.
    /// Fetch node `/status`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, response is non-OK, or decoding fails.
    pub fn get_status(&self) -> Result<Status> {
        let mut builder = DefaultRequestBuilder::new(
            HttpMethod::GET,
            join_torii_url(&self.torii_url, torii_uri::STATUS),
        )
        .headers(self.headers.clone());
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        let resp =
            self.send_builder(builder.header(http::header::ACCEPT, "application/x-norito"))?;
        match decode_status_response(&resp) {
            Ok(status) => Ok(status),
            Err(first_err) => {
                let json_resp = self.send_builder(
                    self.default_request(
                        HttpMethod::GET,
                        join_torii_url(&self.torii_url, torii_uri::STATUS),
                    )
                    .header(http::header::ACCEPT, APPLICATION_JSON),
                )?;
                match decode_status_response(&json_resp) {
                    Ok(status) => Ok(status),
                    Err(json_err) => Err(eyre!(
                        "Norito decode failed: {first}; JSON fallback failed: {second}",
                        first = first_err,
                        second = json_err
                    )),
                }
            }
        }
    }

    #[cfg(test)]
    fn decode_status_for_test(resp: &Response<Vec<u8>>) -> Result<Status> {
        decode_status_response(resp)
    }

    /// Prepares http-request to implement [`Self::get_status`] on your own.
    ///
    /// # Errors
    /// Fails if request build fails
    pub fn prepare_status_request<B: RequestBuilder>(&self) -> B {
        B::new(
            HttpMethod::GET,
            join_torii_url(&self.torii_url, torii_uri::STATUS),
        )
        .headers(self.headers.clone())
    }

    /// Fetch the active Torii API version (block header version string).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, response is non-OK, or decoding fails.
    ///
    pub fn get_server_version(&self) -> Result<String> {
        let resp = self
            .default_request(
                HttpMethod::GET,
                join_torii_url(&self.torii_url, torii_uri::API_VERSION),
            )
            .header(http::header::ACCEPT, "text/plain")
            .build()?
            .send()?;

        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get server version with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or(""),
            ));
        }
        let body = std::str::from_utf8(resp.body())
            .wrap_err("Server version response was not valid UTF-8")?
            .trim();
        if body.is_empty() {
            return Err(eyre!("Server version response was empty"));
        }
        Ok(body.to_string())
    }

    /// Convenience: fetch recent shielded roots as JSON from the app API `/v1/zk/roots` endpoint.
    /// This is an operator/testing helper and not consensus‑critical.
    ///
    /// # Errors
    /// - Network or HTTP build errors
    /// - Non‑200 response
    /// - JSON decode failure
    pub fn get_zk_roots_json(&self, asset_id: &str, max: u32) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/roots");
        let body = norito::json::to_vec(&norito::json!({
            "asset_id": asset_id,
            "max": max,
        }))?;
        let resp = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", "application/json")
                .body(body),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get zk roots with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Fetch current execution witness snapshot as JSON from `/v1/debug/witness`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_debug_witness_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/debug/witness");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get witness (json) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Fetch current execution witness snapshot as Norito-encoded bytes.
    /// Prefers Accept-driven `/v1/debug/witness` and falls back to `.bin`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or both endpoints return non-OK responses.
    pub fn get_debug_witness_norito(&self) -> Result<Vec<u8>> {
        // Try Accept-driven path first
        let url = join_torii_url(&self.torii_url, "v1/debug/witness");
        let resp = self.send_prepared_request(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", "application/x-norito")
                .build()?,
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get witness (norito) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(resp.into_body())
    }

    /// Convenience: POST a ZK verification request to `/v1/zk/verify` with a
    /// Norito-encoded `OpenVerifyEnvelope` in the body.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_verify_norito(&self, body: &[u8]) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/verify");
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", "application/x-norito")
            .body(body.to_vec())
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to verify (norito) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Convenience: POST a ZK verification request to `/v1/zk/verify` with a
    /// JSON DTO describing the proof and verifying key.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_verify_json(&self, value: &norito::json::Value) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/verify");
        let body = norito::json::to_vec(value)?;
        let resp = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", APPLICATION_JSON)
                .body(body),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to verify (json) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Convenience: POST `/v1/aliases/voprf/evaluate` with a hex-encoded blinded element.
    ///
    /// # Errors
    /// Returns an error if request construction, NORITO serialization, or the HTTP call fails.
    pub fn post_alias_voprf_hex(&self, blinded_hex: &str) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/aliases/voprf/evaluate");
        let body = norito::json::to_vec(&norito::json!({
            "blinded_element_hex": blinded_hex,
        }))?;
        self.default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()
    }

    /// Convenience: POST `/v1/aliases/resolve` with an alias string.
    ///
    /// # Errors
    /// Returns an error if request construction, NORITO serialization, or the HTTP call fails.
    pub fn post_alias_resolve(&self, alias: &str) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/aliases/resolve");
        let body = norito::json::to_vec(&norito::json!({ "alias": alias }))?;
        self.default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()
    }

    /// Convenience: POST `/v1/aliases/resolve_index` with an index payload.
    ///
    /// # Errors
    /// Returns an error if request construction, NORITO serialization, or the HTTP call fails.
    pub fn post_alias_resolve_index(&self, index: u64) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/aliases/resolve_index");
        let body = norito::json::to_vec(&norito::json!({ "index": index }))?;
        self.default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()
    }

    /// Convenience: GET `/v1/sorafs/pin` to list manifests in the pin registry.
    ///
    /// # Errors
    /// Returns an error if request construction or the HTTP call fails.
    pub fn get_sorafs_pin_registry(
        &self,
        filter: &SorafsPinListFilter<'_>,
    ) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/sorafs/pin");
        filter
            .apply(self.default_request(HttpMethod::GET, url))
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()
    }

    /// Convenience: GET `/v1/sorafs/pin/{digest}` to inspect a specific manifest record.
    ///
    /// # Errors
    /// Returns an error if request construction or the HTTP call fails.
    pub fn get_sorafs_pin_manifest(&self, digest_hex: &str) -> Result<Response<Vec<u8>>> {
        let path = format!("v1/sorafs/pin/{digest_hex}");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()?;
        self.enforce_alias_policy(&response)?;
        Ok(response)
    }

    /// Convenience: POST `/v1/sorafs/pin/register` to submit a manifest to the pin registry.
    ///
    /// The `manifest` must correspond to the payload referenced by `chunk_digest_sha3_256`.
    /// The method accepts an optional alias binding and successor pointer.
    ///
    /// # Errors
    /// Returns an error if request construction, manifest digest computation, NORITO serialization,
    /// or the HTTP call fails.
    pub fn post_sorafs_pin_register(
        &self,
        params: SorafsPinRegisterArgs<'_>,
    ) -> Result<norito::json::Value> {
        let SorafsPinRegisterArgs {
            authority,
            private_key,
            manifest,
            chunk_digest_sha3_256,
            submitted_epoch,
            alias,
            successor_of,
        } = params;
        let url = join_torii_url(&self.torii_url, "v1/sorafs/pin/register");
        let payload = Self::build_sorafs_pin_register_payload(
            authority,
            private_key,
            manifest,
            chunk_digest_sha3_256,
            submitted_epoch,
            alias,
            successor_of,
        )?;
        let body = norito::json::to_vec(&payload)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?;
        let resp = self.send_prepared_request(resp)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to register pin manifest: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Build a signed DA ingest request using the client's key pair.
    ///
    /// This mirrors `iroha da submit --no-submit`, returning the Norito payload so callers can
    /// persist it to disk or inspect the JSON before publishing.
    pub fn build_da_ingest_request(
        &self,
        payload: impl Into<Vec<u8>>,
        params: &DaIngestParams,
        metadata: ExtraMetadata,
        manifest_bytes: Option<Vec<u8>>,
    ) -> DaIngestRequest {
        build_da_request(
            payload.into(),
            params,
            metadata,
            &self.key_pair,
            manifest_bytes,
        )
    }

    /// Submit a DA blob to `/v1/da/ingest` and return the Torii receipt.
    ///
    /// This is equivalent to running `iroha da submit` with the provided parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if building the request fails or the HTTP submission is rejected.
    pub fn submit_da_blob(
        &self,
        payload: impl Into<Vec<u8>>,
        params: &DaIngestParams,
        metadata: ExtraMetadata,
        manifest_bytes: Option<Vec<u8>>,
    ) -> Result<DaIngestSubmitResult> {
        let request = self.build_da_ingest_request(payload, params, metadata, manifest_bytes);
        self.submit_prepared_da_request(&request)
    }

    /// Build and persist a DA ingest request without submitting it to Torii.
    ///
    /// This mirrors `iroha da submit --no-submit`, writing `da_request.{norito,json}` into
    /// `output_dir` so callers can inspect or publish the request later.
    ///
    /// # Errors
    ///
    /// Returns an error if the request artefacts cannot be encoded or written to disk.
    pub fn write_da_ingest_request(
        &self,
        payload: impl Into<Vec<u8>>,
        params: &DaIngestParams,
        metadata: ExtraMetadata,
        manifest_bytes: Option<Vec<u8>>,
        output_dir: impl AsRef<Path>,
    ) -> Result<DaIngestPersistedPaths> {
        let request = self.build_da_ingest_request(payload, params, metadata, manifest_bytes);
        Self::persist_da_ingest_artifacts(&request, None, None, output_dir)
    }

    /// Submit a DA ingest request and persist the CLI-compatible artefacts.
    ///
    /// Writes `da_request.{norito,json}` and, when present, `da_receipt.{norito,json}` plus
    /// `da_response_headers.json` into `output_dir`, mirroring `iroha da submit`.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP submission fails or any artefact cannot be written to disk.
    pub fn submit_da_blob_to_dir(
        &self,
        payload: impl Into<Vec<u8>>,
        params: &DaIngestParams,
        metadata: ExtraMetadata,
        manifest_bytes: Option<Vec<u8>>,
        output_dir: impl AsRef<Path>,
    ) -> Result<(DaIngestSubmitResult, DaIngestPersistedPaths)> {
        let request = self.build_da_ingest_request(payload, params, metadata, manifest_bytes);
        // Persist the request up front so the caller retains the Norito bytes even if submit fails.
        Self::persist_da_ingest_artifacts(&request, None, None, &output_dir)?;
        let result = self.submit_prepared_da_request(&request)?;
        let persisted = Self::persist_da_ingest_artifacts(
            &request,
            result.receipt.as_ref(),
            result.pdp_commitment.as_ref(),
            output_dir,
        )?;
        Ok((result, persisted))
    }

    fn submit_prepared_da_request(
        &self,
        request: &DaIngestRequest,
    ) -> Result<DaIngestSubmitResult> {
        let body =
            norito::json::to_vec(request).wrap_err("failed to encode DA ingest request JSON")?;
        let url = join_torii_url(&self.torii_url, "v1/da/ingest");
        let response = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?;
        let response = self.send_prepared_request(response)?;
        let status_code = response.status();
        if status_code != StatusCode::ACCEPTED && status_code != StatusCode::OK {
            return Err(
                ResponseReport::with_msg("failed to submit DA ingest request", &response)
                    .unwrap_or_else(core::convert::identity)
                    .into(),
            );
        }
        let (payload, receipt_json, pdp_commitment) = Self::decode_da_ingest_response(
            response.body(),
            response.headers().get(PDP_COMMITMENT_HEADER),
        )?;
        Ok(DaIngestSubmitResult {
            status: payload.status,
            duplicate: payload.duplicate,
            receipt: payload.receipt,
            receipt_json,
            pdp_commitment,
            client_blob_id: request.client_blob_id,
            payload_len: request.payload.len(),
        })
    }

    /// Decode a JSON DA ingest response payload plus optional PDP commitment header.
    fn decode_da_ingest_response(
        response_bytes: &[u8],
        header: Option<&http::header::HeaderValue>,
    ) -> Result<(
        DaIngestResponsePayload,
        Option<String>,
        Option<PdpCommitmentV1>,
    )> {
        let payload: DaIngestResponsePayload = norito::json::from_slice(response_bytes)
            .wrap_err("failed to decode DA ingest response")?;
        let receipt_json = match payload.receipt.as_ref() {
            Some(receipt) => Some(
                norito::json::to_json_pretty(receipt)
                    .wrap_err("failed to render DA ingest receipt JSON")?,
            ),
            None => None,
        };
        let pdp_commitment = match header {
            Some(value) => {
                let raw = value
                    .to_str()
                    .wrap_err("sora-pdp-commitment header must contain ASCII")?;
                Some(decode_pdp_commitment_header(raw)?)
            }
            None => None,
        };
        Ok((payload, receipt_json, pdp_commitment))
    }

    fn persist_da_ingest_artifacts(
        request: &DaIngestRequest,
        receipt: Option<&DaIngestReceipt>,
        pdp_commitment: Option<&PdpCommitmentV1>,
        output_dir: impl AsRef<Path>,
    ) -> Result<DaIngestPersistedPaths> {
        let output_dir = output_dir.as_ref();
        if output_dir.as_os_str().is_empty() {
            return Err(eyre!("DA ingest artifact directory must not be empty"));
        }
        std::fs::create_dir_all(output_dir).wrap_err_with(|| {
            format!(
                "failed to create DA ingest artifact directory `{}`",
                output_dir.display()
            )
        })?;

        let request_raw = output_dir.join("da_request.norito");
        let request_json = output_dir.join("da_request.json");
        let request_bytes =
            to_bytes(request).wrap_err("failed to encode DA ingest request as Norito")?;
        let request_json_body = norito::json::to_json_pretty(request)
            .map_err(|err| eyre!("failed to render DA ingest request JSON: {err}"))?;
        std::fs::write(&request_raw, &request_bytes)
            .wrap_err_with(|| format!("failed to write `{}`", request_raw.display()))?;
        std::fs::write(&request_json, format!("{request_json_body}\n"))
            .wrap_err_with(|| format!("failed to write `{}`", request_json.display()))?;

        let (receipt_raw, receipt_json, response_headers_json) = receipt
            .map(|receipt| -> Result<(_, _, _)> {
                let raw_path = output_dir.join("da_receipt.norito");
                let json_path = output_dir.join("da_receipt.json");
                let receipt_bytes =
                    to_bytes(receipt).wrap_err("failed to encode DA receipt as Norito")?;
                let receipt_json_body = norito::json::to_json_pretty(receipt)
                    .map_err(|err| eyre!("failed to render DA receipt JSON: {err}"))?;
                std::fs::write(&raw_path, &receipt_bytes)
                    .wrap_err_with(|| format!("failed to write `{}`", raw_path.display()))?;
                std::fs::write(&json_path, format!("{receipt_json_body}\n"))
                    .wrap_err_with(|| format!("failed to write `{}`", json_path.display()))?;

                let headers_path = pdp_commitment
                    .map(|commitment| -> Result<_> {
                        let mut headers = JsonMap::new();
                        let encoded = to_bytes(commitment)
                            .wrap_err("failed to encode PDP commitment for header JSON")?;
                        headers.insert(
                            PDP_COMMITMENT_HEADER.into(),
                            JsonValue::from(
                                base64::engine::general_purpose::STANDARD.encode(encoded),
                            ),
                        );
                        let rendered = norito::json::to_json_pretty(&JsonValue::Object(headers))
                            .map_err(|err| {
                                eyre!("failed to render DA response header JSON: {err}")
                            })?;
                        let path = output_dir.join("da_response_headers.json");
                        std::fs::write(&path, format!("{rendered}\n"))
                            .wrap_err_with(|| format!("failed to write `{}`", path.display()))?;
                        Ok(path)
                    })
                    .transpose()?;

                Ok((Some(raw_path), Some(json_path), headers_path))
            })
            .transpose()?
            .unwrap_or((None, None, None));

        Ok(DaIngestPersistedPaths {
            request_raw,
            request_json,
            receipt_raw,
            receipt_json,
            response_headers_json,
        })
    }

    fn build_da_proof_summary_from_session(
        bundle: &DaManifestBundle,
        session: &SorafsFetchOutcome,
        proof: &DaProofConfig,
    ) -> Result<(JsonValue, DaProofConfig)> {
        let payload = session.outcome.assemble_payload();
        let manifest = bundle.decode_manifest()?;
        let applied = Self::apply_sampling_plan(bundle, &manifest, proof);
        let summary = generate_da_proof_summary(&manifest, &payload, &applied)?;
        Ok((summary, applied))
    }

    /// Fetch the canonical DA manifest + chunk-plan bundle for a storage ticket.
    ///
    /// This is equivalent to running `iroha da get-blob --storage-ticket=...` and returns the
    /// Norito manifest bytes, rendered JSON, and chunk plan emitted by Torii.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage ticket is malformed, the HTTP request fails, or the response
    /// payload cannot be decoded.
    pub fn get_da_manifest_bundle(&self, storage_ticket_hex: &str) -> Result<DaManifestBundle> {
        self.get_da_manifest_bundle_with_block_hash(storage_ticket_hex, None)
    }

    /// Fetch the canonical DA manifest bundle and apply a deterministic sampling seed.
    ///
    /// When `block_hash_hex` is provided the request appends `?block_hash=<hash>` so Torii returns a
    /// deterministic sampling plan rooted in `block_hash || client_blob_id`. Callers that do not need
    /// the sampling plan can use [`Self::get_da_manifest_bundle`] for the default behaviour.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage ticket or block hash are malformed, the HTTP request fails,
    /// or the response payload cannot be decoded.
    pub fn get_da_manifest_bundle_with_block_hash(
        &self,
        storage_ticket_hex: &str,
        block_hash_hex: Option<&str>,
    ) -> Result<DaManifestBundle> {
        let normalized = normalize_storage_ticket_hex(storage_ticket_hex)?;
        let query = if let Some(block_hash) = block_hash_hex {
            let normalized_hash = normalize_block_hash_hex(block_hash)?;
            format!("?block_hash={normalized_hash}")
        } else {
            String::new()
        };
        let path = format!("v1/da/manifests/{normalized}{query}");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()?;
        if response.status() != StatusCode::OK {
            return Err(
                ResponseReport::with_msg("failed to fetch DA manifest bundle", &response)
                    .unwrap_or_else(core::convert::identity)
                    .into(),
            );
        }
        let value: JsonValue = norito::json::from_slice(response.body())
            .wrap_err("failed to parse DA manifest response")?;
        DaManifestBundle::from_json(&value)
    }

    /// Fetch a DA manifest bundle and persist the artefacts to `output_dir`.
    ///
    /// This mirrors the behaviour of `iroha da get-blob`, emitting the Norito
    /// manifest bytes plus JSON copies so downstream tooling can reuse the
    /// stored artefacts without invoking the CLI binary.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest bundle cannot be fetched or the artefacts fail to persist.
    pub fn fetch_da_manifest_to_dir(
        &self,
        storage_ticket_hex: &str,
        output_dir: impl AsRef<Path>,
    ) -> Result<DaManifestPersistedPaths> {
        let bundle = self.get_da_manifest_bundle(storage_ticket_hex)?;
        let label = bundle.storage_ticket_hex.clone();
        bundle.persist_to_dir(output_dir, &label)
    }

    /// Build a CAR plan for the provided DA manifest bundle.
    ///
    /// The returned plan can be fed directly into [`Self::sorafs_fetch_via_gateway`] to reproduce
    /// the `iroha da get` behaviour programmatically.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest fails to decode or chunk metadata is invalid.
    pub fn build_da_car_plan(bundle: &DaManifestBundle) -> Result<CarBuildPlan> {
        let manifest = bundle.decode_manifest()?;
        build_car_plan_from_manifest(&manifest)
    }

    fn apply_sampling_plan(
        bundle: &DaManifestBundle,
        manifest: &DaManifestV1,
        config: &DaProofConfig,
    ) -> DaProofConfig {
        let default_sample_count = DaProofConfig::default().sample_count;
        if config.leaf_indexes.is_empty()
            && config.sample_count == default_sample_count
            && let Some(plan) = &bundle.sampling_plan
        {
            let mut leaf_indexes: Vec<usize> = plan
                .samples
                .iter()
                .filter_map(|sample| usize::try_from(sample.index).ok())
                .filter(|idx| *idx < manifest.chunks.len())
                .collect();
            leaf_indexes.sort_unstable();
            leaf_indexes.dedup();
            if !leaf_indexes.is_empty() {
                return DaProofConfig {
                    sample_count: 0,
                    sample_seed: config.sample_seed,
                    leaf_indexes,
                };
            }
        }
        config.clone()
    }

    /// Generate a `PoR` summary for the provided payload using the supplied manifest bundle.
    ///
    /// This mirrors the `iroha da prove --json-out` output so SDK consumers can attach the same
    /// artefacts without shelling out to the CLI.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be decoded, the payload does not match the manifest,
    /// or `PoR` proofs cannot be constructed.
    pub fn prove_da_payload(
        bundle: &DaManifestBundle,
        payload: &[u8],
        proof: &DaProofConfig,
    ) -> Result<JsonValue> {
        let manifest = bundle.decode_manifest()?;
        let applied = Self::apply_sampling_plan(bundle, &manifest, proof);
        generate_da_proof_summary(&manifest, payload, &applied)
    }

    /// Build a CLI-compatible DA proof artefact with manifest/payload annotations.
    ///
    /// This mirrors the JSON that `iroha da prove --json-out` emits, letting SDKs attach
    /// the same provenance metadata without invoking the CLI.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest fails to decode or proof derivation is unsuccessful.
    pub fn build_da_proof_artifact(
        bundle: &DaManifestBundle,
        payload: &[u8],
        proof: &DaProofConfig,
        metadata: &DaProofArtifactMetadata,
    ) -> Result<JsonValue> {
        let manifest = bundle.decode_manifest()?;
        let applied = Self::apply_sampling_plan(bundle, &manifest, proof);
        generate_da_proof_artifact(&manifest, payload, &applied, metadata)
    }

    /// Persist a DA proof artefact to disk (defaults to pretty JSON + newline).
    ///
    /// Returns the rendered artefact so callers can attach it to additional outputs without
    /// re-reading the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the artefact cannot be rendered or written to disk.
    pub fn write_da_proof_artifact(
        bundle: &DaManifestBundle,
        payload: &[u8],
        proof: &DaProofConfig,
        metadata: &DaProofArtifactMetadata,
        output_path: impl AsRef<Path>,
        pretty: bool,
    ) -> Result<JsonValue> {
        let artifact = Self::build_da_proof_artifact(bundle, payload, proof, metadata)?;
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create DA proof artefact directory `{}`",
                    parent.display()
                )
            })?;
        }
        let rendered = if pretty {
            norito::json::to_json_pretty(&artifact)
        } else {
            norito::json::to_json(&artifact)
        }
        .map_err(|err| eyre!("failed to render DA proof artefact JSON: {err}"))?;
        std::fs::write(output_path, format!("{rendered}\n")).wrap_err_with(|| {
            format!(
                "failed to write DA proof artefact to `{}`",
                output_path.display()
            )
        })?;
        Ok(artifact)
    }

    /// Reproduce the `iroha da prove-availability` flow with the Rust client.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be fetched, the bundle reconciliation fails,
    /// or proof generation encounters an error.
    pub async fn prove_da_availability(
        &self,
        storage_ticket_hex: &str,
        gateway_config: SorafsGatewayFetchConfig,
        providers: impl IntoIterator<Item = SorafsGatewayProviderInput>,
        fetch_options: SorafsGatewayFetchOptions,
        proof_config: DaProofConfig,
    ) -> Result<DaAvailabilityProof> {
        let manifest = self.get_da_manifest_bundle(storage_ticket_hex)?;
        let plan = Self::build_da_car_plan(&manifest)?;
        let session = self
            .sorafs_fetch_via_gateway(&plan, gateway_config, providers, fetch_options)
            .await?;
        let (proof_summary, applied_config) =
            Self::build_da_proof_summary_from_session(&manifest, &session, &proof_config)?;
        Ok(DaAvailabilityProof {
            manifest,
            fetch_session: session,
            proof_summary,
            proof_config: applied_config,
        })
    }

    /// Reproduce `iroha da prove-availability`, persisting artefacts to `output_dir`.
    ///
    /// Writes:
    /// - Manifest bundle (`manifest_<ticket>.norito/json`, `chunk_plan_<ticket>.json`)
    /// - Assembled payload (`payload_<ticket>.car`)
    /// - Proof summary JSON (`proof_summary_<ticket>.json`, matching CLI format)
    ///
    /// # Errors
    ///
    /// Returns an error if any network call fails, output can’t be written, or proof generation
    /// encounters an error.
    pub async fn prove_da_availability_to_dir(
        &self,
        storage_ticket_hex: &str,
        gateway_config: SorafsGatewayFetchConfig,
        providers: impl IntoIterator<Item = SorafsGatewayProviderInput>,
        fetch_options: SorafsGatewayFetchOptions,
        proof_config: DaProofConfig,
        output_dir: impl AsRef<Path>,
    ) -> Result<(DaAvailabilityProof, DaAvailabilityProofPersistedPaths)> {
        let output_dir = output_dir.as_ref();
        if output_dir.as_os_str().is_empty() {
            return Err(eyre!("output directory must not be empty"));
        }
        std::fs::create_dir_all(output_dir).wrap_err_with(|| {
            format!(
                "failed to create DA proof output directory `{}`",
                output_dir.display()
            )
        })?;

        let mut fetch_options = fetch_options;
        let scoreboard_path = {
            let configured = fetch_options
                .scoreboard
                .as_ref()
                .and_then(|options| options.persist_path.clone());
            let derived = configured.unwrap_or_else(|| output_dir.join("scoreboard.json"));
            if fetch_options.scoreboard.is_none() {
                fetch_options.scoreboard = Some(SorafsGatewayScoreboardOptions {
                    persist_path: Some(derived.clone()),
                    ..SorafsGatewayScoreboardOptions::default()
                });
            } else if let Some(options) = fetch_options.scoreboard.as_mut()
                && options.persist_path.is_none()
            {
                options.persist_path = Some(derived.clone());
            }
            Some(derived)
        };

        let proof = self
            .prove_da_availability(
                storage_ticket_hex,
                gateway_config,
                providers,
                fetch_options,
                proof_config,
            )
            .await?;

        let manifest_paths = proof
            .manifest
            .persist_to_dir(output_dir, &proof.manifest.storage_ticket_hex)?;
        let manifest_label = manifest_paths
            .manifest_raw
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.strip_prefix("manifest_"))
            .ok_or_else(|| {
                eyre!(
                    "persisted manifest path `{}` missing expected prefix",
                    manifest_paths.manifest_raw.display()
                )
            })?;

        let payload_path = output_dir.join(format!("payload_{manifest_label}.car"));
        let payload_bytes = proof.fetch_session.outcome.assemble_payload();
        std::fs::write(&payload_path, &payload_bytes).wrap_err_with(|| {
            format!(
                "failed to write fetched payload to `{}`",
                payload_path.display()
            )
        })?;

        let proof_summary_path = output_dir.join(format!("proof_summary_{manifest_label}.json"));
        let metadata = DaProofArtifactMetadata::new(
            manifest_paths.manifest_raw.display().to_string(),
            payload_path.display().to_string(),
        );
        let proof_summary = Self::build_da_proof_artifact(
            &proof.manifest,
            &payload_bytes,
            &proof.proof_config,
            &metadata,
        )?;
        let rendered_summary = norito::json::to_json_pretty(&proof_summary)
            .wrap_err("failed to render proof summary JSON")?;
        std::fs::write(&proof_summary_path, format!("{rendered_summary}\n")).wrap_err_with(
            || {
                format!(
                    "failed to write proof summary to `{}`",
                    proof_summary_path.display()
                )
            },
        )?;

        let paths = DaAvailabilityProofPersistedPaths {
            manifest: manifest_paths,
            payload_path,
            proof_summary_path,
            scoreboard_path,
        };
        Ok((proof, paths))
    }

    fn build_sorafs_pin_register_payload(
        authority: &iroha_data_model::account::AccountId,
        private_key: &iroha_crypto::PrivateKey,
        manifest: &sorafs_manifest::ManifestV1,
        chunk_digest_sha3_256: [u8; 32],
        submitted_epoch: u64,
        alias: Option<SorafsPinAlias<'_>>,
        successor_of: Option<[u8; 32]>,
    ) -> Result<norito::json::Value> {
        let manifest_digest = manifest
            .digest()
            .wrap_err("failed to compute manifest digest for pin registration")?;
        let chunker = &manifest.chunking;
        let policy = &manifest.pin_policy;
        let storage_class_label = match policy.storage_class {
            sorafs_manifest::StorageClass::Hot => "Hot",
            sorafs_manifest::StorageClass::Warm => "Warm",
            sorafs_manifest::StorageClass::Cold => "Cold",
        };

        let mut storage_class_map = norito::json::Map::new();
        storage_class_map.insert(
            "type".into(),
            norito::json::Value::from(storage_class_label),
        );
        let mut pin_policy_map = norito::json::Map::new();
        pin_policy_map.insert(
            "min_replicas".into(),
            norito::json::Value::from(u64::from(policy.min_replicas)),
        );
        pin_policy_map.insert(
            "storage_class".into(),
            norito::json::Value::from(storage_class_map),
        );
        pin_policy_map.insert(
            "retention_epoch".into(),
            norito::json::Value::from(policy.retention_epoch),
        );

        let mut map = norito::json::Map::new();
        map.insert(
            "authority".into(),
            norito::json::Value::from(authority.to_string()),
        );
        map.insert(
            "private_key".into(),
            norito::json::to_value(&iroha_data_model::prelude::ExposedPrivateKey(
                private_key.clone(),
            ))?,
        );
        map.insert(
            "chunker_profile_id".into(),
            norito::json::Value::from(u64::from(chunker.profile_id.0)),
        );
        map.insert(
            "chunker_namespace".into(),
            norito::json::Value::from(chunker.namespace.as_str()),
        );
        map.insert(
            "chunker_name".into(),
            norito::json::Value::from(chunker.name.as_str()),
        );
        map.insert(
            "chunker_semver".into(),
            norito::json::Value::from(chunker.semver.as_str()),
        );
        map.insert(
            "chunker_multihash_code".into(),
            norito::json::Value::from(chunker.multihash_code),
        );
        map.insert(
            "pin_policy".into(),
            norito::json::Value::from(pin_policy_map),
        );
        map.insert(
            "manifest_digest_hex".into(),
            norito::json::Value::from(hex::encode(manifest_digest.as_bytes())),
        );
        map.insert(
            "chunk_digest_sha3_256_hex".into(),
            norito::json::Value::from(hex::encode(chunk_digest_sha3_256)),
        );
        map.insert(
            "submitted_epoch".into(),
            norito::json::Value::from(submitted_epoch),
        );

        if let Some(alias) = alias {
            let mut alias_map = norito::json::Map::new();
            alias_map.insert(
                "namespace".into(),
                norito::json::Value::from(alias.namespace),
            );
            alias_map.insert("name".into(), norito::json::Value::from(alias.name));
            alias_map.insert(
                "proof_base64".into(),
                norito::json::Value::from(
                    base64::engine::general_purpose::STANDARD.encode(alias.proof),
                ),
            );
            map.insert("alias".into(), norito::json::Value::from(alias_map));
        }
        if let Some(successor) = successor_of {
            map.insert(
                "successor_of_hex".into(),
                norito::json::Value::from(hex::encode(successor)),
            );
        }

        Ok(norito::json::Value::from(map))
    }

    /// Convenience: GET `/v1/sorafs/aliases` to list manifest alias bindings.
    ///
    /// # Errors
    /// Returns an error if request construction or the HTTP call fails.
    pub fn get_sorafs_aliases(
        &self,
        filter: &SorafsAliasListFilter<'_>,
    ) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/sorafs/aliases");
        filter
            .apply(self.default_request(HttpMethod::GET, url))
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()
    }

    /// Convenience: GET `/v1/sorafs/replication` to list replication orders.
    ///
    /// # Errors
    /// Returns an error if request construction or the HTTP call fails.
    pub fn get_sorafs_replication_orders(
        &self,
        filter: &SorafsReplicationListFilter<'_>,
    ) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/sorafs/replication");
        filter
            .apply(self.default_request(HttpMethod::GET, url))
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()
    }

    /// Convenience: POST `/v1/sorafs/storage/pin` with manifest and payload bytes.
    ///
    /// # Errors
    /// Returns an error if request construction, serialization, or the HTTP call fails.
    pub fn post_sorafs_storage_pin(
        &self,
        manifest_bytes: &[u8],
        payload_bytes: &[u8],
    ) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/sorafs/storage/pin");
        let manifest_b64 = base64::engine::general_purpose::STANDARD.encode(manifest_bytes);
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload_bytes);
        let body = norito::json::to_vec(&norito::json!({
            "manifest_b64": manifest_b64,
            "payload_b64": payload_b64,
        }))?;
        self.default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()
    }

    /// Convenience: POST `/v1/sorafs/storage/token` to mint a stream token.
    ///
    /// # Errors
    /// Returns an error if request construction or the HTTP call fails.
    pub fn post_sorafs_storage_token(
        &self,
        manifest_id_hex: &str,
        provider_id_hex: &str,
        client_id: &str,
        nonce: &str,
        overrides: &SorafsTokenOverrides,
    ) -> Result<Response<Vec<u8>>> {
        let url = join_torii_url(&self.torii_url, "v1/sorafs/storage/token");
        let mut map = norito::json::Map::new();
        map.insert(
            "manifest_id_hex".into(),
            norito::json::Value::from(manifest_id_hex),
        );
        map.insert(
            "provider_id_hex".into(),
            norito::json::Value::from(provider_id_hex),
        );
        map.insert(
            "ttl_secs".into(),
            overrides
                .ttl_secs
                .map_or(norito::json::Value::Null, norito::json::Value::from),
        );
        map.insert(
            "max_streams".into(),
            overrides
                .max_streams
                .map_or(norito::json::Value::Null, |value| {
                    norito::json::Value::from(u64::from(value))
                }),
        );
        map.insert(
            "rate_limit_bytes".into(),
            overrides
                .rate_limit_bytes
                .map_or(norito::json::Value::Null, norito::json::Value::from),
        );
        map.insert(
            "requests_per_minute".into(),
            overrides
                .requests_per_minute
                .map_or(norito::json::Value::Null, norito::json::Value::from),
        );
        let body = norito::json::to_vec(&norito::json::Value::Object(map))?;
        self.default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header(HEADER_SORA_CLIENT, client_id)
            .header(HEADER_SORA_NONCE, nonce)
            .body(body)
            .build()?
            .send()
    }

    /// Execute a multi-provider fetch via the `SoraFS` orchestrator using gateway stream tokens.
    ///
    /// The `options` parameter exposes retry-budget and provider selection controls used when
    /// constructing the underlying orchestrator configuration.
    ///
    /// # Errors
    /// Returns [`SorafsFetchError`] when provider descriptors are invalid or the orchestrator
    /// encounters an error during the fetch loop.
    pub async fn sorafs_fetch_via_gateway(
        &self,
        plan: &CarBuildPlan,
        gateway_config: SorafsGatewayFetchConfig,
        providers: impl IntoIterator<Item = SorafsGatewayProviderInput>,
        options: SorafsGatewayFetchOptions,
    ) -> Result<SorafsFetchOutcome, SorafsFetchError> {
        let provider_inputs: Vec<SorafsGatewayProviderInput> = providers.into_iter().collect();
        #[cfg(test)]
        if let Some(hook) = evidence_http_tests::sorafs_fetch_hook() {
            return hook(plan, &gateway_config, &provider_inputs, &options);
        }
        let (mut orchestrator_config, max_peers) = self.build_sorafs_gateway_fetch_config(&options);
        if let Some(metadata) = orchestrator_config.scoreboard.persist_metadata.as_mut() {
            annotate_scoreboard_with_gateway_context(metadata, &gateway_config);
        }
        orchestrator_fetch_via_gateway(
            orchestrator_config,
            plan,
            gateway_config,
            provider_inputs,
            None,
            max_peers,
        )
        .await
        .map_err(SorafsFetchError::from)
    }

    /// Convenience: POST a ZK submit-proof request to `/v1/zk/submit-proof` with a
    /// Norito-encoded envelope in the body. Returns JSON response with `{ ok, id }`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_submit_proof_norito(&self, body: &[u8]) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/submit-proof");
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", "application/x-norito")
            .body(body.to_vec())
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to submit-proof (norito) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Convenience: POST a ZK submit-proof request to `/v1/zk/submit-proof` with a
    /// JSON DTO describing the proof and verifying key. Returns JSON response with `{ ok, id }`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_submit_proof_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/submit-proof");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?;
        let resp = self.send_prepared_request(resp)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to submit-proof (json) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Convenience: POST a ZK verify-batch request to `/v1/zk/verify-batch` with a
    /// Norito-encoded `Vec<OpenVerifyEnvelope>` in the body. Returns JSON `{ ok, statuses }`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_verify_batch_norito(&self, body: &[u8]) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/verify-batch");
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", "application/x-norito")
            .body(body.to_vec())
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to verify-batch (norito) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Convenience: POST a ZK verify-batch request to `/v1/zk/verify-batch` with a
    /// JSON array of base64-encoded Norito `OpenVerifyEnvelope` items. Returns JSON `{ ok, statuses }`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_verify_batch_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/verify-batch");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to verify-batch (json) with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Convenience: POST `/v1/zk/vote/tally` with a JSON DTO body `{ election_id }`.
    /// Returns JSON `{ finalized: bool, tally: [u64; N] }`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_zk_vote_tally_json(&self, election_id: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/vote/tally");
        let body = norito::json::to_vec(&norito::json!({
            "election_id": election_id,
        }))?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get vote tally with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Upload an attachment to the app API `/v1/zk/attachments`.
    /// Returns JSON metadata `{ id, size, content_type, created_ms }`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is not `201 Created`, or response JSON deserialization fails.
    pub fn post_zk_attachment(
        &self,
        body: &[u8],
        content_type: &str,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/attachments");
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", content_type)
            .body(body.to_vec())
            .build()?
            .send()?;
        if resp.status() != StatusCode::CREATED {
            return Err(eyre!(
                "Failed to upload attachment with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/proposals/deploy-contract` with a JSON DTO body.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_propose_deploy_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/proposals/deploy-contract");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to propose-deploy: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/ballots/zk` with a JSON DTO body.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_ballot_zk_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/ballots/zk");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to submit zk ballot: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/ballots/plain` with a JSON DTO body.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_ballot_plain_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/ballots/plain");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to submit plain ballot: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/protected-namespaces` with a JSON body `{ namespaces: [..] }` to apply directly.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_protected_set_json(
        &self,
        namespaces: &[String],
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/protected-namespaces");
        let mut payload = norito::json::Map::new();
        let ns = namespaces.to_owned();
        payload.insert("namespaces".into(), norito::json::to_value(&ns)?);
        let body = norito::json::to_vec(&norito::json::Value::from(payload))?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to apply protected namespaces: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/finalize` with a JSON DTO body.
    /// Expected body shape: `{ referendum_id: String, proposal_id: Hex64 }`.
    /// Returns JSON with `{ ok, tx_instructions: [{ wire_id, payload_hex }] }`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_finalize_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/finalize");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to build finalize tx: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/enact` with a JSON DTO body.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_enact_json(&self, value: &norito::json::Value) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/enact");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to build enact tx: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/proposals/{id}`
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_proposal_json(&self, id_hex: &str) -> Result<norito::json::Value> {
        let path = format!("v1/gov/proposals/{id_hex}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get proposal: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/locks/{rid}`
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_locks_json(&self, rid: &str) -> Result<norito::json::Value> {
        let path = format!("v1/gov/locks/{rid}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get locks: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/council/current`
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_council_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/council/current");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get council: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/council/audit` (optional `epoch` query)
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_council_audit_json(&self, epoch: Option<u64>) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/council/audit");
        let mut req = self.default_request(HttpMethod::GET, url);
        if let Some(e) = epoch {
            req = req.param("epoch", &e);
        }
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get council audit: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/council/derive-vrf` with a JSON DTO body (feature: `gov_vrf` on server).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_council_derive_vrf_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/council/derive-vrf");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to derive council via VRF: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/council/persist` with a JSON DTO body (feature: `gov_vrf` on server).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_council_persist_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/council/persist");
        let body = norito::json::to_vec(value)?;
        let resp = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", APPLICATION_JSON)
                .body(body),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to persist council: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/gov/council/replace` with a JSON DTO body (feature: `gov_vrf` on server).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn post_gov_council_replace_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/council/replace");
        let body = norito::json::to_vec(value)?;
        let resp = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", APPLICATION_JSON)
                .body(body),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to replace council member: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/referenda/{id}`
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_referendum_json(&self, id: &str) -> Result<norito::json::Value> {
        let path = format!("v1/gov/referenda/{id}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get referendum: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/tally/{id}`
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_tally_json(&self, id: &str) -> Result<norito::json::Value> {
        let path = format!("v1/gov/tally/{id}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get tally: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/unlocks/stats` (operator/audit stats)
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_unlock_stats_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/unlocks/stats");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get unlock stats: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/protected-namespaces` (current protected namespaces list)
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_protected_namespaces_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/gov/protected-namespaces");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get protected namespaces: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/gov/instances/{ns}`
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_instances_by_ns_json(&self, ns: &str) -> Result<norito::json::Value> {
        self.get_gov_instances_by_ns_filtered_json(ns, None, None, None, None, None)
    }

    /// GET `/v1/gov/instances/{ns}` with optional filters and pagination.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_gov_instances_by_ns_filtered_json(
        &self,
        ns: &str,
        contains: Option<&str>,
        hash_prefix: Option<&str>,
        offset: Option<u32>,
        limit: Option<u32>,
        order: Option<&str>,
    ) -> Result<norito::json::Value> {
        let path = format!("v1/gov/instances/{ns}");
        let url = join_torii_url(&self.torii_url, &path);
        let mut req = self.default_request(HttpMethod::GET, url);
        if let Some(v) = contains {
            req = req.param("contains", &v);
        }
        if let Some(v) = hash_prefix {
            req = req.param("hash_prefix", &v);
        }
        if let Some(v) = offset {
            req = req.param("offset", &v);
        }
        if let Some(v) = limit {
            req = req.param("limit", &v);
        }
        if let Some(v) = order {
            req = req.param("order", &v);
        }
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get instances: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/contracts/instances/{ns}` with optional filters and pagination.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or response JSON deserialization fails.
    pub fn get_contracts_instances_by_ns_filtered_json(
        &self,
        ns: &str,
        contains: Option<&str>,
        hash_prefix: Option<&str>,
        offset: Option<u32>,
        limit: Option<u32>,
        order: Option<&str>,
    ) -> Result<norito::json::Value> {
        let path = format!("v1/contracts/instances/{ns}");
        let url = join_torii_url(&self.torii_url, &path);
        let mut req = self.default_request(HttpMethod::GET, url);
        if let Some(v) = contains {
            req = req.param("contains", &v);
        }
        if let Some(v) = hash_prefix {
            req = req.param("hash_prefix", &v);
        }
        if let Some(v) = offset {
            req = req.param("offset", &v);
        }
        if let Some(v) = limit {
            req = req.param("limit", &v);
        }
        if let Some(v) = order {
            req = req.param("order", &v);
        }
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get contracts instances: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/contracts/code-bytes/{code_hash}` and decode base64 into bytes
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or the response JSON lacks `code_b64`.
    pub fn get_contract_code_bytes(&self, code_hash_hex: &str) -> Result<Vec<u8>> {
        let path = format!("v1/contracts/code-bytes/{code_hash_hex}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get code bytes: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let v: norito::json::Value = norito::json::from_slice(resp.body())?;
        let s = v
            .get("code_b64")
            .and_then(|x| x.as_str())
            .ok_or_else(|| eyre!("missing code_b64"))?;
        Ok(base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .unwrap_or_default())
    }

    /// GET `/v1/contracts/code/{code_hash}` and return manifest JSON
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_contract_manifest_json(&self, code_hash_hex: &str) -> Result<norito::json::Value> {
        let path = format!("v1/contracts/code/{code_hash_hex}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get contract manifest: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/contracts/deploy` with JSON body `{ authority, private_key, code_b64 }`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn post_contract_deploy_json(
        &self,
        authority: &iroha_data_model::account::AccountId,
        private_key: &iroha_crypto::PrivateKey,
        code_b64: &str,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/contracts/deploy");
        let mut payload = norito::json::Map::new();
        payload.insert("authority".into(), authority.to_string().into());
        payload.insert(
            "private_key".into(),
            norito::json::to_value(&iroha_data_model::prelude::ExposedPrivateKey(
                private_key.clone(),
            ))?,
        );
        payload.insert("code_b64".into(), code_b64.into());
        let body = norito::json::to_vec(&norito::json::Value::from(payload))?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to deploy contract: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/runtime/abi/active`
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_runtime_abi_active_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/runtime/abi/active");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get runtime ABI active: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/runtime/abi/hash`
    /// Returns `{ policy: "V1", abi_hash_hex: "<64-hex>" }`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_runtime_abi_hash_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/runtime/abi/hash");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get runtime ABI hash: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/node/capabilities`
    /// Returns `{ supported_abi_versions: [..], default_compile_target: n }`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_node_capabilities_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/node/capabilities");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get node capabilities: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/runtime/metrics`
    /// Returns a JSON summary of runtime metrics (ABI count and upgrade events counters).
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_runtime_metrics_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/runtime/metrics");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get runtime metrics: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/runtime/upgrades`
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_runtime_upgrades_json(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/runtime/upgrades");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get runtime upgrades: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// GET `/v1/accounts/{uaid}/portfolio` — aggregated holdings for a UAID.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_uaid_portfolio(&self, uaid: &str) -> Result<UaidPortfolioResponse> {
        let canonical = canonicalize_uaid_literal(uaid, "get_uaid_portfolio.uaid")?;
        let path = format!("v1/accounts/{canonical}/portfolio");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_JSON),
        )?;
        let payload = Self::parse_json_ok_response(&resp, "uaid portfolio request")?;
        UaidPortfolioResponse::from_value(payload)
    }

    /// GET `/v1/space-directory/uaids/{uaid}` — dataspace bindings for a UAID.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_uaid_bindings(&self, uaid: &str) -> Result<UaidBindingsResponse> {
        self.get_uaid_bindings_with_query(uaid, None)
    }

    /// GET `/v1/space-directory/uaids/{uaid}` — dataspace bindings with query parameters.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_uaid_bindings_with_query(
        &self,
        uaid: &str,
        query: Option<UaidBindingsQuery>,
    ) -> Result<UaidBindingsResponse> {
        let canonical = canonicalize_uaid_literal(uaid, "get_uaid_bindings_with_query.uaid")?;
        let path = format!("v1/space-directory/uaids/{canonical}");
        let url = join_torii_url(&self.torii_url, &path);
        let builder = self
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON);
        let builder = if let Some(options) = query {
            options.apply(builder)
        } else {
            builder
        };
        let resp = self.send_builder(builder)?;
        let payload = Self::parse_json_ok_response(&resp, "uaid bindings request")?;
        UaidBindingsResponse::from_value(payload)
    }

    /// GET `/v1/space-directory/uaids/{uaid}/manifests` — capability manifests bound to a UAID.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_uaid_manifests(
        &self,
        uaid: &str,
        query: Option<UaidManifestQuery>,
    ) -> Result<UaidManifestsResponse> {
        let canonical = canonicalize_uaid_literal(uaid, "get_uaid_manifests.uaid")?;
        let path = format!("v1/space-directory/uaids/{canonical}/manifests");
        let url = join_torii_url(&self.torii_url, &path);
        let builder = self
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON);
        let builder = if let Some(options) = query {
            options.apply(builder)
        } else {
            builder
        };
        let resp = self.send_builder(builder)?;
        let payload = Self::parse_json_ok_response(&resp, "uaid manifests request")?;
        UaidManifestsResponse::from_value(payload)
    }

    /// GET `/v1/explorer/accounts/{account_id}/qr` — share-ready QR metadata.
    ///
    /// # Errors
    /// Returns an error if the account id fails validation, the HTTP request fails,
    /// the response is non-OK, or JSON deserialization fails.
    pub fn get_explorer_account_qr(
        &self,
        account_id: &str,
        options: Option<ExplorerAccountQrOptions>,
    ) -> Result<ExplorerAccountQrSnapshot> {
        let trimmed = account_id.trim();
        if trimmed.is_empty() {
            return Err(eyre!(
                "get_explorer_account_qr.account_id must not be empty or whitespace"
            ));
        }
        let path = format!("v1/explorer/accounts/{trimmed}/qr");
        let url = join_torii_url(&self.torii_url, &path);
        let builder = self
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON);
        let builder = options.unwrap_or_default().apply(builder);
        let resp = self.send_builder(builder)?;
        let payload = Self::parse_json_ok_response(&resp, "explorer account qr request")?;
        ExplorerAccountQrSnapshot::from_value(payload)
    }

    /// POST `/v1/runtime/upgrades/propose` with a JSON DTO body (manifest).
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn post_runtime_propose_upgrade_json(
        &self,
        value: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/runtime/upgrades/propose");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to propose runtime upgrade: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/runtime/upgrades/activate/:id` (id hex in path)
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn post_runtime_activate_upgrade_json(&self, id_hex: &str) -> Result<norito::json::Value> {
        let path = format!("v1/runtime/upgrades/activate/{id_hex}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", APPLICATION_JSON)
                .body(Vec::new()),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to activate runtime upgrade: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// POST `/v1/runtime/upgrades/cancel/:id` (id hex in path)
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn post_runtime_cancel_upgrade_json(&self, id_hex: &str) -> Result<norito::json::Value> {
        let path = format!("v1/runtime/upgrades/cancel/{id_hex}");
        let url = join_torii_url(&self.torii_url, &path);
        let resp = self.send_builder(
            self.default_request(HttpMethod::POST, url)
                .header("Content-Type", APPLICATION_JSON)
                .body(Vec::new()),
        )?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to cancel runtime upgrade: {} {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// List attachments via `/v1/zk/attachments`. Returns JSON array of metadata objects.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_attachments_list(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/attachments");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to list attachments with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// List proof records via `/v1/zk/proofs` with optional filters.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_proofs_list_filtered(
        &self,
        filter: &ZkProofsFilter,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/proofs");
        let mut req = self.default_request(HttpMethod::GET, url);
        req = filter.apply(req);
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to list proofs with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Count proof records via `/v1/zk/proofs/count` with optional filters.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or the JSON body is malformed.
    pub fn get_zk_proofs_count(&self, filter: &ZkProofsFilter) -> Result<u64> {
        let url = join_torii_url(&self.torii_url, "v1/zk/proofs/count");
        let mut req = self.default_request(HttpMethod::GET, url);
        req = filter.apply(req);
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to count proofs with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let v: norito::json::Value = norito::json::from_slice(resp.body())?;
        v.get("count")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("invalid count response"))
    }

    /// Fetch a single proof record via `/v1/zk/proof/{backend}/{hash}`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_proof_json(&self, backend: &str, hash_hex: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(
            &self.torii_url,
            &format!("v1/zk/proof/{backend}/{hash_hex}"),
        );
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to fetch proof with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Inspect proof retention configuration and live counters via `/v1/proofs/retention`.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_proof_retention_status(&self) -> Result<iroha_torii_shared::ProofRetentionStatus> {
        let url = join_torii_url(
            &self.torii_url,
            iroha_torii_shared::uri::PROOF_RETENTION_STATUS.trim_start_matches('/'),
        );
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to fetch proof retention status with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// List prover reports via `/v1/zk/prover/reports` with optional server-side filters.
    /// If a filter field is `None`, it is omitted.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_prover_reports_list_filtered(
        &self,
        filter: &ZkProverReportsFilter,
    ) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/prover/reports");
        let mut req = self.default_request(HttpMethod::GET, url);
        req = filter.apply(req);
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to list prover reports with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Get count of prover reports via `/v1/zk/prover/reports/count` with optional filters.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON parse fails.
    pub fn get_zk_prover_reports_count(&self, filter: &ZkProverReportsFilter) -> Result<u64> {
        let url = join_torii_url(&self.torii_url, "v1/zk/prover/reports/count");
        let mut req = self.default_request(HttpMethod::GET, url);
        req = filter.apply(req);
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get prover reports count with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let v: norito::json::Value = norito::json::from_slice(resp.body())?;
        v.get("count")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("invalid count response"))
    }

    /// Fetch attachment bytes by id via `/v1/zk/attachments/{id}`.
    /// Returns the raw bytes and optional content-type.
    /// # Errors
    /// Returns an error if the HTTP request fails or the response is non-OK.
    pub fn get_zk_attachment_raw(&self, id: &str) -> Result<(Vec<u8>, Option<String>)> {
        let url = join_torii_url(&self.torii_url, &format!("v1/zk/attachments/{id}"));
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to fetch attachment with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok().map(str::to_owned));
        Ok((resp.body().clone(), ct))
    }

    /// Delete attachment by id via `/v1/zk/attachments/{id}`.
    /// # Errors
    /// Returns an error if the HTTP request fails or the response is not `204 No Content`.
    pub fn delete_zk_attachment(&self, id: &str) -> Result<()> {
        let url = join_torii_url(&self.torii_url, &format!("v1/zk/attachments/{id}"));
        let resp = self.send_builder(self.default_request(HttpMethod::DELETE, url))?;
        if resp.status() != StatusCode::NO_CONTENT {
            return Err(eyre!(
                "Failed to delete attachment with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(())
    }

    /// List prover reports via `/v1/zk/prover/reports`. Returns a JSON array of reports.
    /// This is a non‑consensus, app‑facing helper.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_prover_reports_list(&self) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, "v1/zk/prover/reports");
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to list prover reports with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Fetch a single prover report JSON by id via `/v1/zk/prover/reports/{id}`.
    /// Returns a JSON object describing the report.
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_prover_report_json(&self, id: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, &format!("v1/zk/prover/reports/{id}"));
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get prover report with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }

    /// Delete a prover report by id via `/v1/zk/prover/reports/{id}`.
    /// # Errors
    /// Returns an error if the HTTP request fails or the response is not `204 No Content`.
    pub fn delete_zk_prover_report(&self, id: &str) -> Result<()> {
        let url = join_torii_url(&self.torii_url, &format!("v1/zk/prover/reports/{id}"));
        let resp = self.send_builder(self.default_request(HttpMethod::DELETE, url))?;
        if resp.status() != StatusCode::NO_CONTENT {
            return Err(eyre!(
                "Failed to delete prover report with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(())
    }

    /// Bulk delete prover reports via `/v1/zk/prover/reports` with filters.
    /// Returns the number of deleted reports.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON parse fails.
    pub fn delete_zk_prover_reports_filtered(&self, filter: &ZkProverReportsFilter) -> Result<u64> {
        let url = join_torii_url(&self.torii_url, "v1/zk/prover/reports");
        let mut req = self.default_request(HttpMethod::DELETE, url);
        req = filter.apply(req);
        let resp = self.send_builder(req)?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to bulk delete prover reports with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        let v: norito::json::Value = norito::json::from_slice(resp.body())?;
        v.get("deleted")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("invalid delete response"))
    }

    /// Convenience: POST `/v1/zk/vk/register` with a JSON DTO body.
    /// # Errors
    /// Returns an error if the HTTP request fails or the response is not `202 Accepted`.
    pub fn post_zk_vk_register(&self, value: &norito::json::Value) -> Result<()> {
        let url = join_torii_url(&self.torii_url, "v1/zk/vk/register");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?;
        let resp = self.send_prepared_request(resp)?;
        if resp.status() != StatusCode::ACCEPTED {
            return Err(eyre!(
                "Failed to register VK with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(())
    }

    /// Convenience: POST `/v1/zk/vk/update` with a JSON DTO body.
    /// # Errors
    /// Returns an error if the HTTP request fails or the response is not `202 Accepted`.
    pub fn post_zk_vk_update(&self, value: &norito::json::Value) -> Result<()> {
        let url = join_torii_url(&self.torii_url, "v1/zk/vk/update");
        let body = norito::json::to_vec(value)?;
        let resp = self
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?;
        let resp = self.send_prepared_request(resp)?;
        if resp.status() != StatusCode::ACCEPTED {
            return Err(eyre!(
                "Failed to update VK with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(())
    }

    /// Convenience: GET `/v1/zk/vk/{backend}/{name}` as JSON Value.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the response is non-OK, or JSON deserialization fails.
    pub fn get_zk_vk_json(&self, backend: &str, name: &str) -> Result<norito::json::Value> {
        let url = join_torii_url(&self.torii_url, &format!("v1/zk/vk/{backend}/{name}"));
        let resp = self.send_builder(self.default_request(HttpMethod::GET, url))?;
        if resp.status() != StatusCode::OK {
            return Err(eyre!(
                "Failed to get VK with HTTP status: {}. {}",
                resp.status(),
                std::str::from_utf8(resp.body()).unwrap_or("")
            ));
        }
        Ok(norito::json::from_slice(resp.body())?)
    }
}

fn tx_confirmation_status_from_pipeline_payload(
    payload: &JsonValue,
) -> Option<TxConfirmationStatus> {
    let status_value = payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?;
    let block_height = pipeline_status_block_height(payload, status_value);
    match status_value {
        JsonValue::String(kind) => tx_confirmation_status_from_kind(kind, None, block_height),
        JsonValue::Object(map) => {
            let kind = map.get("kind").and_then(JsonValue::as_str)?;
            let rejection = if kind == "Rejected" {
                map.get("content").and_then(decode_rejection_reason_value)
            } else {
                None
            };
            tx_confirmation_status_from_kind(kind, rejection, block_height)
        }
        _ => None,
    }
}

fn pipeline_status_block_height(
    payload: &JsonValue,
    status_value: &JsonValue,
) -> Option<NonZeroU64> {
    let from_status = match status_value {
        JsonValue::Object(map) => map
            .get("block_height")
            .and_then(JsonValue::as_u64)
            .and_then(NonZeroU64::new),
        _ => None,
    };
    let from_content = payload
        .get("content")
        .and_then(|content| content.get("block_height"))
        .and_then(JsonValue::as_u64)
        .and_then(NonZeroU64::new);
    let from_payload = payload
        .get("block_height")
        .and_then(JsonValue::as_u64)
        .and_then(NonZeroU64::new);
    from_status.or(from_content).or(from_payload)
}

fn decode_rejection_reason_value(value: &JsonValue) -> Option<TransactionRejectionReason> {
    match value {
        JsonValue::String(encoded) => decode_rejection_reason_base64(encoded)
            .or_else(|| norito::json::from_value(value.clone()).ok()),
        _ => norito::json::from_value(value.clone()).ok(),
    }
}

fn decode_rejection_reason_base64(encoded: &str) -> Option<TransactionRejectionReason> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    decode_from_bytes::<TransactionRejectionReason>(&bytes)
        .ok()
        .or_else(|| TransactionRejectionReason::decode_all(&mut bytes.as_slice()).ok())
}

fn tx_confirmation_status_from_kind(
    kind: &str,
    rejection: Option<TransactionRejectionReason>,
    block_height: Option<NonZeroU64>,
) -> Option<TxConfirmationStatus> {
    match kind {
        "Queued" => Some(TxConfirmationStatus::Queued),
        "Approved" => Some(TxConfirmationStatus::Approved(block_height)),
        "Committed" => Some(TxConfirmationStatus::Committed),
        "Applied" => Some(TxConfirmationStatus::Applied),
        "Rejected" => Some(TxConfirmationStatus::Rejected(rejection)),
        "Expired" => Some(TxConfirmationStatus::Expired),
        _ => None,
    }
}

fn tx_confirmation_status_from_committed_result(
    result: &crate::data_model::transaction::TransactionResult,
) -> TxConfirmationStatus {
    match &result.0 {
        Ok(_) => TxConfirmationStatus::Applied,
        Err(reason) => TxConfirmationStatus::Rejected(Some(reason.clone())),
    }
}

#[doc(hidden)]
pub async fn listen_for_tx_confirmation_stream<S>(
    event_iterator: &mut S,
    hash: HashOf<SignedTransaction>,
    max_queued_duration: Duration,
) -> Result<HashOf<SignedTransaction>>
where
    S: Stream<Item = Result<EventBox>> + Unpin,
{
    listen_for_tx_confirmation_stream_with_status_check(
        event_iterator,
        hash,
        max_queued_duration,
        Duration::ZERO,
        || Ok(None),
    )
    .await
}

#[allow(clippy::too_many_lines)]
async fn listen_for_tx_confirmation_stream_with_status_check<S, F>(
    event_iterator: &mut S,
    hash: HashOf<SignedTransaction>,
    max_queued_duration: Duration,
    poll_interval: Duration,
    mut status_check: F,
) -> Result<HashOf<SignedTransaction>>
where
    S: Stream<Item = Result<EventBox>> + Unpin,
    F: FnMut() -> Result<Option<TxConfirmationStatus>>,
{
    // Keep track of the block height in which the transaction was approved
    // so we can later detect the corresponding block apply event.
    let mut block_height = None;
    // Track when the transaction first entered the queue.
    let mut queued_at: Option<Instant> = None;
    let poll_enabled = poll_interval != Duration::ZERO;
    let poll_interval = if poll_enabled {
        poll_interval
    } else {
        Duration::from_secs(3600)
    };
    let mut poll = tokio::time::interval(poll_interval);
    if poll_enabled {
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    }
    let mut stream_open = true;

    loop {
        tokio::select! {
            biased;
            _ = poll.tick(), if poll_enabled => {
                match status_check() {
                    Ok(Some(status)) => match status {
                        TxConfirmationStatus::Queued => {
                            if let Some(first) = queued_at {
                                let elapsed = first.elapsed();
                                if elapsed > max_queued_duration {
                                    warn!(%hash, ?elapsed, "transaction remained queued");
                                    return Err(tx_confirmation_final_report(eyre!(
                                        "transaction queued for too long"
                                    )));
                                }
                            } else {
                                queued_at = Some(Instant::now());
                                debug!(%hash, "transaction entered queue");
                            }
                        }
                        TxConfirmationStatus::Approved(height) => {
                            if let Some(height) = height {
                                block_height = Some(height);
                            }
                        }
                        TxConfirmationStatus::Committed | TxConfirmationStatus::Applied => {
                            return Ok(hash)
                        }
                        TxConfirmationStatus::Rejected(Some(reason)) => {
                            return Err(tx_confirmation_final_report(tx_rejection_to_report(
                                &reason,
                            )));
                        }
                        TxConfirmationStatus::Rejected(None) => {
                            return Err(tx_confirmation_final_report(eyre!(
                                "Transaction rejected"
                            )));
                        }
                        TxConfirmationStatus::Expired => {
                            return Err(tx_confirmation_final_report(eyre!(
                                "Transaction expired"
                            )));
                        }
                    },
                    Ok(None) => {}
                    Err(err) => {
                        debug!(%hash, ?err, "pipeline status poll failed; retrying");
                    }
                }
            }
            event = event_iterator.next(), if stream_open => {
                match event {
                    Some(Ok(EventBox::Pipeline(this_event))) => {
                        match this_event {
                            PipelineEventBox::Transaction(transaction_event) => {
                                match transaction_event.status() {
                                    TransactionStatus::Queued => {
                                        if let Some(first) = queued_at {
                                            let elapsed = first.elapsed();
                                            if elapsed > max_queued_duration {
                                                warn!(%hash, ?elapsed, "transaction remained queued");
                                                return Err(tx_confirmation_final_report(eyre!(
                                                    "transaction queued for too long"
                                                )));
                                            }
                                            // Duplicate queued notifications are possible; keep waiting.
                                        } else {
                                            queued_at = Some(Instant::now());
                                            debug!(%hash, "transaction entered queue");
                                        }
                                    }
                                    TransactionStatus::Approved => {
                                        debug!(
                                            %hash,
                                            height = ?transaction_event.block_height(),
                                            "transaction approved"
                                        );
                                        block_height = transaction_event.block_height();
                                    }
                                    TransactionStatus::Rejected(reason) => {
                                        warn!(%hash, ?reason, "transaction rejected during confirmation stream");
                                        return Err(tx_confirmation_final_report(
                                            tx_rejection_to_report(reason),
                                        ));
                                    }
                                    TransactionStatus::Expired => {
                                        warn!(%hash, "transaction expired during confirmation stream");
                                        return Err(tx_confirmation_final_report(eyre!(
                                            "Transaction expired"
                                        )));
                                    }
                                }
                            }
                            PipelineEventBox::Block(block_event) => {
                                if Some(block_event.header().height()) == block_height
                                    && matches!(
                                        block_event.status(),
                                        BlockStatus::Applied | BlockStatus::Committed
                                    )
                                {
                                    debug!(
                                        %hash,
                                        height = block_event.header().height().get(),
                                        status = ?block_event.status(),
                                        "transaction commit observed in block event"
                                    );
                                    return Ok(hash);
                                }
                            }
                            PipelineEventBox::Warning(_w) => {
                                // Ignore warnings for tx confirmation flow
                            }
                            PipelineEventBox::Merge(_merge_event) => {
                                // Merge-ledger commits are orthogonal to transaction confirmation flow
                            }
                            PipelineEventBox::Witness(_witness) => {
                                // Witness events do not influence transaction confirmation flow.
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        if poll_enabled {
                            warn!(%hash, ?err, "tx confirmation stream error; falling back to pipeline status query");
                            stream_open = false;
                        } else {
                            return Err(err);
                        }
                    }
                    None => {
                        if poll_enabled {
                            warn!(%hash, "tx confirmation stream closed; falling back to pipeline status query");
                            stream_open = false;
                        } else {
                            warn!(
                                %hash,
                                "event stream closed before tx reached committed/applied"
                            );
                            return Err(eyre!(
                                "Connection dropped without `Committed/Applied` or `Rejected` event",
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
fn contains_tx_hash(
    batch: &crate::data_model::query::QueryOutputBatchBoxTuple,
    target: HashOf<SignedTransaction>,
) -> bool {
    use crate::data_model::query::QueryOutputBatchBox as Batch;
    batch.iter().any(|b| match b {
        Batch::CommittedTransaction(v) => v
            .iter()
            .any(|tx| tx.entrypoint_hash().as_ref() == target.as_ref()),
        _ => false,
    })
}

fn hashes_match(target: &HashOf<SignedTransaction>, entry_hash: impl AsRef<[u8]>) -> bool {
    target.as_ref() == entry_hash.as_ref()
}

#[cfg(test)]
mod tx_hash_tests {
    use std::time::Duration;

    use eyre::eyre;

    use super::hashes_match;
    use crate::{
        crypto::{Hash, HashOf},
        data_model::transaction::{SignedTransaction, TransactionEntrypoint},
    };

    #[test]
    fn hashes_match_compares_bytes() {
        let tx_hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([1_u8; Hash::LENGTH]));
        let entry_match: HashOf<TransactionEntrypoint> =
            HashOf::from_untyped_unchecked(Hash::prehashed([1_u8; Hash::LENGTH]));
        let entry_other: HashOf<TransactionEntrypoint> =
            HashOf::from_untyped_unchecked(Hash::prehashed([2_u8; Hash::LENGTH]));
        let entry_bytes = [1_u8; Hash::LENGTH];

        assert!(hashes_match(&tx_hash, entry_match.as_ref()));
        assert!(!hashes_match(&tx_hash, entry_other.as_ref()));
        assert!(hashes_match(&tx_hash, entry_bytes));
    }

    #[tokio::test]
    async fn retry_transaction_committed_retries_and_succeeds() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        // First attempt fails, second succeeds
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result = super::Client::retry_transaction_committed(
            move || {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err(eyre!("injected failure"))
                } else {
                    Ok(Some(super::TxConfirmationStatus::Committed))
                }
            },
            Duration::from_millis(0),
            3,
        )
        .await;

        let outcome = result
            .unwrap()
            .expect("expected committed transaction outcome");
        assert!(matches!(outcome, super::TxConfirmationStatus::Committed));
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn retry_transaction_committed_retries_on_empty_status() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result = super::Client::retry_transaction_committed(
            move || {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Ok(None)
                } else {
                    Ok(Some(super::TxConfirmationStatus::Committed))
                }
            },
            Duration::from_millis(0),
            3,
        )
        .await;

        let outcome = result
            .unwrap()
            .expect("expected committed transaction outcome");
        assert!(matches!(outcome, super::TxConfirmationStatus::Committed));
        assert!(attempts.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn retry_transaction_committed_ignores_non_terminal_statuses() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result = super::Client::retry_transaction_committed(
            move || {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
                match count {
                    0 => Ok(Some(super::TxConfirmationStatus::Queued)),
                    1 => Ok(Some(super::TxConfirmationStatus::Approved(None))),
                    _ => Ok(Some(super::TxConfirmationStatus::Committed)),
                }
            },
            Duration::from_millis(0),
            3,
        )
        .await;

        let outcome = result
            .unwrap()
            .expect("expected committed transaction outcome");
        assert!(matches!(outcome, super::TxConfirmationStatus::Committed));
        assert!(attempts.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn retry_transaction_committed_propagates_failure() {
        let result = super::Client::retry_transaction_committed(
            || Err(eyre!("always fails")),
            Duration::from_millis(0),
            1,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn retry_transaction_committed_returns_rejection() {
        use crate::data_model::{ValidationFail, transaction::error::TransactionRejectionReason};

        let result = super::Client::retry_transaction_committed(
            || {
                let reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "nope".to_string(),
                ));
                Ok(Some(super::TxConfirmationStatus::Rejected(Some(reason))))
            },
            Duration::from_millis(0),
            0,
        )
        .await;

        let outcome = result
            .unwrap()
            .expect("expected committed transaction outcome");
        assert!(matches!(
            outcome,
            super::TxConfirmationStatus::Rejected(Some(_))
        ));
    }

    #[test]
    fn tx_confirmation_filters_match_expected() {
        use crate::data_model::events::pipeline::{
            BlockEventFilter, BlockStatus, PipelineEventFilterBox, TransactionEventFilter,
        };

        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([3_u8; Hash::LENGTH]));
        let filters = super::Client::tx_confirmation_filters(hash);
        let expected = vec![
            PipelineEventFilterBox::from(TransactionEventFilter::default().for_hash(hash)),
            PipelineEventFilterBox::from(
                BlockEventFilter::default().for_status(BlockStatus::Applied),
            ),
            PipelineEventFilterBox::from(
                BlockEventFilter::default().for_status(BlockStatus::Committed),
            ),
        ];

        assert_eq!(filters, expected);
    }

    #[tokio::test]
    async fn resolve_committed_fallback_returns_hash_or_fallback_error() {
        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([4_u8; Hash::LENGTH]));
        let resolved = super::Client::resolve_committed_fallback(
            || Ok(Some(super::TxConfirmationStatus::Applied)),
            hash,
            Duration::from_millis(0),
            0,
            "fallback context",
            eyre!("fallback error"),
        )
        .await
        .expect("expected fallback to resolve");
        assert_eq!(resolved, hash);

        let err = super::Client::resolve_committed_fallback(
            || Ok(None),
            hash,
            Duration::from_millis(0),
            0,
            "fallback context",
            eyre!("fallback error"),
        )
        .await
        .expect_err("expected fallback error");
        assert!(
            err.to_string().contains("fallback error"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn tx_confirmation_status_from_pipeline_payload_decodes_rejection_reason() {
        use crate::data_model::{ValidationFail, transaction::error::TransactionRejectionReason};

        let reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "nope".to_string(),
        ));
        let reason_value = norito::json::to_value(&reason).expect("encode rejection reason");
        let payload = norito::json!({
            "kind": "Transaction",
            "content": {
                "hash": "deadbeef",
                "status": {
                    "kind": "Rejected",
                    "content": reason_value,
                },
            },
        });

        let status = super::tx_confirmation_status_from_pipeline_payload(&payload);
        assert_eq!(
            status,
            Some(super::TxConfirmationStatus::Rejected(Some(reason)))
        );
    }

    #[test]
    fn tx_confirmation_status_from_pipeline_payload_decodes_base64_rejection_reason() {
        use crate::data_model::{ValidationFail, transaction::error::TransactionRejectionReason};
        use base64::Engine as _;

        let reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "nope".to_string(),
        ));
        let bytes = norito::to_bytes(&reason).expect("encode rejection reason");
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let payload = norito::json!({
            "kind": "Transaction",
            "content": {
                "hash": "deadbeef",
                "status": {
                    "kind": "Rejected",
                    "content": encoded,
                },
            },
        });

        let status = super::tx_confirmation_status_from_pipeline_payload(&payload);
        assert_eq!(
            status,
            Some(super::TxConfirmationStatus::Rejected(Some(reason)))
        );
    }

    #[test]
    fn tx_confirmation_status_from_pipeline_payload_accepts_terminal_kinds() {
        let committed_payload = norito::json!({
            "content": { "status": { "kind": "Committed" } },
        });
        let applied_payload = norito::json!({
            "status": "Applied",
        });

        assert_eq!(
            super::tx_confirmation_status_from_pipeline_payload(&committed_payload),
            Some(super::TxConfirmationStatus::Committed)
        );
        assert_eq!(
            super::tx_confirmation_status_from_pipeline_payload(&applied_payload),
            Some(super::TxConfirmationStatus::Applied)
        );
    }

    #[test]
    fn tx_confirmation_status_from_pipeline_payload_accepts_non_terminal_kinds() {
        let queued_payload = norito::json!({
            "content": { "status": { "kind": "Queued" } },
        });
        let approved_payload = norito::json!({
            "content": {
                "status": { "kind": "Approved", "block_height": 7 },
            },
        });

        assert_eq!(
            super::tx_confirmation_status_from_pipeline_payload(&queued_payload),
            Some(super::TxConfirmationStatus::Queued)
        );
        assert_eq!(
            super::tx_confirmation_status_from_pipeline_payload(&approved_payload),
            Some(super::TxConfirmationStatus::Approved(
                std::num::NonZeroU64::new(7)
            ))
        );
    }

    #[test]
    fn tx_confirmation_status_from_committed_result_maps_outcomes() {
        use crate::data_model::{
            ValidationFail,
            transaction::{TransactionResult, error::TransactionRejectionReason},
            trigger::DataTriggerSequence,
        };

        let ok_result = TransactionResult(Ok(DataTriggerSequence::default()));
        assert_eq!(
            super::tx_confirmation_status_from_committed_result(&ok_result),
            super::TxConfirmationStatus::Applied
        );

        let reason = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "nope".to_string(),
        ));
        let err_result = TransactionResult(Err(reason.clone()));
        assert_eq!(
            super::tx_confirmation_status_from_committed_result(&err_result),
            super::TxConfirmationStatus::Rejected(Some(reason))
        );
    }

    #[test]
    fn tx_confirmation_timeout_report_mentions_config_key() {
        let report = super::Client::tx_confirmation_timeout_report(Duration::from_millis(1500));
        assert!(
            report.to_string().contains("transaction.status_timeout_ms"),
            "unexpected timeout report: {report:?}"
        );
    }

    #[test]
    fn tx_confirmation_error_wraps_timeout_report() {
        let err = eyre!("confirmation stream failed");
        let wrapped = err.wrap_err(super::Client::tx_confirmation_timeout_report(
            Duration::from_secs(1),
        ));
        let messages: Vec<String> = wrapped.chain().map(|cause| cause.to_string()).collect();
        assert!(
            messages
                .iter()
                .any(|message| message.contains("confirmation stream failed")),
            "missing inner confirmation error: {messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("transaction.status_timeout_ms")),
            "missing timeout context: {messages:?}"
        );
    }

    #[test]
    fn committed_transaction_matches_signed_hash_for_external_entrypoint() {
        use iroha_primitives::const_vec::ConstVec;

        use crate::{
            crypto::MerkleProof,
            data_model::{
                prelude::{AccountId, ChainId, DomainId, InstructionBox, TransactionBuilder},
                query::CommittedTransaction,
                transaction::{ExecutionStep, TransactionEntrypoint, TransactionResult},
                trigger::DataTriggerSequence,
            },
        };

        let chain: ChainId = "hash-chain".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let public_key: crate::crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(domain, public_key);
        let private_key: crate::crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let tx = TransactionBuilder::new(chain, authority.clone()).sign(&private_key);
        let entry = TransactionEntrypoint::External(tx.clone());
        let entry_hash = entry.hash();
        let result = TransactionResult(Ok(DataTriggerSequence::default()));
        let committed = CommittedTransaction {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([1_u8; Hash::LENGTH])),
            entrypoint_hash: entry_hash,
            entrypoint_proof: MerkleProof::from_audit_path(0, Vec::new()),
            entrypoint: entry,
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, Vec::new()),
            result,
        };

        assert!(super::Client::committed_transaction_matches_hash(
            &committed,
            tx.hash(),
            entry_hash
        ));
        let other_hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([9_u8; Hash::LENGTH]));
        assert!(super::Client::committed_transaction_matches_hash(
            &committed, other_hash, entry_hash
        ));

        let time_entry = crate::data_model::trigger::TimeTriggerEntrypoint {
            id: "trigger".parse().unwrap(),
            instructions: ExecutionStep(ConstVec::<InstructionBox>::from(vec![])),
            authority,
        };
        let time_entry_hash = time_entry.hash_as_entrypoint();
        let time_result = TransactionResult(Ok(DataTriggerSequence::default()));
        let time_committed = CommittedTransaction {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([2_u8; Hash::LENGTH])),
            entrypoint_hash: time_entry_hash,
            entrypoint_proof: MerkleProof::from_audit_path(0, Vec::new()),
            entrypoint: TransactionEntrypoint::Time(time_entry),
            result_hash: time_result.hash(),
            result_proof: MerkleProof::from_audit_path(0, Vec::new()),
            result: time_result,
        };

        assert!(super::Client::committed_transaction_matches_hash(
            &time_committed,
            tx.hash(),
            time_entry_hash
        ));
        let mismatched_entry_hash: HashOf<TransactionEntrypoint> =
            HashOf::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
        assert!(!super::Client::committed_transaction_matches_hash(
            &time_committed,
            tx.hash(),
            mismatched_entry_hash
        ));
    }
}

#[cfg(test)]
mod tx_confirmation_stream_tests {
    use std::time::Duration;

    use eyre::eyre;
    use futures_util::stream;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    use super::{
        listen_for_tx_confirmation_stream, listen_for_tx_confirmation_stream_with_status_check,
    };
    use crate::{
        crypto::{Hash, HashOf},
        data_model::{
            ValidationFail,
            block::BlockHeader,
            events::{
                EventBox,
                pipeline::{
                    BlockEvent, BlockStatus, PipelineEventBox, TransactionEvent, TransactionStatus,
                },
            },
            nexus::{DataSpaceId, LaneId},
            transaction::SignedTransaction,
            transaction::error::TransactionRejectionReason,
        },
    };

    #[tokio::test]
    async fn queued_timeout_honors_max_duration() {
        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([7_u8; Hash::LENGTH]));
        let queued_event = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
            hash,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        }));
        let (tx, rx) = mpsc::unbounded_channel::<Result<EventBox, eyre::Report>>();
        let mut events = UnboundedReceiverStream::new(rx);
        tokio::spawn(async move {
            let _ = tx.send(Ok(queued_event.clone()));
            tokio::time::sleep(Duration::from_millis(5)).await;
            let _ = tx.send(Ok(queued_event));
        });

        let err = listen_for_tx_confirmation_stream(&mut events, hash, Duration::from_millis(1))
            .await
            .expect_err("queued timeout should error");
        assert!(err.to_string().contains("transaction queued for too long"));
    }

    #[tokio::test]
    async fn polling_confirms_after_stream_close() {
        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([9_u8; Hash::LENGTH]));
        let mut events = stream::empty::<Result<EventBox, eyre::Report>>();
        let mut checks = 0u8;
        let result = listen_for_tx_confirmation_stream_with_status_check(
            &mut events,
            hash,
            Duration::from_secs(1),
            Duration::from_millis(1),
            || {
                checks = checks.saturating_add(1);
                if checks > 0 {
                    Ok(Some(super::TxConfirmationStatus::Committed))
                } else {
                    Ok(None)
                }
            },
        )
        .await;
        assert_eq!(result.unwrap(), hash);
    }

    #[tokio::test]
    async fn polling_approved_updates_block_height_for_block_events() {
        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([10_u8; Hash::LENGTH]));
        let height = std::num::NonZeroU64::new(12).expect("nonzero height");
        let block_event = EventBox::Pipeline(PipelineEventBox::Block(BlockEvent {
            header: BlockHeader {
                height,
                prev_block_hash: None,
                merkle_root: None,
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            },
            status: BlockStatus::Committed,
        }));
        let (tx, rx) = mpsc::unbounded_channel::<Result<EventBox, eyre::Report>>();
        let mut events = UnboundedReceiverStream::new(rx);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let _ = tx.send(Ok(block_event));
        });

        let mut checks = 0u8;
        let result = listen_for_tx_confirmation_stream_with_status_check(
            &mut events,
            hash,
            Duration::from_secs(1),
            Duration::from_millis(1),
            || {
                checks = checks.saturating_add(1);
                Ok(Some(super::TxConfirmationStatus::Approved(Some(height))))
            },
        )
        .await;

        assert_eq!(result.unwrap(), hash);
        assert!(checks > 0);
    }

    #[tokio::test]
    async fn polling_queued_honors_max_duration() {
        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([11_u8; Hash::LENGTH]));
        let mut events = stream::empty::<Result<EventBox, eyre::Report>>();

        let err = listen_for_tx_confirmation_stream_with_status_check(
            &mut events,
            hash,
            Duration::from_millis(20),
            Duration::from_millis(5),
            || Ok(Some(super::TxConfirmationStatus::Queued)),
        )
        .await
        .expect_err("queued timeout should error");
        assert!(err.to_string().contains("transaction queued for too long"));
    }

    #[tokio::test]
    async fn polling_rejects_even_with_busy_stream() {
        let hash: HashOf<SignedTransaction> =
            HashOf::from_untyped_unchecked(Hash::prehashed([12_u8; Hash::LENGTH]));
        let queued_event = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
            hash,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        }));
        let (tx, rx) = mpsc::unbounded_channel::<Result<EventBox, eyre::Report>>();
        let mut events = UnboundedReceiverStream::new(rx);
        for _ in 0..128 {
            let _ = tx.send(Ok(queued_event.clone()));
        }
        let mut checks = 0u8;
        let rejection = TransactionRejectionReason::Validation(ValidationFail::InternalError(
            "rejected".to_string(),
        ));

        let err = listen_for_tx_confirmation_stream_with_status_check(
            &mut events,
            hash,
            Duration::from_secs(1),
            Duration::from_millis(1),
            || {
                checks = checks.saturating_add(1);
                Ok(Some(super::TxConfirmationStatus::Rejected(Some(
                    rejection.clone(),
                ))))
            },
        )
        .await
        .expect_err("expected rejection from status polling");
        assert!(err.to_string().contains("Transaction rejected"));
        assert!(checks > 0);
    }

    #[test]
    fn tx_confirmation_poll_interval_bounds() {
        let fast = super::Client::tx_confirmation_poll_interval(Duration::from_secs(1));
        assert_eq!(fast, Duration::from_millis(200));

        let normal = super::Client::tx_confirmation_poll_interval(Duration::from_secs(10));
        assert_eq!(normal, Duration::from_millis(250));

        let slow = super::Client::tx_confirmation_poll_interval(Duration::from_secs(120));
        assert_eq!(slow, Duration::from_secs(2));
    }

    #[test]
    fn tx_confirmation_connect_timeout_bounds() {
        let fast = super::Client::tx_confirmation_connect_timeout(Duration::from_secs(1));
        assert_eq!(fast, Duration::from_millis(200));

        let normal = super::Client::tx_confirmation_connect_timeout(Duration::from_secs(10));
        assert_eq!(normal, Duration::from_secs(1));

        let slow = super::Client::tx_confirmation_connect_timeout(Duration::from_secs(120));
        assert_eq!(slow, Duration::from_secs(5));

        let tiny = super::Client::tx_confirmation_connect_timeout(Duration::from_millis(100));
        assert_eq!(tiny, Duration::from_millis(100));
    }

    #[test]
    fn final_confirmation_errors_skip_fallback() {
        let rejection = TransactionRejectionReason::Validation(ValidationFail::InternalError(
            "rejected".to_string(),
        ));
        let final_err =
            super::tx_confirmation_final_report(super::tx_rejection_to_report(&rejection));
        assert!(!super::should_fallback_after_confirmation_error(&final_err));

        let transient = eyre!("transient");
        assert!(super::should_fallback_after_confirmation_error(&transient));
    }
}

fn tx_rejection_to_report(
    reason: &crate::data_model::transaction::error::TransactionRejectionReason,
) -> eyre::Report {
    // Attach the innermost cause if available, so callers can match
    // on precise messages like "Failed to find asset: `...`".
    use crate::data_model::{
        ValidationFail, isi::error::InstructionExecutionError, query::error::QueryExecutionFail,
    };

    if let crate::data_model::transaction::error::TransactionRejectionReason::Validation(
        ValidationFail::InstructionFailed(
            InstructionExecutionError::Query(QueryExecutionFail::Find(find))
            | InstructionExecutionError::Find(find),
        ),
    ) = reason
    {
        return eyre::Report::from(find.clone()).wrap_err("Transaction rejected");
    }

    if let crate::data_model::transaction::error::TransactionRejectionReason::Validation(
        ValidationFail::NotPermitted(msg),
    ) = reason
    {
        return eyre!(msg.clone()).wrap_err("Transaction rejected");
    }

    if let crate::data_model::transaction::error::TransactionRejectionReason::Validation(
        ValidationFail::InstructionFailed(instruction_err),
    ) = reason
    {
        if let InstructionExecutionError::AccountAdmission(admission) = instruction_err {
            return eyre::Report::from(admission.clone())
                .wrap_err("Transaction rejected during implicit account admission");
        }

        // Preserve the structured instruction error as the root cause so callers can inspect it.
        return eyre::Report::from(instruction_err.clone()).wrap_err("Transaction rejected");
    }

    // Otherwise, fall back to the string representation to avoid requiring
    // `std::error::Error` impls on the rejection reason (works under `fast_dsl`).
    eyre!(reason.to_string()).wrap_err("Transaction rejected")
}

pub(crate) fn join_torii_url(url: &Url, path: &str) -> Url {
    // This is needed to prevent "https://iroha-peer.jp/peer1/".join("/query") == "https://iroha-peer.jp/query"
    let path = path.strip_prefix('/').unwrap_or(path);

    // This is needed to prevent "https://iroha-peer.jp/peer1".join("query") == "https://iroha-peer.jp/query"
    // Note: trailing slash is added to url at config user layer if needed
    assert!(
        url.path().ends_with('/'),
        "Torii url must end with trailing slash"
    );

    url.join(path).expect("Valid URI")
}

#[cfg(test)]
mod url_join_tests {
    use url::Url;

    use super::join_torii_url;

    #[test]
    fn join_prover_reports_paths() {
        let base = Url::parse("http://localhost:8080/api/").unwrap();
        let u = join_torii_url(&base, "v1/zk/prover/reports");
        assert_eq!(u.as_str(), "http://localhost:8080/api/v1/zk/prover/reports");
        let u2 = join_torii_url(&base, "v1/zk/prover/reports/abcd");
        assert_eq!(
            u2.as_str(),
            "http://localhost:8080/api/v1/zk/prover/reports/abcd"
        );
    }

    #[test]
    fn join_vote_tally_path() {
        let base = Url::parse("http://localhost:8080/api/").unwrap();
        let u = join_torii_url(&base, "v1/zk/vote/tally");
        assert_eq!(u.as_str(), "http://localhost:8080/api/v1/zk/vote/tally");
    }
}

/// Logic for `sync` and `async` Iroha websocket streams
pub mod stream_api {
    use futures_util::{SinkExt, Stream, StreamExt};

    use super::*;
    use crate::{
        http::ws::conn_flow::{Events, Init, InitData},
        http_default::DefaultWebSocketRequestBuilder,
    };

    /// Iterator for getting messages from the `WebSocket` stream.
    pub(super) struct SyncIterator<E> {
        stream: WebSocketStream,
        handler: E,
    }

    impl<E> SyncIterator<E> {
        /// Construct `SyncIterator` and send the subscription request.
        ///
        /// # Errors
        /// - Request failed to build
        /// - `connect` failed
        /// - Sending failed
        /// - Message not received in stream during connection or subscription
        /// - Message is an error
        pub fn new<I: Init<DefaultWebSocketRequestBuilder>>(
            handler: I,
        ) -> Result<SyncIterator<I::Next>> {
            trace!("Creating `SyncIterator`");
            let InitData {
                first_message,
                req,
                next: next_handler,
            } = Init::<http_default::DefaultWebSocketRequestBuilder>::init(handler);

            let mut stream = req.build()?.connect()?;
            stream.send(WebSocketMessage::Binary(first_message.into()))?;

            trace!("`SyncIterator` created successfully");
            Ok(SyncIterator {
                stream,
                handler: next_handler,
            })
        }
    }

    impl<E: Events> Iterator for SyncIterator<E> {
        type Item = Result<E::Event>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                match self.stream.read() {
                    Ok(WebSocketMessage::Binary(message)) => {
                        match self.handler.message(message.to_vec()) {
                            Ok(event) => return Some(Ok(event)),
                            Err(err) => {
                                let preview_len = message.len().min(32);
                                let preview = hex::encode(&message[..preview_len]);
                                let full_hex = hex::encode(&message);
                                tracing::warn!(
                                        ?err,
                                    frame_len = message.len(),
                                    frame_head_hex = %preview,
                                    frame_hex = %full_hex,
                                    "dropping malformed event frame"
                                );
                            }
                        }
                    }
                    Ok(_) => (),
                    Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                        return None;
                    }
                    Err(err) => return Some(Err(err.into())),
                }
            }
        }
    }

    impl<E> Drop for SyncIterator<E> {
        fn drop(&mut self) {
            let mut close = || -> eyre::Result<()> {
                match self.stream.close(None) {
                    Ok(()) => {}
                    Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                        return Ok(());
                    }
                    Err(error) => Err(error)?,
                }
                // NOTE: drive close handshake to completion
                loop {
                    match self.stream.read() {
                        Ok(_) => {}
                        Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                            return Ok(());
                        }
                        Err(error) => Err(error)?,
                    }
                }
            };

            trace!("Closing WebSocket connection");
            let _ = close().map_err(|e| error!(%e));
            trace!("WebSocket connection closed");
        }
    }

    /// Async stream for getting messages from the `WebSocket` stream.
    pub struct AsyncStream<E> {
        stream: AsyncWebSocketStream,
        handler: E,
    }

    impl<E> AsyncStream<E> {
        /// Construct [`AsyncStream`] and send the subscription request.
        ///
        /// # Errors
        /// - Request failed to build
        /// - `connect_async` failed
        /// - Sending failed
        /// - Message not received in stream during connection or subscription
        /// - Message is an error
        #[allow(clippy::future_not_send)]
        pub async fn new<I: Init<DefaultWebSocketRequestBuilder>>(
            handler: I,
        ) -> Result<AsyncStream<I::Next>> {
            trace!("Creating `AsyncStream`");
            let InitData {
                first_message,
                req,
                next: next_handler,
            } = Init::<http_default::DefaultWebSocketRequestBuilder>::init(handler);

            let mut stream = req.build()?.connect_async().await?;
            stream
                .send(WebSocketMessage::Binary(first_message.into()))
                .await?;

            trace!("`AsyncStream` created successfully");
            Ok(AsyncStream {
                stream,
                handler: next_handler,
            })
        }
    }

    impl<E: Send> AsyncStream<E> {
        /// Close websocket
        /// # Errors
        /// - Server fails to send `Close` message
        /// - Closing the websocket connection itself fails.
        pub async fn close(mut self) {
            let close = async {
                <_ as SinkExt<_>>::close(&mut self.stream).await?;
                Ok(())
            };

            trace!("Closing WebSocket connection");
            let _ = close.await.map_err(|e: eyre::Report| error!(%e));
            trace!("WebSocket connection closed");
        }
    }

    impl<E: Events + Unpin> Stream for AsyncStream<E> {
        type Item = Result<E::Event>;

        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            loop {
                break match futures_util::ready!(self.stream.poll_next_unpin(cx)) {
                    Some(Ok(WebSocketMessage::Binary(message))) => {
                        match self.handler.message(message.to_vec()) {
                            Ok(event) => std::task::Poll::Ready(Some(Ok(event))),
                            Err(err) => {
                                let preview_len = message.len().min(32);
                                let preview = hex::encode(&message[..preview_len]);
                                let full_hex = hex::encode(&message);
                                tracing::warn!(
                                    ?err,
                                    frame_len = message.len(),
                                    frame_head_hex = %preview,
                                    frame_hex = %full_hex,
                                    "dropping malformed event frame"
                                );
                                continue;
                            }
                        }
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(err)) => std::task::Poll::Ready(Some(Err(err.into()))),
                    None => std::task::Poll::Ready(None),
                };
            }
        }
    }
}

/// Logic related to Events API client implementation.
pub mod events_api {

    use super::*;
    use crate::http::ws::{
        conn_flow::{Events as FlowEvents, Init as FlowInit, InitData},
        transform_ws_url,
    };

    /// Events API flow. For documentation and usage examples, refer to [`crate::http::ws::conn_flow`].
    pub mod flow {
        use super::*;

        /// Initialization struct for Events API flow.
        pub struct Init {
            /// TORII URL
            url: Url,
            /// HTTP request headers
            headers: HashMap<String, String>,
            /// Event filter
            filters: Vec<EventFilterBox>,
        }

        impl Init {
            /// Construct new item with provided filter, headers and url.
            ///
            /// # Errors
            /// Fails if [`transform_ws_url`] fails.
            #[inline]
            pub(in super::super) fn new(
                filters: Vec<EventFilterBox>,
                headers: HashMap<String, String>,
                url: Url,
            ) -> Result<Self> {
                Ok(Self {
                    url: transform_ws_url(url)?,
                    headers,
                    filters,
                })
            }
        }

        impl<R: RequestBuilder> FlowInit<R> for Init {
            type Next = Events;

            fn init(self) -> InitData<R, Self::Next> {
                let Self {
                    url,
                    headers,
                    filters,
                } = self;

                let msg = norito::to_bytes(&EventSubscriptionRequest::new(filters))
                    .expect("encode event subscription request");
                InitData::new(R::new(HttpMethod::GET, url).headers(headers), msg, Events)
            }
        }

        /// Events handler for Events API flow
        #[derive(Debug, Copy, Clone)]
        pub struct Events;

        impl FlowEvents for Events {
            type Event = crate::data_model::prelude::EventBox;

            fn message(&self, message: Vec<u8>) -> Result<Self::Event> {
                let event_socket_message: EventMessage = decode_from_bytes(&message)?;
                Ok(event_socket_message.into())
            }
        }
    }

    /// Iterator for getting events from the `WebSocket` stream.
    pub(super) type EventIterator = stream_api::SyncIterator<flow::Events>;

    /// Async stream for getting events from the `WebSocket` stream.
    pub type AsyncEventStream = stream_api::AsyncStream<flow::Events>;
}

mod blocks_api {
    use super::*;
    use crate::http::ws::{
        conn_flow::{Events as FlowEvents, Init as FlowInit, InitData},
        transform_ws_url,
    };

    /// Blocks API flow. For documentation and usage examples, refer to [`crate::http::ws::conn_flow`].
    pub mod flow {
        use std::num::NonZeroU64;

        use super::*;
        use crate::data_model::block::stream::*;

        /// Initialization struct for Blocks API flow.
        pub struct Init {
            /// Block height from which to start streaming blocks
            height: NonZeroU64,
            /// HTTP request headers
            headers: HashMap<String, String>,
            /// TORII URL
            url: Url,
        }

        impl Init {
            /// Construct new item with provided headers and url.
            ///
            /// # Errors
            /// If [`transform_ws_url`] fails.
            #[inline]
            pub(in super::super) fn new(
                height: NonZeroU64,
                headers: HashMap<String, String>,
                url: Url,
            ) -> Result<Self> {
                Ok(Self {
                    height,
                    headers,
                    url: transform_ws_url(url)?,
                })
            }
        }

        impl<R: RequestBuilder> FlowInit<R> for Init {
            type Next = Events;

            fn init(self) -> InitData<R, Self::Next> {
                let Self {
                    height,
                    headers,
                    url,
                } = self;

                let msg = norito::to_bytes(&BlockSubscriptionRequest::new(height))
                    .expect("encode block subscription request");
                InitData::new(R::new(HttpMethod::GET, url).headers(headers), msg, Events)
            }
        }

        /// Events handler for Blocks API flow
        #[derive(Debug, Copy, Clone)]
        pub struct Events;

        impl FlowEvents for Events {
            type Event = SignedBlock;

            fn message(&self, message: Vec<u8>) -> Result<Self::Event> {
                let block_message: BlockMessage = decode_from_bytes(&message)?;
                Ok(block_message.into())
            }
        }
    }

    /// Iterator for getting blocks from the `WebSocket` stream.
    pub(super) type BlockIterator = stream_api::SyncIterator<flow::Events>;

    /// Async stream for getting blocks from the `WebSocket` stream.
    pub type AsyncBlockStream = stream_api::AsyncStream<flow::Events>;
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus::{
                LaneBlockCommitment, LaneLiquidityProfile, LaneSettlementReceipt, LaneSwapMetadata,
                LaneVolatilityClass, SumeragiBlockSyncRosterStatus, SumeragiDaGateReason,
                SumeragiDaGateSatisfaction, SumeragiDaGateStatus, SumeragiKuraStoreStatus,
                SumeragiMembershipMismatchStatus, SumeragiMembershipStatus,
                SumeragiMissingBlockFetchStatus, SumeragiPeerKeyPolicyStatus,
                SumeragiPendingRbcStatus, SumeragiQcEntry, SumeragiQcSnapshot,
                SumeragiRbcStoreStatus, SumeragiStatusWire, SumeragiValidationRejectStatus,
                SumeragiViewChangeCauseStatus,
            },
        },
        da::{
            ingest::DaStripeLayout,
            manifest::{ChunkCommitment, ChunkRole, DaManifestV1},
            types::{
                BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                ExtraMetadata, RetentionPolicy, StorageTicketId,
            },
        },
        nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    };
    use iroha_telemetry::metrics::GovernanceStatus;
    use iroha_test_samples::gen_account_in;
    use sorafs_car::multi_fetch::{ChunkReceipt, FetchOutcome, FetchProvider, ProviderReport};
    use sorafs_orchestrator::{PolicyReport, PolicyStatus, prelude::ChunkStore};
    use tempfile::tempdir;

    use super::{
        default_alias_policy,
        evidence_http_tests::{
            SnapshotStore, base_url, client_with_base_url, json_response, respond_with,
            with_mock_http, with_mock_sorafs_fetch,
        },
        *,
    };
    use crate::http::ws::conn_flow::Events as _;
    use crate::{
        config::{BasicAuth, Config},
        da::{DaSampledChunk, DaSamplingPlan, PDP_COMMITMENT_HEADER},
        http::{Method as HttpMethod, Response as HttpResponse, StatusCode},
        secrecy::SecretString,
    };

    const LOGIN: &str = "mad_hatter";
    const PASSWORD: &str = "ilovetea";
    // `mad_hatter:ilovetea` encoded with base64
    const ENCRYPTED_CREDENTIALS: &str = "bWFkX2hhdHRlcjppbG92ZXRlYQ==";

    #[allow(clippy::too_many_lines)]
    fn sample_sumeragi_status_with_relay() -> (SumeragiStatusWire, LaneRelayEnvelope) {
        let subject_hash = Hash::prehashed([0xAA; Hash::LENGTH]);
        let highest = HashOf::<BlockHeader>::from_untyped_unchecked(subject_hash);
        let settlement = LaneBlockCommitment {
            block_height: 12,
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(7),
            tx_count: 1,
            total_local_micro: 500_000,
            total_xor_due_micro: 250_000,
            total_xor_after_haircut_micro: 240_000,
            total_xor_variance_micro: 10_000,
            swap_metadata: Some(LaneSwapMetadata {
                epsilon_bps: 35,
                twap_window_seconds: 120,
                liquidity_profile: LaneLiquidityProfile::Tier2,
                twap_local_per_xor: "123.456".to_owned(),
                volatility_class: LaneVolatilityClass::Elevated,
            }),
            receipts: vec![LaneSettlementReceipt {
                source_id: [0x11; 32],
                local_amount_micro: 500_000,
                xor_due_micro: 250_000,
                xor_after_haircut_micro: 240_000,
                xor_variance_micro: 10_000,
                timestamp_ms: 1_724_000_000_000,
            }],
        };
        let da_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0xDD; Hash::LENGTH],
        )));
        let mut block_header = BlockHeader::new(
            NonZeroU64::new(12).expect("nonzero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        block_header.set_da_commitments_hash(da_hash);
        let execution_qc = iroha_data_model::consensus::ExecutionQcRecord {
            subject_block_hash: block_header.hash(),
            parent_state_root: Hash::prehashed([0xEA; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0xEE; Hash::LENGTH]),
            height: block_header.height().get(),
            view: 5,
            epoch: 1,
            signers_bitmap: vec![0b1010_0001],
            bls_aggregate_signature: vec![0xAB; 48],
        };
        let relay_envelope = LaneRelayEnvelope::new(
            block_header,
            Some(execution_qc),
            da_hash,
            settlement.clone(),
            256,
        )
        .expect("construct lane relay envelope");
        let status = SumeragiStatusWire {
            mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
            staged_mode_tag: None,
            staged_mode_activation_height: None,
            mode_activation_lag_blocks: None,
            mode_flip_kill_switch: true,
            mode_flip_blocked: false,
            mode_flip_success_total: 0,
            mode_flip_fail_total: 0,
            mode_flip_blocked_total: 0,
            last_mode_flip_timestamp_ms: None,
            last_mode_flip_error: None,
            consensus_caps: None,
            leader_index: 2,
            highest_qc_height: 12,
            highest_qc_view: 5,
            highest_qc_subject: Some(highest),
            locked_qc_height: 11,
            locked_qc_view: 4,
            locked_qc_subject: None,
            commit_certificate:
                iroha_data_model::block::consensus::SumeragiCommitCertificateStatus::default(),
            commit_quorum: iroha_data_model::block::consensus::SumeragiCommitQuorumStatus::default(
            ),
            view_change_proof_accepted_total: 5,
            view_change_proof_stale_total: 6,
            view_change_proof_rejected_total: 7,
            view_change_suggest_total: 8,
            view_change_install_total: 9,
            view_change_causes: SumeragiViewChangeCauseStatus::default(),
            gossip_fallback_total: 3,
            block_created_dropped_by_lock_total: 1,
            block_created_hint_mismatch_total: 2,
            block_created_proposal_mismatch_total: 0,
            validation_reject_total: 0,
            validation_reject_reason: None,
            validation_rejects: SumeragiValidationRejectStatus::default(),
            peer_key_policy: SumeragiPeerKeyPolicyStatus::default(),
            block_sync_roster: SumeragiBlockSyncRosterStatus::default(),
            pacemaker_backpressure_deferrals_total: 6,
            commit_pipeline_tick_total: 0,
            da_reschedule_total: 0,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: 0,
                last_targets: 0,
                last_dwell_ms: 0,
            },
            da_gate: SumeragiDaGateStatus {
                reason: SumeragiDaGateReason::None,
                last_satisfied: SumeragiDaGateSatisfaction::None,
                missing_local_data_total: 0,
                manifest_guard_total: 0,
            },
            kura_store: SumeragiKuraStoreStatus {
                failures_total: 0,
                abort_total: 0,
                last_retry_attempt: 0,
                last_retry_backoff_ms: 0,
                last_height: 0,
                last_view: 0,
                last_hash: None,
                ..Default::default()
            },
            rbc_store: SumeragiRbcStoreStatus {
                sessions: 0,
                bytes: 0,
                pressure_level: 0,
                backpressure_deferrals_total: 0,
                evictions_total: 0,
                recent_evictions: Vec::new(),
            },
            pending_rbc: SumeragiPendingRbcStatus::default(),
            tx_queue_depth: 7,
            tx_queue_capacity: 20,
            tx_queue_saturated: true,
            epoch_length_blocks: 100,
            epoch_commit_deadline_offset: 60,
            epoch_reveal_deadline_offset: 80,
            prf_epoch_seed: Some([0xBB; 32]),
            prf_height: 12,
            prf_view: 5,
            vrf_penalty_epoch: 3,
            vrf_committed_no_reveal_total: 1,
            vrf_no_participation_total: 0,
            vrf_late_reveals_total: 2,
            consensus_penalties_applied_total: 0,
            consensus_penalties_pending: 0,
            vrf_penalties_applied_total: 0,
            vrf_penalties_pending: 0,
            membership: SumeragiMembershipStatus {
                height: 9,
                view: 4,
                epoch: 2,
                view_hash: Some([0xCC; 32]),
            },
            membership_mismatch: SumeragiMembershipMismatchStatus::default(),
            lane_commitments: vec![iroha_data_model::block::consensus::SumeragiLaneCommitment {
                block_height: 12,
                lane_id: LaneId::new(1),
                tx_count: 4,
                total_chunks: 2,
                rbc_bytes_total: 128,
                teu_total: 42,
                block_hash: highest,
            }],
            dataspace_commitments: vec![
                iroha_data_model::block::consensus::SumeragiDataspaceCommitment {
                    block_height: 12,
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(7),
                    tx_count: 2,
                    total_chunks: 1,
                    rbc_bytes_total: 64,
                    teu_total: 21,
                    block_hash: highest,
                },
            ],
            lane_settlement_commitments: vec![settlement],
            lane_relay_envelopes: vec![relay_envelope.clone()],
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: vec![iroha_data_model::block::consensus::SumeragiLaneGovernance {
                lane_id: LaneId::new(1),
                alias: "alpha-lane".to_owned(),
                governance: Some("module.v1".to_owned()),
                manifest_required: true,
                manifest_ready: true,
                manifest_path: Some("/etc/iroha/lanes/alpha.toml".to_owned()),
                validator_ids: vec!["validator@test".to_owned()],
                quorum: Some(2),
                protected_namespaces: vec!["finance".to_owned()],
                runtime_upgrade: Some(
                    iroha_data_model::block::consensus::SumeragiRuntimeUpgradeHook {
                        allow: true,
                        require_metadata: true,
                        metadata_key: Some("upgrade_id".to_owned()),
                        allowed_ids: vec!["alpha-upgrade".to_owned()],
                    },
                ),
            }],
            worker_loop: iroha_data_model::block::consensus::SumeragiWorkerLoopStatus::default(),
            commit_inflight:
                iroha_data_model::block::consensus::SumeragiCommitInflightStatus::default(),
        };
        (status, relay_envelope)
    }

    fn config_factory() -> Config {
        let (account_id, key_pair) = gen_account_in("wonderland");
        Config {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            key_pair,
            account: account_id,
            torii_api_url: "http://127.0.0.1:8080".parse().unwrap(),
            torii_api_version: crate::config::default_torii_api_version(),
            torii_api_min_proof_version: crate::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                .to_string(),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            basic_auth: None,
            transaction_add_nonce: false,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(10),
            connect_queue_root: crate::config::default_connect_queue_root(),
            sorafs_alias_cache: default_alias_policy(),
            sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
            sorafs_rollout_phase: SorafsRolloutPhase::Canary,
        }
    }

    #[test]
    fn events_ws_flow_uses_framed_norito() {
        use std::num::NonZeroU64;

        use crate::data_model::events::{
            EventBox, EventFilterBox,
            pipeline::{
                PipelineEventBox, PipelineEventFilterBox, PipelineWarning, TransactionEventFilter,
            },
            stream::{EventMessage, EventSubscriptionRequest},
        };

        let filters = vec![EventFilterBox::Pipeline(
            PipelineEventFilterBox::Transaction(TransactionEventFilter::default()),
        )];
        let init = events_api::flow::Init::new(
            filters.clone(),
            HashMap::new(),
            "http://127.0.0.1:8080".parse().expect("valid url"),
        )
        .expect("init events handler");
        let init_data = <events_api::flow::Init as crate::http::ws::conn_flow::Init<
            crate::http_default::DefaultWebSocketRequestBuilder,
        >>::init(init);
        let decoded: EventSubscriptionRequest = norito::decode_from_bytes(&init_data.first_message)
            .expect("decode event subscription request");
        assert_eq!(decoded.filters, filters);
        assert!(decoded.proof_backend.is_none());
        assert!(decoded.proof_call_hash.is_none());
        assert!(decoded.proof_envelope_hash.is_none());

        let header = BlockHeader::new(NonZeroU64::new(1).expect("height"), None, None, None, 0, 0);
        let warning = PipelineWarning {
            header,
            kind: "test".to_string(),
            details: "details".to_string(),
        };
        let event = EventBox::Pipeline(PipelineEventBox::Warning(warning));
        let bytes = norito::to_bytes(&EventMessage(event.clone())).expect("encode event message");
        let decoded_event = events_api::flow::Events
            .message(bytes)
            .expect("decode event message");
        assert_eq!(decoded_event, event);
    }

    #[test]
    fn blocks_ws_flow_uses_framed_norito() {
        use std::num::NonZeroU64;

        use crate::{
            crypto::{PrivateKey, PublicKey},
            data_model::{
                block::{SignedBlock, stream::BlockMessage, stream::BlockSubscriptionRequest},
                prelude::{AccountId, ChainId, DomainId, TransactionBuilder},
            },
        };

        let height = NonZeroU64::new(1).expect("height");
        let init = blocks_api::flow::Init::new(
            height,
            HashMap::new(),
            "http://127.0.0.1:8080".parse().expect("valid url"),
        )
        .expect("init blocks handler");
        let init_data = <blocks_api::flow::Init as crate::http::ws::conn_flow::Init<
            crate::http_default::DefaultWebSocketRequestBuilder,
        >>::init(init);
        let decoded: BlockSubscriptionRequest = norito::decode_from_bytes(&init_data.first_message)
            .expect("decode block subscription request");
        assert_eq!(decoded.0, height);

        let chain: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("chain id");
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key");
        let private_key: PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .expect("private key");
        let authority = AccountId::new(domain, public_key);
        let tx = TransactionBuilder::new(chain, authority).sign(&private_key);
        let block = SignedBlock::genesis(vec![tx], &private_key, None, None);

        let bytes = norito::to_bytes(&BlockMessage(block.clone())).expect("encode block message");
        let decoded_block = blocks_api::flow::Events
            .message(bytes)
            .expect("decode block message");
        assert_eq!(decoded_block, block);
    }

    #[test]
    fn build_da_ingest_request_uses_client_context() {
        let client = client_with_base_url(base_url());
        let params = DaIngestParams {
            lane_id: LaneId::new(7),
            epoch: 11,
            ..DaIngestParams::default()
        };
        let metadata = ExtraMetadata::default();
        let manifest_bytes = Some(vec![0xAA, 0xBB]);
        let payload = vec![0x10, 0x20, 0x30, 0x40];
        let request = client.build_da_ingest_request(
            payload.clone(),
            &params,
            metadata.clone(),
            manifest_bytes.clone(),
        );
        assert_eq!(request.payload, payload);
        assert_eq!(request.metadata, metadata);
        assert_eq!(request.norito_manifest, manifest_bytes);
        assert_eq!(request.lane_id, params.lane_id);
        assert_eq!(request.submitter, client.key_pair.public_key().clone());
        let expected_blob = blake3::hash(&payload);
        assert_eq!(request.client_blob_id.as_ref(), expected_blob.as_bytes());
    }

    fn inline_chunk_profile(profile_id: u32) -> sorafs_manifest::ChunkingProfileV1 {
        use sorafs_manifest::{ChunkingProfileV1, ProfileId, chunker_registry};

        ChunkingProfileV1 {
            profile_id: ProfileId(profile_id),
            namespace: "inline".into(),
            name: "inline".into(),
            semver: "0.0.0".into(),
            min_size: 512,
            target_size: 512,
            max_size: 512,
            break_mask: 1,
            multihash_code: chunker_registry::DEFAULT_MULTIHASH_CODE,
            aliases: vec!["inline.inline@0.0.0".into()],
        }
    }

    fn inline_pdp_commitment(
        profile_id: u32,
        manifest_digest: [u8; 32],
        roots: ([u8; 32], [u8; 32]),
        heights: (u8, u8),
        sample_window: u64,
        sealed_at: u64,
    ) -> sorafs_manifest::pdp::PdpCommitmentV1 {
        use sorafs_manifest::pdp::{HashAlgorithmV1, PdpCommitmentV1};

        PdpCommitmentV1 {
            version: 1,
            manifest_digest,
            chunk_profile: inline_chunk_profile(profile_id),
            commitment_root_hot: roots.0,
            commitment_root_segment: roots.1,
            hash_algorithm: HashAlgorithmV1::Blake3_256,
            hot_tree_height: u16::from(heights.0),
            segment_tree_height: u16::from(heights.1),
            sample_window: u16::try_from(sample_window)
                .expect("sample window should fit in commitment layout"),
            sealed_at,
        }
    }

    fn sample_ingest_receipt() -> DaIngestReceipt {
        DaIngestReceipt {
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(0),
            epoch: 0,
            blob_hash: BlobDigest::new([0x22; 32]),
            chunk_root: BlobDigest::new([0x33; 32]),
            manifest_hash: BlobDigest::new([0x44; 32]),
            storage_ticket: StorageTicketId::new([0x55; 32]),
            pdp_commitment: None,
            stripe_layout: DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            },
            queued_at_unix: 1,
            rent_quote: DaRentQuote::default(),
            operator_signature: iroha_crypto::Signature::from_bytes(&[0u8; 64]),
        }
    }

    #[test]
    fn submit_da_blob_posts_payload_and_parses_response() {
        use base64::Engine as _;

        let client = client_with_base_url(base_url());
        let params = DaIngestParams::default();
        let metadata = ExtraMetadata::default();
        let payload = vec![0xAB, 0xCD, 0xEF];
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let commitment_b64 = base64::engine::general_purpose::STANDARD.encode(
            norito::to_bytes(&inline_pdp_commitment(
                0,
                [1u8; 32],
                ([2u8; 32], [3u8; 32]),
                (1, 1),
                1,
                1,
            ))
            .expect("encode commitment"),
        );
        let receipt = DaIngestReceipt {
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(0),
            epoch: 0,
            blob_hash: BlobDigest::new([0x22; 32]),
            chunk_root: BlobDigest::new([0x33; 32]),
            manifest_hash: BlobDigest::new([0x44; 32]),
            storage_ticket: StorageTicketId::new([0x55; 32]),
            pdp_commitment: None,
            stripe_layout: DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            },
            queued_at_unix: 1,
            rent_quote: DaRentQuote::default(),
            operator_signature: iroha_crypto::Signature::from_bytes(&[0u8; 64]),
        };
        let response_payload = DaIngestResponsePayload {
            status: "accepted".into(),
            duplicate: false,
            receipt: Some(receipt.clone()),
        };
        let response_body =
            norito::json::to_vec(&response_payload).expect("encode response payload");
        let response = HttpResponse::builder()
            .status(StatusCode::ACCEPTED)
            .header("content-type", APPLICATION_JSON)
            .header(PDP_COMMITMENT_HEADER, commitment_b64)
            .body(response_body.clone())
            .expect("response build");

        let result = with_mock_http(respond_with(&snapshots, response), || {
            client.submit_da_blob(payload.clone(), &params, metadata.clone(), None)
        })
        .expect("submit da blob");
        assert_eq!(result.status, "accepted");
        assert!(!result.duplicate);
        assert_eq!(result.payload_len, payload.len());
        let expected_blob = BlobDigest::from_hash(blake3::hash(&payload));
        assert_eq!(result.client_blob_id, expected_blob);
        assert_eq!(result.receipt, Some(receipt));
        let receipt_json = result.receipt_json.expect("receipt json");
        assert!(receipt_json.contains("queued_at_unix"));
        assert!(result.pdp_commitment.is_some());
        assert_eq!(
            result.client_blob_id.as_ref(),
            blake3::hash(&payload).as_bytes()
        );

        let store = snapshots.lock().expect("lock snapshots");
        assert_eq!(store.len(), 1);
        let snapshot = &store[0];
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert_eq!(snapshot.url.path(), "/v1/da/ingest");
        assert!(
            snapshot
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("accept")
                    && value == APPLICATION_JSON),
            "missing Accept header"
        );
        let echoed: DaIngestRequest =
            norito::json::from_slice(&snapshot.body).expect("decode echoed request");
        assert_eq!(echoed.payload, payload);
        assert_eq!(echoed.lane_id, params.lane_id);
    }

    #[test]
    fn write_da_ingest_request_persists_request_files() {
        let client = client_with_base_url(base_url());
        let params = DaIngestParams::default();
        let dir = tempdir().expect("tempdir");

        let paths = client
            .write_da_ingest_request(
                vec![0xDE, 0xAD],
                &params,
                ExtraMetadata::default(),
                None,
                dir.path(),
            )
            .expect("persist DA request");

        assert!(paths.request_raw.exists());
        assert!(paths.request_json.exists());
        assert!(paths.receipt_raw.is_none());
        assert!(paths.receipt_json.is_none());
        assert!(paths.response_headers_json.is_none());

        let raw_bytes = fs::read(&paths.request_raw).expect("read request bytes");
        let decoded: DaIngestRequest =
            decode_from_bytes(&raw_bytes).expect("decode persisted request");
        assert_eq!(decoded.chunk_size, params.chunk_size);
        let lane_id = decoded.lane_id;
        assert_eq!(lane_id.as_u32(), params.lane_id.as_u32());

        let request_json = fs::read(&paths.request_json).expect("read request json");
        let value: JsonValue = norito::json::from_slice(&request_json).expect("parse request json");
        assert_eq!(value.get("total_size").and_then(JsonValue::as_u64), Some(2));
    }

    #[test]
    fn submit_da_blob_to_dir_persists_cli_artifacts() {
        use base64::Engine as _;

        let client = client_with_base_url(base_url());
        let params = DaIngestParams::default();
        let payload = vec![0x01, 0x02, 0x03];
        let dir = tempdir().expect("tempdir");
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let commitment =
            inline_pdp_commitment(7, [0xAA; 32], ([0xBB; 32], [0xCC; 32]), (2, 1), 2, 99);
        let commitment_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&commitment).expect("encode commitment"));
        let receipt = sample_ingest_receipt();
        let response_payload = DaIngestResponsePayload {
            status: "accepted".into(),
            duplicate: false,
            receipt: Some(receipt.clone()),
        };
        let response_body =
            norito::json::to_vec(&response_payload).expect("encode response payload");
        let response = HttpResponse::builder()
            .status(StatusCode::ACCEPTED)
            .header("content-type", APPLICATION_JSON)
            .header(PDP_COMMITMENT_HEADER, commitment_b64.clone())
            .body(response_body.clone())
            .expect("response build");

        let (result, paths) = with_mock_http(respond_with(&snapshots, response), || {
            client.submit_da_blob_to_dir(
                payload.clone(),
                &params,
                ExtraMetadata::default(),
                None,
                dir.path(),
            )
        })
        .expect("submit da blob");

        assert!(paths.request_raw.exists());
        assert!(paths.request_json.exists());
        assert!(paths.receipt_raw.as_ref().is_some_and(|path| path.exists()));
        assert!(
            paths
                .receipt_json
                .as_ref()
                .is_some_and(|path| path.exists())
        );
        assert!(
            paths
                .response_headers_json
                .as_ref()
                .is_some_and(|path| path.exists())
        );

        let receipt_bytes =
            fs::read(paths.receipt_raw.as_ref().expect("receipt path")).expect("read receipt");
        let decoded: DaIngestReceipt =
            decode_from_bytes(&receipt_bytes).expect("decode persisted receipt");
        assert_eq!(decoded.client_blob_id, receipt.client_blob_id);

        let header_path = paths
            .response_headers_json
            .as_ref()
            .expect("headers path present");
        let header_json: JsonValue =
            norito::json::from_slice(&fs::read(header_path).expect("read headers json"))
                .expect("parse header json");
        assert_eq!(
            header_json
                .get(PDP_COMMITMENT_HEADER)
                .and_then(JsonValue::as_str),
            Some(commitment_b64.as_str())
        );
        assert_eq!(result.receipt, Some(receipt));

        let store = snapshots.lock().expect("lock snapshots");
        assert_eq!(store.len(), 1);
        assert_eq!(store[0].url.path(), "/v1/da/ingest");
    }

    #[test]
    fn build_da_proof_summary_from_session_emits_proofs() {
        let payload = vec![0xAB, 0xCD, 0xEF, 0x01];
        let bundle = manifest_bundle_from_payload(&payload);
        let session = sample_fetch_session(&payload);
        let (summary, applied) = Client::build_da_proof_summary_from_session(
            &bundle,
            &session,
            &DaProofConfig::default(),
        )
        .expect("proof summary");
        assert_eq!(applied.sample_count, DaProofConfig::default().sample_count);
        let expected_hash = hex::encode(blake3::hash(&payload).as_bytes());
        assert_eq!(
            summary.get("blob_hash").and_then(JsonValue::as_str),
            Some(expected_hash.as_str())
        );
        assert!(summary.get("proofs").is_some());
    }

    #[test]
    fn build_da_proof_summary_prefers_sampling_plan_indices() {
        let payload = vec![0x11, 0x22, 0x33, 0x44];
        let mut bundle = manifest_bundle_from_payload(&payload);
        bundle.sampling_plan = Some(DaSamplingPlan {
            assignment_hash: BlobDigest::new([0xCC; 32]),
            sample_window: 4,
            sample_seed: None,
            samples: vec![DaSampledChunk {
                index: 0,
                role: ChunkRole::Data,
                group: 0,
            }],
        });
        let session = sample_fetch_session(&payload);
        let (summary, applied) = Client::build_da_proof_summary_from_session(
            &bundle,
            &session,
            &DaProofConfig::default(),
        )
        .expect("proof summary with sampling plan");
        assert_eq!(applied.sample_count, 0);
        assert_eq!(applied.leaf_indexes, vec![0]);
        assert!(summary.get("proofs").is_some());
    }

    #[test]
    fn build_da_car_plan_matches_manifest() {
        let (bundle, payload) = sample_da_manifest_bundle();
        let manifest = bundle.decode_manifest().expect("manifest decode");
        let plan = Client::build_da_car_plan(&bundle).expect("car plan");
        assert_eq!(plan.chunk_profile.min_size, manifest.chunk_size as usize);
        assert_eq!(plan.chunk_profile.target_size, manifest.chunk_size as usize);
        assert_eq!(plan.chunk_profile.max_size, manifest.chunk_size as usize);
        assert_eq!(plan.content_length, manifest.total_size);
        assert_eq!(plan.content_length, payload.len() as u64);
        assert_eq!(plan.chunks.len(), manifest.chunks.len());
        let first_chunk = plan.chunks.first().expect("chunk entry");
        let expected_chunk = manifest.chunks.first().expect("manifest chunk");
        assert_eq!(first_chunk.offset, expected_chunk.offset);
        assert_eq!(first_chunk.length, expected_chunk.length);
        assert_eq!(first_chunk.digest, *expected_chunk.commitment.as_ref());
        assert_eq!(plan.files.len(), 1);
        let file = &plan.files[0];
        assert_eq!(file.path, vec!["payload.bin".to_owned()]);
        assert_eq!(file.chunk_count, manifest.chunks.len());
        assert_eq!(file.size, manifest.total_size);
    }

    #[test]
    fn prove_da_payload_validates_payload_and_emits_summary() {
        let (bundle, payload) = sample_da_manifest_bundle();
        let summary =
            Client::prove_da_payload(&bundle, &payload, &DaProofConfig::default()).expect("proof");
        let expected_hash = hex::encode(blake3::hash(&payload).as_bytes());
        assert_eq!(
            summary.get("blob_hash").and_then(JsonValue::as_str),
            Some(expected_hash.as_str())
        );
        assert!(summary.get("proofs").is_some());
    }

    #[test]
    fn prove_da_payload_rejects_mismatched_payload() {
        let (bundle, payload) = sample_da_manifest_bundle();
        let mut corrupted = payload.clone();
        if let Some(first) = corrupted.first_mut() {
            *first ^= 0xFF;
        } else {
            panic!("non-empty payload");
        }
        let err = Client::prove_da_payload(&bundle, &corrupted, &DaProofConfig::default())
            .expect_err("payload mismatch should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("payload hash mismatch") || msg.contains("ingest payload"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn get_da_manifest_bundle_with_block_hash_appends_query_and_parses_sampling() {
        let (mut bundle, _) = sample_da_manifest_bundle();
        let assignment_hash = BlobDigest::new([0xBB; 32]);
        bundle.sampling_plan = Some(DaSamplingPlan {
            assignment_hash,
            sample_window: 3,
            sample_seed: None,
            samples: vec![DaSampledChunk {
                index: 4,
                role: ChunkRole::GlobalParity,
                group: 2,
            }],
        });
        let response = manifest_bundle_response(&mut bundle);
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let block_hash_hex = hex::encode(Hash::new(b"block-seed").as_ref());

        let fetched = with_mock_http(respond_with(&snapshots, response), || {
            client.get_da_manifest_bundle_with_block_hash(
                &bundle.storage_ticket_hex,
                Some(&block_hash_hex),
            )
        })
        .expect("fetch manifest");

        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        let expected_query = format!("block_hash={block_hash_hex}");
        assert_eq!(
            snapshot.url.path(),
            format!("/v1/da/manifests/{}", bundle.storage_ticket_hex)
        );
        assert_eq!(snapshot.url.query(), Some(expected_query.as_str()));

        let sampling = fetched.sampling_plan.expect("sampling plan");
        assert_eq!(sampling.assignment_hash, assignment_hash);
        assert_eq!(sampling.sample_window, 3);
        assert_eq!(sampling.samples.len(), 1);
        let sample = &sampling.samples[0];
        assert_eq!(sample.index, 4);
        assert_eq!(sample.role, ChunkRole::GlobalParity);
        assert_eq!(sample.group, 2);
    }

    fn sample_gateway_fetch_inputs(
        bundle: &DaManifestBundle,
    ) -> (
        SorafsGatewayFetchConfig,
        Vec<SorafsGatewayProviderInput>,
        SorafsGatewayFetchOptions,
    ) {
        let gateway_config = SorafsGatewayFetchConfig {
            manifest_id_hex: bundle.storage_ticket_hex.clone(),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: Some("sdk-tests".into()),
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let providers = vec![SorafsGatewayProviderInput {
            name: "provider-1".into(),
            provider_id_hex: hex::encode([0x11u8; 32]),
            base_url: "https://gateway.test".into(),
            stream_token_b64: base64::engine::general_purpose::STANDARD.encode(b"stream-token"),
            privacy_events_url: None,
        }];
        let options = SorafsGatewayFetchOptions {
            telemetry_region: Some("test-region".into()),
            ..SorafsGatewayFetchOptions::default()
        };
        (gateway_config, providers, options)
    }

    #[test]
    fn prove_da_availability_uses_gateway_fetch_hook() {
        type FetchCall = (u64, usize, String, Option<String>, usize);

        let (mut bundle, payload) = sample_da_manifest_bundle();
        let response = manifest_bundle_response(&mut bundle);
        let client = client_with_base_url(base_url());

        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let fetch_calls: Arc<Mutex<Vec<FetchCall>>> = Arc::new(Mutex::new(Vec::new()));
        let (gateway_config, providers, options) = sample_gateway_fetch_inputs(&bundle);

        let proof = with_mock_http(respond_with(&snapshots, response), || {
            with_mock_sorafs_fetch(
                {
                    let payload = payload.clone();
                    let fetch_calls = Arc::clone(&fetch_calls);
                    move |plan, cfg, provider_inputs, opts| {
                        fetch_calls.lock().expect("lock fetch calls").push((
                            plan.content_length,
                            plan.chunks.len(),
                            cfg.manifest_id_hex.clone(),
                            opts.telemetry_region.clone(),
                            provider_inputs.len(),
                        ));
                        Ok(sample_fetch_session(&payload))
                    }
                },
                || {
                    tokio::runtime::Runtime::new().expect("runtime").block_on(
                        client.prove_da_availability(
                            &bundle.storage_ticket_hex,
                            gateway_config.clone(),
                            providers.clone(),
                            options.clone(),
                            DaProofConfig::default(),
                        ),
                    )
                },
            )
        })
        .expect("availability proof");

        assert_eq!(proof.manifest.storage_ticket_hex, bundle.storage_ticket_hex);
        assert_eq!(
            proof
                .fetch_session
                .outcome
                .chunks
                .first()
                .expect("chunk")
                .as_slice(),
            payload.as_slice()
        );
        let expected_hash = hex::encode(blake3::hash(&payload).as_bytes());
        assert_eq!(
            proof
                .proof_summary
                .get("blob_hash")
                .and_then(JsonValue::as_str),
            Some(expected_hash.as_str())
        );

        let calls = fetch_calls.lock().expect("lock fetch calls");
        assert_eq!(calls.len(), 1);
        let (content_length, chunk_count, manifest_id, region, provider_count) = &calls[0];
        assert_eq!(*content_length, payload.len() as u64);
        assert_eq!(*chunk_count, 1);
        assert_eq!(manifest_id, &gateway_config.manifest_id_hex);
        assert_eq!(region.as_deref(), Some("test-region"));
        assert_eq!(*provider_count, providers.len());

        let store = snapshots.lock().expect("lock snapshot store");
        assert_eq!(store.len(), 1);
        assert!(
            store[0]
                .url
                .path()
                .ends_with(&format!("/v1/da/manifests/{}", bundle.storage_ticket_hex))
        );
    }

    #[test]
    fn prove_da_availability_to_dir_persists_artifacts() {
        use base64::Engine as _;

        let (mut bundle, payload) = sample_da_manifest_bundle();
        let response = manifest_bundle_response(&mut bundle);
        let client = client_with_base_url(base_url());
        let dir = tempdir().expect("tempdir");
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let gateway_config = SorafsGatewayFetchConfig {
            manifest_id_hex: bundle.storage_ticket_hex.clone(),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: Some("sdk-tests".into()),
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let providers = vec![SorafsGatewayProviderInput {
            name: "provider-1".into(),
            provider_id_hex: hex::encode([0x11u8; 32]),
            base_url: "https://gateway.test".into(),
            stream_token_b64: base64::engine::general_purpose::STANDARD.encode(b"stream-token"),
            privacy_events_url: None,
        }];
        let proof_config = DaProofConfig {
            sample_count: 2,
            sample_seed: 99,
            leaf_indexes: vec![0],
        };

        let (proof, paths) = with_mock_http(respond_with(&snapshots, response), || {
            with_mock_sorafs_fetch(
                {
                    let payload = payload.clone();
                    move |_plan, _cfg, _providers, _opts| Ok(sample_fetch_session(&payload))
                },
                || {
                    tokio::runtime::Runtime::new().expect("runtime").block_on(
                        client.prove_da_availability_to_dir(
                            &bundle.storage_ticket_hex,
                            gateway_config.clone(),
                            providers.clone(),
                            SorafsGatewayFetchOptions::default(),
                            proof_config.clone(),
                            dir.path(),
                        ),
                    )
                },
            )
        })
        .expect("persisted proof");

        assert!(paths.manifest.manifest_raw.exists());
        assert!(paths.manifest.manifest_json.exists());
        assert!(paths.manifest.chunk_plan.exists());
        assert!(paths.payload_path.exists());
        assert!(paths.proof_summary_path.exists());

        let payload_disk = fs::read(&paths.payload_path).expect("read payload");
        assert_eq!(payload_disk, payload);

        let summary_json = fs::read_to_string(&paths.proof_summary_path).expect("summary json");
        let summary: JsonValue =
            norito::json::from_str(&summary_json).expect("parse proof summary json");
        let expected_manifest_path = paths.manifest.manifest_raw.display().to_string();
        assert_eq!(
            summary.get("manifest_path").and_then(JsonValue::as_str),
            Some(expected_manifest_path.as_str())
        );
        assert_eq!(proof.proof_config.sample_count, proof_config.sample_count);
        assert!(
            summary
                .get("proofs")
                .and_then(JsonValue::as_array)
                .is_some()
        );
        let expected_scoreboard = dir.path().join("scoreboard.json");
        assert_eq!(
            paths.scoreboard_path.as_deref(),
            Some(expected_scoreboard.as_path())
        );
    }

    #[test]
    fn prove_da_availability_to_dir_honours_existing_scoreboard_path() {
        use base64::Engine as _;

        let (mut bundle, payload) = sample_da_manifest_bundle();
        let response = manifest_bundle_response(&mut bundle);
        let client = client_with_base_url(base_url());
        let dir = tempdir().expect("tempdir");
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let gateway_config = SorafsGatewayFetchConfig {
            manifest_id_hex: bundle.storage_ticket_hex.clone(),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: Some("sdk-tests".into()),
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let providers = vec![SorafsGatewayProviderInput {
            name: "provider-1".into(),
            provider_id_hex: hex::encode([0x11u8; 32]),
            base_url: "https://gateway.test".into(),
            stream_token_b64: base64::engine::general_purpose::STANDARD.encode(b"stream-token"),
            privacy_events_url: None,
        }];
        let scoreboard_override = dir.path().join("custom_scoreboard.json");
        let fetch_options = SorafsGatewayFetchOptions {
            scoreboard: Some(SorafsGatewayScoreboardOptions {
                persist_path: Some(scoreboard_override.clone()),
                now_unix_secs: Some(7),
                metadata: None,
                telemetry_source_label: Some("custom".into()),
            }),
            ..Default::default()
        };

        let (_proof, paths) = with_mock_http(respond_with(&snapshots, response), || {
            with_mock_sorafs_fetch(
                {
                    let payload = payload.clone();
                    let scoreboard_override = scoreboard_override.clone();
                    move |_plan, _cfg, _providers, options| {
                        assert_eq!(
                            options
                                .scoreboard
                                .as_ref()
                                .and_then(|opt| opt.persist_path.as_ref()),
                            Some(&scoreboard_override)
                        );
                        Ok(sample_fetch_session(&payload))
                    }
                },
                || {
                    tokio::runtime::Runtime::new().expect("runtime").block_on(
                        client.prove_da_availability_to_dir(
                            &bundle.storage_ticket_hex,
                            gateway_config.clone(),
                            providers.clone(),
                            fetch_options.clone(),
                            DaProofConfig::default(),
                            dir.path(),
                        ),
                    )
                },
            )
        })
        .expect("prove da availability with scoreboard override");

        assert_eq!(
            paths.scoreboard_path.as_deref(),
            Some(scoreboard_override.as_path())
        );
    }

    #[test]
    fn fetch_da_manifest_to_dir_persists_outputs() {
        let (mut bundle, _) = sample_da_manifest_bundle();
        let response = manifest_bundle_response(&mut bundle);

        let client = client_with_base_url(base_url());
        let dir = tempdir().expect("tempdir");
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let paths = with_mock_http(respond_with(&snapshots, response), || {
            client.fetch_da_manifest_to_dir(&format!("0x{}", bundle.storage_ticket_hex), dir.path())
        })
        .expect("persist manifest bundle");

        assert!(paths.manifest_raw.exists());
        assert!(paths.manifest_json.exists());
        assert!(paths.chunk_plan.exists());
        assert_eq!(
            fs::read(&paths.manifest_raw).expect("read manifest"),
            bundle.manifest_bytes
        );
        let manifest_json = fs::read_to_string(&paths.manifest_json).expect("read manifest json");
        let manifest_value: JsonValue =
            norito::json::from_slice(manifest_json.as_bytes()).expect("manifest json parses");
        assert!(manifest_value.is_object());
        let chunk_plan_json = fs::read_to_string(&paths.chunk_plan).expect("read chunk plan");
        let chunk_plan_value: JsonValue =
            norito::json::from_slice(chunk_plan_json.as_bytes()).expect("chunk plan json parses");
        assert!(chunk_plan_value.is_object());

        let store = snapshots.lock().expect("lock snapshot store");
        assert_eq!(store.len(), 1);
        assert!(
            store[0]
                .url
                .path()
                .ends_with(&format!("/v1/da/manifests/{}", bundle.storage_ticket_hex))
        );
    }

    #[test]
    fn decode_da_ingest_response_handles_missing_receipt() {
        let payload = DaIngestResponsePayload {
            status: "queued".into(),
            duplicate: true,
            receipt: None,
        };
        let bytes = norito::json::to_vec(&payload).expect("encode response payload");
        let (decoded, receipt_json, pdp_commitment) =
            Client::decode_da_ingest_response(&bytes, None).expect("decode response");
        assert_eq!(decoded.status, "queued");
        assert!(decoded.duplicate);
        assert!(decoded.receipt.is_none());
        assert!(receipt_json.is_none());
        assert!(pdp_commitment.is_none());
    }
    #[test]
    fn sorafs_gateway_fetch_config_defaults_apply_chain_region() {
        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions::default();
        let (config, max_peers) = client.build_sorafs_gateway_fetch_config(&options);
        let expected_region = client.chain.to_string();
        let default_retry = OrchestratorConfig::default().fetch.per_chunk_retry_limit;
        assert_eq!(
            config.telemetry_region.as_deref(),
            Some(expected_region.as_str())
        );
        assert_eq!(config.fetch.per_chunk_retry_limit, default_retry);
        assert!(max_peers.is_none());
        assert_eq!(config.anonymity_policy, AnonymityPolicy::GuardPq);
        assert_eq!(config.write_mode, WriteModeHint::ReadOnly);
    }

    #[test]
    fn sorafs_gateway_fetch_config_uses_config_stage() {
        let mut cfg = config_factory();
        cfg.sorafs_anonymity_policy = AnonymityPolicy::MajorityPq;
        let client = Client::new(cfg);
        let options = SorafsGatewayFetchOptions::default();
        let (config, _) = client.build_sorafs_gateway_fetch_config(&options);
        assert_eq!(config.anonymity_policy, AnonymityPolicy::MajorityPq);
    }

    #[test]
    fn sorafs_gateway_fetch_config_applies_overrides() {
        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions {
            retry_budget: Some(5),
            max_peers: Some(3),
            telemetry_region: Some("sea".to_string()),
            transport_policy: Some(TransportPolicy::DirectOnly),
            anonymity_policy: Some(AnonymityPolicy::StrictPq),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, max_peers) = client.build_sorafs_gateway_fetch_config(&options);
        assert_eq!(config.fetch.per_chunk_retry_limit, Some(5));
        assert_eq!(config.telemetry_region.as_deref(), Some("sea"));
        assert_eq!(max_peers, Some(3));
        assert_eq!(config.transport_policy, TransportPolicy::DirectOnly);
        assert_eq!(config.anonymity_policy, AnonymityPolicy::StrictPq);
    }

    #[test]
    fn sorafs_gateway_fetch_config_sanitises_zero_values() {
        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions {
            retry_budget: Some(0),
            max_peers: Some(0),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, max_peers) = client.build_sorafs_gateway_fetch_config(&options);
        let expected_region = client.chain.to_string();
        assert_eq!(
            config.telemetry_region.as_deref(),
            Some(expected_region.as_str())
        );
        assert_eq!(config.fetch.per_chunk_retry_limit, None);
        assert!(max_peers.is_none());
        assert_eq!(config.transport_policy, TransportPolicy::SoranetPreferred);
    }

    #[test]
    fn sorafs_gateway_fetch_config_enforces_pq_only_for_uploads() {
        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions {
            write_mode_hint: Some(WriteModeHint::UploadPqOnly),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, _) = client.build_sorafs_gateway_fetch_config(&options);
        assert_eq!(config.write_mode, WriteModeHint::UploadPqOnly);
        assert_eq!(config.transport_policy, TransportPolicy::SoranetStrict);
        assert_eq!(config.anonymity_policy, AnonymityPolicy::StrictPq);
    }

    #[test]
    fn sorafs_gateway_fetch_config_respects_explicit_policy_with_write_mode() {
        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions {
            transport_policy: Some(TransportPolicy::SoranetPreferred),
            anonymity_policy: Some(AnonymityPolicy::MajorityPq),
            write_mode_hint: Some(WriteModeHint::UploadPqOnly),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, _) = client.build_sorafs_gateway_fetch_config(&options);
        assert_eq!(config.write_mode, WriteModeHint::UploadPqOnly);
        assert_eq!(config.transport_policy, TransportPolicy::SoranetPreferred);
        assert_eq!(config.anonymity_policy, AnonymityPolicy::MajorityPq);
    }

    #[test]
    fn sorafs_gateway_fetch_config_includes_policy_override() {
        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions {
            policy_override: PolicyOverride::new(
                Some(TransportPolicy::SoranetStrict),
                Some(AnonymityPolicy::StrictPq),
            ),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, _) = client.build_sorafs_gateway_fetch_config(&options);
        assert_eq!(
            config.policy_override.transport_policy,
            Some(TransportPolicy::SoranetStrict)
        );
        assert_eq!(
            config.policy_override.anonymity_policy,
            Some(AnonymityPolicy::StrictPq)
        );
    }

    #[test]
    fn sorafs_gateway_fetch_config_matches_policy_override_fixture() {
        let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/sorafs_gateway/policy_override/override.json");
        let fixture_bytes = std::fs::read(&fixture_path).expect("read policy override fixture");
        let fixture: norito::json::Value =
            norito::json::from_slice(&fixture_bytes).expect("parse policy override fixture");
        let expected = fixture
            .get("policy_override")
            .cloned()
            .expect("fixture policy_override");

        let client = Client::new(config_factory());
        let options = SorafsGatewayFetchOptions {
            policy_override: PolicyOverride::new(
                Some(TransportPolicy::SoranetStrict),
                Some(AnonymityPolicy::StrictPq),
            ),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, _) = client.build_sorafs_gateway_fetch_config(&options);
        let serialized = sorafs_orchestrator::bindings::config_to_json(&config);
        let actual = serialized
            .get("policy_override")
            .cloned()
            .expect("config policy_override");
        assert_eq!(actual, expected);
    }

    #[test]
    fn sorafs_gateway_fetch_config_applies_scoreboard_overrides() {
        let client = Client::new(config_factory());
        let persist_path = PathBuf::from("/tmp/sorafs_scoreboard.json");
        let metadata = norito::json!({
            "capture_id": "unit-test",
            "fixture": "multi_peer_parity_v1"
        });
        let options = SorafsGatewayFetchOptions {
            scoreboard: Some(SorafsGatewayScoreboardOptions {
                persist_path: Some(persist_path.clone()),
                now_unix_secs: Some(42),
                metadata: Some(metadata.clone()),
                telemetry_source_label: None,
            }),
            ..SorafsGatewayFetchOptions::default()
        };
        let (config, _) = client.build_sorafs_gateway_fetch_config(&options);
        assert_eq!(config.scoreboard.persist_path.as_ref(), Some(&persist_path));
        assert_eq!(config.scoreboard.now_unix_secs, 42);
        let persisted = config
            .scoreboard
            .persist_metadata
            .clone()
            .expect("metadata persisted");
        let object = persisted
            .as_object()
            .expect("metadata should remain a JSON object");
        assert_eq!(
            object.get("capture_id"),
            metadata.as_object().and_then(|obj| obj.get("capture_id"))
        );
        assert_eq!(
            object.get("fixture"),
            metadata.as_object().and_then(|obj| obj.get("fixture"))
        );
        assert_eq!(
            object.get("telemetry_source").and_then(JsonValue::as_str),
            Some("chain:00000000-0000-0000-0000-000000000000")
        );
        assert_eq!(
            object.get("assume_now").and_then(JsonValue::as_u64),
            Some(42)
        );
    }

    #[test]
    fn gateway_scoreboard_metadata_records_manifest_context() {
        let mut metadata = ensure_scoreboard_metadata(None, Some("region:test"), 0);
        let config = SorafsGatewayFetchConfig {
            manifest_id_hex: "ABCDEF00ABCDEF00ABCDEF00ABCDEF00ABCDEF00ABCDEF00ABCDEF00ABCDEF00"
                .into(),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: Some("ZW52ZWxvcGU=".into()),
            client_id: Some("sdk".into()),
            expected_manifest_cid_hex: Some("C0FFEE".into()),
            blinded_cid_b64: Some("YmFzZQ".into()),
            salt_epoch: Some(7),
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };

        annotate_scoreboard_with_gateway_context(&mut metadata, &config);

        let object = metadata
            .as_object()
            .expect("metadata should stay a JSON object");
        assert_eq!(
            object
                .get("gateway_manifest_id")
                .and_then(JsonValue::as_str),
            Some("abcdef00abcdef00abcdef00abcdef00abcdef00abcdef00abcdef00abcdef00")
        );
        assert_eq!(
            object
                .get("gateway_manifest_cid")
                .and_then(JsonValue::as_str),
            Some("c0ffee")
        );
        assert_eq!(
            object
                .get("gateway_manifest_provided")
                .and_then(JsonValue::as_bool),
            Some(true)
        );
    }

    #[test]
    fn gateway_scoreboard_metadata_handles_missing_envelope_and_cid() {
        let mut metadata = ensure_scoreboard_metadata(None, Some("region:test"), 0);
        let config = SorafsGatewayFetchConfig {
            manifest_id_hex: "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface"
                .into(),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };

        annotate_scoreboard_with_gateway_context(&mut metadata, &config);

        let object = metadata
            .as_object()
            .expect("metadata should stay a JSON object");
        assert_eq!(
            object
                .get("gateway_manifest_id")
                .and_then(JsonValue::as_str),
            Some("feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface")
        );
        assert!(
            object
                .get("gateway_manifest_cid")
                .is_some_and(JsonValue::is_null)
        );
        assert_eq!(
            object
                .get("gateway_manifest_provided")
                .and_then(JsonValue::as_bool),
            Some(false)
        );
    }

    #[test]
    fn get_uaid_portfolio_parses_payload() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let uaid_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let payload = format!(
            r#"{{
  "uaid":"uaid:{uaid_hex}",
  "totals":{{"accounts":2,"positions":3}},
  "dataspaces":[
    {{
      "dataspace_id":7,
      "dataspace_alias":"retail",
      "accounts":[
        {{
          "account_id":"alice@nexus",
          "label":"primary",
          "assets":[
            {{
              "asset_id":"cash#nexus::alice@nexus",
              "asset_definition_id":"cash#nexus",
              "quantity":"500"
            }}
          ]
        }}
      ]
    }}
  ]
}}"#
        );
        let uaid_upper = uaid_hex.to_uppercase();
        let response = json_response(StatusCode::OK, &payload);
        let result = with_mock_http(respond_with(&snapshots, response), || {
            client.get_uaid_portfolio(&format!("UAID:{uaid_upper}"))
        })
        .expect("portfolio call succeeds");

        assert_eq!(result.uaid, format!("uaid:{uaid_hex}"));
        assert_eq!(result.totals.accounts, 2);
        assert_eq!(result.totals.positions, 3);
        assert_eq!(result.dataspaces.len(), 1);
        let dataspace = &result.dataspaces[0];
        assert_eq!(dataspace.dataspace_id, 7);
        assert_eq!(dataspace.dataspace_alias.as_deref(), Some("retail"));
        assert_eq!(dataspace.accounts.len(), 1);
        assert_eq!(dataspace.accounts[0].assets.len(), 1);
        assert_eq!(dataspace.accounts[0].assets[0].quantity, "500");

        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(
            snapshot.url.path(),
            format!("/v1/accounts/uaid:{uaid_hex}/portfolio")
        );
        assert!(
            snapshot
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("accept")
                    && value == APPLICATION_JSON),
            "missing Accept header"
        );
    }

    #[test]
    fn get_uaid_bindings_parses_payload() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let uaid_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let payload = format!(
            r#"{{
  "uaid":"uaid:{uaid_hex}",
  "dataspaces":[
    {{
      "dataspace_id":0,
      "dataspace_alias":"global",
      "accounts":["alice@global"]
    }},
    {{
      "dataspace_id":11,
      "dataspace_alias":"cbdc",
      "accounts":["wholesale@cbdc","ops@cbdc"]
    }}
  ]
}}"#
        );
        let response = json_response(StatusCode::OK, &payload);
        let result = with_mock_http(respond_with(&snapshots, response), || {
            client.get_uaid_bindings(&format!("uaid:{uaid_hex}"))
        })
        .expect("bindings call succeeds");
        assert_eq!(result.dataspaces.len(), 2);
        assert_eq!(result.dataspaces[1].accounts.len(), 2);

        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(
            snapshot.url.path(),
            format!("/v1/space-directory/uaids/uaid:{uaid_hex}")
        );
    }

    #[test]
    fn get_uaid_manifests_supports_query_and_parsing() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let uaid_hex = "0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";
        let manifest_hash = "b1".repeat(32);
        let manifest_json = include_str!(
            "../../../fixtures/space_directory/capability/cbdc_wholesale.manifest.json"
        );
        let payload = format!(
            r#"{{
  "uaid":"uaid:{uaid}",
  "total":1,
  "manifests":[
    {{
      "dataspace_id":11,
      "dataspace_alias":"cbdc",
      "manifest_hash":"{hash}",
      "status":"Active",
      "lifecycle":{{"activated_epoch":4097,"expired_epoch":null,"revocation":null}},
      "accounts":["wholesale@cbdc"],
      "manifest":{manifest}
    }}
  ]
}}"#,
            manifest = manifest_json.trim(),
            uaid = uaid_hex,
            hash = manifest_hash.as_str()
        );
        let response = json_response(StatusCode::OK, &payload);
        let query = UaidManifestQuery {
            dataspace_id: Some(11),
            status: Some(UaidManifestStatusFilter::Inactive),
            limit: Some(5),
            offset: Some(2),
            address_format: None,
        };
        let result = with_mock_http(respond_with(&snapshots, response), || {
            client.get_uaid_manifests(&format!("uaid:{uaid_hex}"), Some(query))
        })
        .expect("manifests call succeeds");

        assert_eq!(result.total, Some(1));
        assert_eq!(result.manifests.len(), 1);
        let record = &result.manifests[0];
        assert_eq!(record.status, UaidManifestStatus::Active);
        assert_eq!(record.manifest.entries.len(), 1);
        assert_eq!(record.lifecycle.activated_epoch, Some(4097));
        assert_eq!(record.manifest_hash, manifest_hash);

        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(
            snapshot.url.path(),
            format!("/v1/space-directory/uaids/uaid:{uaid_hex}/manifests")
        );
        assert_eq!(
            snapshot.url.query(),
            Some("dataspace=11&status=inactive&limit=5&offset=2")
        );
    }

    #[test]
    fn get_public_lane_validators_sets_address_format() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let response = json_response(StatusCode::OK, r#"{"lane_id":7,"total":0,"items":[]}"#);
        let snapshot_store = Arc::clone(&snapshots);
        let payload = with_mock_http(respond_with(&snapshot_store, response), || {
            client.get_public_lane_validators(LaneId::new(7), Some(AddressFormat::Compressed))
        })
        .expect("request succeeds");
        assert_eq!(payload["lane_id"].as_u64(), Some(7));
        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.url.path(), "/v1/nexus/public_lanes/7/validators");
        let has_compressed = snapshot
            .url
            .query_pairs()
            .any(|pair| pair == ("address_format".into(), "compressed".into()));
        assert!(has_compressed);
    }

    #[test]
    fn get_public_lane_stake_filters_validator() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let response = json_response(StatusCode::OK, r#"{"lane_id":1,"total":0,"items":[]}"#);
        let snapshot_store = Arc::clone(&snapshots);
        let payload = with_mock_http(respond_with(&snapshot_store, response), || {
            client.get_public_lane_stake(
                LaneId::new(1),
                Some("validator@lane"),
                Some(AddressFormat::Ih58),
            )
        })
        .expect("request succeeds");
        assert_eq!(payload["total"].as_u64(), Some(0));
        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.url.path(), "/v1/nexus/public_lanes/1/stake");
        let pairs: Vec<_> = snapshot.url.query_pairs().collect();
        assert!(pairs.contains(&("validator".into(), "validator@lane".into())));
        assert!(pairs.contains(&("address_format".into(), "ih58".into())));
    }

    #[test]
    fn get_public_lane_pending_rewards_sets_filters() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let response = json_response(StatusCode::OK, r#"{"lane_id":0,"total":0,"items":[]}"#);
        let snapshot_store = Arc::clone(&snapshots);
        let payload = with_mock_http(respond_with(&snapshot_store, response), || {
            client.get_public_lane_pending_rewards(
                LaneId::new(0),
                "validator@lane",
                Some(5),
                Some(AddressFormat::Compressed),
            )
        })
        .expect("request succeeds");
        assert_eq!(payload["total"].as_u64(), Some(0));
        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(
            snapshot.url.path(),
            "/v1/nexus/public_lanes/0/rewards/pending"
        );
        let pairs: Vec<_> = snapshot.url.query_pairs().collect();
        assert!(pairs.contains(&("account".into(), "validator@lane".into())));
        assert!(pairs.contains(&("upto_epoch".into(), "5".into())));
        assert!(pairs.contains(&("address_format".into(), "compressed".into())));
    }

    #[test]
    fn get_explorer_account_qr_parses_payload_and_applies_options() {
        let account_id = "alice@wonderland";
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let compressed_payload = r#"{
  "canonical_id":"alice@wonderland",
  "literal":"snx1aliceacct@wonderland",
  "address_format":"compressed",
  "network_prefix":73,
  "error_correction":"M",
  "modules":192,
  "qr_version":5,
  "svg":"<svg compressed />"
}"#;
        let response = json_response(StatusCode::OK, compressed_payload);
        let options = ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        };
        let snapshot_store = Arc::clone(&snapshots);
        let qr = with_mock_http(respond_with(&snapshot_store, response), || {
            client.get_explorer_account_qr(account_id, Some(options))
        })
        .expect("explorer QR request succeeds");
        assert_eq!(qr.canonical_id, account_id);
        assert_eq!(qr.literal, "snx1aliceacct@wonderland");
        assert_eq!(qr.address_format, AddressFormat::Compressed);
        assert_eq!(qr.network_prefix, 73);
        assert_eq!(qr.modules, 192);
        assert_eq!(qr.qr_version, 5);
        assert_eq!(qr.error_correction, "M");
        let snapshot = snapshots
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(
            snapshot.url.path(),
            format!("/v1/explorer/accounts/{account_id}/qr")
        );
        assert_eq!(snapshot.url.query(), Some("address_format=compressed"));
        assert!(
            snapshot.headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case("accept") && value == APPLICATION_JSON
            }),
            "request should set Accept: application/json"
        );

        // Default call should omit the query parameter and decode IH58 payloads.
        let snapshots_default: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let default_payload = r#"{
  "canonical_id":"alice@wonderland",
  "literal":"alice@wonderland",
  "address_format":"ih58",
  "network_prefix":73,
  "error_correction":"M",
  "modules":192,
  "qr_version":5,
  "svg":"<svg ih58 />"
}"#;
        let default_response = json_response(StatusCode::OK, default_payload);
        let default_qr = with_mock_http(respond_with(&snapshots_default, default_response), || {
            client.get_explorer_account_qr(account_id, None)
        })
        .expect("default explorer QR request succeeds");
        assert_eq!(default_qr.address_format, AddressFormat::Ih58);
        assert_eq!(default_qr.literal, account_id);
        let default_snapshot = snapshots_default
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("default snapshot captured");
        assert_eq!(default_snapshot.url.query(), None);
    }

    #[test]
    fn txs_same_except_for_nonce_have_different_hashes() {
        let client = Client::new(Config {
            transaction_add_nonce: true,
            ..config_factory()
        });

        let build_transaction =
            || client.build_transaction(Vec::<InstructionBox>::new(), Metadata::default());
        let tx1 = build_transaction();
        let tx2 = build_transaction();
        assert_ne!(tx1.hash(), tx2.hash());

        let tx2 = {
            let mut tx = TransactionBuilder::new(client.chain.clone(), client.account.clone())
                .with_executable(tx1.instructions().clone())
                .with_metadata(tx1.metadata().clone());

            tx.set_creation_time(tx1.creation_time());
            if let Some(nonce) = tx1.nonce() {
                tx.set_nonce(nonce);
            }
            if let Some(transaction_ttl) = client.transaction_ttl {
                tx.set_ttl(transaction_ttl);
            }

            client.sign_transaction(tx)
        };
        assert_eq!(tx1.hash(), tx2.hash());
    }

    #[test]
    fn submit_transaction_uses_norito_content_type_header() {
        let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let responder = respond_with(
            &store,
            HttpResponse::builder()
                .status(StatusCode::OK)
                .body(Vec::new())
                .expect("response build"),
        );

        with_mock_http(responder, || {
            let mut client = client_with_base_url(base_url());
            client
                .headers
                .insert("Content-Type".to_string(), "text/plain".to_string());
            let tx = client.build_transaction(Vec::<InstructionBox>::new(), Metadata::default());
            client
                .submit_transaction(&tx)
                .expect("transaction submission succeeds");
        });

        let snapshot = store
            .lock()
            .expect("snapshot lock")
            .first()
            .cloned()
            .expect("snapshot captured");
        let content_type_headers: Vec<_> = snapshot
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .collect();
        assert_eq!(
            content_type_headers.len(),
            1,
            "expected a single Content-Type header: {:?}",
            snapshot.headers
        );
        assert_eq!(
            content_type_headers[0].1.as_str(),
            APPLICATION_NORITO,
            "transaction requests must advertise Norito payloads"
        );
    }

    #[test]
    fn authorization_header() {
        let config = Config {
            basic_auth: Some(BasicAuth {
                web_login: LOGIN.parse().expect("Failed to create valid `WebLogin`"),
                password: SecretString::new(PASSWORD.to_owned()),
            }),
            ..config_factory()
        };
        let client = Client::with_headers(config, HashMap::new());

        let value = client
            .headers
            .get("Authorization")
            .expect("Expected `Authorization` header");
        let expected_value = format!("Basic {ENCRYPTED_CREDENTIALS}");
        assert_eq!(value, &expected_value);
    }

    #[test]
    fn transaction_response_handler_returns_err_on_bad_request() {
        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Vec::new())
            .unwrap();
        assert!(TransactionResponseHandler::handle(&response).is_err());
    }

    #[test]
    fn transaction_response_handler_accepts_accepted() {
        let response = Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Vec::new())
            .unwrap();
        assert!(TransactionResponseHandler::handle(&response).is_ok());
    }

    #[test]
    fn decode_status_response_returns_err_on_internal_server_error() {
        let response = Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::new())
            .expect("Failed to build response");
        assert!(Client::decode_status_for_test(&response).is_err());
    }

    #[test]
    fn decode_parameters_response_returns_err_on_bad_status() {
        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Vec::new())
            .expect("Failed to build response");
        assert!(decode_parameters_for_test(&response).is_err());
    }

    #[test]
    fn decode_parameters_response_parses_json_payload() {
        let params = iroha_data_model::parameter::Parameters::default();
        let body = norito::json::to_vec(&params).expect("serialize parameters");
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(body)
            .expect("build response");

        let decoded = decode_parameters_for_test(&response).expect("decode parameters");
        assert_eq!(decoded, params);
    }

    #[test]
    fn decode_status_allows_json_fallback() {
        use iroha_telemetry::metrics::{CryptoStatus, StackStatus, Status as S};
        // Minimal JSON body with required fields
        let body = norito::json::to_vec(&S {
            peers: 0,
            blocks: 0,
            blocks_non_empty: 0,
            commit_time_ms: 0,
            txs_approved: 0,
            txs_rejected: 0,
            uptime: Uptime(Duration::from_secs(0)),
            view_changes: 0,
            queue_size: 0,
            da_reschedule_total: 0,
            tx_gossip: TxGossipSnapshot::default(),
            crypto: CryptoStatus::default(),
            stack: StackStatus::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        })
        .unwrap();

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(body)
            .unwrap();

        let decoded = Client::decode_status_for_test(&response).expect("json fallback should work");
        assert_eq!(decoded.peers, 0);
    }

    #[test]
    fn decode_status_prefers_framed_payload() {
        use iroha_telemetry::metrics::Status as S;

        let status = S {
            peers: 5,
            blocks: 9,
            ..S::default()
        };

        let body = norito::to_bytes(&status).expect("serialize status");
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(body)
            .unwrap();

        let decoded = Client::decode_status_for_test(&response).expect("framed decode should work");
        assert_eq!(decoded.peers, status.peers);
        assert_eq!(decoded.blocks, status.blocks);
    }

    #[allow(clippy::too_many_lines)]
    fn base_sumeragi_status(
        block_header: &BlockHeader,
        settlement: LaneBlockCommitment,
        relay: LaneRelayEnvelope,
    ) -> SumeragiStatusWire {
        let block_hash = block_header.hash();
        SumeragiStatusWire {
            mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
            staged_mode_tag: None,
            staged_mode_activation_height: None,
            mode_activation_lag_blocks: None,
            mode_flip_kill_switch: true,
            mode_flip_blocked: false,
            mode_flip_success_total: 0,
            mode_flip_fail_total: 0,
            mode_flip_blocked_total: 0,
            last_mode_flip_timestamp_ms: None,
            last_mode_flip_error: None,
            consensus_caps: None,
            leader_index: 0,
            highest_qc_height: 0,
            highest_qc_view: 0,
            highest_qc_subject: None,
            locked_qc_height: 0,
            locked_qc_view: 0,
            locked_qc_subject: None,
            commit_certificate:
                iroha_data_model::block::consensus::SumeragiCommitCertificateStatus::default(),
            commit_quorum: iroha_data_model::block::consensus::SumeragiCommitQuorumStatus::default(
            ),
            view_change_proof_accepted_total: 0,
            view_change_proof_stale_total: 0,
            view_change_proof_rejected_total: 0,
            view_change_suggest_total: 0,
            view_change_install_total: 0,
            view_change_causes: SumeragiViewChangeCauseStatus::default(),
            gossip_fallback_total: 0,
            block_created_dropped_by_lock_total: 0,
            block_created_hint_mismatch_total: 0,
            block_created_proposal_mismatch_total: 0,
            validation_reject_total: 0,
            validation_reject_reason: None,
            validation_rejects: SumeragiValidationRejectStatus::default(),
            peer_key_policy: SumeragiPeerKeyPolicyStatus::default(),
            block_sync_roster: SumeragiBlockSyncRosterStatus::default(),
            pacemaker_backpressure_deferrals_total: 0,
            commit_pipeline_tick_total: 0,
            da_reschedule_total: 0,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: 0,
                last_targets: 0,
                last_dwell_ms: 0,
            },
            da_gate: SumeragiDaGateStatus {
                reason: SumeragiDaGateReason::None,
                last_satisfied: SumeragiDaGateSatisfaction::None,
                missing_local_data_total: 0,
                manifest_guard_total: 0,
            },
            kura_store: SumeragiKuraStoreStatus {
                failures_total: 0,
                abort_total: 0,
                last_retry_attempt: 0,
                last_retry_backoff_ms: 0,
                last_height: 0,
                last_view: 0,
                last_hash: None,
                ..Default::default()
            },
            rbc_store: SumeragiRbcStoreStatus {
                sessions: 0,
                bytes: 0,
                pressure_level: 0,
                backpressure_deferrals_total: 0,
                evictions_total: 0,
                recent_evictions: Vec::new(),
            },
            pending_rbc: SumeragiPendingRbcStatus::default(),
            tx_queue_depth: 0,
            tx_queue_capacity: 0,
            tx_queue_saturated: false,
            epoch_length_blocks: 0,
            epoch_commit_deadline_offset: 0,
            epoch_reveal_deadline_offset: 0,
            prf_epoch_seed: None,
            prf_height: 0,
            prf_view: 0,
            vrf_penalty_epoch: 0,
            vrf_committed_no_reveal_total: 0,
            vrf_no_participation_total: 0,
            vrf_late_reveals_total: 0,
            consensus_penalties_applied_total: 0,
            consensus_penalties_pending: 0,
            vrf_penalties_applied_total: 0,
            vrf_penalties_pending: 0,
            membership: SumeragiMembershipStatus {
                height: 0,
                view: 0,
                epoch: 0,
                view_hash: None,
            },
            membership_mismatch: SumeragiMembershipMismatchStatus::default(),
            lane_commitments: vec![iroha_data_model::block::consensus::SumeragiLaneCommitment {
                block_height: 1,
                lane_id: LaneId::new(0),
                tx_count: 0,
                total_chunks: 0,
                rbc_bytes_total: 0,
                teu_total: 0,
                block_hash,
            }],
            dataspace_commitments: vec![
                iroha_data_model::block::consensus::SumeragiDataspaceCommitment {
                    block_height: 1,
                    lane_id: LaneId::new(0),
                    dataspace_id: DataSpaceId::new(0),
                    tx_count: 0,
                    total_chunks: 0,
                    rbc_bytes_total: 0,
                    teu_total: 0,
                    block_hash,
                },
            ],
            lane_settlement_commitments: vec![settlement],
            lane_relay_envelopes: vec![relay],
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: vec![iroha_data_model::block::consensus::SumeragiLaneGovernance {
                lane_id: LaneId::new(0),
                alias: "single".to_owned(),
                governance: None,
                manifest_required: false,
                manifest_ready: false,
                manifest_path: None,
                validator_ids: Vec::new(),
                quorum: None,
                protected_namespaces: Vec::new(),
                runtime_upgrade: None,
            }],
            worker_loop: iroha_data_model::block::consensus::SumeragiWorkerLoopStatus::default(),
            commit_inflight:
                iroha_data_model::block::consensus::SumeragiCommitInflightStatus::default(),
        }
    }

    fn sample_sumeragi_status() -> SumeragiStatusWire {
        let block_header = BlockHeader::new(
            NonZeroU64::new(1).expect("nonzero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        let settlement = LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            tx_count: 0,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
        };
        let relay = LaneRelayEnvelope::new(block_header, None, None, settlement.clone(), 0)
            .expect("construct relay envelope");

        base_sumeragi_status(&block_header, settlement, relay)
    }

    #[test]
    fn get_sumeragi_status_prefers_norito_and_handles_json() {
        let status = sample_sumeragi_status();

        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let norito_body = norito::to_bytes(&status).expect("serialize status");
        let norito_response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(norito_body)
            .unwrap();
        let decoded = with_mock_http(respond_with(&snapshots, norito_response), || {
            client.get_sumeragi_status()
        })
        .expect("call succeeds");
        assert_eq!(decoded, status);
        let snapshot = snapshots
            .lock()
            .expect("lock snapshots")
            .first()
            .cloned()
            .expect("snapshot captured");
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(snapshot.url.path(), "/v1/sumeragi/status");
        let accept_header: HashMap<_, _> = snapshot
            .headers
            .iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
            .collect();
        assert_eq!(
            accept_header.get("accept"),
            Some(&APPLICATION_NORITO.to_owned()),
            "client should request Norito payloads for sumeragi status"
        );

        let json_snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let json_body =
            norito::json::to_vec(&status).expect("serialize sumeragi status to JSON payload");
        let json_response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .body(json_body)
            .unwrap();
        let decoded_json = with_mock_http(respond_with(&json_snapshots, json_response), || {
            client.get_sumeragi_status()
        })
        .expect("json call succeeds");
        assert_eq!(decoded_json, status);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn sumeragi_status_wire_roundtrip_to_json_preserves_fields() {
        use norito::json::Value;

        let subject_hash = Hash::prehashed([0xAA; Hash::LENGTH]);
        let highest = HashOf::<BlockHeader>::from_untyped_unchecked(subject_hash);
        let settlement = LaneBlockCommitment {
            block_height: 12,
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(7),
            tx_count: 1,
            total_local_micro: 500_000,
            total_xor_due_micro: 250_000,
            total_xor_after_haircut_micro: 240_000,
            total_xor_variance_micro: 10_000,
            swap_metadata: Some(LaneSwapMetadata {
                epsilon_bps: 35,
                twap_window_seconds: 120,
                liquidity_profile: LaneLiquidityProfile::Tier2,
                twap_local_per_xor: "123.456".to_owned(),
                volatility_class: LaneVolatilityClass::Elevated,
            }),
            receipts: vec![LaneSettlementReceipt {
                source_id: [0x11; 32],
                local_amount_micro: 500_000,
                xor_due_micro: 250_000,
                xor_after_haircut_micro: 240_000,
                xor_variance_micro: 10_000,
                timestamp_ms: 1_724_000_000_000,
            }],
        };
        let da_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0xDD; Hash::LENGTH],
        )));
        let mut block_header = BlockHeader::new(
            NonZeroU64::new(12).expect("nonzero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        block_header.set_da_commitments_hash(da_hash);
        let execution_qc = iroha_data_model::consensus::ExecutionQcRecord {
            subject_block_hash: block_header.hash(),
            parent_state_root: Hash::prehashed([0xEA; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0xEE; Hash::LENGTH]),
            height: block_header.height().get(),
            view: 5,
            epoch: 1,
            signers_bitmap: vec![0b1010_0001],
            bls_aggregate_signature: vec![0xAB; 48],
        };
        let relay_envelope = LaneRelayEnvelope::new(
            block_header,
            Some(execution_qc),
            da_hash,
            settlement.clone(),
            256,
        )
        .expect("construct lane relay envelope");
        let status = SumeragiStatusWire {
            mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
            staged_mode_tag: None,
            staged_mode_activation_height: None,
            mode_activation_lag_blocks: None,
            mode_flip_kill_switch: true,
            mode_flip_blocked: false,
            mode_flip_success_total: 0,
            mode_flip_fail_total: 0,
            mode_flip_blocked_total: 0,
            last_mode_flip_timestamp_ms: None,
            last_mode_flip_error: None,
            consensus_caps: None,
            leader_index: 2,
            highest_qc_height: 12,
            highest_qc_view: 5,
            highest_qc_subject: Some(highest),
            locked_qc_height: 11,
            locked_qc_view: 4,
            locked_qc_subject: None,
            commit_certificate:
                iroha_data_model::block::consensus::SumeragiCommitCertificateStatus::default(),
            commit_quorum: iroha_data_model::block::consensus::SumeragiCommitQuorumStatus::default(
            ),
            view_change_proof_accepted_total: 5,
            view_change_proof_stale_total: 6,
            view_change_proof_rejected_total: 7,
            view_change_suggest_total: 8,
            view_change_install_total: 9,
            view_change_causes: SumeragiViewChangeCauseStatus::default(),
            gossip_fallback_total: 3,
            block_created_dropped_by_lock_total: 1,
            block_created_hint_mismatch_total: 2,
            block_created_proposal_mismatch_total: 0,
            validation_reject_total: 0,
            validation_reject_reason: None,
            validation_rejects: SumeragiValidationRejectStatus::default(),
            peer_key_policy: SumeragiPeerKeyPolicyStatus::default(),
            block_sync_roster: SumeragiBlockSyncRosterStatus::default(),
            pacemaker_backpressure_deferrals_total: 6,
            commit_pipeline_tick_total: 0,
            da_reschedule_total: 0,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: 0,
                last_targets: 0,
                last_dwell_ms: 0,
            },
            da_gate: SumeragiDaGateStatus {
                reason: SumeragiDaGateReason::None,
                last_satisfied: SumeragiDaGateSatisfaction::None,
                missing_local_data_total: 0,
                manifest_guard_total: 0,
            },
            kura_store: SumeragiKuraStoreStatus {
                failures_total: 0,
                abort_total: 0,
                last_retry_attempt: 0,
                last_retry_backoff_ms: 0,
                last_height: 0,
                last_view: 0,
                last_hash: None,
                ..Default::default()
            },
            rbc_store: SumeragiRbcStoreStatus {
                sessions: 0,
                bytes: 0,
                pressure_level: 0,
                backpressure_deferrals_total: 0,
                evictions_total: 0,
                recent_evictions: Vec::new(),
            },
            pending_rbc: SumeragiPendingRbcStatus::default(),
            tx_queue_depth: 7,
            tx_queue_capacity: 20,
            tx_queue_saturated: true,
            epoch_length_blocks: 100,
            epoch_commit_deadline_offset: 60,
            epoch_reveal_deadline_offset: 80,
            prf_epoch_seed: Some([0xBB; 32]),
            prf_height: 12,
            prf_view: 5,
            vrf_penalty_epoch: 3,
            vrf_committed_no_reveal_total: 1,
            vrf_no_participation_total: 0,
            vrf_late_reveals_total: 2,
            consensus_penalties_applied_total: 0,
            consensus_penalties_pending: 0,
            vrf_penalties_applied_total: 0,
            vrf_penalties_pending: 0,
            membership: SumeragiMembershipStatus {
                height: 9,
                view: 4,
                epoch: 2,
                view_hash: Some([0xCC; 32]),
            },
            membership_mismatch: SumeragiMembershipMismatchStatus::default(),
            lane_commitments: vec![iroha_data_model::block::consensus::SumeragiLaneCommitment {
                block_height: 12,
                lane_id: LaneId::new(1),
                tx_count: 4,
                total_chunks: 2,
                rbc_bytes_total: 128,
                teu_total: 42,
                block_hash: highest,
            }],
            dataspace_commitments: vec![
                iroha_data_model::block::consensus::SumeragiDataspaceCommitment {
                    block_height: 12,
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(7),
                    tx_count: 2,
                    total_chunks: 1,
                    rbc_bytes_total: 64,
                    teu_total: 21,
                    block_hash: highest,
                },
            ],
            lane_settlement_commitments: vec![settlement.clone()],
            lane_relay_envelopes: vec![relay_envelope.clone()],
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: vec![iroha_data_model::block::consensus::SumeragiLaneGovernance {
                lane_id: LaneId::new(1),
                alias: "alpha-lane".to_owned(),
                governance: Some("module.v1".to_owned()),
                manifest_required: true,
                manifest_ready: true,
                manifest_path: Some("/etc/iroha/lanes/alpha.toml".to_owned()),
                validator_ids: vec!["validator@test".to_owned()],
                quorum: Some(2),
                protected_namespaces: vec!["finance".to_owned()],
                runtime_upgrade: Some(
                    iroha_data_model::block::consensus::SumeragiRuntimeUpgradeHook {
                        allow: true,
                        require_metadata: true,
                        metadata_key: Some("upgrade_id".to_owned()),
                        allowed_ids: vec!["alpha-upgrade".to_owned()],
                    },
                ),
            }],
            worker_loop: iroha_data_model::block::consensus::SumeragiWorkerLoopStatus::default(),
            commit_inflight:
                iroha_data_model::block::consensus::SumeragiCommitInflightStatus::default(),
        };

        let json = sumeragi_status_json_payload(&status);
        let root = json
            .as_object()
            .expect("sumeragi status JSON root is an object");
        for (key, expected) in [
            ("leader_index", 2),
            ("view_change_proof_accepted_total", 5),
            ("view_change_proof_stale_total", 6),
            ("view_change_proof_rejected_total", 7),
            ("view_change_suggest_total", 8),
            ("view_change_install_total", 9),
            ("gossip_fallback_total", 3),
            ("block_created_dropped_by_lock_total", 1),
            ("block_created_hint_mismatch_total", 2),
            ("block_created_proposal_mismatch_total", 0),
            ("pacemaker_backpressure_deferrals_total", 6),
            ("commit_pipeline_tick_total", 0),
        ] {
            assert_eq!(
                root.get(key).and_then(Value::as_u64),
                Some(expected),
                "{key} mismatch"
            );
        }
        assert_eq!(
            root.get("lane_governance_sealed_total")
                .and_then(Value::as_u64),
            Some(0),
            "lane_governance_sealed_total mismatch"
        );
        assert_eq!(
            root.get("lane_governance_sealed_aliases")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0),
            "lane_governance_sealed_aliases mismatch"
        );

        let lane_governance = root
            .get("lane_governance")
            .and_then(Value::as_array)
            .expect("lane governance array");
        assert_eq!(
            lane_governance.len(),
            1,
            "expected one lane governance entry"
        );
        let lane_entry = lane_governance[0]
            .as_object()
            .expect("lane governance entry object");
        assert_eq!(
            lane_entry.get("lane_id").and_then(Value::as_u64),
            Some(1),
            "lane id mismatch"
        );
        assert_eq!(
            lane_entry.get("alias").and_then(Value::as_str),
            Some("alpha-lane"),
            "alias mismatch"
        );
        assert_eq!(
            lane_entry
                .get("governance")
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some("module.v1".to_owned()),
            "governance mismatch"
        );
        assert_eq!(
            lane_entry
                .get("manifest_path")
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some("/etc/iroha/lanes/alpha.toml".to_owned()),
            "manifest path mismatch"
        );
        assert_eq!(
            lane_entry.get("quorum").and_then(Value::as_u64),
            Some(2),
            "quorum mismatch"
        );
        let validator_ids = lane_entry
            .get("validator_ids")
            .and_then(Value::as_array)
            .expect("validator ids array");
        assert_eq!(
            validator_ids
                .first()
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some("validator@test".to_owned()),
            "validator id mismatch"
        );
        let runtime_hook = lane_entry
            .get("runtime_upgrade")
            .and_then(Value::as_object)
            .expect("runtime upgrade object");
        assert_eq!(
            runtime_hook.get("allow").and_then(Value::as_bool),
            Some(true),
            "runtime allow mismatch"
        );
        assert_eq!(
            runtime_hook
                .get("require_metadata")
                .and_then(Value::as_bool),
            Some(true),
            "runtime require_metadata mismatch"
        );
        assert_eq!(
            runtime_hook
                .get("metadata_key")
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some("upgrade_id".to_owned()),
            "runtime metadata key mismatch"
        );
        let allowed_ids = runtime_hook
            .get("allowed_ids")
            .and_then(Value::as_array)
            .expect("allowed ids array");
        assert_eq!(
            allowed_ids
                .first()
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some("alpha-upgrade".to_owned()),
            "allowed id mismatch"
        );
        let settlement_entries = root
            .get("lane_settlement_commitments")
            .and_then(Value::as_array)
            .expect("lane settlement entries array");
        assert_eq!(
            settlement_entries.len(),
            1,
            "lane settlement commitments length mismatch"
        );
        let expected_settlement =
            norito::json::to_value(&settlement).expect("serialize lane settlement commitment");
        assert_eq!(
            settlement_entries[0], expected_settlement,
            "lane settlement commitment mismatch"
        );

        let highest_qc = root
            .get("highest_qc")
            .and_then(Value::as_object)
            .expect("highest qc object");
        for (key, expected) in [("height", 12), ("view", 5)] {
            assert_eq!(
                highest_qc.get(key).and_then(Value::as_u64),
                Some(expected),
                "{key} mismatch"
            );
        }

        let epoch = root
            .get("epoch")
            .and_then(Value::as_object)
            .expect("epoch object");
        for (key, expected) in [
            ("length_blocks", 100u64),
            ("commit_deadline_offset", 60u64),
            ("reveal_deadline_offset", 80u64),
        ] {
            assert_eq!(
                epoch.get(key).and_then(Value::as_u64),
                Some(expected),
                "{key} mismatch"
            );
        }

        let membership = root
            .get("membership")
            .and_then(Value::as_object)
            .expect("membership object");
        for (key, expected) in [("height", 9u64), ("view", 4u64), ("epoch", 2u64)] {
            assert_eq!(
                membership.get(key).and_then(Value::as_u64),
                Some(expected),
                "{key} mismatch"
            );
        }
        let expected_membership_hash = bytes_to_hex(&[0xCC; 32]);
        assert_eq!(
            membership.get("view_hash").and_then(Value::as_str),
            Some(expected_membership_hash.as_str())
        );
        let membership_mismatch = root
            .get("membership_mismatch")
            .and_then(Value::as_object)
            .expect("membership mismatch object");
        assert!(
            membership_mismatch
                .get("active_peers")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
        );
        assert!(
            membership_mismatch
                .get("last_peer")
                .is_some_and(Value::is_null)
        );
        assert_eq!(
            membership_mismatch
                .get("last_height")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            membership_mismatch.get("last_view").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            membership_mismatch
                .get("last_epoch")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert!(
            membership_mismatch
                .get("last_local_hash")
                .is_some_and(Value::is_null)
        );
        assert!(
            membership_mismatch
                .get("last_remote_hash")
                .is_some_and(Value::is_null)
        );
        assert_eq!(
            membership_mismatch
                .get("last_timestamp_ms")
                .and_then(Value::as_u64),
            Some(0)
        );
        let expected_subject = format!("{highest}");
        assert_eq!(
            highest_qc.get("subject_block_hash").and_then(Value::as_str),
            Some(expected_subject.as_str())
        );
        assert!(
            root.get("lane_commitments")
                .and_then(Value::as_array)
                .is_some()
        );
        assert!(
            root.get("dataspace_commitments")
                .and_then(Value::as_array)
                .is_some()
        );
        let tx_queue = root
            .get("tx_queue")
            .and_then(Value::as_object)
            .expect("tx queue object");
        for (key, expected) in [("depth", 7), ("capacity", 20)] {
            assert_eq!(
                tx_queue.get(key).and_then(Value::as_u64),
                Some(expected),
                "{key} mismatch"
            );
        }
        assert_eq!(
            tx_queue.get("saturated").and_then(Value::as_bool),
            Some(true)
        );

        let worker_loop = root
            .get("worker_loop")
            .and_then(Value::as_object)
            .expect("worker loop object");
        let queue_diagnostics = worker_loop
            .get("queue_diagnostics")
            .and_then(Value::as_object)
            .expect("worker loop queue diagnostics object");
        let blocked_total = queue_diagnostics
            .get("blocked_total")
            .and_then(Value::as_object)
            .expect("blocked_total object");
        assert_eq!(
            blocked_total.get("vote_rx").and_then(Value::as_u64),
            Some(0)
        );
        let dropped_total = queue_diagnostics
            .get("dropped_total")
            .and_then(Value::as_object)
            .expect("dropped_total object");
        assert_eq!(
            dropped_total.get("vote_rx").and_then(Value::as_u64),
            Some(0)
        );

        let commit_inflight = root
            .get("commit_inflight")
            .and_then(Value::as_object)
            .expect("commit inflight object");
        assert_eq!(
            commit_inflight.get("active").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            commit_inflight.get("timeout_ms").and_then(Value::as_u64),
            Some(0)
        );
        let pause_queue_depths = commit_inflight
            .get("pause_queue_depths")
            .and_then(Value::as_object)
            .expect("commit pause queue depths object");
        assert_eq!(
            pause_queue_depths.get("vote_rx").and_then(Value::as_u64),
            Some(0)
        );

        let prf = root
            .get("prf")
            .and_then(Value::as_object)
            .expect("prf object present");
        for (key, expected) in [("height", 12), ("view", 5)] {
            assert_eq!(
                prf.get(key).and_then(Value::as_u64),
                Some(expected),
                "{key} mismatch"
            );
        }
        let expected_seed = bytes_to_hex(&[0xBB; 32]);
        assert_eq!(
            prf.get("epoch_seed").and_then(Value::as_str),
            Some(expected_seed.as_str())
        );
        assert_eq!(
            root.get("vrf_penalty_epoch").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            root.get("vrf_committed_no_reveal_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            root.get("vrf_no_participation_total")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            root.get("vrf_late_reveals_total").and_then(Value::as_u64),
            Some(2)
        );
    }

    #[test]
    fn get_sumeragi_status_wire_verifies_lane_relay_envelopes() {
        let client = client_with_base_url(base_url());
        let (status, relay_envelope) = sample_sumeragi_status_with_relay();
        assert_eq!(status.lane_relay_envelopes, vec![relay_envelope]);
        let body = norito::to_bytes(&status).expect("encode status payload");
        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(body)
            .unwrap();
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let decoded = with_mock_http(respond_with(&snapshots, response), || {
            client.get_sumeragi_status_wire()
        })
        .expect("decode sumeragi status");
        assert_eq!(decoded.lane_settlement_commitments.len(), 1);
        assert_eq!(decoded.lane_relay_envelopes.len(), 1);
    }

    #[test]
    fn get_sumeragi_status_wire_rejects_invalid_lane_relay_hash() {
        let client = client_with_base_url(base_url());
        let (mut status, relay_envelope) = sample_sumeragi_status_with_relay();
        let mut tampered = relay_envelope;
        tampered.settlement_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0xFF; Hash::LENGTH]));
        status.lane_relay_envelopes = vec![tampered];
        let body = norito::to_bytes(&status).expect("encode status payload");
        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(body)
            .unwrap();
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let result = with_mock_http(respond_with(&snapshots, response), || {
            client.get_sumeragi_status_wire()
        });
        assert!(result.is_err(), "tampered relay should be rejected");
    }

    #[test]
    fn get_cross_lane_transfer_proofs_returns_verified_envelopes() {
        let client = client_with_base_url(base_url());
        let (status, relay_envelope) = sample_sumeragi_status_with_relay();
        let body = norito::to_bytes(&status).expect("encode status payload");
        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(body)
            .unwrap();
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let proofs = with_mock_http(respond_with(&snapshots, response), || {
            client.get_cross_lane_transfer_proofs()
        })
        .expect("decode lane relays");
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0].envelope(), &relay_envelope);
    }

    #[test]
    fn get_cross_lane_transfer_proofs_rejects_duplicate_keys() {
        let client = client_with_base_url(base_url());
        let (mut status, relay_envelope) = sample_sumeragi_status_with_relay();
        status.lane_relay_envelopes = vec![relay_envelope.clone(), relay_envelope];
        let body = norito::to_bytes(&status).expect("encode status payload");
        let response = HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(body)
            .unwrap();
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

        let err = with_mock_http(respond_with(&snapshots, response), || {
            client.get_cross_lane_transfer_proofs()
        })
        .expect_err("duplicates should be rejected");
        assert!(
            err.to_string().contains("duplicate relay envelope"),
            "expected duplicate detection error, got {err:?}"
        );
    }

    #[test]
    fn sorafs_pin_filter_sets_query_params() {
        let client = Client::new(config_factory());
        let url = join_torii_url(&client.torii_url, "v1/sorafs/pin");
        let filter = SorafsPinListFilter {
            limit: Some(25),
            offset: Some(5),
            status: Some("approved"),
        };
        let request = filter
            .apply(client.default_request(HttpMethod::GET, url))
            .build()
            .expect("build request");
        assert_eq!(
            request.uri().query(),
            Some("limit=25&offset=5&status=approved")
        );
    }

    #[test]
    fn sorafs_alias_filter_sets_query_params() {
        let client = Client::new(config_factory());
        let url = join_torii_url(&client.torii_url, "v1/sorafs/aliases");
        let filter = SorafsAliasListFilter {
            limit: Some(10),
            offset: Some(3),
            namespace: Some("docs"),
            manifest_digest: Some("deadbeef"),
        };
        let request = filter
            .apply(client.default_request(HttpMethod::GET, url))
            .build()
            .expect("build request");
        assert_eq!(
            request.uri().query(),
            Some("limit=10&offset=3&namespace=docs&manifest_digest=deadbeef")
        );
    }

    #[test]
    fn sorafs_replication_filter_sets_query_params() {
        let client = Client::new(config_factory());
        let url = join_torii_url(&client.torii_url, "v1/sorafs/replication");
        let filter = SorafsReplicationListFilter {
            limit: Some(50),
            offset: Some(2),
            status: Some("completed"),
            manifest_digest: Some("abc123"),
        };
        let request = filter
            .apply(client.default_request(HttpMethod::GET, url))
            .build()
            .expect("build request");
        assert_eq!(
            request.uri().query(),
            Some("limit=50&offset=2&status=completed&manifest_digest=abc123")
        );
    }

    #[test]
    fn uaid_bindings_query_sets_address_format_param() {
        let client = Client::new(config_factory());
        let url = join_torii_url(&client.torii_url, "v1/space-directory/uaids/demo");
        let query = UaidBindingsQuery {
            address_format: Some(AddressFormat::Compressed),
        };
        let request = query
            .apply(client.default_request(HttpMethod::GET, url))
            .build()
            .expect("build request");
        assert_eq!(request.uri().query(), Some("address_format=compressed"));
    }

    #[test]
    fn canonicalize_uaid_literal_is_case_insensitive() {
        let suffix = "ABCDEF00".repeat(8);
        let literal = format!("UAID:{suffix}");
        let canonical =
            canonicalize_uaid_literal(&literal, "tests.uaid").expect("canonicalize literal");
        assert_eq!(canonical, format!("uaid:{}", suffix.to_ascii_lowercase()));
    }

    #[test]
    fn sumeragi_qc_snapshot_roundtrip_to_json_preserves_fields() {
        let qc = SumeragiQcSnapshot {
            highest_qc: SumeragiQcEntry {
                height: 14,
                view: 6,
                subject_block_hash: None,
            },
            locked_qc: SumeragiQcEntry {
                height: 13,
                view: 5,
                subject_block_hash: None,
            },
        };

        let json = sumeragi_qc_json_payload(qc);
        let highest = json
            .get("highest_qc")
            .and_then(norito::json::Value::as_object)
            .expect("highest qc object");
        assert_eq!(
            highest.get("height").and_then(norito::json::Value::as_u64),
            Some(14)
        );
        assert_eq!(
            highest.get("view").and_then(norito::json::Value::as_u64),
            Some(6)
        );
        let locked = json
            .get("locked_qc")
            .and_then(norito::json::Value::as_object)
            .expect("locked qc object");
        assert_eq!(
            locked.get("height").and_then(norito::json::Value::as_u64),
            Some(13)
        );
        assert_eq!(
            locked.get("view").and_then(norito::json::Value::as_u64),
            Some(5)
        );
    }

    #[test]
    fn build_da_proof_artifact_matches_cli_schema() {
        let (bundle, payload) = sample_da_manifest_bundle();
        let metadata = DaProofArtifactMetadata::new(
            "artifacts/sample_manifest.norito",
            "artifacts/sample_payload.car",
        );
        let artifact = Client::build_da_proof_artifact(
            &bundle,
            &payload,
            &DaProofConfig::default(),
            &metadata,
        )
        .expect("artifact");
        let map = artifact.as_object().expect("artifact object");
        assert_eq!(
            map.get("manifest_path").and_then(JsonValue::as_str),
            Some("artifacts/sample_manifest.norito")
        );
        assert_eq!(
            map.get("payload_path").and_then(JsonValue::as_str),
            Some("artifacts/sample_payload.car")
        );
        assert!(
            map.get("proofs")
                .and_then(JsonValue::as_array)
                .is_some_and(|array| !array.is_empty()),
            "proof array missing"
        );
    }

    #[test]
    fn write_da_proof_artifact_persists_json() {
        let (bundle, payload) = sample_da_manifest_bundle();
        let metadata =
            DaProofArtifactMetadata::new("manifests/latest.norito", "payloads/latest.car");
        let temp_dir = tempdir().expect("temp dir");
        let output_path = temp_dir.path().join("proofs/proof.json");
        Client::write_da_proof_artifact(
            &bundle,
            &payload,
            &DaProofConfig::default(),
            &metadata,
            &output_path,
            true,
        )
        .expect("write artifact");
        let contents = std::fs::read_to_string(&output_path).expect("read proof artefact");
        assert!(contents.contains("manifests/latest.norito"));
        assert!(contents.ends_with('\n'));
    }

    fn manifest_bundle_from_payload(payload: &[u8]) -> DaManifestBundle {
        let mut store = ChunkStore::new();
        store.ingest_bytes(payload);
        let profile = ErasureProfile::default();
        let data_shards = usize::from(profile.data_shards);
        let chunk_commitments = store
            .chunks()
            .iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let idx = u32::try_from(idx).expect("chunk index fits in u32");
                let stripe_id = u32::try_from(idx as usize / data_shards).unwrap_or(u32::MAX);
                ChunkCommitment::new_with_role(
                    idx,
                    chunk.offset,
                    chunk.length,
                    ChunkDigest::new(chunk.blake3),
                    ChunkRole::Data,
                    stripe_id,
                )
            })
            .collect::<Vec<_>>();
        let blob_hash = BlobDigest::new(*store.payload_digest().as_bytes());
        let chunk_root = BlobDigest::new(*store.por_tree().root());
        let chunk_size = chunk_commitments
            .first()
            .map_or(0, |commitment| commitment.length);
        let total_stripes = u32::try_from(
            chunk_commitments
                .len()
                .div_ceil(usize::from(profile.data_shards)),
        )
        .expect("stripe count fits in u32");
        let shards_per_stripe =
            u32::from(profile.data_shards.saturating_add(profile.parity_shards));
        let ticket_bytes = *blake3::hash(payload).as_bytes();
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: blob_hash,
            lane_id: LaneId::new(0),
            epoch: 0,
            blob_class: BlobClass::NexusLaneSidecar,
            codec: BlobCodec::new("application/octet-stream"),
            blob_hash,
            chunk_root,
            storage_ticket: StorageTicketId::new(ticket_bytes),
            total_size: payload.len() as u64,
            chunk_size,
            total_stripes,
            shards_per_stripe,
            erasure_profile: profile,
            retention_policy: RetentionPolicy::default(),
            rent_quote: DaRentQuote::default(),
            chunks: chunk_commitments,
            ipa_commitment: chunk_root,
            metadata: ExtraMetadata::default(),
            issued_at_unix: 0,
        };
        let manifest_bytes = norito::to_bytes(&manifest).expect("serialize manifest");
        DaManifestBundle {
            storage_ticket_hex: hex::encode(manifest.storage_ticket.as_ref()),
            client_blob_id_hex: hex::encode(manifest.client_blob_id.as_ref()),
            blob_hash_hex: hex::encode(manifest.blob_hash.as_ref()),
            chunk_root_hex: hex::encode(manifest.chunk_root.as_ref()),
            manifest_hash_hex: hex::encode(blake3::hash(&manifest_bytes).as_bytes()),
            lane_id: u64::from(manifest.lane_id.as_u32()),
            epoch: manifest.epoch,
            manifest_len: manifest_bytes.len() as u64,
            manifest_bytes,
            manifest_json: JsonValue::Null,
            chunk_plan: JsonValue::Object(JsonMap::new()),
            sampling_plan: None,
        }
    }

    fn sample_da_manifest_bundle() -> (DaManifestBundle, Vec<u8>) {
        let payload = vec![0xAB; 32];
        let bundle = manifest_bundle_from_payload(&payload);
        (bundle, payload)
    }

    fn manifest_bundle_response(bundle: &mut DaManifestBundle) -> HttpResponse<Vec<u8>> {
        let manifest: DaManifestV1 =
            norito::decode_from_bytes(&bundle.manifest_bytes).expect("decode manifest");
        if bundle.manifest_json.is_null() {
            bundle.manifest_json =
                norito::json::value::to_value(&manifest).expect("render manifest json");
        }
        if bundle.chunk_plan.is_null() {
            bundle.chunk_plan = JsonValue::Object(JsonMap::from_iter([(
                "chunk_fetch_specs".into(),
                JsonValue::Array(Vec::new()),
            )]));
        }
        let sampling_plan_value = bundle.sampling_plan.as_ref().map(|plan| {
            let role_label = |role: ChunkRole| match role {
                ChunkRole::Data => "data",
                ChunkRole::LocalParity => "local_parity",
                ChunkRole::GlobalParity => "global_parity",
                ChunkRole::StripeParity => "stripe_parity",
            };
            let mut map = JsonMap::new();
            map.insert(
                "assignment_hash".into(),
                JsonValue::String(hex::encode(plan.assignment_hash.as_bytes())),
            );
            map.insert(
                "sample_window".into(),
                JsonValue::from(u64::from(plan.sample_window)),
            );
            if let Some(seed) = plan.sample_seed {
                map.insert("sample_seed".into(), JsonValue::String(hex::encode(seed)));
            }
            map.insert(
                "samples".into(),
                JsonValue::Array(
                    plan.samples
                        .iter()
                        .map(|sample| {
                            JsonValue::Object(JsonMap::from_iter([
                                ("index".into(), JsonValue::from(u64::from(sample.index))),
                                (
                                    "role".into(),
                                    JsonValue::String(role_label(sample.role).to_owned()),
                                ),
                                ("group".into(), JsonValue::from(u64::from(sample.group))),
                            ]))
                        })
                        .collect(),
                ),
            );
            JsonValue::Object(map)
        });
        let manifest_b64 = base64::engine::general_purpose::STANDARD.encode(&bundle.manifest_bytes);
        let mut response_map = JsonMap::from_iter([
            (
                "storage_ticket".into(),
                JsonValue::String(bundle.storage_ticket_hex.clone()),
            ),
            (
                "client_blob_id".into(),
                JsonValue::String(bundle.client_blob_id_hex.clone()),
            ),
            (
                "blob_hash".into(),
                JsonValue::String(bundle.blob_hash_hex.clone()),
            ),
            (
                "chunk_root".into(),
                JsonValue::String(bundle.chunk_root_hex.clone()),
            ),
            (
                "manifest_hash".into(),
                JsonValue::String(bundle.manifest_hash_hex.clone()),
            ),
            ("lane_id".into(), JsonValue::from(bundle.lane_id)),
            ("epoch".into(), JsonValue::from(bundle.epoch)),
            ("manifest_len".into(), JsonValue::from(bundle.manifest_len)),
            ("manifest_norito".into(), JsonValue::String(manifest_b64)),
            ("manifest".into(), bundle.manifest_json.clone()),
            ("chunk_plan".into(), bundle.chunk_plan.clone()),
        ]);
        if let Some(plan_value) = sampling_plan_value {
            response_map.insert("sampling_plan".into(), plan_value);
        }
        let response_value = JsonValue::Object(response_map);
        HttpResponse::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .body(norito::json::to_vec(&response_value).expect("encode manifest response"))
            .expect("response build")
    }

    fn sample_fetch_session(payload: &[u8]) -> SorafsFetchOutcome {
        let provider = Arc::new(FetchProvider::new("provider-1"));
        let outcome = FetchOutcome {
            chunks: vec![payload.to_owned()],
            chunk_receipts: vec![ChunkReceipt {
                chunk_index: 0,
                provider: provider.id().clone(),
                attempts: 1,
                latency_ms: 0.0,
                bytes: u32::try_from(payload.len()).expect("payload length fits u32"),
            }],
            provider_reports: vec![ProviderReport {
                provider: provider.clone(),
                successes: 1,
                failures: 0,
                disabled: false,
            }],
        };
        SorafsFetchOutcome {
            outcome,
            policy_report: PolicyReport {
                policy: AnonymityPolicy::GuardPq,
                effective_policy: AnonymityPolicy::GuardPq,
                total_candidates: 1,
                pq_candidates: 1,
                selected_soranet_total: 1,
                selected_pq: 1,
                status: PolicyStatus::Met,
                fallback_reason: None,
            },
            local_proxy_manifest: None,
            car_verification: None,
            taikai_cache_stats: None,
            taikai_cache_queue: None,
        }
    }

    #[cfg(test)]
    mod join_torii_url {
        use url::Url;

        use super::*;

        fn do_test(url: &str, path: &str, expected: &str) {
            let url = Url::parse(url).unwrap();
            let actual = join_torii_url(&url, path);
            assert_eq!(actual.as_str(), expected);
        }

        #[test]
        fn path_with_slash() {
            do_test("https://iroha.jp/", "/query", "https://iroha.jp/query");
            do_test(
                "https://iroha.jp/peer-1/",
                "/query",
                "https://iroha.jp/peer-1/query",
            );
        }

        #[test]
        fn path_without_slash() {
            do_test("https://iroha.jp/", "query", "https://iroha.jp/query");
            do_test(
                "https://iroha.jp/peer-1/",
                "query",
                "https://iroha.jp/peer-1/query",
            );
        }

        #[test]
        #[should_panic(expected = "Torii url must end with trailing slash")]
        fn panic_if_url_without_trailing_slash() {
            do_test("https://iroha.jp/peer-1", "query", "should panic");
        }
    }

    #[test]
    fn tx_rejection_preserves_find_error() {
        use crate::data_model::{
            prelude::*,
            query::error::{FindError, QueryExecutionFail},
            transaction::error::TransactionRejectionReason,
        };

        let asset_def: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let reason = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            crate::data_model::isi::error::InstructionExecutionError::Query(
                QueryExecutionFail::Find(FindError::AssetDefinition(asset_def)),
            ),
        ));

        let err = tx_rejection_to_report(&reason);

        let needle = "Failed to find asset definition:";
        let mut found = err.to_string().contains(needle);
        let mut e: &(dyn std::error::Error + 'static) = err.as_ref();
        while !found {
            match e.source() {
                Some(src) => {
                    if src.to_string().contains(needle) {
                        found = true;
                        break;
                    }
                    e = src;
                }
                None => break,
            }
        }
        assert!(found, "error chain should contain find message: {err:?}");
    }
}

#[cfg(test)]
mod response_report {
    use super::*;

    #[test]
    fn with_msg_returns_err_on_non_utf8_body() {
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(vec![0xff])
            .unwrap();
        assert!(ResponseReport::with_msg("test", &response).is_err());
    }
}
