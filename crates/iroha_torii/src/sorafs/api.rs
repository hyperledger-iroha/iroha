#![cfg(feature = "app_api")]
#![allow(clippy::result_large_err)]

//! HTTP handlers for SoraFS discovery endpoints.

use std::{
    borrow::Cow,
    convert::{Infallible, TryInto},
    fs,
    net::SocketAddr,
    num::NonZeroU32,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use blake3::hash as blake3_hash;
use futures::StreamExt;
use hex::{ToHex, encode};
use http::header::{AGE, CACHE_CONTROL, HeaderName, RETRY_AFTER, WARNING};
use hyper::body::Body as HyperBody;
use iroha_core::state::StateReadOnly;
use iroha_crypto::HashOf;
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    da::{ingest::DaStripeLayout, manifest::ChunkRole},
};
use iroha_logger::{debug, error, warn};
use norito::json::{self, Map, Number, Value};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarWriter, FilePlan,
    fetch_plan::chunk_fetch_specs_to_json,
    por_json::sample_to_map,
    verifier::{CarVerifier, CarVerifyError},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    AdvertEndpoint, AdvertValidationError, CapabilityTlv, CapabilityType, EndpointKind,
    EndpointMetadata, EndpointMetadataKey, ManifestV1, PathDiversityPolicy, ProofStreamKind,
    ProofStreamRequestError, ProofStreamRequestV1, ProofStreamTier, ProviderAdvertBodyV1,
    ProviderAdvertV1, ProviderCapabilityRangeV1, QosHints, RendezvousTopic, StakePointer,
    StreamBudgetV1, StreamTokenBodyV1, TransportHintV1, TransportProtocol,
    capacity::CapacityTelemetryV1,
    chunker_registry,
    por::{AuditVerdictV1, PorChallengeV1, PorProofV1},
    potr::{PotrReceiptV1, PotrSignatureAlgorithm, PotrSignatureV1, PotrStatus},
};
use sorafs_node::{
    NodeStorageError, PorTrackerError,
    capacity::CapacityUsageSnapshot,
    metering::{FeeProjection, MeteringSnapshot},
    store::{ChunkRoleMetadata, StorageError as StorageBackendError, StoredManifest},
    telemetry::TelemetryError,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::{RwLock, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use urlencoding::decode;

use crate::{
    JsonBody, SharedAppState, json_entry, json_object,
    routing::MaybeTelemetry,
    sorafs::{
        AdmissionRegistry, AliasCacheEnforcement, AliasCachePolicyExt, AliasCachePolicyHttpExt,
        AliasProofEvaluationExt, AliasProofState, BLINDED_CID_LEN, CacheDecision,
        PinSubmissionPolicy, QuotaExceeded, SorafsAction, StreamTokenConcurrencyPermit,
        StreamTokenHeaderError, StreamTokenIssuerError, StreamTokenQuotaExceeded, TokenOverrides,
        decode_token_base64,
        discovery::{
            AdvertError, AdvertIngest, AdvertIngestResult, AdvertWarning, ProviderAdvertCache,
            capability_name,
        },
        encode_token_base64,
        gateway::{
            ClientFingerprint, DenylistKind, PerceptualMatchBasis, PerceptualObservation,
            PolicyViolation, RateLimitError, RequestContext, SORA_TLS_STATE_HEADER,
        },
        pin::PinAuthError,
        registry::{
            CapacitySnapshot, GovernanceSummary, ManifestLineageSummary, PinRegistryMetricsSummary,
            PinRegistrySnapshot, RegistryAlias, RegistryError, RegistryManifest,
            RegistryReplicationOrder, collect_pin_registry, collect_snapshot, lineage_to_json,
            record_pin_registry_metrics,
        },
    },
    utils::extractors::{ExtractAccept, JsonOnly, NoritoJson},
};

const HEADER_SORA_REQ_BLINDED_CID: &str = "sora-req-blinded-cid";
const HEADER_SORA_REQ_SALT_EPOCH: &str = "sora-req-salt-epoch";
const HEADER_SORA_REQ_NONCE: &str = "sora-req-nonce";
const HEADER_SORA_CONTENT_CID: &str = "sora-content-cid";
const BLINDED_PATH_PLACEHOLDER: &str = "~blinded";
const HEADER_DAG_SCOPE: &str = "dag-scope";
const HEADER_SORA_CHUNKER: &str = "x-sorafs-chunker";
const HEADER_SORA_NONCE: &str = "x-sorafs-nonce";
const HEADER_SORA_STREAM_TOKEN: &str = "x-sorafs-stream-token";
const HEADER_SORA_VERIFYING_KEY: &str = "x-sorafs-verifying-key";
const HEADER_SORA_CHUNK_RANGE: &str = "x-sora-chunk-range";
const HEADER_SORA_CHUNK_DIGEST: &str = "x-sorafs-chunk-digest";
const HEADER_SORA_NAME: &str = "sora-name";
const HEADER_SORA_PROOF: &str = "sora-proof";
const HEADER_SORA_MANIFEST_ENVELOPE: &str = "x-sorafs-manifest-envelope";
const HEADER_SORA_CID: &str = "sora-cid";
const HEADER_SORA_REGION: &str = "x-sorafs-region";
const HEADER_SORA_POLICY_TAGS: &str = "x-sorafs-policy-tags";
const HEADER_SORA_MODERATION_SLUGS: &str = "x-sorafs-moderation-slugs";
const HEADER_SORA_CACHE_TTL: &str = "x-sorafs-cache-ttl";
const HEADER_SORA_PROOF_STATUS: &str = "sora-proof-status";
const HEADER_SORA_CLIENT: &str = "x-sorafs-client";
const HEADER_SORA_TOKEN_ID: &str = "x-sorafs-token-id";
const HEADER_SORA_CLIENT_QUOTA_REMAINING: &str = "x-sorafs-client-quota-remaining";
const HEADER_SORA_REQUEST_ID: &str = "x-sorafs-request-id";
const HEADER_SORA_POTR_REQUEST: &str = "sora-potr-request";
const HEADER_SORA_POTR_RECEIPT: &str = "sora-potr-receipt";
const HEADER_SORA_POTR_STATUS: &str = "sora-potr-status";
const HEADER_SORA_PERCEPTUAL_HASH: &str = "x-sorafs-perceptual-hash";
const HEADER_SORA_PERCEPTUAL_EMBEDDING: &str = "x-sorafs-perceptual-embedding";
const MIME_CAR: &str = "application/vnd.ipld.car";
const MIME_OCTET_STREAM: &str = "application/octet-stream";
const TELEMETRY_ENDPOINT_CAR_RANGE: &str = "/v1/sorafs/storage/car/range";
const TELEMETRY_ENDPOINT_CHUNK: &str = "/v1/sorafs/storage/chunk";
const TELEMETRY_ENDPOINT_PROVIDER_ADVERT: &str = "/v1/sorafs/provider/advert";
const TELEMETRY_ENDPOINT_MANIFEST: &str = "/v1/sorafs/storage/manifest";
const TELEMETRY_ENDPOINT_PLAN: &str = "/v1/sorafs/storage/plan";
const TELEMETRY_ENDPOINT_POR_SAMPLE: &str = "/v1/sorafs/storage/por/sample";
const TELEMETRY_ENDPOINT_PROOF_STREAM: &str = "/v1/sorafs/proof/stream";
const RANGE_THROTTLE_REASON_QUOTA: &str = "quota";
const RANGE_THROTTLE_REASON_CONCURRENCY: &str = "concurrency";
const RANGE_THROTTLE_REASON_BYTE_RATE: &str = "byte_rate";
const RANGE_CAPABILITY_FEATURE_PROVIDERS: &str = "providers";
const RANGE_CAPABILITY_FEATURE_SPARSE: &str = "supports_sparse_offsets";
const RANGE_CAPABILITY_FEATURE_ALIGNMENT: &str = "requires_alignment";
const RANGE_CAPABILITY_FEATURE_MERKLE: &str = "supports_merkle_proof";
const RANGE_CAPABILITY_FEATURE_STREAM_BUDGET: &str = "stream_budget";
const RANGE_CAPABILITY_FEATURE_TRANSPORT_HINTS: &str = "transport_hints";

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct ResponseError(Box<Response>);

type ApiResult<T> = Result<T, ResponseError>;

impl From<Response> for ResponseError {
    fn from(response: Response) -> Self {
        Self(Box::new(response))
    }
}

impl ResponseError {
    fn into_response(self) -> Response {
        *self.0
    }

    fn status(&self) -> StatusCode {
        self.0.status()
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        ResponseError::into_response(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkerSupport {
    Supported,
    Unsupported,
    Unknown,
}

fn chunker_support_state(
    state: &SharedAppState,
    profile: &str,
) -> (ChunkerSupport, Option<[u8; 32]>, Vec<String>) {
    let usage = state.sorafs_node.capacity_usage();
    let provider_id = usage.provider_id;
    let supported_profiles = usage
        .chunkers
        .iter()
        .map(|entry| entry.handle.clone())
        .collect::<Vec<_>>();

    if provider_id.is_none() || usage.chunkers.is_empty() {
        return (ChunkerSupport::Unknown, provider_id, supported_profiles);
    }

    let supported = usage
        .chunkers
        .iter()
        .any(|entry| entry.handle.eq_ignore_ascii_case(profile));
    let support = if supported {
        ChunkerSupport::Supported
    } else {
        ChunkerSupport::Unsupported
    };

    (support, provider_id, supported_profiles)
}

#[allow(clippy::result_large_err)]
fn enforce_chunker_support_gateway(
    state: &SharedAppState,
    profile: &str,
    scope: &'static str,
) -> Result<(), Response> {
    if !state.sorafs_gateway_config.enforce_capabilities {
        return Ok(());
    }
    let (support, provider_id, supported_profiles) = chunker_support_state(state, profile);
    match support {
        ChunkerSupport::Supported => Ok(()),
        ChunkerSupport::Unsupported => {
            let supported_values = supported_profiles
                .iter()
                .map(|handle| Value::String(handle.clone()))
                .collect::<Vec<_>>();
            Err(gateway_refusal_response(
                state,
                StatusCode::NOT_ACCEPTABLE,
                "unsupported_chunker",
                format!("chunk profile {profile} is not enabled on this provider"),
                Some(profile),
                provider_id.as_ref(),
                scope,
                [
                    ("profile", Value::from(profile.to_string())),
                    ("supported_profiles", Value::Array(supported_values)),
                ],
            ))
        }
        ChunkerSupport::Unknown => Err(gateway_refusal_response(
            state,
            StatusCode::SERVICE_UNAVAILABLE,
            "chunker_support_unknown",
            "gateway cannot confirm chunk profile support for this provider",
            Some(profile),
            provider_id.as_ref(),
            scope,
            [("profile", Value::from(profile.to_string()))],
        )),
    }
}

fn registry_chunker_error(profile: &str, supported_profiles: &[String]) -> Response {
    let mut details = Map::new();
    details.insert("profile".into(), Value::String(profile.to_string()));
    let supported_values = supported_profiles
        .iter()
        .map(|handle| Value::String(handle.clone()))
        .collect();
    details.insert("supported_profiles".into(), Value::Array(supported_values));

    let mut body = Map::new();
    body.insert("error".into(), Value::String("unsupported_chunker".into()));
    body.insert(
        "message".into(),
        Value::String(format!(
            "chunk profile {profile} is not enabled on this provider"
        )),
    );
    body.insert("details".into(), Value::Object(details));
    (StatusCode::NOT_ACCEPTABLE, JsonBody(Value::Object(body))).into_response()
}

fn registry_chunker_unknown(profile: &str) -> Response {
    let mut details = Map::new();
    details.insert("profile".into(), Value::String(profile.to_string()));
    let mut body = Map::new();
    body.insert(
        "error".into(),
        Value::String("chunker_support_unknown".into()),
    );
    body.insert(
        "message".into(),
        Value::String("registry cannot confirm chunk profile support for this provider".into()),
    );
    body.insert("details".into(), Value::Object(details));
    (
        StatusCode::SERVICE_UNAVAILABLE,
        JsonBody(Value::Object(body)),
    )
        .into_response()
}

fn next_request_id() -> String {
    let counter = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:032x}{counter:016x}")
}

#[derive(Debug, Clone)]
struct PotrProbeParams {
    deadline_ms: u32,
    tier: ProofStreamTier,
    request_id: Option<[u8; 16]>,
    trace_id: Option<[u8; 16]>,
}

#[derive(Debug, Clone)]
struct PotrProbeContext {
    params: PotrProbeParams,
    wall_start: SystemTime,
    timer_start: Instant,
}

fn begin_potr_probe(
    headers: &HeaderMap,
    wall_start: SystemTime,
    timer_start: Instant,
) -> Result<Option<PotrProbeContext>, Response> {
    let Some(params) = parse_potr_request_header(headers)? else {
        return Ok(None);
    };
    Ok(Some(PotrProbeContext {
        params,
        wall_start,
        timer_start,
    }))
}

fn parse_potr_request_header(headers: &HeaderMap) -> Result<Option<PotrProbeParams>, Response> {
    let Some(raw_value) = headers.get(HEADER_SORA_POTR_REQUEST) else {
        return Ok(None);
    };
    let value_str = raw_value.to_str().map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            "sora-potr-request header must be valid ASCII",
        )
    })?;

    let mut deadline_ms = None;
    let mut tier = None;
    let mut request_id = None;
    let mut trace_id = None;

    for token in value_str.split(';') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, raw) = trimmed.split_once('=').ok_or_else(|| {
            json_error(StatusCode::BAD_REQUEST, "invalid sora-potr-request token")
        })?;
        let value = raw.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "deadline" => {
                deadline_ms = Some(parse_deadline_ms(value)?);
            }
            "tier" => {
                tier = Some(parse_potr_tier(value)?);
            }
            "request-id" => {
                request_id = Some(parse_hex_128bits(value, "request-id")?);
            }
            "trace-id" => {
                trace_id = Some(parse_hex_128bits(value, "trace-id")?);
            }
            unknown => {
                return Err(json_error(
                    StatusCode::BAD_REQUEST,
                    format!("unsupported sora-potr-request key `{unknown}`"),
                ));
            }
        }
    }

    let deadline_ms = deadline_ms.ok_or_else(|| {
        json_error(
            StatusCode::BAD_REQUEST,
            "sora-potr-request header missing deadline parameter",
        )
    })?;
    let tier = tier.ok_or_else(|| {
        json_error(
            StatusCode::BAD_REQUEST,
            "sora-potr-request header missing tier parameter",
        )
    })?;

    Ok(Some(PotrProbeParams {
        deadline_ms,
        tier,
        request_id,
        trace_id,
    }))
}

fn parse_deadline_ms(value: &str) -> Result<u32, Response> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.ends_with("ms") {
        let number = &lower[..lower.len() - 2];
        let parsed: u32 = number.parse().map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "deadline parameter must be a positive integer with `ms` or `s` suffix",
            )
        })?;
        if parsed == 0 {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "deadline must be greater than zero",
            ));
        }
        return Ok(parsed);
    }
    if lower.ends_with('s') {
        let number = &lower[..lower.len() - 1];
        let seconds: u64 = number.parse().map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "deadline parameter must be a positive integer with `ms` or `s` suffix",
            )
        })?;
        if seconds == 0 {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "deadline must be greater than zero",
            ));
        }
        let millis = seconds.saturating_mul(1_000);
        let clamped = u32::try_from(millis).unwrap_or(u32::MAX);
        return Ok(clamped);
    }
    Err(json_error(
        StatusCode::BAD_REQUEST,
        "deadline parameter must include `ms` or `s` suffix",
    ))
}

fn parse_potr_tier(value: &str) -> Result<ProofStreamTier, Response> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hot" => Ok(ProofStreamTier::Hot),
        "warm" => Ok(ProofStreamTier::Warm),
        "archive" => Ok(ProofStreamTier::Archive),
        other => Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("unsupported PoTR tier `{other}`"),
        )),
    }
}

fn parse_hex_128bits(value: &str, field: &'static str) -> Result<[u8; 16], Response> {
    let trimmed = value.trim();
    if trimmed.len() != 32 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("{field} must be a 32-character hex string"),
        ));
    }
    let bytes = hex::decode(trimmed).map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("{field} must be a 32-character hex string"),
        )
    })?;
    let mut array = [0u8; 16];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn system_time_to_millis(time: SystemTime, field: &'static str) -> Result<u64, Response> {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("{field} timestamp precedes unix epoch"),
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn finalize_potr_receipt(
    state: &SharedAppState,
    probe: &PotrProbeContext,
    manifest: &StoredManifest,
    provider_id: &[u8; 32],
    range_start: u64,
    range_end: u64,
    status: PotrStatus,
    latency_ms: u32,
    note: Option<String>,
) -> Result<(PotrReceiptV1, String, &'static str), Response> {
    let requested_at_ms = system_time_to_millis(probe.wall_start, "request")?;
    let responded_wall = SystemTime::now();
    let responded_at_ms = system_time_to_millis(responded_wall, "response")?;
    let recorded_at_ms = responded_at_ms;

    let manifest_digest = *manifest.manifest_digest();

    let mut receipt = PotrReceiptV1 {
        version: sorafs_manifest::potr::POTR_RECEIPT_VERSION_V1,
        manifest_digest,
        provider_id: *provider_id,
        tier: probe.params.tier,
        deadline_ms: probe.params.deadline_ms,
        latency_ms,
        status,
        requested_at_ms,
        responded_at_ms,
        recorded_at_ms,
        range_start,
        range_end,
        request_id: probe.params.request_id,
        trace_id: probe.params.trace_id,
        note,
        gateway_signature: None,
        provider_signature: None,
    };

    let issuer = state.stream_token_issuer.as_ref().ok_or_else(|| {
        json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "PoTR signing is disabled on this gateway",
        )
    })?;

    let receipt_bytes = norito::to_bytes(&receipt).map_err(|err| {
        debug!(?err, "failed to encode PoTR receipt");
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to encode PoTR receipt",
        )
    })?;

    let signature = issuer.sign_bytes(&receipt_bytes);
    receipt.gateway_signature = Some(PotrSignatureV1 {
        algorithm: PotrSignatureAlgorithm::Ed25519,
        public_key: issuer.verifying_key_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    });

    receipt.validate().map_err(|err| {
        error!(?err, "generated PoTR receipt failed validation");
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "generated PoTR receipt failed validation",
        )
    })?;

    let receipt_b64 = BASE64_STANDARD.encode(receipt_bytes);
    let status_label = match status {
        PotrStatus::Success => "success",
        PotrStatus::MissedDeadline => "missed_deadline",
        PotrStatus::ProviderError => "provider_error",
        PotrStatus::GatewayError => "gateway_error",
        PotrStatus::ClientCancelled => "client_cancelled",
    };

    Ok((receipt, receipt_b64, status_label))
}

fn potr_signature_to_json(signature: &PotrSignatureV1) -> Value {
    let mut sig_map = Map::new();
    let algorithm = match signature.algorithm {
        PotrSignatureAlgorithm::Ed25519 => "ed25519",
        PotrSignatureAlgorithm::Dilithium3 => "dilithium3",
    };
    sig_map.insert("algorithm".into(), Value::from(algorithm));
    sig_map.insert(
        "public_key_hex".into(),
        Value::from(encode(&signature.public_key)),
    );
    sig_map.insert(
        "signature_b64".into(),
        Value::from(BASE64_STANDARD.encode(&signature.signature)),
    );
    Value::Object(sig_map)
}

#[allow(clippy::too_many_arguments)]
fn gateway_refusal_response<I>(
    state: &SharedAppState,
    status: StatusCode,
    error_code: &'static str,
    reason: impl Into<Cow<'static, str>>,
    profile: Option<&str>,
    provider_id: Option<&[u8; 32]>,
    scope: &'static str,
    details: I,
) -> Response
where
    I: IntoIterator<Item = (&'static str, Value)>,
{
    let request_id = next_request_id();
    let reason_cow = reason.into();
    let provider_hex = provider_id.map(hex::encode);
    let profile_label = profile.unwrap_or("unknown");

    let mut details_map = json::Map::new();
    details_map.insert("request_id".into(), Value::from(request_id.clone()));
    for (key, value) in details {
        details_map.insert(key.into(), value);
    }
    let details_value = Value::Object(details_map.clone());

    let mut body_map = json::Map::new();
    body_map.insert("error".into(), Value::from(error_code));
    body_map.insert("reason".into(), Value::from(reason_cow.as_ref()));
    body_map.insert("details".into(), details_value.clone());
    let response_body = Value::Object(body_map);

    let mut response = (status, JsonBody(response_body)).into_response();
    response.headers_mut().insert(
        HeaderName::from_static(HEADER_SORA_REQUEST_ID),
        header_value(&request_id, HEADER_SORA_REQUEST_ID),
    );

    let status_code = status.as_u16();
    state.telemetry.with_metrics(|metrics| {
        metrics.record_sorafs_gateway_refusal(
            status_code,
            error_code,
            profile_label,
            provider_hex.as_deref().unwrap_or(""),
            scope,
        );
    });

    warn!(
        target: "sorafs_gateway",
        %request_id,
        error = error_code,
        reason = reason_cow.as_ref(),
        profile = profile_label,
        provider_id_hex = provider_hex.as_deref().unwrap_or(""),
        scope,
        status = status.as_u16(),
        details = ?details_map,
        "SoraFS gateway refusal emitted"
    );

    response
}

struct AliasPresentation {
    json: Value,
    evaluation: crate::sorafs::AliasProofEvaluation,
    decision: crate::sorafs::CacheDecision,
    status_label: String,
    alias_label: String,
    manifest_digest_hex: String,
    proof_b64: String,
}

fn prepare_alias_presentation(
    alias: &RegistryAlias,
    lineage: &ManifestLineageSummary,
    governance: &GovernanceSummary,
    policy: &crate::sorafs::AliasCachePolicy,
    enforcement: crate::sorafs::AliasCacheEnforcement,
    telemetry: &crate::routing::MaybeTelemetry,
    now_secs: u64,
) -> ApiResult<AliasPresentation> {
    let base = match alias.to_json() {
        Ok(value) => value,
        Err(err) => {
            error!(
                alias = alias.alias_label(),
                ?err,
                "failed to serialize alias JSON"
            );
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize alias record",
            )
            .into());
        }
    };

    let proof_b64 = alias.proof_b64().to_owned();
    let proof_bytes = match base64::engine::general_purpose::STANDARD.decode(proof_b64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => {
            error!(
                alias = alias.alias_label(),
                ?err,
                "failed to decode alias proof"
            );
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to decode alias proof",
            )
            .into());
        }
    };

    let proof_bundle = match crate::sorafs::decode_alias_proof(&proof_bytes) {
        Ok(bundle) => bundle,
        Err(err) => {
            error!(
                alias = alias.alias_label(),
                ?err,
                "alias proof failed structural validation"
            );
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid alias proof bundle",
            )
            .into());
        }
    };

    let evaluation = policy.evaluate(&proof_bundle, now_secs);
    let decision = crate::sorafs::alias_cache::evaluate_cache_decision(
        &evaluation,
        lineage,
        governance,
        enforcement,
        now_secs,
    );
    let status_label = decision.status_label(&evaluation);
    let result = match decision.outcome {
        crate::sorafs::CacheDecisionOutcome::Serve | crate::sorafs::CacheDecisionOutcome::Hold => {
            "success"
        }
        crate::sorafs::CacheDecisionOutcome::Refuse => "error",
    };
    telemetry.with_metrics(|metrics| {
        metrics.record_sorafs_alias_cache(result, &status_label, evaluation.age.as_secs_f64());
    });

    let mut map = match base {
        Value::Object(map) => map,
        other => {
            return Ok(AliasPresentation {
                json: other,
                evaluation,
                decision,
                status_label,
                alias_label: alias.alias_label().to_string(),
                manifest_digest_hex: alias.manifest_digest_hex().to_string(),
                proof_b64,
            });
        }
    };

    fn insert_json_field(
        map: &mut Map,
        alias_label: &str,
        key: &str,
        value: Result<Value, json::Error>,
    ) -> ApiResult<()> {
        match value {
            Ok(value) => {
                map.insert(key.into(), value);
                Ok(())
            }
            Err(err) => {
                error!(
                    alias = alias_label,
                    ?err,
                    "failed to serialize `{key}` field for alias response"
                );
                Err(json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to serialize alias metadata",
                )
                .into())
            }
        }
    }

    map.insert("cache_state".into(), Value::String(status_label.clone()));
    map.insert("status_label".into(), Value::String(status_label.clone()));
    map.insert("lineage".into(), lineage_to_json(lineage));
    map.insert(
        "cache_rotation_due".into(),
        Value::Bool(evaluation.rotation_due),
    );

    insert_json_field(
        &mut map,
        alias.alias_label(),
        "cache_age_seconds",
        json::to_value(&evaluation.age.as_secs()),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "proof_generated_at_unix",
        json::to_value(&evaluation.generated_at_unix),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "proof_expires_at_unix",
        json::to_value(&evaluation.expires_at_unix),
    )?;
    if let Some(expires_in) = evaluation.expires_in {
        insert_json_field(
            &mut map,
            alias.alias_label(),
            "proof_expires_in_seconds",
            json::to_value(&expires_in.as_secs()),
        )?;
    }
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "policy_positive_ttl_secs",
        json::to_value(&policy.positive_ttl_secs()),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "policy_refresh_window_secs",
        json::to_value(&policy.refresh_window_secs()),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "policy_hard_expiry_secs",
        json::to_value(&policy.hard_expiry_secs()),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "policy_rotation_max_age_secs",
        json::to_value(&policy.rotation_max_age_secs()),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "policy_successor_grace_secs",
        json::to_value(&policy.successor_grace_secs()),
    )?;
    insert_json_field(
        &mut map,
        alias.alias_label(),
        "policy_governance_grace_secs",
        json::to_value(&policy.governance_grace_secs()),
    )?;

    map.insert(
        "cache_evaluation".into(),
        build_cache_evaluation_json(&evaluation, &decision, policy),
    );

    map.insert(
        "cache_decision".into(),
        Value::String(decision_outcome_str(decision.outcome).to_owned()),
    );
    map.insert(
        "cache_reasons".into(),
        Value::Array(
            decision
                .reasons
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );

    Ok(AliasPresentation {
        json: Value::Object(map),
        evaluation,
        decision,
        status_label,
        alias_label: alias.alias_label().to_string(),
        manifest_digest_hex: alias.manifest_digest_hex().to_string(),
        proof_b64,
    })
}

fn decision_outcome_str(outcome: crate::sorafs::CacheDecisionOutcome) -> &'static str {
    match outcome {
        crate::sorafs::CacheDecisionOutcome::Serve => "serve",
        crate::sorafs::CacheDecisionOutcome::Hold => "hold",
        crate::sorafs::CacheDecisionOutcome::Refuse => "refuse",
    }
}

fn optional_rfc3339(unix: Option<u64>) -> Option<String> {
    let seconds = i64::try_from(unix?).ok()?;
    let timestamp = OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    timestamp.format(&Rfc3339).ok()
}

fn build_cache_evaluation_json(
    evaluation: &crate::sorafs::AliasProofEvaluation,
    decision: &crate::sorafs::CacheDecision,
    policy: &crate::sorafs::AliasCachePolicy,
) -> Value {
    let mut map = Map::new();
    map.insert(
        "decision".into(),
        Value::String(decision_outcome_str(decision.outcome).to_owned()),
    );
    map.insert(
        "reasons".into(),
        Value::Array(
            decision
                .reasons
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    map.insert(
        "ttl_expires_at".into(),
        optional_rfc3339(Some(evaluation.expires_at_unix))
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "ttl_expires_at_unix".into(),
        json::to_value(&evaluation.expires_at_unix).unwrap_or(Value::Null),
    );
    map.insert(
        "serve_until".into(),
        optional_rfc3339(decision.serve_until_unix)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "serve_until_unix".into(),
        json::to_value(&decision.serve_until_unix).unwrap_or(Value::Null),
    );
    map.insert(
        "successor".into(),
        successor_evaluation_json(&decision.successor),
    );
    map.insert(
        "governance".into(),
        governance_evaluation_json(&decision.governance),
    );
    map.insert(
        "policy_successor_grace_secs".into(),
        json::to_value(&policy.successor_grace_secs()).unwrap_or(Value::Null),
    );
    map.insert(
        "policy_governance_grace_secs".into(),
        json::to_value(&policy.governance_grace_secs()).unwrap_or(Value::Null),
    );
    Value::Object(map)
}

fn successor_evaluation_json(successor: &crate::sorafs::SuccessorAssessment) -> Value {
    let mut map = Map::new();
    map.insert("exists".into(), Value::Bool(successor.exists));
    map.insert(
        "head_hex".into(),
        successor
            .head_hex
            .as_ref()
            .map(|hex| Value::String(hex.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert("approved".into(), Value::Bool(successor.approved));
    map.insert(
        "approved_at".into(),
        optional_rfc3339(successor.approved_at_unix)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "approved_at_unix".into(),
        json::to_value(&successor.approved_at_unix).unwrap_or(Value::Null),
    );
    map.insert(
        "depth_to_head".into(),
        json::to_value(&successor.depth_to_head).unwrap_or(Value::Null),
    );
    map.insert(
        "anomalies".into(),
        Value::Array(
            successor
                .anomalies
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    Value::Object(map)
}

fn governance_evaluation_json(governance: &crate::sorafs::GovernanceAssessment) -> Value {
    let mut map = Map::new();
    map.insert(
        "ref_ids".into(),
        Value::Array(
            governance
                .ref_ids
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    map.insert("revoked".into(), Value::Bool(governance.revoked));
    map.insert("frozen".into(), Value::Bool(governance.frozen));
    map.insert("rotated".into(), Value::Bool(governance.rotated));
    let mut flags = Map::new();
    flags.insert("revoked".into(), Value::Bool(governance.revoked));
    flags.insert("frozen".into(), Value::Bool(governance.frozen));
    flags.insert("rotated".into(), Value::Bool(governance.rotated));
    map.insert("flags".into(), Value::Object(flags));
    map.insert(
        "effective_at_unix".into(),
        json::to_value(&governance.effective_at_unix).unwrap_or(Value::Null),
    );
    map.insert(
        "effective_at".into(),
        optional_rfc3339(governance.effective_at_unix)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    Value::Object(map)
}

fn decision_warning_header(decision: &crate::sorafs::CacheDecision) -> Option<HeaderValue> {
    if matches!(decision.outcome, crate::sorafs::CacheDecisionOutcome::Hold) {
        if has_reason(decision, "ApprovedSuccessorGrace") {
            return Some(HeaderValue::from_static(
                r#"199 - "alias proof successor grace""#,
            ));
        }
        if has_reason(decision, "GovernanceGrace") {
            return Some(HeaderValue::from_static(
                r#"199 - "alias proof governance grace""#,
            ));
        }
        if has_reason(decision, "RefreshWindow") {
            return Some(HeaderValue::from_static(
                r#"199 - "alias proof refresh in-flight""#,
            ));
        }
    }

    if matches!(
        decision.outcome,
        crate::sorafs::CacheDecisionOutcome::Refuse
    ) {
        if has_reason(decision, "ApprovedSuccessor") {
            return Some(HeaderValue::from_static(
                r#"110 - "alias proof superseded""#,
            ));
        }
        if has_reason(decision, "GovernanceRevoked") {
            return Some(HeaderValue::from_static(
                r#"110 - "alias proof governance revoked""#,
            ));
        }
        if has_reason(decision, "GovernanceFrozen") {
            return Some(HeaderValue::from_static(
                r#"110 - "alias proof governance frozen""#,
            ));
        }
        if has_reason(decision, "GovernanceRotated") {
            return Some(HeaderValue::from_static(
                r#"110 - "alias proof governance rotated""#,
            ));
        }
        if has_reason(decision, "ExpiredTTL") {
            return Some(HeaderValue::from_static(r#"110 - "alias proof stale""#));
        }
        if has_reason(decision, "HardExpired") {
            return Some(HeaderValue::from_static(r#"111 - "alias proof expired""#));
        }
    }

    None
}

fn status_code_for_decision(decision: &crate::sorafs::CacheDecision) -> StatusCode {
    if has_reason(decision, "GovernanceRevoked") {
        StatusCode::GONE
    } else if has_reason(decision, "GovernanceFrozen") {
        StatusCode::FORBIDDEN
    } else if has_reason(decision, "GovernanceRotated") {
        StatusCode::SERVICE_UNAVAILABLE
    } else if has_reason(decision, "ApprovedSuccessor") || has_reason(decision, "HardExpired") {
        StatusCode::PRECONDITION_FAILED
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

fn message_for_decision(decision: &crate::sorafs::CacheDecision) -> &'static str {
    if has_reason(decision, "GovernanceRevoked") {
        "alias proof revoked by governance"
    } else if has_reason(decision, "GovernanceFrozen") {
        "alias proof frozen by governance"
    } else if has_reason(decision, "GovernanceRotated") {
        "alias proof rotated; refresh required"
    } else if has_reason(decision, "ApprovedSuccessor") {
        "alias proof superseded; successor manifest active"
    } else if has_reason(decision, "HardExpired") {
        "alias proof expired; refresh required"
    } else if has_reason(decision, "ExpiredTTL") {
        "alias proof stale; refresh required"
    } else {
        "alias proof unavailable; refresh required"
    }
}

fn retry_after_header(seconds: u64) -> Option<HeaderValue> {
    if seconds == 0 {
        None
    } else {
        Some(
            HeaderValue::from_str(&seconds.to_string())
                .expect("retry-after header must be ASCII digits"),
        )
    }
}

fn has_reason(decision: &crate::sorafs::CacheDecision, needle: &str) -> bool {
    decision.reasons.iter().any(|reason| reason == needle)
}

fn alias_policy_error_response(
    alias: &AliasPresentation,
    policy: &crate::sorafs::AliasCachePolicy,
    status: StatusCode,
    message: &str,
) -> Response {
    let mut entries = vec![
        json_entry("error", message.to_string()),
        json_entry("alias", alias.alias_label.clone()),
        json_entry("cache_state", Value::String(alias.status_label.clone())),
        json_entry(
            "proof_generated_at_unix",
            json::to_value(&alias.evaluation.generated_at_unix).unwrap_or(Value::Null),
        ),
        json_entry(
            "proof_expires_at_unix",
            json::to_value(&alias.evaluation.expires_at_unix).unwrap_or(Value::Null),
        ),
        json_entry(
            "proof_age_seconds",
            json::to_value(&alias.evaluation.age.as_secs()).unwrap_or(Value::Null),
        ),
    ];
    if let Some(remaining) = alias.evaluation.expires_in {
        entries.push(json_entry(
            "proof_expires_in_seconds",
            json::to_value(&remaining.as_secs()).unwrap_or(Value::Null),
        ));
    }
    entries.push(json_entry(
        "cache_decision",
        Value::String(decision_outcome_str(alias.decision.outcome).to_owned()),
    ));
    entries.push(json_entry(
        "cache_reasons",
        Value::Array(
            alias
                .decision
                .reasons
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    ));
    entries.push(json_entry(
        "cache_serve_until_unix",
        json::to_value(&alias.decision.serve_until_unix).unwrap_or(Value::Null),
    ));
    entries.push(json_entry(
        "status_label",
        Value::String(alias.status_label.clone()),
    ));
    let response_body = json_object(entries);

    let mut response = (status, JsonBody(response_body)).into_response();
    let headers = response.headers_mut();
    if let Ok(name) = HeaderValue::from_str(&alias.alias_label) {
        headers.insert(HeaderName::from_static(HEADER_SORA_NAME), name);
    }
    if let Ok(label) = HeaderValue::from_str(&alias.status_label) {
        headers.insert(HeaderName::from_static(HEADER_SORA_PROOF_STATUS), label);
    }
    headers.insert(AGE, alias.evaluation.age_header());
    if let Some(warning) = decision_warning_header(&alias.decision) {
        headers.append(WARNING, warning);
    } else if let Some(warning) = alias.evaluation.warning_header() {
        headers.append(WARNING, warning);
    }
    let mut cache_control_set = false;
    if has_reason(&alias.decision, "GovernanceRevoked") {
        headers.insert(CACHE_CONTROL, policy.revocation_cache_control_header());
        cache_control_set = true;
        if let Some(retry) = retry_after_header(policy.revocation_ttl_secs()) {
            headers.insert(RETRY_AFTER, retry);
        }
    }
    if status == StatusCode::NOT_FOUND {
        headers.insert(CACHE_CONTROL, policy.negative_cache_control_header());
        cache_control_set = true;
        if let Some(retry) = retry_after_header(policy.negative_ttl_secs()) {
            headers.insert(RETRY_AFTER, retry);
        }
    }
    if status == StatusCode::SERVICE_UNAVAILABLE && !headers.contains_key(RETRY_AFTER) {
        if let Some(retry) = retry_after_header(policy.refresh_window_secs()) {
            headers.insert(RETRY_AFTER, retry);
        }
    }
    if !cache_control_set {
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::sorafs::{
        CacheDecision, CacheDecisionOutcome, GovernanceAssessment, SuccessorAssessment,
    };

    fn empty_decision() -> CacheDecision {
        CacheDecision {
            outcome: CacheDecisionOutcome::Serve,
            reasons: Vec::new(),
            serve_until_unix: None,
            successor: SuccessorAssessment {
                exists: false,
                head_hex: None,
                approved: false,
                approved_at_unix: None,
                depth_to_head: 0,
                anomalies: Vec::new(),
            },
            governance: GovernanceAssessment {
                ref_ids: Vec::new(),
                revoked: false,
                frozen: false,
                rotated: false,
                effective_at_unix: None,
            },
        }
    }

    fn decision_with(reason: &str, outcome: CacheDecisionOutcome) -> CacheDecision {
        let mut decision = empty_decision();
        decision.outcome = outcome;
        decision.reasons.push(reason.to_owned());
        decision
    }

    #[test]
    fn parse_cache_control_max_age_extracts_value() {
        assert_eq!(
            parse_cache_control_max_age("max-age=120, public"),
            Some(120)
        );
        assert_eq!(parse_cache_control_max_age("public, s-maxage=30"), None);
    }

    #[test]
    fn parse_policy_tags_header_splits_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_POLICY_TAGS),
            HeaderValue::from_static("alpha, beta , ,"),
        );
        let tags = parse_policy_tags_header(&headers, HEADER_SORA_POLICY_TAGS);
        assert_eq!(tags, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn warning_header_exposes_refresh_states() {
        let mut decision = decision_with("ApprovedSuccessorGrace", CacheDecisionOutcome::Hold);
        let header = decision_warning_header(&decision).expect("successor warning");
        assert_eq!(
            header.to_str().unwrap(),
            r#"199 - "alias proof successor grace""#
        );

        decision.reasons = vec!["GovernanceGrace".into()];
        let header = decision_warning_header(&decision).expect("governance warning");
        assert_eq!(
            header.to_str().unwrap(),
            r#"199 - "alias proof governance grace""#
        );

        decision.reasons = vec!["RefreshWindow".into()];
        let header = decision_warning_header(&decision).expect("refresh window warning");
        assert_eq!(
            header.to_str().unwrap(),
            r#"199 - "alias proof refresh in-flight""#
        );

        decision.reasons.clear();
        assert!(
            decision_warning_header(&decision).is_none(),
            "hold without reasons should not emit warning"
        );
    }

    #[test]
    fn warning_header_labels_stale_and_expired_proofs() {
        let stale = decision_with("ExpiredTTL", CacheDecisionOutcome::Refuse);
        let header = decision_warning_header(&stale).expect("stale warning");
        assert_eq!(header.to_str().unwrap(), r#"110 - "alias proof stale""#);

        let hard_expired = decision_with("HardExpired", CacheDecisionOutcome::Refuse);
        let header = decision_warning_header(&hard_expired).expect("hard expired warning");
        assert_eq!(header.to_str().unwrap(), r#"111 - "alias proof expired""#);

        let mixed = decision_with("ApprovedSuccessor", CacheDecisionOutcome::Refuse);
        let header = decision_warning_header(&mixed).expect("successor refusal warning");
        assert_eq!(
            header.to_str().unwrap(),
            r#"110 - "alias proof superseded""#
        );
    }

    #[test]
    fn status_and_messages_align_with_alias_policy() {
        let stale = decision_with("ExpiredTTL", CacheDecisionOutcome::Refuse);
        assert_eq!(
            status_code_for_decision(&stale),
            StatusCode::SERVICE_UNAVAILABLE,
            "soft-expired proofs should trigger 503 for refresh"
        );
        assert_eq!(
            message_for_decision(&stale),
            "alias proof stale; refresh required"
        );

        let hard_expired = decision_with("HardExpired", CacheDecisionOutcome::Refuse);
        assert_eq!(
            status_code_for_decision(&hard_expired),
            StatusCode::PRECONDITION_FAILED,
            "hard-expired proofs must surface 412"
        );
        assert_eq!(
            message_for_decision(&hard_expired),
            "alias proof expired; refresh required"
        );
    }
}
#[derive(Debug, Clone, Copy)]
struct ByteRange {
    start: u64,
    end_inclusive: u64,
}

impl ByteRange {
    fn len(&self) -> u64 {
        self.end_inclusive
            .saturating_sub(self.start)
            .saturating_add(1)
    }
}

#[cfg(feature = "app_api")]
#[derive(
    Default, Clone, crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize,
)]
/// JSON payload accepted by the `/v1/sorafs/storage/pin` endpoint.
pub struct StoragePinRequestDto {
    /// Base64-encoded Norito manifest bytes.
    pub manifest_b64: String,
    /// Base64-encoded payload matching the manifest contents.
    pub payload_b64: String,
    /// Optional erasure-layout metadata to persist alongside the manifest.
    pub stripe_layout: Option<DaStripeLayout>,
    /// Optional per-chunk role annotations (data/parity + group id).
    pub chunk_roles: Option<Vec<ChunkRoleDto>>,
}

#[cfg(feature = "app_api")]
#[derive(
    Copy, Clone, Debug, crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize,
)]
/// Chunk role hint used when persisting DA layout metadata.
pub struct ChunkRoleDto {
    /// Role of the chunk within the 2D layout. Defaults to `Data` when omitted.
    pub role: Option<ChunkRole>,
    /// Optional group identifier (stripe index); defaults to the chunk index.
    pub group_id: Option<u32>,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload returned when a manifest is successfully pinned.
pub struct StoragePinResponseDto {
    /// Hex-encoded manifest identifier assigned by the node.
    pub manifest_id_hex: String,
    /// Hex-encoded digest of the pinned payload.
    pub payload_digest_hex: String,
    /// Total payload length in bytes.
    pub content_length: u64,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by the `/v1/sorafs/storage/fetch` endpoint.
pub struct StorageFetchRequestDto {
    /// Hex-encoded manifest identifier to read from.
    pub manifest_id_hex: String,
    /// Byte offset within the payload to begin reading.
    pub offset: u64,
    /// Number of bytes to retrieve from the payload.
    pub length: u64,
    /// Optional provider identifier used for policy enforcement.
    pub provider_id_hex: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload returned when streaming back a payload range.
pub struct StorageFetchResponseDto {
    /// Hex-encoded manifest identifier that was read.
    pub manifest_id_hex: String,
    /// Byte offset served within the payload.
    pub offset: u64,
    /// Number of bytes returned in `data_b64`.
    pub length: u64,
    /// Base64-encoded chunk data extracted from storage.
    pub data_b64: String,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by `/v1/sorafs/storage/token`.
pub struct StreamTokenRequestDto {
    /// Hex-encoded manifest identifier.
    pub manifest_id_hex: String,
    /// Hex-encoded provider identifier (32 bytes).
    pub provider_id_hex: String,
    /// Optional override for TTL (seconds).
    pub ttl_secs: Option<u64>,
    /// Optional override for max concurrent streams.
    pub max_streams: Option<u16>,
    /// Optional override for sustained throughput (bytes per second).
    pub rate_limit_bytes: Option<u64>,
    /// Optional override for refresh allowance (requests per minute).
    pub requests_per_minute: Option<u32>,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by the `/v1/sorafs/storage/por-sample` endpoint.
pub struct StoragePorSampleRequestDto {
    /// Hex-encoded manifest identifier to sample against.
    pub manifest_id_hex: String,
    /// Desired number of samples (capped by leaf count).
    pub count: u64,
    /// Optional deterministic seed for repeatable sampling.
    pub seed: Option<u64>,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by the `/v1/sorafs/proof/stream` endpoint.
pub struct ProofStreamRequestDto {
    /// Hex-encoded manifest digest (32 bytes).
    pub manifest_digest_hex: String,
    /// Hex-encoded provider identifier (32 bytes).
    pub provider_id_hex: String,
    /// Proof kind requested (`por`, `pdp`, `potr`).
    pub proof_kind: String,
    /// Optional sample count (required for PoR/PDP).
    pub sample_count: Option<u32>,
    /// Optional deadline in milliseconds (required for PoTR).
    pub deadline_ms: Option<u32>,
    /// Optional deterministic seed for sampling.
    pub sample_seed: Option<u64>,
    /// Base64-encoded 16-byte nonce supplied by the client.
    pub nonce_b64: String,
    /// Optional hex-encoded orchestrator job identifier (16 bytes).
    pub orchestrator_job_id_hex: Option<String>,
    /// Optional tier hint (`hot`, `warm`, `archive`).
    pub tier: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by `/v1/sorafs/storage/por-challenge`.
pub struct StoragePorChallengeDto {
    /// Base64-encoded Norito PoR challenge payload.
    pub challenge_b64: String,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by `/v1/sorafs/storage/por-proof`.
pub struct StoragePorProofDto {
    /// Base64-encoded Norito PoR proof payload.
    pub proof_b64: String,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload accepted by `/v1/sorafs/storage/por-verdict`.
pub struct StoragePorVerdictDto {
    /// Base64-encoded Norito PoR verdict payload.
    pub verdict_b64: String,
}

#[cfg(feature = "app_api")]
#[derive(Clone, Copy, crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload returned by `/v1/sorafs/storage/state`.
pub struct StorageStateResponseDto {
    /// Total bytes currently stored on disk.
    pub bytes_used: u64,
    /// Maximum capacity configured for the node.
    pub bytes_capacity: u64,
    /// Current pin queue depth.
    pub pin_queue_depth: u64,
    /// Number of fetch workers in flight.
    pub fetch_inflight: u64,
    /// Smoothed bytes-per-second served by fetch workers.
    pub fetch_bytes_per_sec: u64,
    /// Number of PoR workers active.
    pub por_inflight: u64,
    /// Total PoR samples marked successful during the current window.
    pub por_samples_success_total: u64,
    /// Total PoR samples marked failed during the current window.
    pub por_samples_failed_total: u64,
    /// Fetch queue utilisation (basis points).
    pub fetch_utilisation_bps: u32,
    /// Pin queue utilisation (basis points).
    pub pin_queue_utilisation_bps: u32,
    /// PoR worker utilisation (basis points).
    pub por_utilisation_bps: u32,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonSerialize)]
/// Provider-level payload returned by `/v1/sorafs/por/ingestion/{manifest}`.
pub struct PorIngestionProviderStatusDto {
    /// Hex-encoded provider identifier.
    pub provider_id_hex: String,
    /// Outstanding PoR challenges for this manifest/provider pair.
    pub pending_challenges: u64,
    /// Oldest epoch identifier currently pending.
    pub oldest_epoch_id: Option<u64>,
    /// Earliest response deadline across pending challenges.
    pub oldest_response_deadline_unix: Option<u64>,
    /// Timestamp for the most recent successful verdict.
    pub last_success_unix: Option<u64>,
    /// Timestamp for the most recent failed verdict.
    pub last_failure_unix: Option<u64>,
    /// Total failure count observed.
    pub failures_total: u64,
    /// Current consecutive failure streak.
    pub consecutive_failures: u64,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonSerialize)]
/// JSON payload returned by `/v1/sorafs/por/ingestion/{manifest}`.
pub struct PorIngestionStatusResponseDto {
    /// Hex-encoded manifest digest.
    pub manifest_digest_hex: String,
    /// Provider entries associated with the manifest.
    pub providers: Vec<PorIngestionProviderStatusDto>,
}

#[cfg(feature = "app_api")]
#[derive(crate::json_macros::JsonDeserialize, crate::json_macros::JsonSerialize)]
/// JSON payload returned by `/v1/sorafs/storage/manifest/{manifest_id_hex}`.
pub struct StorageManifestResponseDto {
    /// Hex-encoded manifest identifier.
    pub manifest_id_hex: String,
    /// Base64-encoded Norito manifest payload.
    pub manifest_b64: String,
    /// Hex-encoded manifest digest.
    pub manifest_digest_hex: String,
    /// Hex-encoded payload digest.
    pub payload_digest_hex: String,
    /// Total payload length in bytes.
    pub content_length: u64,
    /// Number of stored chunks.
    pub chunk_count: u64,
    /// Chunk profile handle recorded on ingest.
    pub chunk_profile_handle: String,
    /// Timestamp (seconds since UNIX epoch) when the manifest was stored.
    pub stored_at_unix_secs: u64,
}

const DEFAULT_LIST_LIMIT: usize = 50;
const MAX_LIST_LIMIT: usize = 500;

#[derive(Debug, Default)]
struct PinListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    status: Option<String>,
}

#[derive(Debug, Default)]
struct AliasListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    namespace: Option<String>,
    manifest_digest: Option<String>,
}

#[derive(Debug, Default)]
struct ReplicationListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    status: Option<String>,
    manifest_digest: Option<String>,
}

impl PinListQuery {
    fn parse(raw: Option<&str>) -> ApiResult<Self> {
        let mut query = Self::default();
        walk_query_params(raw, |key, value| match key {
            "limit" => parse_u32_field(&mut query.limit, "limit", value),
            "offset" => parse_u32_field(&mut query.offset, "offset", value),
            "status" => {
                query.status = Some(value.to_owned());
                Ok(())
            }
            _ => Ok(()),
        })?;
        Ok(query)
    }
}

impl AliasListQuery {
    fn parse(raw: Option<&str>) -> ApiResult<Self> {
        let mut query = Self::default();
        walk_query_params(raw, |key, value| match key {
            "limit" => parse_u32_field(&mut query.limit, "limit", value),
            "offset" => parse_u32_field(&mut query.offset, "offset", value),
            "namespace" => {
                query.namespace = Some(value.to_owned());
                Ok(())
            }
            "manifest_digest" => {
                query.manifest_digest = Some(value.to_owned());
                Ok(())
            }
            _ => Ok(()),
        })?;
        Ok(query)
    }
}

impl ReplicationListQuery {
    fn parse(raw: Option<&str>) -> ApiResult<Self> {
        let mut query = Self::default();
        walk_query_params(raw, |key, value| match key {
            "limit" => parse_u32_field(&mut query.limit, "limit", value),
            "offset" => parse_u32_field(&mut query.offset, "offset", value),
            "status" => {
                query.status = Some(value.to_owned());
                Ok(())
            }
            "manifest_digest" => {
                query.manifest_digest = Some(value.to_owned());
                Ok(())
            }
            _ => Ok(()),
        })?;
        Ok(query)
    }
}

fn parse_u32_field(target: &mut Option<u32>, name: &str, raw: &str) -> ApiResult<()> {
    if raw.is_empty() {
        *target = None;
        return Ok(());
    }

    raw.parse::<u32>().map_or_else(
        |_| {
            Err(ResponseError::from(json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid {name} value `{raw}`"),
            )))
        },
        |value| {
            *target = Some(value);
            Ok(())
        },
    )
}

fn ensure_percent_sequences(segment: &str, context: &str) -> Result<(), ResponseError> {
    let bytes = segment.as_bytes();
    let mut index = 0;
    while let Some(pos) = segment[index..].find('%') {
        let abs = index + pos;
        if abs + 2 >= bytes.len() {
            return Err(ResponseError::from(json_error(
                StatusCode::BAD_REQUEST,
                format!("failed to decode {context} `{segment}`"),
            )));
        }
        let hi = bytes[abs + 1] as char;
        let lo = bytes[abs + 2] as char;
        if !hi.is_ascii_hexdigit() || !lo.is_ascii_hexdigit() {
            return Err(ResponseError::from(json_error(
                StatusCode::BAD_REQUEST,
                format!("failed to decode {context} `{segment}`"),
            )));
        }
        index = abs + 3;
    }
    Ok(())
}

fn walk_query_params(
    raw: Option<&str>,
    mut visitor: impl FnMut(&str, &str) -> ApiResult<()>,
) -> ApiResult<()> {
    if let Some(raw) = raw {
        for pair in raw.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut parts = pair.splitn(2, '=');
            let raw_key = parts.next().unwrap_or("");
            let raw_value = parts.next().unwrap_or("");
            ensure_percent_sequences(raw_key, "query key")?;
            ensure_percent_sequences(raw_value, "query value")?;
            let key = decode(raw_key).map_err(|_| {
                ResponseError::from(json_error(
                    StatusCode::BAD_REQUEST,
                    format!("failed to decode query key `{raw_key}`"),
                ))
            })?;
            let value = decode(raw_value).map_err(|_| {
                ResponseError::from(json_error(
                    StatusCode::BAD_REQUEST,
                    format!("failed to decode query value `{raw_value}`"),
                ))
            })?;
            visitor(&key, &value)?;
        }
    }
    Ok(())
}

#[derive(Copy, Clone)]
enum PinStatusFilter {
    Pending,
    Approved,
    Retired,
}

#[derive(Copy, Clone)]
enum ReplicationStatusFilter {
    Pending,
    Completed,
    Expired,
}

pub(crate) async fn handle_get_sorafs_providers(State(state): State<SharedAppState>) -> Response {
    let Some(cache) = state.sorafs_cache.clone() else {
        return feature_disabled("sorafs discovery API is not enabled on this node");
    };

    let mut cache_guard = cache.write().await;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let pruned = cache_guard.prune_stale(now);
    if pruned > 0 {
        warn!(removed = pruned, "pruned stale SoraFS provider adverts");
    }
    record_range_capability_metrics(&cache_guard, &state.telemetry);
    let mut providers = Vec::new();
    for record in cache_guard.records() {
        match advert_to_json(
            record.fingerprint(),
            record.advert(),
            record.known_capabilities(),
            record.warnings(),
        ) {
            Ok(value) => providers.push(value),
            Err(err) => {
                error!(%err, "failed to serialize provider advert to JSON");
            }
        }
    }

    let response = json_object(vec![
        json_entry("count", providers.len() as u64),
        json_entry("providers", Value::Array(providers)),
    ]);
    JsonBody(response).into_response()
}

pub(crate) async fn handle_post_sorafs_provider_advert(
    State(state): State<SharedAppState>,
    body: Bytes,
) -> Response {
    let Some(cache) = state.sorafs_cache.clone() else {
        return feature_disabled("sorafs discovery API is not enabled on this node");
    };

    let telemetry = state.telemetry.clone();

    let advert = match norito::decode_from_bytes::<ProviderAdvertV1>(body.as_ref()) {
        Ok(value) => value,
        Err(err) => {
            warn!(?err, "failed to decode provider advert payload");
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid provider advert payload: {err}"),
            );
        }
    };

    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(err) => {
            error!(?err, "system clock error while ingesting provider advert");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "system time error while ingesting advert",
            );
        }
    };

    let advert_profile = advert.body.profile_id.clone();
    let advert_provider_id = advert.body.provider_id;

    let mut guard = cache.write().await;
    match guard.ingest(advert, now) {
        Ok(AdvertIngestResult { outcome, warnings }) => {
            if !warnings.is_empty() {
                warn!(
                    ?warnings,
                    "provider advert ingested with downgrade warnings"
                );
                let telemetry = telemetry.clone();
                for warning in &warnings {
                    telemetry.with_metrics(|tel| {
                        tel.inc_torii_sorafs_admission("warn", warning.telemetry_reason())
                    });
                }
            }

            let response = match outcome {
                AdvertIngest::Stored { fingerprint } => {
                    debug!("stored new provider advert");
                    telemetry
                        .with_metrics(|tel| tel.inc_torii_sorafs_admission("accepted", "stored"));
                    advert_ingest_response("stored", &fingerprint, &warnings)
                }
                AdvertIngest::Replaced { fingerprint } => {
                    debug!("replaced provider advert");
                    telemetry
                        .with_metrics(|tel| tel.inc_torii_sorafs_admission("accepted", "replaced"));
                    advert_ingest_response("replaced", &fingerprint, &warnings)
                }
                AdvertIngest::Duplicate { fingerprint } => {
                    debug!("duplicate provider advert ignored");
                    telemetry.with_metrics(|tel| {
                        tel.inc_torii_sorafs_admission("accepted", "duplicate")
                    });
                    advert_ingest_response("duplicate", &fingerprint, &warnings)
                }
            };
            record_range_capability_metrics(&guard, &telemetry);
            response
        }
        Err(err) => {
            warn!(?err, "failed to ingest provider advert");
            let reason = admission_error_reason(&err);
            telemetry.with_metrics(|tel| tel.inc_torii_sorafs_admission("rejected", reason));
            match err {
                AdvertError::UnknownCapabilities { capabilities } => {
                    let capability_labels: Vec<Value> = capabilities
                        .into_iter()
                        .map(|cap| Value::from(capability_name(cap).to_string()))
                        .collect();
                    gateway_refusal_response(
                        &state,
                        StatusCode::PRECONDITION_REQUIRED,
                        "unsupported_capability",
                        format!(
                            "provider advert declares unsupported capabilities for profile {}",
                            advert_profile
                        ),
                        Some(&advert_profile),
                        Some(&advert_provider_id),
                        TELEMETRY_ENDPOINT_PROVIDER_ADVERT,
                        [("capabilities", Value::Array(capability_labels))],
                    )
                }
                other => json_error(StatusCode::BAD_REQUEST, format_advert_error(other)),
            }
        }
    }
}

pub(crate) async fn handle_get_sorafs_capacity_state(
    State(state): State<SharedAppState>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return feature_disabled("sorafs capacity registry API is not enabled on this node");
    }

    let snapshot = {
        let view = state.state.view();
        collect_snapshot(view.world())
    };

    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(err) => {
            error!(?err, "failed to collect capacity registry snapshot");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to collect capacity registry snapshot",
            );
        }
    };

    let local_usage = state.sorafs_node.capacity_usage();
    let local_metering = state.sorafs_node.metering_snapshot();
    let telemetry_preview =
        build_telemetry_preview_json(state.sorafs_node.build_capacity_telemetry());
    let fee_projection = local_usage
        .provider_id
        .and_then(|id| FeeProjection::from_snapshot(id, &local_metering));

    match snapshot_to_json(
        snapshot,
        &local_usage,
        &local_metering,
        telemetry_preview,
        fee_projection.as_ref(),
    ) {
        Ok(value) => JsonBody(value).into_response(),
        Err(err) => {
            error!(?err, "failed to serialize capacity registry snapshot");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize capacity registry snapshot",
            )
        }
    }
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_storage_state(
    State(state): State<SharedAppState>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let schedulers = state.sorafs_node.schedulers();
    let telemetry = schedulers.telemetry_snapshot();
    let utilisation = schedulers.utilisation_snapshot();

    let response = StorageStateResponseDto {
        bytes_used: telemetry.bytes_used,
        bytes_capacity: telemetry.bytes_capacity,
        pin_queue_depth: telemetry.pin_queue_depth as u64,
        fetch_inflight: telemetry.fetch_inflight as u64,
        fetch_bytes_per_sec: telemetry.fetch_bytes_per_sec,
        por_inflight: telemetry.por_inflight as u64,
        por_samples_success_total: telemetry.por_samples_success,
        por_samples_failed_total: telemetry.por_samples_failed,
        fetch_utilisation_bps: utilisation.fetch_utilisation_bps,
        pin_queue_utilisation_bps: utilisation.pin_queue_utilisation_bps,
        por_utilisation_bps: utilisation.por_utilisation_bps,
    };

    record_storage_metrics(&state);

    let value = match norito::json::to_value(&response) {
        Ok(value) => value,
        Err(err) => {
            error!(?err, "failed to encode storage scheduler state");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to encode storage scheduler state",
            );
        }
    };

    JsonBody(value).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_por_ingestion(
    State(state): State<SharedAppState>,
    Path(manifest_hex): Path<String>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    let manifest_digest = match parse_hex_fixed::<32>(&manifest_hex, "manifest_digest_hex") {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(StatusCode::BAD_REQUEST, &err);
        }
    };
    let status = match state.sorafs_node.por_ingestion_status(&manifest_digest) {
        Ok(status) => status,
        Err(NodeStorageError::Disabled) => return storage_disabled_response(),
        Err(NodeStorageError::Storage(err)) => {
            return node_storage_error_response(NodeStorageError::Storage(err));
        }
    };
    if status.providers.is_empty() {
        return json_error(
            StatusCode::NOT_FOUND,
            "no PoR ingestion data recorded for the requested manifest",
        );
    }
    record_storage_metrics(&state);
    let providers = status
        .providers
        .into_iter()
        .map(|entry| PorIngestionProviderStatusDto {
            provider_id_hex: hex::encode(entry.provider_id),
            pending_challenges: entry.pending_challenges,
            oldest_epoch_id: entry.oldest_epoch_id,
            oldest_response_deadline_unix: entry.oldest_response_deadline_unix,
            last_success_unix: entry.last_success_unix,
            last_failure_unix: entry.last_failure_unix,
            failures_total: entry.failures_total,
            consecutive_failures: entry.consecutive_failures,
        })
        .collect::<Vec<_>>();
    let response = PorIngestionStatusResponseDto {
        manifest_digest_hex: hex::encode(status.manifest_digest),
        providers,
    };
    let value = match norito::json::to_value(&response) {
        Ok(value) => value,
        Err(err) => {
            error!(?err, "failed to encode PoR ingestion status response");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to encode PoR ingestion status response",
            );
        }
    };
    JsonBody(value).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_storage_manifest(
    State(state): State<SharedAppState>,
    Path(manifest_id_hex): Path<String>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let stored = match state.sorafs_node.manifest_metadata(&manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    if let Err(response) = enforce_chunker_support_gateway(
        &state,
        stored.chunk_profile_handle(),
        TELEMETRY_ENDPOINT_MANIFEST,
    ) {
        return response;
    }

    let manifest_bytes = match fs::read(stored.manifest_path()) {
        Ok(bytes) => bytes,
        Err(err) => {
            error!(?err, "failed to read stored manifest bytes");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to read stored manifest bytes",
            );
        }
    };

    let manifest_b64 = base64::engine::general_purpose::STANDARD.encode(manifest_bytes.as_slice());

    let response = StorageManifestResponseDto {
        manifest_id_hex,
        manifest_b64,
        manifest_digest_hex: hex::encode(stored.manifest_digest()),
        payload_digest_hex: hex::encode(stored.payload_digest()),
        content_length: stored.content_length(),
        chunk_count: stored.chunk_count() as u64,
        chunk_profile_handle: stored.chunk_profile_handle().to_owned(),
        stored_at_unix_secs: stored.stored_at_unix_secs(),
    };

    let value = match norito::json::to_value(&response) {
        Ok(value) => value,
        Err(err) => {
            error!(?err, "failed to encode manifest response");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to encode manifest response",
            );
        }
    };

    JsonBody(value).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_storage_plan(
    State(state): State<SharedAppState>,
    Path(manifest_id_hex): Path<String>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let stored = match state.sorafs_node.manifest_metadata(&manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    if let Err(response) = enforce_chunker_support_gateway(
        &state,
        stored.chunk_profile_handle(),
        TELEMETRY_ENDPOINT_PLAN,
    ) {
        return response;
    }

    let manifest_v1 = match stored.load_manifest() {
        Ok(manifest) => manifest,
        Err(err) => return storage_backend_error(err),
    };

    let chunk_profile = match chunk_profile_for_manifest(&manifest_v1) {
        Ok(profile) => profile,
        Err(err) => return err.into_response(),
    };

    let taikai_hint = match sorafs_car::taikai_segment_hint_from_sorafs_manifest(&manifest_v1) {
        Ok(hint) => hint,
        Err(err) => {
            error!(
                ?err,
                manifest = manifest_id_hex,
                "failed to derive Taikai segment hint from manifest"
            );
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to derive Taikai metadata",
            );
        }
    };

    let plan = stored.to_car_plan_with_hint(chunk_profile, taikai_hint.clone());
    let specs = plan.chunk_fetch_specs();

    let chunk_digests = specs
        .iter()
        .map(|spec| Value::String(hex::encode(spec.digest)))
        .collect::<Vec<_>>();
    let chunks_value = chunk_fetch_specs_to_json(&plan);

    let mut plan_map = Map::new();
    plan_map.insert("chunk_count".into(), Value::from(specs.len() as u64));
    plan_map.insert("content_length".into(), Value::from(plan.content_length));
    plan_map.insert(
        "payload_digest_blake3".into(),
        Value::String(hex::encode(plan.payload_digest.as_bytes())),
    );
    plan_map.insert(
        "chunk_profile_handle".into(),
        Value::String(stored.chunk_profile_handle().to_owned()),
    );
    plan_map.insert("chunk_digests_blake3".into(), Value::Array(chunk_digests));
    plan_map.insert("chunks".into(), chunks_value);

    let mut root = Map::new();
    root.insert("manifest_id_hex".into(), Value::String(manifest_id_hex));
    root.insert("plan".into(), Value::Object(plan_map));

    JsonBody(Value::Object(root)).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_pin_registry(
    State(state): State<SharedAppState>,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    accept: Option<ExtractAccept>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return feature_disabled("sorafs pin registry API is not enabled on this node");
    }

    let query = match PinListQuery::parse(raw_query.as_deref()) {
        Ok(query) => query,
        Err(err) => return err.into_response(),
    };

    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|hdr| &hdr.0)) {
        Ok(fmt) => fmt,
        Err(err) => return err.into_response(),
    };

    let (attestation, snapshot) = match pin_snapshot_with_attestation(&state) {
        Ok(snapshot) => snapshot,
        Err(err) => return err.into_response(),
    };

    let status_filter = match query.status.as_deref() {
        Some(value) => match parse_pin_status_filter(value) {
            Some(filter) => Some(filter),
            None => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "invalid status filter `{value}`; expected pending, approved, or retired"
                    ),
                );
            }
        },
        None => None,
    };

    let mut manifests: Vec<&RegistryManifest> = snapshot.manifests.iter().collect();
    if let Some(filter) = status_filter {
        manifests.retain(|manifest| filter.matches(manifest.status_label()));
    }
    let policy = &state.sorafs_alias_cache_policy;
    let enforcement = state.sorafs_alias_enforcement;
    let now_secs = crate::sorafs::unix_now_secs();

    let total = manifests.len();
    let limit = normalize_limit(query.limit);
    let offset = normalize_offset(query.offset, total);
    let end = offset.saturating_add(limit).min(total);

    let page = &manifests[offset..end];
    let mut manifests_json = Vec::with_capacity(page.len());
    for manifest in page {
        let mut value = match manifest.to_json() {
            Ok(value) => value,
            Err(err) => {
                error!(?err, "failed to serialize pin registry manifests");
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to serialize pin registry manifests",
                );
            }
        };
        if let Value::Object(mut map) = value {
            let lineage = snapshot.lineage_for(manifest.digest_hex());
            map.insert("lineage".into(), lineage_to_json(&lineage));
            if let Some(alias) = snapshot
                .aliases
                .iter()
                .find(|alias| alias.manifest_digest_hex() == manifest.digest_hex())
            {
                let governance = manifest.governance_summary();
                match prepare_alias_presentation(
                    alias,
                    &lineage,
                    &governance,
                    policy,
                    enforcement,
                    &state.telemetry,
                    now_secs,
                ) {
                    Ok(presentation) => {
                        map.insert(
                            "cache_evaluation".into(),
                            build_cache_evaluation_json(
                                &presentation.evaluation,
                                &presentation.decision,
                                policy,
                            ),
                        );
                    }
                    Err(err) => return err.into_response(),
                }
            }
            value = Value::Object(map);
        }
        manifests_json.push(value);
    }

    let response = json_object(vec![
        json_entry("attestation", attestation),
        json_entry("total_count", total as u64),
        json_entry("returned_count", manifests_json.len() as u64),
        json_entry("offset", offset as u64),
        json_entry("limit", limit as u64),
        json_entry("manifests", Value::Array(manifests_json)),
    ]);
    crate::utils::respond_value_with_format(response, format)
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_pin_manifest(
    State(state): State<SharedAppState>,
    Path(digest_hex): Path<String>,
    accept: Option<ExtractAccept>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return feature_disabled("sorafs pin registry API is not enabled on this node");
    }

    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|hdr| &hdr.0)) {
        Ok(fmt) => fmt,
        Err(err) => return err.into_response(),
    };

    let canonical_digest = match parse_manifest_digest_hex(&digest_hex) {
        Ok(bytes) => hex::encode(bytes),
        Err(err) => return err.into_response(),
    };

    let (attestation, snapshot) = match pin_snapshot_with_attestation(&state) {
        Ok(snapshot) => snapshot,
        Err(err) => return err.into_response(),
    };

    let manifest = match snapshot
        .manifests
        .iter()
        .find(|manifest| manifest.digest_hex() == canonical_digest)
    {
        Some(manifest) => manifest,
        None => {
            return json_error(
                StatusCode::NOT_FOUND,
                format!("manifest `{}` not found in pin registry", digest_hex),
            );
        }
    };

    let chunker_handle = manifest.chunker_handle();
    if state.sorafs_gateway_config.enforce_capabilities {
        let (support, _, supported_profiles) = chunker_support_state(&state, &chunker_handle);
        match support {
            ChunkerSupport::Supported => {}
            ChunkerSupport::Unsupported => {
                return registry_chunker_error(&chunker_handle, &supported_profiles);
            }
            ChunkerSupport::Unknown => {
                return registry_chunker_unknown(&chunker_handle);
            }
        }
    }

    let manifest_lineage = snapshot.lineage_for(&canonical_digest);
    let manifest_json = match manifest.to_json() {
        Ok(mut value) => {
            if let Value::Object(mut map) = value {
                map.insert("lineage".into(), lineage_to_json(&manifest_lineage));
                value = Value::Object(map);
            }
            value
        }
        Err(err) => {
            error!(?err, "failed to serialize pin registry manifest");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize pin registry manifest",
            );
        }
    };

    let policy = &state.sorafs_alias_cache_policy;
    let enforcement = state.sorafs_alias_enforcement;
    let now_secs = crate::sorafs::unix_now_secs();

    let aliases = snapshot
        .aliases
        .iter()
        .filter(|alias| alias.manifest_digest_hex() == canonical_digest)
        .collect::<Vec<_>>();
    let mut alias_presentations = Vec::with_capacity(aliases.len());
    let mut alias_values = Vec::with_capacity(aliases.len());
    for alias in aliases {
        let manifest_for_alias = match snapshot.manifest_by_digest(alias.manifest_digest_hex()) {
            Some(record) => record,
            None => {
                error!(
                    alias = alias.alias_label(),
                    manifest = alias.manifest_digest_hex(),
                    "alias references manifest missing from registry snapshot"
                );
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "alias `{}` references unknown manifest `{}`",
                        alias.alias_label(),
                        alias.manifest_digest_hex()
                    ),
                )
                .into_response();
            }
        };
        let lineage_summary = if manifest_for_alias.digest_hex() == canonical_digest {
            manifest_lineage.clone()
        } else {
            snapshot.lineage_for(alias.manifest_digest_hex())
        };
        let governance_summary = manifest_for_alias.governance_summary();

        match prepare_alias_presentation(
            alias,
            &lineage_summary,
            &governance_summary,
            policy,
            enforcement,
            &state.telemetry,
            now_secs,
        ) {
            Ok(presentation) => {
                alias_values.push(presentation.json.clone());
                alias_presentations.push(presentation);
            }
            Err(err) => return err.into_response(),
        }
    }

    if let Some(primary) = alias_presentations.first() {
        if matches!(
            primary.decision.outcome,
            crate::sorafs::CacheDecisionOutcome::Refuse
        ) {
            warn!(
                alias = %primary.alias_label,
                reasons = ?primary.decision.reasons,
                "alias proof rejected; refusing manifest response"
            );
            let status = status_code_for_decision(&primary.decision);
            let message = message_for_decision(&primary.decision);
            return alias_policy_error_response(primary, policy, status, message);
        }
    }

    let orders = snapshot
        .replication_orders
        .iter()
        .filter(|order| order.manifest_digest_hex() == canonical_digest)
        .collect::<Vec<_>>();
    let order_values = match orders
        .iter()
        .map(|order| order.to_json())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(values) => values,
        Err(err) => {
            error!(?err, "failed to serialize replication orders");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize replication orders",
            );
        }
    };

    let response = json_object(vec![
        json_entry("attestation", attestation),
        json_entry("manifest", manifest_json),
        json_entry("aliases", Value::Array(alias_values)),
        json_entry("replication_orders", Value::Array(order_values)),
    ]);

    let mut http_response = crate::utils::respond_value_with_format(response, format);
    if let Some(primary) = alias_presentations.first() {
        let headers = http_response.headers_mut();
        if let Ok(name) = HeaderValue::from_str(&primary.alias_label) {
            headers.insert(HeaderName::from_static(HEADER_SORA_NAME), name);
        }
        if let Ok(cid) = HeaderValue::from_str(&primary.manifest_digest_hex) {
            headers.insert(HeaderName::from_static(HEADER_SORA_CID), cid);
        }
        if let Ok(proof) = HeaderValue::from_str(&primary.proof_b64) {
            headers.insert(HeaderName::from_static(HEADER_SORA_PROOF), proof);
        }
        headers.insert(CACHE_CONTROL, policy.cache_control_header());
        if let Ok(status) = HeaderValue::from_str(&primary.status_label) {
            headers.insert(HeaderName::from_static(HEADER_SORA_PROOF_STATUS), status);
        }
        headers.insert(AGE, primary.evaluation.age_header());
        if let Some(warning) = decision_warning_header(&primary.decision) {
            headers.append(WARNING, warning);
        } else if let Some(warning) = primary.evaluation.warning_header() {
            headers.append(WARNING, warning);
        }
    }

    http_response
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_aliases(
    State(state): State<SharedAppState>,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    accept: Option<ExtractAccept>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return feature_disabled("sorafs pin registry API is not enabled on this node");
    }

    let query = match AliasListQuery::parse(raw_query.as_deref()) {
        Ok(query) => query,
        Err(err) => return err.into_response(),
    };

    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|hdr| &hdr.0)) {
        Ok(fmt) => fmt,
        Err(err) => return err.into_response(),
    };

    let (attestation, snapshot) = match pin_snapshot_with_attestation(&state) {
        Ok(snapshot) => snapshot,
        Err(err) => return err.into_response(),
    };

    let namespace_filter = query.namespace.as_deref().map(str::to_ascii_lowercase);
    let digest_filter = query
        .manifest_digest
        .as_deref()
        .map(str::to_ascii_lowercase);

    let mut aliases: Vec<&RegistryAlias> = snapshot.aliases.iter().collect();
    if let Some(namespace) = namespace_filter {
        aliases.retain(|alias| alias.namespace().eq_ignore_ascii_case(&namespace));
    }
    if let Some(digest) = digest_filter {
        aliases.retain(|alias| alias.manifest_digest_hex().eq_ignore_ascii_case(&digest));
    }

    let total = aliases.len();
    let limit = normalize_limit(query.limit);
    let offset = normalize_offset(query.offset, total);
    let end = offset.saturating_add(limit).min(total);

    let page = &aliases[offset..end];
    let mut alias_values = Vec::with_capacity(page.len());
    let policy = &state.sorafs_alias_cache_policy;
    let enforcement = state.sorafs_alias_enforcement;
    let now_secs = crate::sorafs::unix_now_secs();
    for alias in page {
        let lineage_summary = snapshot.lineage_for(alias.manifest_digest_hex());
        let manifest_for_alias = match snapshot.manifest_by_digest(alias.manifest_digest_hex()) {
            Some(manifest) => manifest,
            None => {
                error!(
                    alias = alias.alias_label(),
                    manifest = alias.manifest_digest_hex(),
                    "alias references manifest missing from registry snapshot"
                );
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "alias `{}` references unknown manifest `{}`",
                        alias.alias_label(),
                        alias.manifest_digest_hex()
                    ),
                )
                .into_response();
            }
        };
        let governance_summary = manifest_for_alias.governance_summary();
        match prepare_alias_presentation(
            alias,
            &lineage_summary,
            &governance_summary,
            policy,
            enforcement,
            &state.telemetry,
            now_secs,
        ) {
            Ok(presentation) => alias_values.push(presentation.json),
            Err(err) => return err.into_response(),
        }
    }

    let response = json_object(vec![
        json_entry("attestation", attestation),
        json_entry("total_count", total as u64),
        json_entry("returned_count", alias_values.len() as u64),
        json_entry("offset", offset as u64),
        json_entry("limit", limit as u64),
        json_entry("aliases", Value::Array(alias_values)),
    ]);
    crate::utils::respond_value_with_format(response, format)
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_replication_orders(
    State(state): State<SharedAppState>,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    accept: Option<ExtractAccept>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return feature_disabled("sorafs pin registry API is not enabled on this node");
    }

    let query = match ReplicationListQuery::parse(raw_query.as_deref()) {
        Ok(query) => query,
        Err(err) => return err.into_response(),
    };

    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|hdr| &hdr.0)) {
        Ok(fmt) => fmt,
        Err(err) => return err.into_response(),
    };

    let (attestation, snapshot) = match pin_snapshot_with_attestation(&state) {
        Ok(snapshot) => snapshot,
        Err(err) => return err.into_response(),
    };

    let status_filter = match query.status.as_deref() {
        Some(value) => match parse_replication_status_filter(value) {
            Some(filter) => Some(filter),
            None => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "invalid status filter `{value}`; expected pending, completed, or expired"
                    ),
                );
            }
        },
        None => None,
    };

    let digest_filter = query
        .manifest_digest
        .as_deref()
        .map(str::to_ascii_lowercase);

    let mut orders: Vec<&RegistryReplicationOrder> = snapshot.replication_orders.iter().collect();
    if let Some(filter) = status_filter {
        orders.retain(|order| filter.matches(order.status_label()));
    }
    if let Some(digest) = digest_filter {
        orders.retain(|order| order.manifest_digest_hex().eq_ignore_ascii_case(&digest));
    }

    let total = orders.len();
    let limit = normalize_limit(query.limit);
    let offset = normalize_offset(query.offset, total);
    let end = offset.saturating_add(limit).min(total);

    let page = &orders[offset..end];
    let order_values = match page
        .iter()
        .map(|order| order.to_json())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(values) => values,
        Err(err) => {
            error!(?err, "failed to serialize replication orders");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize replication orders",
            );
        }
    };

    let response = json_object(vec![
        json_entry("attestation", attestation),
        json_entry("total_count", total as u64),
        json_entry("returned_count", order_values.len() as u64),
        json_entry("offset", offset as u64),
        json_entry("limit", limit as u64),
        json_entry("replication_orders", Value::Array(order_values)),
    ]);
    crate::utils::respond_value_with_format(response, format)
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_pin(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    JsonOnly(req): JsonOnly<StoragePinRequestDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    if let Err(err) = state.sorafs_pin_policy.enforce(&headers, Some(remote.ip())) {
        return pin_auth_error_response(err);
    }
    let provider_id = state
        .sorafs_node
        .capacity_usage()
        .provider_id
        .unwrap_or([0u8; 32]);
    if let Err(err) = state
        .sorafs_limits
        .enforce(SorafsAction::StoragePin, &provider_id)
    {
        return storage_pin_quota_response(err);
    }

    let manifest_bytes =
        match base64::engine::general_purpose::STANDARD.decode(req.manifest_b64.as_bytes()) {
            Ok(bytes) => bytes,
            Err(err) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!("invalid base64 in manifest_b64: {err}"),
                );
            }
        };
    let manifest: ManifestV1 = match norito::decode_from_bytes(&manifest_bytes) {
        Ok(value) => value,
        Err(err) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid manifest payload: {err}"),
            );
        }
    };

    let payload = match base64::engine::general_purpose::STANDARD.decode(req.payload_b64.as_bytes())
    {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid base64 in payload_b64: {err}"),
            );
        }
    };

    if manifest.content_length != payload.len() as u64 {
        return json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "manifest content_length {} does not match payload length {}",
                manifest.content_length,
                payload.len()
            ),
        );
    }

    let profile = match chunk_profile_for_manifest(&manifest) {
        Ok(profile) => profile,
        Err(err) => return err.into_response(),
    };

    let plan = match CarBuildPlan::single_file_with_profile(&payload, profile) {
        Ok(plan) => plan,
        Err(err) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("failed to derive chunk plan: {err}"),
            );
        }
    };

    let stripe_layout = if let Some(layout) = req.stripe_layout {
        if layout.total_stripes == 0 || layout.shards_per_stripe == 0 {
            return json_error(
                StatusCode::BAD_REQUEST,
                "stripe_layout requires non-zero totals",
            );
        }
        Some(layout)
    } else {
        None
    };

    let chunk_roles = match req.chunk_roles {
        Some(roles) => {
            if stripe_layout.is_none() {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "chunk_roles requires stripe_layout to be provided alongside",
                );
            }
            if roles.len() != plan.chunks.len() {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "chunk_roles length {} does not match chunk count {}",
                        roles.len(),
                        plan.chunks.len()
                    ),
                );
            }
            let mapped = roles
                .into_iter()
                .enumerate()
                .map(|(idx, role)| ChunkRoleMetadata {
                    role: role.role.unwrap_or_default(),
                    group_id: role.group_id.unwrap_or(idx as u32),
                })
                .collect();
            Some(mapped)
        }
        None => None,
    };

    let mut reader = &payload[..];
    let manifest_id = match state.sorafs_node.ingest_manifest_with_layout(
        &manifest,
        &plan,
        &mut reader,
        stripe_layout,
        chunk_roles,
    ) {
        Ok(id) => id,
        Err(err) => return node_storage_error_response(err),
    };

    let response = StoragePinResponseDto {
        manifest_id_hex: manifest_id,
        payload_digest_hex: hex::encode(plan.payload_digest.as_bytes()),
        content_length: plan.content_length,
    };

    record_storage_metrics(&state);

    let value = norito::json::to_value(&response).unwrap_or(Value::Null);
    (StatusCode::OK, JsonBody(value)).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_fetch(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    JsonOnly(req): JsonOnly<StorageFetchRequestDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    if req.length == 0 {
        return json_error(StatusCode::BAD_REQUEST, "length must be greater than zero");
    }
    if req.length > usize::MAX as u64 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "requested length exceeds supported range",
        );
    }

    let manifest = match state.sorafs_node.manifest_metadata(&req.manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    let provider_id_from_request = if let Some(hex) = req.provider_id_hex.as_deref() {
        match decode_hex_32(hex) {
            Ok(bytes) => Some(bytes),
            Err(message) => return json_error(StatusCode::BAD_REQUEST, message),
        }
    } else {
        None
    };
    let provider_id =
        provider_id_from_request.or_else(|| state.sorafs_node.capacity_usage().provider_id);

    if let Err(response) =
        enforce_gateway_policy_for_request(&state, &headers, &manifest, provider_id, remote)
    {
        return response;
    }

    if state.sorafs_gateway_config.enforce_capabilities {
        match provider_id.as_ref() {
            Some(provider) => match state.provider_supports_chunk_range(provider) {
                Some(true) => {}
                Some(false) => {
                    return chunk_range_capability_missing_response(
                        &state,
                        manifest.chunk_profile_handle(),
                        provider,
                        TELEMETRY_ENDPOINT_CHUNK,
                    );
                }
                None => {
                    return chunk_range_capability_unknown_response(
                        &state,
                        manifest.chunk_profile_handle(),
                        provider,
                        TELEMETRY_ENDPOINT_CHUNK,
                    );
                }
            },
            None => {
                return chunk_range_capability_provider_missing_response(
                    &state,
                    manifest.chunk_profile_handle(),
                    TELEMETRY_ENDPOINT_CHUNK,
                );
            }
        }
    }

    let length = req.length as usize;
    let data = match state
        .sorafs_node
        .read_payload_range(&req.manifest_id_hex, req.offset, length)
    {
        Ok(bytes) => bytes,
        Err(err) => return node_storage_error_response(err),
    };

    record_storage_metrics(&state);

    let response = StorageFetchResponseDto {
        manifest_id_hex: req.manifest_id_hex,
        offset: req.offset,
        length: data.len() as u64,
        data_b64: base64::engine::general_purpose::STANDARD.encode(data),
    };

    let value = norito::json::to_value(&response).unwrap_or(Value::Null);
    (StatusCode::OK, JsonBody(value)).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_token(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    JsonOnly(req): JsonOnly<StreamTokenRequestDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let Some(issuer) = state.stream_token_issuer() else {
        return feature_disabled("stream token issuance is not enabled on this node");
    };

    let client_id = match headers.get(HEADER_SORA_CLIENT) {
        Some(value) => match value.to_str() {
            Ok(id) if !id.trim().is_empty() => id.to_string(),
            Ok(_) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "X-SoraFS-Client header must not be empty",
                );
            }
            Err(_) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "X-SoraFS-Client header must contain valid ASCII",
                );
            }
        },
        None => return json_error(StatusCode::BAD_REQUEST, "missing X-SoraFS-Client header"),
    };

    let nonce = match headers.get(HEADER_SORA_NONCE) {
        Some(value) => match value.to_str() {
            Ok(nonce) if !nonce.trim().is_empty() => nonce.to_string(),
            Ok(_) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "X-SoraFS-Nonce header must not be empty",
                );
            }
            Err(_) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "X-SoraFS-Nonce header must contain valid ASCII",
                );
            }
        },
        None => return json_error(StatusCode::BAD_REQUEST, "missing X-SoraFS-Nonce header"),
    };

    let manifest = match state.sorafs_node.manifest_metadata(&req.manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    let provider_id = match decode_hex_32(&req.provider_id_hex) {
        Ok(bytes) => bytes,
        Err(message) => return json_error(StatusCode::BAD_REQUEST, message),
    };

    let overrides = TokenOverrides {
        ttl_secs: req.ttl_secs,
        max_streams: req.max_streams,
        rate_limit_bytes: req.rate_limit_bytes,
        requests_per_minute: req.requests_per_minute,
    };

    let token_issue = match issuer.issue_token(
        &client_id,
        manifest.manifest_cid().to_vec(),
        provider_id,
        manifest.chunk_profile_handle().to_string(),
        overrides,
    ) {
        Ok(token) => token,
        Err(err) => {
            if let StreamTokenIssuerError::ClientQuotaExceeded {
                limit,
                retry_after_secs,
                ..
            } = err
            {
                let mut response = json_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    format!(
                        "stream token issuance quota exceeded (limit {limit} requests per minute)"
                    ),
                );
                let headers = response.headers_mut();
                headers.insert(
                    header::RETRY_AFTER,
                    HeaderValue::from_str(&retry_after_secs.to_string()).unwrap_or_else(|_| {
                        panic!("Retry-After header produced invalid value: {retry_after_secs}")
                    }),
                );
                headers.insert(
                    header::HeaderName::from_static(HEADER_SORA_CLIENT),
                    header_value(&client_id, "X-SoraFS-Client"),
                );
                headers.insert(
                    header::HeaderName::from_static(HEADER_SORA_NONCE),
                    header_value(&nonce, "X-SoraFS-Nonce"),
                );
                headers.insert(
                    header::HeaderName::from_static(HEADER_SORA_CLIENT_QUOTA_REMAINING),
                    header_value("0", "X-SoraFS-Client-Quota-Remaining"),
                );
                return response;
            }
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to issue stream token: {err}"),
            );
        }
    };

    let body_value = stream_token_body_json(&token_issue.token.body);
    let token_base64 = match encode_token_base64(&token_issue.token) {
        Ok(encoded) => encoded,
        Err(err) => {
            error!(?err, "failed to encode stream token payload");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to encode stream token",
            );
        }
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_NONCE),
        header_value(&nonce, "X-SoraFS-Nonce"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_CLIENT),
        header_value(&client_id, "X-SoraFS-Client"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_TOKEN_ID),
        header_value(&token_issue.token.body.token_id, "X-SoraFS-Token-Id"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_VERIFYING_KEY),
        header_value(
            &hex::encode(issuer.verifying_key_bytes()),
            "X-SoraFS-Verifying-Key",
        ),
    );
    let quota_header = token_issue
        .remaining_quota
        .map(|quota| quota.to_string())
        .unwrap_or_else(|| "unlimited".to_owned());
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_CLIENT_QUOTA_REMAINING),
        header_value(&quota_header, "X-SoraFS-Client-Quota-Remaining"),
    );
    response_headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));

    let token_value = json_object(vec![
        json_entry("body", body_value),
        json_entry(
            "signature_hex",
            Value::String(hex::encode(&token_issue.token.signature)),
        ),
        json_entry("encoded", Value::String(token_base64.clone())),
    ]);
    let response = json_object(vec![
        json_entry("token", token_value),
        json_entry("token_base64", Value::String(token_base64)),
    ]);
    (StatusCode::OK, response_headers, JsonBody(response)).into_response()
}

#[cfg(feature = "app_api")]
fn stream_token_body_json(body: &StreamTokenBodyV1) -> Value {
    let mut obj = Map::new();
    obj.insert("token_id".into(), Value::from(body.token_id.clone()));
    obj.insert(
        "manifest_cid_hex".into(),
        Value::from(body.manifest_cid.encode_hex::<String>()),
    );
    obj.insert(
        "provider_id_hex".into(),
        Value::from(body.provider_id.encode_hex::<String>()),
    );
    obj.insert(
        "profile_handle".into(),
        Value::from(body.profile_handle.clone()),
    );
    obj.insert("max_streams".into(), Value::from(body.max_streams));
    obj.insert("ttl_epoch".into(), Value::from(body.ttl_epoch));
    obj.insert(
        "rate_limit_bytes".into(),
        Value::from(body.rate_limit_bytes),
    );
    obj.insert("issued_at".into(), Value::from(body.issued_at));
    obj.insert(
        "requests_per_minute".into(),
        Value::from(body.requests_per_minute),
    );
    obj.insert(
        "token_pk_version".into(),
        Value::from(body.token_pk_version),
    );
    Value::Object(obj)
}

#[derive(Debug)]
struct RangeFetchConcurrencyGuard {
    telemetry: MaybeTelemetry,
    permit: Option<StreamTokenConcurrencyPermit>,
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
fn round_clamped_u32(value: f64) -> u32 {
    let rounded = value.round();
    let bounded = rounded.clamp(0.0, f64::from(u32::MAX));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        bounded as u32
    }
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
fn round_clamped_u64(value: f64) -> u64 {
    let rounded = value.round();
    let bounded = rounded.clamp(0.0, u64::MAX as f64);
    #[allow(clippy::cast_sign_loss)]
    {
        bounded as u64
    }
}

impl RangeFetchConcurrencyGuard {
    fn new(telemetry: MaybeTelemetry, permit: Option<StreamTokenConcurrencyPermit>) -> Self {
        if permit.is_some() {
            telemetry
                .with_metrics(iroha_core::telemetry::Telemetry::inc_sorafs_range_fetch_concurrency);
        }
        Self { telemetry, permit }
    }

    #[cfg(test)]
    fn has_permit(&self) -> bool {
        self.permit.is_some()
    }
}

impl Drop for RangeFetchConcurrencyGuard {
    fn drop(&mut self) {
        if self.permit.is_some() {
            self.telemetry
                .with_metrics(iroha_core::telemetry::Telemetry::dec_sorafs_range_fetch_concurrency);
        }
    }
}

#[allow(clippy::result_large_err)]
fn enforce_stream_token_for_request(
    state: &SharedAppState,
    headers: &HeaderMap,
    manifest: &StoredManifest,
    requested_bytes: u64,
) -> Result<(RangeFetchConcurrencyGuard, StreamTokenBodyV1), Response> {
    let Some(issuer) = state.stream_token_issuer() else {
        return Err(feature_disabled(
            "stream token enforcement is not enabled on this node",
        ));
    };
    let telemetry = state.telemetry.clone();

    let token_header = headers.get(HEADER_SORA_STREAM_TOKEN).ok_or_else(|| {
        json_error(
            StatusCode::UNAUTHORIZED,
            "missing X-SoraFS-Stream-Token header",
        )
    })?;
    let token_str = token_header
        .to_str()
        .map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "X-SoraFS-Stream-Token header must contain valid ASCII",
            )
        })?
        .trim();

    if token_str.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "X-SoraFS-Stream-Token header must not be empty",
        ));
    }

    let token = match decode_token_base64(token_str) {
        Ok(token) => token,
        Err(StreamTokenHeaderError::InvalidEncoding) => {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "stream token must be base64 encoded",
            ));
        }
        Err(StreamTokenHeaderError::InvalidPayload(_)) => {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "invalid stream token payload",
            ));
        }
    };

    if let Err(err) = token.verify(issuer.verifying_key()) {
        error!(?err, "stream token signature verification failed");
        return Err(json_error(
            StatusCode::UNAUTHORIZED,
            "stream token signature invalid",
        ));
    }

    if token.body.token_pk_version != issuer.key_version() {
        return Err(json_error(
            StatusCode::UNAUTHORIZED,
            "stream token signed with unexpected key version",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            error!(
                ?err,
                "system clock before UNIX epoch while validating token"
            );
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to validate stream token",
            )
        })?
        .as_secs();
    if now > token.body.ttl_epoch {
        return Err(json_error(
            StatusCode::UNAUTHORIZED,
            "stream token has expired",
        ));
    }

    if token.body.manifest_cid.as_slice() != manifest.manifest_cid() {
        return Err(json_error(
            StatusCode::FORBIDDEN,
            "stream token does not authorise this manifest",
        ));
    }
    if token.body.profile_handle != manifest.chunk_profile_handle() {
        return Err(json_error(
            StatusCode::CONFLICT,
            "stream token chunker handle mismatch",
        ));
    }

    if let Some(provider_id) = state.sorafs_node.capacity_usage().provider_id {
        if provider_id != token.body.provider_id {
            return Err(json_error(
                StatusCode::FORBIDDEN,
                "stream token provider mismatch",
            ));
        }
    }

    match state
        .stream_token_quota()
        .try_acquire(&token.body.token_id, token.body.requests_per_minute)
    {
        Ok(()) => {}
        Err(StreamTokenQuotaExceeded { retry_after_secs }) => {
            telemetry.with_metrics(|metrics| {
                metrics.inc_sorafs_range_fetch_throttle(RANGE_THROTTLE_REASON_QUOTA)
            });
            let mut response = json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "stream token request quota exceeded",
            );
            if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
            return Err(response);
        }
    }

    let concurrency_permit = match state
        .stream_token_concurrency()
        .try_acquire(&token.body.token_id, token.body.max_streams)
    {
        Ok(guard) => guard,
        Err(_) => {
            telemetry.with_metrics(|metrics| {
                metrics.inc_sorafs_range_fetch_throttle(RANGE_THROTTLE_REASON_CONCURRENCY)
            });
            return Err(json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "stream token max_streams exceeded",
            ));
        }
    };
    let concurrency_guard = RangeFetchConcurrencyGuard::new(telemetry.clone(), concurrency_permit);

    let allowed_bytes = token.body.rate_limit_bytes;
    if allowed_bytes != 0 && requested_bytes > allowed_bytes {
        telemetry.with_metrics(|metrics| {
            metrics.inc_sorafs_range_fetch_throttle(RANGE_THROTTLE_REASON_BYTE_RATE)
        });
        drop(concurrency_guard);
        return Err(json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "stream token rate limit exceeded",
        ));
    }

    Ok((concurrency_guard, token.body))
}

fn manifest_envelope_present(headers: &HeaderMap) -> bool {
    let has_full_envelope = headers
        .get(HEADER_SORA_MANIFEST_ENVELOPE)
        .map(|value| !value.as_bytes().is_empty())
        .unwrap_or(false);
    if has_full_envelope {
        return true;
    }

    let proof_b64 = headers
        .get(HEADER_SORA_PROOF)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let alias = headers
        .get(HEADER_SORA_NAME)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (proof_b64, alias) = match (proof_b64, alias) {
        (Some(proof_b64), Some(alias)) => (proof_b64, alias),
        _ => return false,
    };
    let proof_bytes = match BASE64_STANDARD.decode(proof_b64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let bundle = match crate::sorafs::decode_alias_proof(&proof_bytes) {
        Ok(bundle) => bundle,
        Err(_) => return false,
    };
    if !bundle.binding.alias.eq_ignore_ascii_case(alias) {
        return false;
    }
    if let Some(cid) = headers
        .get(HEADER_SORA_CID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let decoded = match hex::decode(cid) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        if decoded != bundle.binding.manifest_cid {
            return false;
        }
    }
    true
}

fn parse_policy_tags_header(headers: &HeaderMap, name: &str) -> Vec<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_cache_control_max_age(value: &str) -> Option<u64> {
    value.split(',').find_map(|directive| {
        let trimmed = directive.trim();
        let rest = trimmed.strip_prefix("max-age=")?;
        rest.trim().parse::<u64>().ok()
    })
}

fn parse_cache_ttl(headers: &HeaderMap) -> Option<u64> {
    if let Some(raw) = headers
        .get(HEADER_SORA_CACHE_TTL)
        .and_then(|value| value.to_str().ok())
    {
        if let Ok(ttl) = raw.trim().parse::<u64>() {
            return Some(ttl);
        }
    }

    headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_cache_control_max_age)
}

fn parse_gateway_region(headers: &HeaderMap) -> Option<String> {
    headers
        .get(HEADER_SORA_REGION)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_perceptual_fingerprint_header(
    headers: &HeaderMap,
    name: &str,
    label: &str,
) -> Result<Option<[u8; 32]>, Response> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    let raw = value.to_str().map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("{label} header must be valid ASCII"),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("{label} header must not be empty"),
        ));
    }
    let decoded = hex::decode(trimmed).map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("{label} header must be 32-byte hex"),
        )
    })?;
    let bytes: [u8; 32] = decoded.try_into().map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("{label} header must be 32-byte hex"),
        )
    })?;
    Ok(Some(bytes))
}

#[derive(Debug)]
struct ManifestResolution {
    manifest_id: String,
    blinded_b64: Option<String>,
}

fn resolve_manifest_request(
    state: &SharedAppState,
    manifest_param: &str,
    headers: &HeaderMap,
) -> Result<ManifestResolution, Response> {
    let Some(raw_blinded) = headers.get(HEADER_SORA_REQ_BLINDED_CID) else {
        return Ok(ManifestResolution {
            manifest_id: manifest_param.to_owned(),
            blinded_b64: None,
        });
    };

    let blinded_str = raw_blinded.to_str().map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            "Sora-Req-Blinded-CID header must contain ASCII data",
        )
    })?;
    let trimmed = blinded_str.trim();
    if trimmed.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Sora-Req-Blinded-CID header must not be empty",
        ));
    }
    let decoded = URL_SAFE_NO_PAD.decode(trimmed.as_bytes()).map_err(|err| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("invalid Sora-Req-Blinded-CID header: {err}"),
        )
    })?;
    if decoded.len() != BLINDED_CID_LEN {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("Sora-Req-Blinded-CID must decode to {BLINDED_CID_LEN} bytes"),
        ));
    }
    let blinded_array: [u8; BLINDED_CID_LEN] = decoded
        .as_slice()
        .try_into()
        .expect("length checked above; qed");

    if let Some(nonce_header) = headers.get(HEADER_SORA_REQ_NONCE) {
        nonce_header.to_str().map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "Sora-Req-Nonce header must contain ASCII data",
            )
        })?;
    }

    let epoch_header = headers.get(HEADER_SORA_REQ_SALT_EPOCH).ok_or_else(|| {
        json_error(
            StatusCode::PRECONDITION_REQUIRED,
            "Sora-Req-Salt-Epoch header is required when requesting by blinded CID",
        )
    })?;
    let epoch_str = epoch_header.to_str().map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            "Sora-Req-Salt-Epoch header must contain ASCII digits",
        )
    })?;
    let epoch = epoch_str.trim().parse::<u32>().map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            "Sora-Req-Salt-Epoch header must be an unsigned integer",
        )
    })?;

    let resolver = state.blinded_resolver().ok_or_else(|| {
        json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "blinded CID support is not configured on this gateway",
        )
    })?;
    let storage = state
        .sorafs_node
        .storage()
        .ok_or_else(storage_disabled_response)?;
    let manifests = storage.manifests();

    let manifest_id = match resolver.resolve_manifest_id(&manifests, epoch, &blinded_array) {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Err(json_error(
                StatusCode::NOT_FOUND,
                "no manifest matches the supplied blinded CID",
            ));
        }
        Err(crate::sorafs::BlindedResolveError::UnknownEpoch(missing_epoch)) => {
            return Err(json_error(
                StatusCode::PRECONDITION_REQUIRED,
                format!("salt epoch {missing_epoch} is not available on this gateway"),
            ));
        }
    };

    if !manifest_param.is_empty()
        && manifest_param != BLINDED_PATH_PLACEHOLDER
        && manifest_param != manifest_id
    {
        return Err(json_error(
            StatusCode::CONFLICT,
            "manifest identifier in the path does not match the blinded CID",
        ));
    }

    Ok(ManifestResolution {
        manifest_id,
        blinded_b64: Some(trimmed.to_owned()),
    })
}

fn gateway_client_fingerprint(remote: SocketAddr, headers: &HeaderMap) -> ClientFingerprint {
    let mut identifier = remote.ip().to_string();
    if let Some(client_header) = headers
        .get(HEADER_SORA_CLIENT)
        .and_then(|value| value.to_str().ok())
    {
        let trimmed = client_header.trim();
        if !trimmed.is_empty() {
            identifier.push('|');
            identifier.push_str(trimmed);
        }
    }
    ClientFingerprint::from_identifier(&identifier)
}

#[allow(clippy::result_large_err)]
fn enforce_gateway_policy_for_request(
    state: &SharedAppState,
    headers: &HeaderMap,
    manifest: &StoredManifest,
    provider_id: Option<[u8; 32]>,
    remote: SocketAddr,
) -> Result<(), Response> {
    let fingerprint = gateway_client_fingerprint(remote, headers);
    let now = SystemTime::now();
    let monotonic_now = Instant::now();
    let mut context = RequestContext::new(&fingerprint, now, monotonic_now)
        .with_manifest_digest(manifest.manifest_digest())
        .with_content_cid(manifest.manifest_cid())
        .with_manifest_envelope(manifest_envelope_present(headers))
        .with_remote_addr(remote);

    if let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    {
        context = context.with_canonical_host(host.to_string());
    }

    if let Some(region) = parse_gateway_region(headers) {
        context = context.with_region(region);
    }

    if let Some(ttl) = parse_cache_ttl(headers) {
        context = context.with_cache_ttl_secs(ttl);
    }

    let policy_tags = parse_policy_tags_header(headers, HEADER_SORA_POLICY_TAGS);
    if !policy_tags.is_empty() {
        context = context.with_policy_tags(policy_tags);
    }

    let moderation_slugs = parse_policy_tags_header(headers, HEADER_SORA_MODERATION_SLUGS);
    if !moderation_slugs.is_empty() {
        context = context.with_moderation_slugs(moderation_slugs);
    }

    let perceptual_hash = parse_perceptual_fingerprint_header(
        headers,
        HEADER_SORA_PERCEPTUAL_HASH,
        "x-sorafs-perceptual-hash",
    )?;
    let perceptual_embedding = parse_perceptual_fingerprint_header(
        headers,
        HEADER_SORA_PERCEPTUAL_EMBEDDING,
        "x-sorafs-perceptual-embedding",
    )?;
    if perceptual_hash.is_some() || perceptual_embedding.is_some() {
        let observation =
            PerceptualObservation::new(perceptual_hash.as_ref(), perceptual_embedding.as_ref());
        context = context.with_perceptual_observation(observation);
    }

    if let Some(provider_id) = provider_id.as_ref() {
        context = context.with_provider_id(provider_id);
    }

    match state.evaluate_gateway_policy(context) {
        Ok(()) => Ok(()),
        Err(violation) => {
            let (policy_reason, policy_detail) = violation.telemetry_labels();
            let provider_hex = provider_id.as_ref().map(hex::encode).unwrap_or_default();
            warn!(
                ?violation,
                client = ?remote,
                policy_reason,
                policy_detail,
                provider_id_hex = provider_hex,
                "gateway policy denied request"
            );
            Err(gateway_policy_violation_response(
                violation,
                provider_id.as_ref(),
            ))
        }
    }
}

fn format_system_time(value: SystemTime) -> Option<String> {
    let datetime = OffsetDateTime::from(value).replace_nanosecond(0).ok()?;
    datetime.format(&Rfc3339).ok()
}

fn gateway_policy_violation_response(
    violation: PolicyViolation,
    provider_id: Option<&[u8; 32]>,
) -> Response {
    match violation {
        PolicyViolation::ManifestEnvelopeMissing => {
            let body = json_object(vec![
                json_entry("error", Value::from("manifest_envelope_required")),
                json_entry(
                    "message",
                    Value::from("manifest envelope must be attached to the request"),
                ),
            ]);
            (StatusCode::PRECONDITION_REQUIRED, JsonBody(body)).into_response()
        }
        PolicyViolation::MissingProviderId => {
            let body = json_object(vec![
                json_entry("error", Value::from("provider_id_missing")),
                json_entry(
                    "message",
                    Value::from("stream token provider identifier is required"),
                ),
            ]);
            (StatusCode::PRECONDITION_REQUIRED, JsonBody(body)).into_response()
        }
        PolicyViolation::AdmissionUnavailable => {
            let body = json_object(vec![
                json_entry("error", Value::from("admission_unavailable")),
                json_entry(
                    "message",
                    Value::from("gateway admission registry is unavailable"),
                ),
            ]);
            (StatusCode::PRECONDITION_FAILED, JsonBody(body)).into_response()
        }
        PolicyViolation::ProviderNotAdmitted { provider_id } => {
            let body = json_object(vec![
                json_entry("error", Value::from("provider_not_admitted")),
                json_entry("provider_id_hex", Value::from(hex::encode(provider_id))),
                json_entry(
                    "message",
                    Value::from("provider is not admitted according to governance records"),
                ),
            ]);
            (StatusCode::PRECONDITION_FAILED, JsonBody(body)).into_response()
        }
        PolicyViolation::Denylisted(hit) => {
            let hit = hit.as_ref();
            let kind_label = match hit.kind() {
                DenylistKind::Provider(_) => "provider",
                DenylistKind::ManifestDigest(_) => "manifest",
                DenylistKind::Cid(_) => "cid",
                DenylistKind::Url(_) => "url",
                DenylistKind::AccountId(_) => "account_id",
                DenylistKind::AccountAlias(_) => "account_alias",
                DenylistKind::PerceptualFamily { .. } => "perceptual_family",
            };

            let mut entries = vec![
                json_entry("error", Value::from("denylisted")),
                json_entry("kind", Value::from(kind_label)),
            ];

            match hit.kind() {
                DenylistKind::Provider(id) | DenylistKind::ManifestDigest(id) => {
                    entries.push(json_entry("value_hex", Value::from(hex::encode(id))));
                }
                DenylistKind::Cid(cid) => {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(cid);
                    entries.push(json_entry("cid_b64", Value::from(encoded)));
                }
                DenylistKind::Url(url) => {
                    entries.push(json_entry("url", Value::from(url.clone())));
                }
                DenylistKind::AccountId(account_id) => {
                    entries.push(json_entry("account_id", Value::from(account_id.clone())));
                }
                DenylistKind::AccountAlias(alias) => {
                    entries.push(json_entry("account_alias", Value::from(alias.clone())));
                }
                DenylistKind::PerceptualFamily {
                    family_id,
                    variant_id,
                } => {
                    entries.push(json_entry(
                        "family_id_hex",
                        Value::from(hex::encode(family_id)),
                    ));
                    if let Some(variant) = variant_id {
                        entries.push(json_entry(
                            "variant_id_hex",
                            Value::from(hex::encode(variant)),
                        ));
                    }
                    if let Some(perceptual) = hit.perceptual_match() {
                        match perceptual.basis() {
                            PerceptualMatchBasis::Hash {
                                expected,
                                hamming_distance,
                                radius,
                                ..
                            } => {
                                entries.push(json_entry(
                                    "perceptual_hash_hex",
                                    Value::from(hex::encode(expected)),
                                ));
                                entries.push(json_entry(
                                    "perceptual_hamming_distance",
                                    Value::from(*hamming_distance as u64),
                                ));
                                entries.push(json_entry(
                                    "perceptual_hamming_radius",
                                    Value::from(*radius as u64),
                                ));
                            }
                            PerceptualMatchBasis::Embedding { expected, .. } => {
                                entries.push(json_entry(
                                    "perceptual_embedding_hex",
                                    Value::from(hex::encode(expected)),
                                ));
                            }
                        }
                        if let Some(attack) = perceptual.attack_vector() {
                            entries
                                .push(json_entry("attack_vector", Value::from(attack.to_string())));
                        }
                    }
                }
            }
            if let Some(provider) = provider_id {
                entries.push(json_entry(
                    "provider_id_hex",
                    Value::from(hex::encode(provider)),
                ));
            }

            if let Some(alias) = hit.entry().alias() {
                if !matches!(hit.kind(), DenylistKind::AccountAlias(_)) {
                    entries.push(json_entry("alias", Value::from(alias.to_string())));
                }
            }

            if let Some(jurisdiction) = hit.entry().jurisdiction() {
                entries.push(json_entry(
                    "jurisdiction",
                    Value::from(jurisdiction.to_string()),
                ));
            }
            if let Some(reason) = hit.entry().reason() {
                entries.push(json_entry("reason", Value::from(reason.to_string())));
            }
            if let Some(expires_at) = hit.entry().expires_at() {
                if let Some(formatted) = format_system_time(expires_at) {
                    entries.push(json_entry("expires_at", Value::from(formatted)));
                }
            }

            let body = json_object(entries);
            (StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, JsonBody(body)).into_response()
        }
        PolicyViolation::RateLimited(error) => {
            let mut headers = HeaderMap::new();
            let (status, code, retry_after) = match error {
                RateLimitError::Limited { retry_after } => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    Some(retry_after),
                ),
                RateLimitError::Banned { retry_after } => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "temporarily_banned",
                    retry_after,
                ),
            };

            if let Some(duration) = retry_after {
                let seconds = duration.as_secs().max(1);
                headers.insert(
                    RETRY_AFTER,
                    HeaderValue::from_str(&seconds.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("1")),
                );
            }

            let body = json_object(vec![
                json_entry("error", Value::from(code)),
                json_entry(
                    "message",
                    Value::from("gateway rejected the request due to rate limiting"),
                ),
            ]);

            (status, headers, JsonBody(body)).into_response()
        }
        PolicyViolation::CdnTtlExceeded {
            allowed_secs,
            observed_secs,
        } => {
            let mut entries = vec![
                json_entry("error", Value::from("ttl_override_required")),
                json_entry("allowed_ttl_secs", Value::from(allowed_secs)),
                json_entry(
                    "message",
                    Value::from("request cache TTL exceeds GAR policy"),
                ),
            ];
            if let Some(observed) = observed_secs {
                entries.push(json_entry(
                    "observed_ttl_secs",
                    Value::Number(Number::from(observed)),
                ));
            }
            (
                StatusCode::PRECONDITION_FAILED,
                JsonBody(json_object(entries)),
            )
                .into_response()
        }
        PolicyViolation::CdnPurgeRequired { required_tags } => {
            let body = json_object(vec![
                json_entry("error", Value::from("purge_required")),
                json_entry(
                    "required_tags",
                    Value::Array(
                        required_tags
                            .iter()
                            .cloned()
                            .map(Value::from)
                            .collect::<Vec<Value>>(),
                    ),
                ),
                json_entry(
                    "message",
                    Value::from("gar policy requires purge tags before serving content"),
                ),
            ]);
            (StatusCode::PRECONDITION_REQUIRED, JsonBody(body)).into_response()
        }
        PolicyViolation::CdnModerationRequired { required_slugs } => {
            let body = json_object(vec![
                json_entry("error", Value::from("moderation_required")),
                json_entry(
                    "required_slugs",
                    Value::Array(
                        required_slugs
                            .iter()
                            .cloned()
                            .map(Value::from)
                            .collect::<Vec<Value>>(),
                    ),
                ),
                json_entry(
                    "message",
                    Value::from("gar policy requires moderation before serving content"),
                ),
            ]);
            (StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, JsonBody(body)).into_response()
        }
        PolicyViolation::CdnRateCeilingExceeded {
            ceiling_rps,
            retry_after,
        } => {
            let mut headers = HeaderMap::new();
            if let Some(duration) = retry_after {
                let seconds = duration.as_secs().max(1);
                headers.insert(
                    RETRY_AFTER,
                    HeaderValue::from_str(&seconds.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("1")),
                );
            }
            let body = json_object(vec![
                json_entry("error", Value::from("cdn_rate_ceiling")),
                json_entry("ceiling_rps", Value::from(ceiling_rps)),
                json_entry(
                    "message",
                    Value::from("gar policy rate ceiling reached for this host"),
                ),
            ]);
            (StatusCode::TOO_MANY_REQUESTS, headers, JsonBody(body)).into_response()
        }
        PolicyViolation::CdnGeofenceDenied { region } => {
            let mut entries = vec![
                json_entry("error", Value::from("geofence_denied")),
                json_entry(
                    "message",
                    Value::from("gar policy blocks this region or requires region metadata"),
                ),
            ];
            if let Some(region) = region {
                entries.push(json_entry("region", Value::from(region.clone())));
            }
            (
                StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                JsonBody(json_object(entries)),
            )
                .into_response()
        }
        PolicyViolation::CdnLegalHoldActive => {
            let body = json_object(vec![
                json_entry("error", Value::from("legal_hold")),
                json_entry(
                    "message",
                    Value::from("gar policy is under legal hold; serving is blocked"),
                ),
            ]);
            (StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, JsonBody(body)).into_response()
        }
    }
}

#[cfg(test)]
mod gateway_policy_violation_tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use axum::body::to_bytes;
    use tokio::runtime::Runtime;

    use super::*;
    use crate::sorafs::gateway::{
        DenylistEntryBuilder, DenylistHit, DenylistKind, PerceptualMatch,
    };

    fn response_json(response: Response) -> Value {
        let runtime = Runtime::new().expect("tokio runtime");
        runtime.block_on(async {
            let bytes = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("collect response body");
            norito::json::from_slice(&bytes).expect("decode response json")
        })
    }

    #[test]
    fn policy_status_codes_are_normalized() {
        let response =
            gateway_policy_violation_response(PolicyViolation::ManifestEnvelopeMissing, None);
        let status = response.status();
        let body = response_json(response);
        assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(
            body.get("error").and_then(Value::as_str),
            Some("manifest_envelope_required")
        );

        let response = gateway_policy_violation_response(PolicyViolation::MissingProviderId, None);
        let status = response.status();
        let body = response_json(response);
        assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(
            body.get("error").and_then(Value::as_str),
            Some("provider_id_missing")
        );

        let response =
            gateway_policy_violation_response(PolicyViolation::AdmissionUnavailable, None);
        let status = response.status();
        let body = response_json(response);
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);
        assert_eq!(
            body.get("error").and_then(Value::as_str),
            Some("admission_unavailable")
        );

        let denylisted = DenylistHit::new_for_tests(
            DenylistKind::Provider([0xAA; 32]),
            DenylistEntryBuilder::default().build(),
            None,
        );
        let response = gateway_policy_violation_response(
            PolicyViolation::Denylisted(Box::new(denylisted)),
            Some(&[0xAA; 32]),
        );
        let status = response.status();
        let body = response_json(response);
        assert_eq!(status, StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        assert_eq!(
            body.get("error").and_then(Value::as_str),
            Some("denylisted")
        );
    }

    #[test]
    fn policy_matrix_fixture_matches_handlers() {
        let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/sorafs_gateway/1.0.0/policy_matrix.json");
        let bytes = fs::read(&matrix_path).expect("read policy matrix fixture");
        let value: Value = norito::json::from_slice(&bytes).expect("parse policy matrix");
        let array = value
            .as_array()
            .expect("policy matrix must be an array of objects");
        let mut entries = HashMap::new();
        for entry in array {
            let obj = entry
                .as_object()
                .expect("policy matrix entry must be an object");
            let id = obj
                .get("id")
                .and_then(Value::as_str)
                .expect("policy matrix entry missing id");
            let status = obj
                .get("status")
                .and_then(Value::as_u64)
                .and_then(|code| u16::try_from(code).ok())
                .expect("policy matrix entry missing status");
            let error = obj.get("error").and_then(Value::as_str).map(str::to_owned);
            entries.insert(id.to_string(), (status, error));
        }

        let check_response =
            |id: &str, response: Response, entries: &HashMap<String, (u16, Option<String>)>| {
                let status = response.status().as_u16();
                let body = response_json(response);
                let error = body.get("error").and_then(Value::as_str).map(str::to_owned);
                let Some((expected_status, expected_error)) = entries.get(id) else {
                    panic!("policy matrix entry {id} missing");
                };
                assert_eq!(&status, expected_status, "status mismatch for {id}");
                assert_eq!(&error, expected_error, "error mismatch for {id}");
            };

        check_response(
            "B2",
            gateway_policy_violation_response(PolicyViolation::ManifestEnvelopeMissing, None),
            &entries,
        );
        check_response(
            "B5",
            gateway_policy_violation_response(
                PolicyViolation::ProviderNotAdmitted {
                    provider_id: [0xBB; 32],
                },
                Some(&[0xBB; 32]),
            ),
            &entries,
        );
        let denylisted = DenylistHit::new_for_tests(
            DenylistKind::Provider([0xAA; 32]),
            DenylistEntryBuilder::default().build(),
            None,
        );
        check_response(
            "D1",
            gateway_policy_violation_response(
                PolicyViolation::Denylisted(Box::new(denylisted)),
                Some(&[0xAA; 32]),
            ),
            &entries,
        );
    }

    #[test]
    fn denylisted_perceptual_response_exposes_metadata() {
        let entry = DenylistEntryBuilder::default()
            .reason("perceptual block")
            .build();
        let family_id = [0x11; 16];
        let variant_id = [0x22; 16];
        let canonical_hash = [0xAA; 32];
        let observed_hash = [0xAB; 32];
        let perceptual_match =
            PerceptualMatch::hash(observed_hash, canonical_hash, 3, 6, Some("attack".into()));
        let hit = DenylistHit::new_for_tests(
            DenylistKind::PerceptualFamily {
                family_id,
                variant_id: Some(variant_id),
            },
            entry,
            Some(perceptual_match),
        );
        let response = gateway_policy_violation_response(
            PolicyViolation::Denylisted(Box::new(hit)),
            Some(&[0xCC; 32]),
        );
        let status = response.status();
        let body = response_json(response);
        assert_eq!(status, StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        assert_eq!(
            body.get("kind").and_then(Value::as_str),
            Some("perceptual_family")
        );
        let family_hex = hex::encode(family_id);
        assert_eq!(
            body.get("family_id_hex").and_then(Value::as_str),
            Some(family_hex.as_str())
        );
        let variant_hex = hex::encode(variant_id);
        assert_eq!(
            body.get("variant_id_hex").and_then(Value::as_str),
            Some(variant_hex.as_str())
        );
        let hash_hex = hex::encode(canonical_hash);
        assert_eq!(
            body.get("perceptual_hash_hex").and_then(Value::as_str),
            Some(hash_hex.as_str())
        );
        assert_eq!(
            body.get("perceptual_hamming_distance")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            body.get("perceptual_hamming_radius")
                .and_then(Value::as_u64),
            Some(6)
        );
        assert_eq!(
            body.get("attack_vector").and_then(Value::as_str),
            Some("attack")
        );
        assert_eq!(
            body.get("provider_id_hex").and_then(Value::as_str),
            Some(hex::encode([0xCC; 32]).as_str())
        );
    }

    #[test]
    fn manifest_envelope_detection_covers_required_paths() {
        let mut headers = HeaderMap::new();
        assert!(!manifest_envelope_present(&headers));

        headers.insert(
            HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
            HeaderValue::from_static("ZW52"),
        );
        assert!(manifest_envelope_present(&headers));

        let mut proof_headers = HeaderMap::new();
        proof_headers.insert(
            HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );
        proof_headers.insert(
            HeaderName::from_static(HEADER_SORA_NAME),
            HeaderValue::from_static("alias/test"),
        );
        assert!(manifest_envelope_present(&proof_headers));

        let mut mismatched = HeaderMap::new();
        mismatched.insert(
            HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );
        mismatched.insert(
            HeaderName::from_static(HEADER_SORA_NAME),
            HeaderValue::from_static("alias/other"),
        );
        assert!(!manifest_envelope_present(&mismatched));
    }
}

fn chunk_range_capability_missing_response(
    state: &SharedAppState,
    profile: &str,
    provider_id: &[u8; 32],
    scope: &'static str,
) -> Response {
    gateway_refusal_response(
        state,
        StatusCode::PRECONDITION_FAILED,
        "capability_missing",
        "provider advert does not declare chunk_range_fetch capability",
        Some(profile),
        Some(provider_id),
        scope,
        [
            ("capability", Value::from("chunk_range_fetch")),
            ("provider_id_hex", Value::from(hex::encode(provider_id))),
        ],
    )
}

fn chunk_range_capability_unknown_response(
    state: &SharedAppState,
    profile: &str,
    provider_id: &[u8; 32],
    scope: &'static str,
) -> Response {
    gateway_refusal_response(
        state,
        StatusCode::PRECONDITION_FAILED,
        "capability_state_unknown",
        "gateway cannot confirm provider capability advertisement",
        Some(profile),
        Some(provider_id),
        scope,
        [
            ("capability", Value::from("chunk_range_fetch")),
            ("provider_id_hex", Value::from(hex::encode(provider_id))),
        ],
    )
}

fn chunk_range_capability_provider_missing_response(
    state: &SharedAppState,
    profile: &str,
    scope: &'static str,
) -> Response {
    gateway_refusal_response(
        state,
        StatusCode::PRECONDITION_REQUIRED,
        "provider_id_missing",
        "provider identifier is required for capability enforcement",
        Some(profile),
        None,
        scope,
        [],
    )
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_storage_car_range(
    State(state): State<SharedAppState>,
    Path(manifest_id_hex): Path<String>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let ManifestResolution {
        manifest_id,
        blinded_b64,
    } = match resolve_manifest_request(&state, &manifest_id_hex, &headers) {
        Ok(resolution) => resolution,
        Err(response) => return response,
    };
    let manifest_id_hex = manifest_id;

    let request_timer = Instant::now();
    let request_wall_start = SystemTime::now();
    let potr_probe = match begin_potr_probe(&headers, request_wall_start, request_timer) {
        Ok(probe) => probe,
        Err(response) => return response,
    };

    let range_header = match headers.get(header::RANGE) {
        Some(value) => value,
        None => return json_error(StatusCode::BAD_REQUEST, "missing Range header"),
    };
    let range_str = match range_header.to_str() {
        Ok(value) => value,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Range header must be valid ASCII"),
    };

    let manifest = match state.sorafs_node.manifest_metadata(&manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    if let Err(response) = enforce_chunker_support_gateway(
        &state,
        manifest.chunk_profile_handle(),
        TELEMETRY_ENDPOINT_CAR_RANGE,
    ) {
        return response;
    }

    let manifest_profile = manifest.chunk_profile_handle().to_string();

    if let Some(value) = headers.get(HEADER_DAG_SCOPE) {
        match value.to_str() {
            Ok(scope) if scope.eq_ignore_ascii_case("block") => {}
            Ok(received) => {
                return gateway_refusal_response(
                    &state,
                    StatusCode::PRECONDITION_REQUIRED,
                    "missing_header",
                    "dag-scope header must equal 'block'",
                    Some(&manifest_profile),
                    None,
                    TELEMETRY_ENDPOINT_CAR_RANGE,
                    [
                        ("header", Value::from("Sora-Dag-Scope")),
                        ("expected", Value::from("block")),
                        ("received", Value::from(received.to_string())),
                    ],
                );
            }
            Err(_) => {
                return gateway_refusal_response(
                    &state,
                    StatusCode::PRECONDITION_REQUIRED,
                    "missing_header",
                    "dag-scope header must be valid ASCII",
                    Some(&manifest_profile),
                    None,
                    TELEMETRY_ENDPOINT_CAR_RANGE,
                    [
                        ("header", Value::from("Sora-Dag-Scope")),
                        ("error", Value::from("invalid_ascii")),
                    ],
                );
            }
        }
    } else {
        return gateway_refusal_response(
            &state,
            StatusCode::PRECONDITION_REQUIRED,
            "missing_header",
            "dag-scope header is required for trustless range requests",
            Some(&manifest_profile),
            None,
            TELEMETRY_ENDPOINT_CAR_RANGE,
            [("header", Value::from("Sora-Dag-Scope"))],
        );
    }

    let nonce_header = match headers.get(HEADER_SORA_NONCE) {
        Some(value) => value.clone(),
        None => return json_error(StatusCode::BAD_REQUEST, "missing X-SoraFS-Nonce header"),
    };

    let chunker_header = match headers.get(HEADER_SORA_CHUNKER) {
        Some(value) => match value.to_str() {
            Ok(chunker) => chunker.trim(),
            Err(_) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "X-SoraFS-Chunker header must be valid ASCII",
                );
            }
        },
        None => return json_error(StatusCode::BAD_REQUEST, "missing X-SoraFS-Chunker header"),
    };

    let manifest = match state.sorafs_node.manifest_metadata(&manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    if let Err(response) = enforce_chunker_support_gateway(
        &state,
        manifest.chunk_profile_handle(),
        TELEMETRY_ENDPOINT_CHUNK,
    ) {
        return response;
    }

    let manifest_profile = manifest.chunk_profile_handle().to_string();

    if !chunker_header.eq_ignore_ascii_case(manifest.chunk_profile_handle()) {
        return gateway_refusal_response(
            &state,
            StatusCode::NOT_ACCEPTABLE,
            "unsupported_chunker",
            format!("chunk profile {chunker_header} is not enabled on this gateway"),
            Some(&manifest_profile),
            None,
            TELEMETRY_ENDPOINT_CAR_RANGE,
            [
                ("profile", Value::from(chunker_header.to_string())),
                ("manifest_profile", Value::from(manifest_profile.clone())),
            ],
        );
    }

    if let Some(value) = headers.get(header::ACCEPT_ENCODING) {
        if let Ok(raw_encodings) = value.to_str() {
            let gzip_requested = raw_encodings
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("gzip"));
            if gzip_requested {
                return gateway_refusal_response(
                    &state,
                    StatusCode::NOT_ACCEPTABLE,
                    "unsupported_encoding",
                    format!("gzip compression is not allowed for {}", manifest_profile),
                    Some(&manifest_profile),
                    None,
                    TELEMETRY_ENDPOINT_CAR_RANGE,
                    [("encoding", Value::from("gzip"))],
                );
            }
        }
    }

    let alias_header = headers
        .get(HEADER_SORA_NAME)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let total_length = manifest.content_length();
    if total_length == 0 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "manifest payload is empty; range requests are not supported",
        );
    }

    let byte_range = match parse_range_header(range_str, total_length) {
        Ok(range) => range,
        Err(RangeParseError::Invalid(message)) => {
            return json_error(StatusCode::BAD_REQUEST, message);
        }
        Err(RangeParseError::Unsatisfiable(message)) => {
            return range_not_satisfiable(total_length, message);
        }
    };

    let length = byte_range.len();
    if length == 0 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "requested range length must be greater than zero",
        );
    }
    if length > usize::MAX as u64 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "requested range exceeds server limits",
        );
    }

    let expected_chunk_count = match ensure_chunk_alignment(&manifest, byte_range) {
        Ok(count) => count,
        Err(message) => return range_not_satisfiable(total_length, message),
    };

    let (stream_token_guard, stream_token_body) =
        match enforce_stream_token_for_request(&state, &headers, &manifest, length) {
            Ok(result) => result,
            Err(response) => return response,
        };
    let provider_id = state
        .sorafs_node
        .capacity_usage()
        .provider_id
        .or(Some(stream_token_body.provider_id));
    if let Err(response) =
        enforce_gateway_policy_for_request(&state, &headers, &manifest, provider_id, remote)
    {
        return response;
    }
    if state.sorafs_gateway_config.enforce_capabilities {
        match provider_id.as_ref() {
            Some(provider) => match state.provider_supports_chunk_range(provider) {
                Some(true) => {}
                Some(false) => {
                    drop(stream_token_guard);
                    return chunk_range_capability_missing_response(
                        &state,
                        &manifest_profile,
                        provider,
                        TELEMETRY_ENDPOINT_CAR_RANGE,
                    );
                }
                None => {
                    drop(stream_token_guard);
                    return chunk_range_capability_unknown_response(
                        &state,
                        &manifest_profile,
                        provider,
                        TELEMETRY_ENDPOINT_CAR_RANGE,
                    );
                }
            },
            None => {
                drop(stream_token_guard);
                return chunk_range_capability_provider_missing_response(
                    &state,
                    &manifest_profile,
                    TELEMETRY_ENDPOINT_CAR_RANGE,
                );
            }
        }
    }

    let manifest_payload = match manifest.load_manifest() {
        Ok(manifest_payload) => manifest_payload,
        Err(err) => return storage_backend_error(err),
    };

    if let Some(alias) = alias_header.as_deref() {
        let alias_matches = manifest_payload
            .chunking
            .aliases
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(alias));
        if !alias_matches {
            drop(stream_token_guard);
            return gateway_refusal_response(
                &state,
                StatusCode::PRECONDITION_FAILED,
                "manifest_variant_missing",
                format!(
                    "requested manifest alias `{alias}` is not bound in the governance envelope"
                ),
                Some(&manifest_profile),
                provider_id.as_ref(),
                TELEMETRY_ENDPOINT_CAR_RANGE,
                [("alias", Value::from(alias.to_string()))],
            );
        }
    }

    let chunk_profile = match chunk_profile_for_manifest(&manifest_payload) {
        Ok(profile) => profile,
        Err(err) => return err.into_response(),
    };

    let taikai_hint = match sorafs_car::taikai_segment_hint_from_sorafs_manifest(&manifest_payload)
    {
        Ok(hint) => hint,
        Err(err) => {
            error!(
                ?err,
                manifest = manifest_id_hex,
                "failed to derive Taikai segment hint from manifest"
            );
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to derive Taikai metadata",
            );
        }
    };

    let full_plan = manifest.to_car_plan_with_hint(chunk_profile, taikai_hint.clone());

    let chunk_slice = match manifest.chunk_slice(byte_range.start, length as usize) {
        Ok(slice) => slice,
        Err(StorageBackendError::RangeOutOfBounds { .. }) => {
            return range_not_satisfiable(
                total_length,
                "requested range is not aligned with stored chunk boundaries".to_string(),
            );
        }
        Err(err) => {
            error!(?err, "failed to derive chunk slice for CAR range response");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to assemble CAR range",
            );
        }
    };

    let range_payload = match state.sorafs_node.read_payload_range(
        &manifest_id_hex,
        byte_range.start,
        length as usize,
    ) {
        Ok(bytes) => bytes,
        Err(err) => return node_storage_error_response(err),
    };

    record_storage_metrics(&state);

    debug_assert_eq!(range_payload.len() as u64, length);

    let mut range_chunks = Vec::with_capacity(chunk_slice.chunk_count());
    let mut relative_offset = 0u64;
    for record in &chunk_slice.chunks {
        range_chunks.push(CarChunk {
            offset: relative_offset,
            length: record.length,
            digest: record.digest,
            taikai_segment_hint: taikai_hint.clone(),
        });
        relative_offset += u64::from(record.length);
    }

    let sub_plan = CarBuildPlan {
        chunk_profile,
        payload_digest: blake3_hash(&range_payload),
        content_length: length,
        chunks: range_chunks,
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: chunk_slice.chunk_count(),
            size: length,
        }],
    };

    let mut car_bytes = Vec::new();
    let writer = match CarWriter::new(&sub_plan, &range_payload) {
        Ok(writer) => writer,
        Err(err) => {
            error!(?err, "failed to initialise CAR writer for range response");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to assemble CAR range",
            );
        }
    };
    if let Err(err) = writer.write_to(&mut car_bytes) {
        error!(?err, "failed to write CAR range response");
        if matches!(err, sorafs_car::CarWriteError::DigestMismatch { .. }) {
            drop(stream_token_guard);
            const DIGEST_MISMATCH_REASON: &str =
                "car verification failed due to mismatched chunk digest";
            return gateway_refusal_response(
                &state,
                StatusCode::UNPROCESSABLE_ENTITY,
                DIGEST_MISMATCH_REASON,
                DIGEST_MISMATCH_REASON,
                Some(&manifest_profile),
                provider_id.as_ref(),
                TELEMETRY_ENDPOINT_CAR_RANGE,
                [("error_kind", Value::from("chunk_digest_mismatch"))],
            );
        }
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to assemble CAR range",
        );
    }

    let report = match CarVerifier::verify_block_car(
        &manifest_payload,
        &full_plan,
        &car_bytes,
        Some(byte_range.start..=byte_range.end_inclusive),
    ) {
        Ok(report) => report,
        Err(err) => {
            error!(
                ?err,
                manifest_id = manifest_id_hex,
                "CAR range verification failed"
            );
            drop(stream_token_guard);
            return car_verification_refusal(
                &state,
                &manifest_profile,
                provider_id.as_ref(),
                TELEMETRY_ENDPOINT_CAR_RANGE,
                err,
            );
        }
    };
    if report.payload_bytes != length {
        error!(
            expected = length,
            actual = report.payload_bytes,
            "CAR verification payload length mismatch"
        );
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "verified CAR payload length mismatch",
        );
    }
    #[cfg(debug_assertions)]
    debug_assert_eq!(report.chunk_indices.len(), expected_chunk_count);
    #[cfg(not(debug_assertions))]
    let _ = expected_chunk_count;
    let verified_chunk_count = report.chunk_indices.len();

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(MIME_CAR));
    response_headers.insert(
        header::CONTENT_LENGTH,
        header_value(&car_bytes.len().to_string(), "Content-Length"),
    );
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    let content_range = format!(
        "bytes {}-{}/{}",
        byte_range.start, byte_range.end_inclusive, total_length
    );
    response_headers.insert(
        header::CONTENT_RANGE,
        header_value(&content_range, "Content-Range"),
    );
    let chunk_range_value = format!(
        "start={};end={};chunks={}",
        byte_range.start, byte_range.end_inclusive, verified_chunk_count
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_CHUNK_RANGE),
        header_value(&chunk_range_value, "X-Sora-Chunk-Range"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_CHUNKER),
        header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_NONCE),
        nonce_header,
    );

    if let Some(token) = headers.get(HEADER_SORA_STREAM_TOKEN) {
        response_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            token.clone(),
        );
    }
    if let Some(snapshot) = state.current_tls_snapshot() {
        response_headers.insert(
            HeaderName::from_static(SORA_TLS_STATE_HEADER),
            header_value(&snapshot.header_value(), "X-Sora-TLS-State"),
        );
    }
    if let Some(blinded_value) = blinded_b64.as_ref() {
        response_headers.insert(
            HeaderName::from_static(HEADER_SORA_REQ_BLINDED_CID),
            header_value(blinded_value, "Sora-Req-Blinded-CID"),
        );
    }
    response_headers.insert(
        HeaderName::from_static(HEADER_SORA_CONTENT_CID),
        header_value(&manifest_id_hex, "Sora-Content-CID"),
    );

    let response_status = StatusCode::PARTIAL_CONTENT;
    let latency_ms = request_timer.elapsed().as_secs_f64() * 1_000.0;
    #[cfg(feature = "telemetry")]
    let payload_bytes = u64::try_from(car_bytes.len()).unwrap_or(u64::MAX);
    #[cfg(feature = "telemetry")]
    {
        let provider_hex = provider_id.as_ref().map(encode);
        state.telemetry.with_metrics(|metrics| {
            metrics.record_sorafs_chunk_range(
                TELEMETRY_ENDPOINT_CAR_RANGE,
                response_status.as_u16(),
                payload_bytes,
                Some(chunker_header),
                Some(manifest_profile.as_str()),
                provider_hex.as_deref(),
                None,
                Some(latency_ms),
            );
        });
    }

    if let Some(ref probe) = potr_probe {
        let provider = match provider_id.as_ref() {
            Some(value) => value,
            None => {
                drop(stream_token_guard);
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "sora-potr-request requires provider_id",
                );
            }
        };

        let latency_clamped = round_clamped_u32(latency_ms);
        let (potr_status, note) = if latency_clamped > probe.params.deadline_ms {
            (
                PotrStatus::MissedDeadline,
                Some(format!(
                    "latency {latency_clamped}ms exceeded deadline {}ms",
                    probe.params.deadline_ms
                )),
            )
        } else {
            (PotrStatus::Success, None)
        };

        match finalize_potr_receipt(
            &state,
            probe,
            &manifest,
            provider,
            byte_range.start,
            byte_range.end_inclusive,
            potr_status,
            latency_clamped,
            note,
        ) {
            Ok((receipt, receipt_b64, status_label)) => {
                if let Err(err) = state.sorafs_node.record_potr_receipt(receipt) {
                    error!(?err, "failed to record PoTR receipt");
                }
                response_headers.insert(
                    HeaderName::from_static(HEADER_SORA_POTR_RECEIPT),
                    header_value(&receipt_b64, "Sora-PoTR-Receipt"),
                );
                response_headers.insert(
                    HeaderName::from_static(HEADER_SORA_POTR_STATUS),
                    header_value(status_label, "Sora-PoTR-Status"),
                );
            }
            Err(response) => {
                drop(stream_token_guard);
                return response;
            }
        }
    }

    drop(stream_token_guard);

    (response_status, response_headers, car_bytes).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_get_sorafs_storage_chunk(
    State(state): State<SharedAppState>,
    Path((manifest_id_hex, chunk_digest_hex)): Path<(String, String)>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let ManifestResolution {
        manifest_id,
        blinded_b64,
    } = match resolve_manifest_request(&state, &manifest_id_hex, &headers) {
        Ok(resolution) => resolution,
        Err(response) => return response,
    };
    let manifest_id_hex = manifest_id;

    let request_timer = Instant::now();
    let request_wall_start = SystemTime::now();
    let potr_probe = match begin_potr_probe(&headers, request_wall_start, request_timer) {
        Ok(probe) => probe,
        Err(response) => return response,
    };

    let nonce_header = match headers.get(HEADER_SORA_NONCE) {
        Some(value) => value.clone(),
        None => return json_error(StatusCode::BAD_REQUEST, "missing X-SoraFS-Nonce header"),
    };

    let manifest = match state.sorafs_node.manifest_metadata(&manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };
    let manifest_profile = manifest.chunk_profile_handle().to_string();

    if let Some(chunker_header) = headers.get(HEADER_SORA_CHUNKER) {
        match chunker_header.to_str() {
            Ok(chunker)
                if chunker
                    .trim()
                    .eq_ignore_ascii_case(manifest.chunk_profile_handle()) => {}
            Ok(chunker) => {
                return gateway_refusal_response(
                    &state,
                    StatusCode::NOT_ACCEPTABLE,
                    "unsupported_chunker",
                    format!("chunk profile {chunker} is not enabled on this gateway"),
                    Some(&manifest_profile),
                    None,
                    TELEMETRY_ENDPOINT_CHUNK,
                    [
                        ("profile", Value::from(chunker.to_string())),
                        ("manifest_profile", Value::from(manifest_profile.clone())),
                    ],
                );
            }
            Err(_) => {
                return gateway_refusal_response(
                    &state,
                    StatusCode::BAD_REQUEST,
                    "unsupported_chunker",
                    "X-SoraFS-Chunker header must be valid ASCII",
                    Some(&manifest_profile),
                    None,
                    TELEMETRY_ENDPOINT_CHUNK,
                    [("profile", Value::from("invalid_ascii"))],
                );
            }
        }
    }

    let digest = match parse_chunk_digest_hex(&chunk_digest_hex) {
        Ok(digest) => digest,
        Err(message) => return json_error(StatusCode::BAD_REQUEST, message),
    };

    let record = match state.sorafs_node.chunk_by_digest(&manifest_id_hex, &digest) {
        Ok(record) => record,
        Err(err) => return node_storage_error_response(err),
    };

    let (stream_token_guard, stream_token_body) =
        match enforce_stream_token_for_request(&state, &headers, &manifest, record.length as u64) {
            Ok(result) => result,
            Err(response) => return response,
        };
    let provider_id = state
        .sorafs_node
        .capacity_usage()
        .provider_id
        .or(Some(stream_token_body.provider_id));
    if let Err(response) =
        enforce_gateway_policy_for_request(&state, &headers, &manifest, provider_id, remote)
    {
        return response;
    }
    if state.sorafs_gateway_config.enforce_capabilities {
        match provider_id.as_ref() {
            Some(provider) => match state.provider_supports_chunk_range(provider) {
                Some(true) => {}
                Some(false) => {
                    drop(stream_token_guard);
                    return chunk_range_capability_missing_response(
                        &state,
                        &manifest_profile,
                        provider,
                        TELEMETRY_ENDPOINT_CHUNK,
                    );
                }
                None => {
                    drop(stream_token_guard);
                    return chunk_range_capability_unknown_response(
                        &state,
                        &manifest_profile,
                        provider,
                        TELEMETRY_ENDPOINT_CHUNK,
                    );
                }
            },
            None => {
                drop(stream_token_guard);
                return chunk_range_capability_provider_missing_response(
                    &state,
                    &manifest_profile,
                    TELEMETRY_ENDPOINT_CHUNK,
                );
            }
        }
    }

    let (_, bytes) = match state
        .sorafs_node
        .read_chunk_by_digest(&manifest_id_hex, &digest)
    {
        Ok(result) => result,
        Err(err) => return node_storage_error_response(err),
    };

    record_storage_metrics(&state);

    let end = record
        .offset
        .checked_add(u64::from(record.length))
        .and_then(|value| value.checked_sub(1))
        .unwrap_or(record.offset);

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(MIME_OCTET_STREAM),
    );
    response_headers.insert(
        header::CONTENT_LENGTH,
        header_value(&bytes.len().to_string(), "Content-Length"),
    );
    let chunk_range_value = format!("start={};end={};chunks=1", record.offset, end);
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_CHUNK_RANGE),
        header_value(&chunk_range_value, "X-Sora-Chunk-Range"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_CHUNK_DIGEST),
        header_value(&chunk_digest_hex, "X-SoraFS-Chunk-Digest"),
    );
    response_headers.insert(
        header::HeaderName::from_static(HEADER_SORA_NONCE),
        nonce_header,
    );
    if let Some(token) = headers.get(HEADER_SORA_STREAM_TOKEN) {
        response_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            token.clone(),
        );
    }

    if let Some(snapshot) = state.current_tls_snapshot() {
        response_headers.insert(
            HeaderName::from_static(SORA_TLS_STATE_HEADER),
            header_value(&snapshot.header_value(), "X-Sora-TLS-State"),
        );
    }
    if let Some(blinded_value) = blinded_b64.as_ref() {
        response_headers.insert(
            HeaderName::from_static(HEADER_SORA_REQ_BLINDED_CID),
            header_value(blinded_value, "Sora-Req-Blinded-CID"),
        );
    }
    response_headers.insert(
        HeaderName::from_static(HEADER_SORA_CONTENT_CID),
        header_value(&manifest_id_hex, "Sora-Content-CID"),
    );

    let response_status = StatusCode::OK;
    let latency_ms = request_timer.elapsed().as_secs_f64() * 1_000.0;
    #[cfg(feature = "telemetry")]
    let payload_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    #[cfg(feature = "telemetry")]
    {
        let chunker_metric_label = headers
            .get(HEADER_SORA_CHUNKER)
            .and_then(|value| value.to_str().ok())
            .map(|chunker| chunker.trim().to_string());
        let provider_hex = provider_id.as_ref().map(encode);
        state.telemetry.with_metrics(|metrics| {
            metrics.record_sorafs_chunk_range(
                TELEMETRY_ENDPOINT_CHUNK,
                response_status.as_u16(),
                payload_bytes,
                chunker_metric_label.as_deref(),
                Some(manifest_profile.as_str()),
                provider_hex.as_deref(),
                None,
                Some(latency_ms),
            );
        });
    }

    if let Some(ref probe) = potr_probe {
        let provider = match provider_id.as_ref() {
            Some(value) => value,
            None => {
                drop(stream_token_guard);
                return json_error(
                    StatusCode::BAD_REQUEST,
                    "sora-potr-request requires provider_id",
                );
            }
        };
        let latency_clamped = round_clamped_u32(latency_ms);
        let (potr_status, note) = if latency_clamped > probe.params.deadline_ms {
            (
                PotrStatus::MissedDeadline,
                Some(format!(
                    "latency {latency_clamped}ms exceeded deadline {}ms",
                    probe.params.deadline_ms
                )),
            )
        } else {
            (PotrStatus::Success, None)
        };

        match finalize_potr_receipt(
            &state,
            probe,
            &manifest,
            provider,
            record.offset,
            end,
            potr_status,
            latency_clamped,
            note,
        ) {
            Ok((receipt, receipt_b64, status_label)) => {
                if let Err(err) = state.sorafs_node.record_potr_receipt(receipt) {
                    error!(?err, "failed to record PoTR receipt");
                }
                response_headers.insert(
                    HeaderName::from_static(HEADER_SORA_POTR_RECEIPT),
                    header_value(&receipt_b64, "Sora-PoTR-Receipt"),
                );
                response_headers.insert(
                    HeaderName::from_static(HEADER_SORA_POTR_STATUS),
                    header_value(status_label, "Sora-PoTR-Status"),
                );
            }
            Err(response) => {
                drop(stream_token_guard);
                return response;
            }
        }
    }

    drop(stream_token_guard);

    (response_status, response_headers, bytes).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_por_sample(
    State(state): State<SharedAppState>,
    JsonOnly(req): JsonOnly<StoragePorSampleRequestDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    if req.count == 0 {
        return json_error(StatusCode::BAD_REQUEST, "count must be greater than zero");
    }
    if req.count > usize::MAX as u64 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "requested sample count exceeds supported range",
        );
    }

    let manifest = match state.sorafs_node.manifest_metadata(&req.manifest_id_hex) {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    if let Err(response) = enforce_chunker_support_gateway(
        &state,
        manifest.chunk_profile_handle(),
        TELEMETRY_ENDPOINT_POR_SAMPLE,
    ) {
        return response;
    }

    let count = req.count as usize;
    let seed = req.seed.unwrap_or(0);

    let samples = match state
        .sorafs_node
        .sample_por(&req.manifest_id_hex, count, seed)
    {
        Ok(samples) => samples,
        Err(err) => return node_storage_error_response(err),
    };

    let sample_values = samples
        .into_iter()
        .map(|(flat_idx, proof)| Value::Object(sample_to_map(flat_idx, &proof)))
        .collect::<Vec<_>>();

    let response = json_object(vec![
        json_entry("manifest_id_hex", Value::from(req.manifest_id_hex)),
        json_entry("count", Value::from(sample_values.len() as u64)),
        json_entry("seed", Value::from(seed)),
        json_entry("samples", Value::Array(sample_values)),
    ]);

    (StatusCode::OK, JsonBody(response)).into_response()
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_proof_stream(
    State(state): State<SharedAppState>,
    JsonOnly(req): JsonOnly<ProofStreamRequestDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }

    let manifest_digest_hex_request = req.manifest_digest_hex.trim().to_ascii_lowercase();
    if manifest_digest_hex_request.is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "manifest_digest_hex must be provided and non-empty",
        );
    }

    let provider_id_hex = req.provider_id_hex.trim().to_ascii_lowercase();
    if provider_id_hex.is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "provider_id_hex must be provided and non-empty",
        );
    }

    let proof_kind = match parse_proof_kind(&req.proof_kind) {
        Some(kind) => kind,
        None => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "unsupported proof_kind; expected `por`, `pdp`, or `potr`",
            );
        }
    };

    if matches!(proof_kind, ProofStreamKind::Pdp) {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "proof_kind is not available yet; only `por` and `potr` are supported",
        );
    }

    let nonce = match decode_nonce(&req.nonce_b64) {
        Ok(nonce) => nonce,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err),
    };

    let orchestrator_job_id = match req
        .orchestrator_job_id_hex
        .as_ref()
        .map(|value| parse_hex_fixed::<16>(value, "orchestrator_job_id_hex"))
        .transpose()
    {
        Ok(id) => id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err),
    };

    let tier = match parse_tier(req.tier.as_deref()) {
        Ok(tier) => tier,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err),
    };

    let manifest_digest =
        match parse_hex_fixed::<32>(&manifest_digest_hex_request, "manifest_digest_hex") {
            Ok(digest) => digest,
            Err(err) => return json_error(StatusCode::BAD_REQUEST, &err),
        };

    let provider_id = match parse_hex_fixed::<32>(&provider_id_hex, "provider_id_hex") {
        Ok(id) => id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err),
    };

    let manifest = match state
        .sorafs_node
        .manifest_metadata_by_digest(&manifest_digest)
    {
        Ok(manifest) => manifest,
        Err(err) => return node_storage_error_response(err),
    };

    let manifest_digest = *manifest.manifest_digest();
    let manifest_digest_hex = hex::encode(manifest_digest);
    let manifest_root_cid_hex = hex::encode(manifest.manifest_cid());

    let request = ProofStreamRequestV1 {
        manifest_digest,
        provider_id,
        proof_kind,
        sample_count: req.sample_count,
        deadline_ms: req.deadline_ms,
        sample_seed: req.sample_seed,
        nonce,
        orchestrator_job_id,
        tier,
    };

    if let Err(err) = request.validate() {
        return json_error(StatusCode::BAD_REQUEST, request_error_message(err));
    }

    if let Err(response) = enforce_chunker_support_gateway(
        &state,
        manifest.chunk_profile_handle(),
        TELEMETRY_ENDPOINT_PROOF_STREAM,
    ) {
        return response;
    }

    match proof_kind {
        ProofStreamKind::Por => {
            let sample_count = request
                .sample_count
                .expect("validation guarantees sample_count for PoR");
            let count = sample_count as usize;
            let seed = request.sample_seed.unwrap_or(0);

            state.telemetry.with_metrics(|metrics| {
                metrics.inc_sorafs_proof_stream_inflight("por");
            });

            let start = Instant::now();
            let samples = match state
                .sorafs_node
                .sample_por(&manifest_root_cid_hex, count, seed)
            {
                Ok(samples) => samples,
                Err(err) => {
                    state.telemetry.with_metrics(|metrics| {
                        metrics.record_sorafs_proof_stream_event(
                            "por",
                            "error",
                            Some("storage_error"),
                            Some(provider_id_hex.as_str()),
                            req.tier.as_deref(),
                            None,
                        );
                        metrics.dec_sorafs_proof_stream_inflight("por");
                    });
                    return node_storage_error_response(err);
                }
            };

            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let sample_len = samples.len();
            let per_item_latency = if sample_len == 0 {
                elapsed_ms
            } else {
                elapsed_ms / sample_len as f64
            };

            let sample_seed = request.sample_seed;
            let orchestrator_job_hex = req
                .orchestrator_job_id_hex
                .as_ref()
                .map(|job| job.trim().to_ascii_lowercase());
            let requested_tier_label = req
                .tier
                .as_ref()
                .map(|tier| tier.trim().to_ascii_lowercase());
            let telemetry = state.telemetry.clone();
            let manifest_digest_hex_stream = manifest_digest_hex.clone();
            let manifest_root_cid_hex_stream = manifest_root_cid_hex.clone();
            let provider_id_hex_stream = provider_id_hex.clone();
            let (tx, rx) = mpsc::channel::<Bytes>(16);
            let stream = ReceiverStream::new(rx).map(Ok::<Bytes, Infallible>);
            let body = Body::from_stream(stream);
            tokio::spawn(async move {
                let sender = tx;
                let mut delivered: usize = 0;
                for (flat_index, proof) in samples {
                    let mut map = sample_to_map(flat_index, &proof);
                    map.insert(
                        "manifest_digest_hex".into(),
                        Value::from(manifest_digest_hex_stream.clone()),
                    );
                    map.insert(
                        "manifest_cid_hex".into(),
                        Value::from(manifest_root_cid_hex_stream.clone()),
                    );
                    map.insert(
                        "provider_id_hex".into(),
                        Value::from(provider_id_hex_stream.clone()),
                    );
                    map.insert("proof_kind".into(), Value::from("por"));
                    map.insert("result".into(), Value::from("success"));
                    map.insert(
                        "latency_ms".into(),
                        Value::from(round_clamped_u64(per_item_latency)),
                    );
                    map.insert("failure_reason".into(), Value::Null);
                    if let Some(seed) = sample_seed {
                        map.insert("sample_seed".into(), Value::from(seed));
                    }
                    if let Some(job_hex) = orchestrator_job_hex.as_ref() {
                        map.insert(
                            "orchestrator_job_id_hex".into(),
                            Value::from(job_hex.clone()),
                        );
                    }
                    if let Some(tier_label) = requested_tier_label.as_ref() {
                        map.insert("tier".into(), Value::from(tier_label.clone()));
                    }
                    let mut rendered =
                        json::to_vec(&Value::Object(map)).expect("serialize PoR proof item");
                    rendered.push(b'\n');
                    if sender.send(Bytes::from(rendered)).await.is_err() {
                        break;
                    }
                    delivered += 1;
                }
                telemetry.with_metrics(|metrics| {
                    for _ in 0..delivered.max(1) {
                        metrics.record_sorafs_proof_stream_event(
                            "por",
                            "success",
                            None,
                            Some(provider_id_hex_stream.as_str()),
                            requested_tier_label.as_deref(),
                            Some(per_item_latency),
                        );
                    }
                    metrics.dec_sorafs_proof_stream_inflight("por");
                });
            });

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-ndjson")
                .body(body)
                .unwrap()
        }
        ProofStreamKind::Potr => {
            let _deadline_ms = request
                .deadline_ms
                .expect("validation guarantees deadline for PoTR");

            state.telemetry.with_metrics(|metrics| {
                metrics.inc_sorafs_proof_stream_inflight("potr");
            });

            let receipts =
                state
                    .sorafs_node
                    .potr_receipts(&manifest_digest, &provider_id, request.tier);

            let orchestrator_job_hex = req
                .orchestrator_job_id_hex
                .as_ref()
                .map(|job| job.trim().to_ascii_lowercase());
            let requested_tier_label = req
                .tier
                .as_ref()
                .map(|tier| tier.trim().to_ascii_lowercase());
            let telemetry = state.telemetry.clone();
            let manifest_digest_hex_stream = manifest_digest_hex.clone();
            let manifest_root_cid_hex_stream = manifest_root_cid_hex.clone();
            let provider_id_hex_stream = provider_id_hex.clone();
            let (tx, rx) = mpsc::channel::<Bytes>(16);
            let stream = ReceiverStream::new(rx).map(Ok::<Bytes, Infallible>);
            let body = Body::from_stream(stream);

            tokio::spawn(async move {
                let sender = tx;
                for receipt in receipts {
                    let (result_label, metrics_reason) = match receipt.status {
                        PotrStatus::Success => ("success", None),
                        PotrStatus::MissedDeadline => ("failure", Some("missed_deadline")),
                        PotrStatus::ProviderError => ("failure", Some("provider_error")),
                        PotrStatus::GatewayError => ("failure", Some("gateway_error")),
                        PotrStatus::ClientCancelled => ("failure", Some("client_cancelled")),
                    };

                    let failure_text = receipt.note.as_ref().map_or_else(
                        || metrics_reason.map(ToOwned::to_owned),
                        |note| {
                            if note.trim().is_empty() {
                                metrics_reason.map(ToOwned::to_owned)
                            } else {
                                Some(note.clone())
                            }
                        },
                    );

                    let mut map = Map::new();
                    map.insert(
                        "manifest_digest_hex".into(),
                        Value::from(manifest_digest_hex_stream.clone()),
                    );
                    map.insert(
                        "manifest_cid_hex".into(),
                        Value::from(manifest_root_cid_hex_stream.clone()),
                    );
                    map.insert(
                        "provider_id_hex".into(),
                        Value::from(provider_id_hex_stream.clone()),
                    );
                    map.insert("proof_kind".into(), Value::from("potr"));
                    map.insert("result".into(), Value::from(result_label));
                    map.insert(
                        "latency_ms".into(),
                        Value::from(u64::from(receipt.latency_ms)),
                    );
                    map.insert(
                        "deadline_ms".into(),
                        Value::from(u64::from(receipt.deadline_ms)),
                    );
                    let receipt_tier_label = match receipt.tier {
                        ProofStreamTier::Hot => "hot",
                        ProofStreamTier::Warm => "warm",
                        ProofStreamTier::Archive => "archive",
                    };
                    map.insert("tier".into(), Value::from(receipt_tier_label));
                    map.insert(
                        "requested_at_ms".into(),
                        Value::from(receipt.requested_at_ms),
                    );
                    map.insert(
                        "responded_at_ms".into(),
                        Value::from(receipt.responded_at_ms),
                    );
                    map.insert("range_start".into(), Value::from(receipt.range_start));
                    map.insert("range_end".into(), Value::from(receipt.range_end));
                    if let Some(request_id) = receipt.request_id {
                        map.insert("request_id".into(), Value::from(encode(request_id)));
                    }
                    if let Some(trace_id) = receipt.trace_id {
                        map.insert("trace_id".into(), Value::from(encode(trace_id)));
                    }
                    map.insert("recorded_at_ms".into(), Value::from(receipt.recorded_at_ms));
                    if let Some(reason) = failure_text.as_ref() {
                        map.insert("failure_reason".into(), Value::from(reason.clone()));
                    } else {
                        map.insert("failure_reason".into(), Value::Null);
                    }
                    if let Some(note) = receipt.note.as_ref() {
                        if !note.trim().is_empty() {
                            map.insert("note".into(), Value::from(note.clone()));
                        }
                    }
                    if let Some(job_hex) = orchestrator_job_hex.as_ref() {
                        map.insert(
                            "orchestrator_job_id_hex".into(),
                            Value::from(job_hex.clone()),
                        );
                    }
                    if let Some(tier_label) = requested_tier_label.as_ref() {
                        map.insert("requested_tier".into(), Value::from(tier_label.clone()));
                    }
                    if let Some(signature) = receipt.gateway_signature.as_ref() {
                        map.insert(
                            "gateway_signature".into(),
                            potr_signature_to_json(signature),
                        );
                    }
                    if let Some(signature) = receipt.provider_signature.as_ref() {
                        map.insert(
                            "provider_signature".into(),
                            potr_signature_to_json(signature),
                        );
                    }

                    let mut rendered =
                        json::to_vec(&Value::Object(map)).expect("serialize PoTR receipt item");
                    rendered.push(b'\n');
                    if sender.send(Bytes::from(rendered)).await.is_err() {
                        break;
                    }

                    telemetry.with_metrics(|metrics| {
                        metrics.record_sorafs_proof_stream_event(
                            "potr",
                            result_label,
                            metrics_reason,
                            Some(provider_id_hex_stream.as_str()),
                            Some(receipt_tier_label),
                            Some(f64::from(receipt.latency_ms)),
                        );
                    });
                }

                telemetry.with_metrics(|metrics| {
                    metrics.dec_sorafs_proof_stream_inflight("potr");
                });
            });

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-ndjson")
                .body(body)
                .unwrap()
        }
        ProofStreamKind::Pdp => json_error(
            StatusCode::NOT_IMPLEMENTED,
            "proof_kind is not available yet; only `por` and `potr` are supported",
        ),
    }
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_por_challenge(
    State(state): State<SharedAppState>,
    JsonOnly(req): JsonOnly<StoragePorChallengeDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    match decode_por_payload::<PorChallengeV1>(&req.challenge_b64, "challenge") {
        Ok(challenge) => match state.sorafs_node.record_por_challenge(&challenge) {
            Ok(()) => JsonBody(json_object(vec![json_entry("status", "accepted")])).into_response(),
            Err(err) => por_tracker_error_response(err),
        },
        Err(err) => err.into_response(),
    }
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_por_proof(
    State(state): State<SharedAppState>,
    JsonOnly(req): JsonOnly<StoragePorProofDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    match decode_por_payload::<PorProofV1>(&req.proof_b64, "proof") {
        Ok(proof) => match state.sorafs_node.record_por_proof(&proof) {
            Ok(()) => JsonBody(json_object(vec![json_entry("status", "accepted")])).into_response(),
            Err(err) => por_tracker_error_response(err),
        },
        Err(err) => err.into_response(),
    }
}

#[cfg(feature = "app_api")]
pub(crate) async fn handle_post_sorafs_storage_por_verdict(
    State(state): State<SharedAppState>,
    JsonOnly(req): JsonOnly<StoragePorVerdictDto>,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return storage_disabled_response();
    }
    match decode_por_payload::<AuditVerdictV1>(&req.verdict_b64, "verdict") {
        Ok(verdict) => match state.sorafs_node.record_por_verdict(&verdict) {
            Ok(verdict_stats) => {
                let response = json_object(vec![
                    json_entry("status", "accepted"),
                    json_entry("success_samples", verdict_stats.stats.success_samples),
                    json_entry("failed_samples", verdict_stats.stats.failed_samples),
                ]);
                JsonBody(response).into_response()
            }
            Err(err) => por_tracker_error_response(err),
        },
        Err(err) => err.into_response(),
    }
}

fn header_value(value: impl AsRef<str>, name: &str) -> HeaderValue {
    HeaderValue::from_str(value.as_ref())
        .unwrap_or_else(|_| panic!("{name} header produced invalid value: {}", value.as_ref()))
}

#[cfg(test)]
fn alias_proof_header(alias: &str) -> HeaderValue {
    header_value(&alias_proof_b64(alias), "Sora-Proof")
}

#[cfg(test)]
fn alias_proof_b64(alias: &str) -> String {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use sorafs_manifest::{
        CouncilSignature,
        pin_registry::{
            AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
        },
    };

    let binding = AliasBindingV1 {
        alias: alias.to_string(),
        manifest_cid: vec![0x42; 32],
        bound_at: 1,
        expiry_epoch: 10,
    };
    let merkle_path = Vec::new();
    let registry_root = alias_merkle_root(&binding, &merkle_path).expect("alias merkle root");
    let mut bundle = AliasProofBundleV1 {
        binding,
        registry_root,
        registry_height: 1,
        generated_at_unix: 1_700_000_000,
        expires_at_unix: 1_700_003_600,
        merkle_path,
        council_signatures: Vec::new(),
    };
    let message = alias_proof_signature_digest(&bundle);
    let keypair = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    let signature = Signature::new(keypair.private_key(), message.as_ref());
    let mut signer = [0u8; 32];
    signer.copy_from_slice(keypair.public_key().to_bytes().1);
    bundle.council_signatures.push(CouncilSignature {
        signer,
        signature: signature.payload().to_vec(),
    });

    let bytes = norito::to_bytes(&bundle).expect("encode alias proof bundle");
    BASE64_STANDARD.encode(bytes)
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], String> {
    let trimmed = value.trim();
    if trimmed.len() != 64 {
        return Err(format!(
            "provider_id_hex must be exactly 64 hex characters (found {})",
            trimmed.len()
        ));
    }
    let bytes = hex::decode(trimmed).map_err(|err| format!("invalid provider_id_hex: {err}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "provider_id_hex must decode to 32 bytes (found {})",
            bytes.len()
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

#[derive(Debug)]
enum RangeParseError {
    Invalid(String),
    Unsatisfiable(String),
}

impl RangeParseError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    fn unsatisfiable(message: impl Into<String>) -> Self {
        Self::Unsatisfiable(message.into())
    }
}

fn parse_range_header(value: &str, total_length: u64) -> Result<ByteRange, RangeParseError> {
    let trimmed = value.trim();
    if !trimmed.starts_with("bytes=") {
        return Err(RangeParseError::invalid(
            "Range header must use the 'bytes' unit",
        ));
    }
    let spec = trimmed.strip_prefix("bytes=").unwrap_or(trimmed);
    if spec.is_empty() {
        return Err(RangeParseError::invalid(
            "Range header is missing start and end offsets",
        ));
    }
    if spec.contains(',') {
        return Err(RangeParseError::invalid(
            "Multiple ranges are not supported",
        ));
    }
    let (start_str, end_str) = spec
        .split_once('-')
        .ok_or_else(|| RangeParseError::invalid("Range header must contain '-' separator"))?;
    if start_str.is_empty() {
        return Err(RangeParseError::invalid(
            "Range start offset must be specified",
        ));
    }
    let start = start_str
        .parse::<u64>()
        .map_err(|_| RangeParseError::invalid("Range start offset is not a valid integer"))?;
    let end = if end_str.is_empty() {
        total_length
            .checked_sub(1)
            .ok_or_else(|| RangeParseError::invalid("Manifest payload is empty"))?
    } else {
        end_str
            .parse::<u64>()
            .map_err(|_| RangeParseError::invalid("Range end offset is not a valid integer"))?
    };
    if start > end {
        return Err(RangeParseError::invalid(
            "Range start offset exceeds end offset",
        ));
    }
    if end >= total_length {
        return Err(RangeParseError::unsatisfiable(
            "Requested range exceeds payload length",
        ));
    }
    Ok(ByteRange {
        start,
        end_inclusive: end,
    })
}

fn ensure_chunk_alignment(manifest: &StoredManifest, range: ByteRange) -> Result<usize, String> {
    let end_exclusive = range
        .end_inclusive
        .checked_add(1)
        .ok_or_else(|| "Range end offset overflowed".to_string())?;

    let mut cursor = range.start;
    let mut chunk_count = 0usize;
    let mut started = false;

    for index in 0..manifest.chunk_count() {
        let chunk = manifest
            .chunk(index)
            .ok_or_else(|| "Chunk metadata is missing".to_string())?;
        let chunk_start = chunk.offset;
        let chunk_end = chunk_start + u64::from(chunk.length);

        if chunk_end <= range.start {
            continue;
        }

        if !started {
            if chunk_start != range.start {
                return Err(format!(
                    "Range start offset {} does not align with chunk boundary {}",
                    range.start, chunk_start
                ));
            }
            started = true;
        } else if chunk_start != cursor {
            return Err(format!("Range crosses chunk boundary at offset {}", cursor));
        }

        cursor = chunk_end;
        chunk_count += 1;

        if cursor == end_exclusive {
            return Ok(chunk_count);
        }
        if cursor > end_exclusive {
            return Err("Range ends mid-chunk".to_string());
        }
    }

    if !started {
        Err("Range start offset does not match any chunk boundary".to_string())
    } else {
        Err("Range does not terminate at a chunk boundary".to_string())
    }
}

fn parse_chunk_digest_hex(hex: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex).map_err(|_| "chunk digest must be hex encoded".to_string())?;
    if bytes.len() != 32 {
        return Err("chunk digest must be 32 bytes (64 hex characters)".to_string());
    }
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&bytes);
    Ok(digest)
}

fn range_not_satisfiable(total_length: u64, message: String) -> Response {
    let mut response = json_error(StatusCode::RANGE_NOT_SATISFIABLE, message);
    if let Ok(value) = HeaderValue::from_str(&format!("bytes */{}", total_length)) {
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    response
}

#[cfg(all(test, feature = "app_api"))]
mod app_api_tests {
    use std::{
        fmt, fs,
        io::Write,
        num::NonZeroU32,
        path::PathBuf,
        sync::{
            Arc, RwLock,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use axum::body;
    use blake3::hash;
    use ed25519_dalek::{Signer as _, SigningKey};
    use iroha_config::parameters::actual::SorafsTokenConfig;
    use sorafs_car::{
        CarBuildPlan,
        multi_fetch::{
            FetchOptions, FetchProvider, ProviderMetadata, RangeCapability, StreamBudget,
            fetch_plan_parallel,
        },
    };
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, CouncilSignature, DagCodecId, ManifestBuilder, PinPolicy,
        ProviderAdvertV1,
        pin_registry::{
            AliasBindingV1, AliasProofBundleV1, ReplicationOrderV1, alias_merkle_root,
            alias_proof_signature_digest,
        },
        provider_admission::ProviderAdmissionEnvelopeV1,
    };
    use sorafs_node::{config::StorageConfig, store::StorageBackend};
    use tempfile::{NamedTempFile, TempDir, tempdir};

    use super::*;
    use crate::{
        mk_app_state_for_tests,
        sorafs::{AdmissionRegistry, StreamTokenIssuer},
        utils::extractors::JsonOnly,
    };

    #[test]
    fn walk_query_params_decodes_percent_encoding() {
        let mut pairs = Vec::new();
        walk_query_params(Some("alias%2Fname=docs%2Froot"), |key, value| {
            pairs.push((key.to_string(), value.to_string()));
            Ok(())
        })
        .expect("query decoding succeeds");
        assert_eq!(pairs, [("alias/name".to_string(), "docs/root".to_string())]);
    }

    #[test]
    fn walk_query_params_rejects_invalid_percent_sequences() {
        let err = walk_query_params(Some("%ZZ=value"), |_, _| Ok(())).expect_err("should fail");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn parse_range_header_parses_valid_range() {
        let range = parse_range_header("bytes=0-1023", 4096).expect("valid range");
        assert_eq!(range.start, 0);
        assert_eq!(range.end_inclusive, 1023);
    }

    #[test]
    fn parse_range_header_rejects_multiple_ranges() {
        let err = parse_range_header("bytes=0-1,2-3", 10).expect_err("should reject multi-range");
        assert!(matches!(
            err,
            RangeParseError::Invalid(message) if message.contains("Multiple ranges")
        ));
    }

    #[test]
    fn parse_range_header_rejects_unsatisfiable_range() {
        let err = parse_range_header("bytes=0-10", 5).expect_err("should reject overshoot");
        assert!(matches!(
            err,
            RangeParseError::Unsatisfiable(message) if message.contains("exceeds payload length")
        ));
    }

    #[test]
    fn ensure_chunk_alignment_accepts_aligned_range() {
        let temp_dir = tempdir().expect("temp dir");
        let backend = StorageBackend::new(
            StorageConfig::builder()
                .enabled(true)
                .data_dir(temp_dir.path().join("storage"))
                .build(),
        )
        .expect("backend init");

        let payload = b"deterministic payload for alignment";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03, 0x04])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");
        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");

        let range = ByteRange {
            start: 0,
            end_inclusive: stored.content_length().saturating_sub(1),
        };
        let chunk_count = ensure_chunk_alignment(&stored, range).expect("aligned range");
        assert!(chunk_count > 0);
    }

    #[test]
    fn ensure_chunk_alignment_rejects_misaligned_range() {
        let temp_dir = tempdir().expect("temp dir");
        let backend = StorageBackend::new(
            StorageConfig::builder()
                .enabled(true)
                .data_dir(temp_dir.path().join("storage"))
                .build(),
        )
        .expect("backend init");

        let payload = b"payload for misalignment check";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0x0A, 0x0B, 0x0C])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");
        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");

        let range = ByteRange {
            start: 1,
            end_inclusive: stored.content_length().saturating_sub(1),
        };
        let err = ensure_chunk_alignment(&stored, range).expect_err("range should be rejected");
        assert!(err.contains("chunk boundary"));
    }

    #[test]
    fn parse_chunk_digest_hex_validates_length() {
        let err = parse_chunk_digest_hex("deadbeef").expect_err("digest too short");
        assert!(err.contains("32 bytes"));
    }

    #[test]
    fn decode_hex_32_rejects_invalid_input() {
        assert!(
            decode_hex_32("deadbeef").is_err(),
            "should reject short input"
        );
        assert!(
            decode_hex_32(&"zz".repeat(32)).is_err(),
            "should reject non-hex characters"
        );
    }

    #[test]
    fn decode_hex_32_decodes_expected_bytes() {
        let hex = hex::encode([0xAA; 32]);
        let decoded = decode_hex_32(&hex).expect("decode succeeds");
        assert_eq!(decoded, [0xAA; 32]);
    }
}
pub(crate) fn decode_por_payload<T>(payload_b64: &str, kind: &str) -> ApiResult<T>
where
    T: for<'de> norito::NoritoDeserialize<'de>,
{
    let bytes = match base64::engine::general_purpose::STANDARD.decode(payload_b64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid base64 in {kind}_b64: {err}"),
            )
            .into());
        }
    };

    match norito::decode_from_bytes(&bytes) {
        Ok(value) => Ok(value),
        Err(err) => Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("invalid {kind} payload: {err}"),
        )
        .into()),
    }
}

pub(crate) fn por_tracker_error_response(err: PorTrackerError) -> Response {
    let status = match err {
        PorTrackerError::ChallengeInvalid(_)
        | PorTrackerError::ProofInvalid(_)
        | PorTrackerError::VerdictInvalid(_)
        | PorTrackerError::MismatchManifest
        | PorTrackerError::MismatchProvider
        | PorTrackerError::SampleCountMismatch { .. }
        | PorTrackerError::DuplicateProof
        | PorTrackerError::ProofDigestMismatch => StatusCode::BAD_REQUEST,
        PorTrackerError::ChallengeConflict => StatusCode::CONFLICT,
        PorTrackerError::UnknownChallenge => StatusCode::NOT_FOUND,
    };
    json_error(status, err.to_string())
}

fn advert_ingest_response(
    status: &str,
    fingerprint: &[u8; 32],
    warnings: &[AdvertWarning],
) -> Response {
    let warnings_json = warnings
        .iter()
        .map(|warning| Value::String(warning.as_str().to_string()))
        .collect::<Vec<_>>();
    let response = json_object(vec![
        json_entry("status", status.to_string()),
        json_entry("fingerprint_hex", fingerprint.encode_hex::<String>()),
        json_entry("warnings", Value::Array(warnings_json)),
    ]);
    (StatusCode::OK, JsonBody(response)).into_response()
}

fn snapshot_to_json(
    snapshot: CapacitySnapshot,
    local_usage: &CapacityUsageSnapshot,
    metering: &MeteringSnapshot,
    telemetry_preview: Option<Value>,
    fee_projection: Option<&FeeProjection>,
) -> Result<Value, json::Error> {
    let declaration_count = snapshot.declarations.len();
    let ledger_count = snapshot.fee_ledger.len();
    let credit_count = snapshot.credit_ledger.len();

    let mut declarations_json = Vec::with_capacity(declaration_count);
    for declaration in snapshot.declarations {
        declarations_json.push(declaration.into_json()?);
    }

    let mut ledger_json = Vec::with_capacity(ledger_count);
    for entry in snapshot.fee_ledger {
        ledger_json.push(entry.into_json()?);
    }

    let mut credit_json = Vec::with_capacity(credit_count);
    for entry in snapshot.credit_ledger {
        credit_json.push(entry.into_json()?);
    }

    let mut disputes_json = Vec::with_capacity(snapshot.disputes.len());
    for dispute in snapshot.disputes {
        disputes_json.push(dispute.into_json()?);
    }

    Ok(json_object(vec![
        json_entry("declaration_count", declaration_count as u64),
        json_entry("declarations", Value::Array(declarations_json)),
        json_entry("ledger_count", ledger_count as u64),
        json_entry("fee_ledger", Value::Array(ledger_json)),
        json_entry("credit_ledger_count", credit_count as u64),
        json_entry("credit_ledger", Value::Array(credit_json)),
        json_entry("disputes", Value::Array(disputes_json)),
        json_entry(
            "local_usage",
            local_usage_to_json(local_usage, metering, telemetry_preview, fee_projection)?,
        ),
    ]))
}

fn local_usage_to_json(
    usage: &CapacityUsageSnapshot,
    metering: &MeteringSnapshot,
    telemetry_preview: Option<Value>,
    fee_projection: Option<&FeeProjection>,
) -> Result<Value, json::Error> {
    let Some(provider_id) = usage.provider_id else {
        return Ok(Value::Null);
    };

    let mut root = Map::new();
    root.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(provider_id)),
    );
    root.insert(
        "committed_total_gib".into(),
        json::to_value(&usage.committed_total_gib)?,
    );
    root.insert(
        "allocated_total_gib".into(),
        json::to_value(&usage.allocated_total_gib)?,
    );
    root.insert(
        "available_total_gib".into(),
        json::to_value(&usage.available_total_gib)?,
    );

    let chunkers = usage
        .chunkers
        .iter()
        .map(|chunker| {
            let mut entry = Map::new();
            entry.insert("handle".into(), Value::String(chunker.handle.clone()));
            entry.insert(
                "committed_gib".into(),
                json::to_value(&chunker.committed_gib)?,
            );
            entry.insert(
                "allocated_gib".into(),
                json::to_value(&chunker.allocated_gib)?,
            );
            entry.insert(
                "available_gib".into(),
                json::to_value(&chunker.available_gib)?,
            );
            Ok(Value::Object(entry))
        })
        .collect::<Result<Vec<_>, json::Error>>()?;
    root.insert("chunkers".into(), Value::Array(chunkers));

    let lanes = usage
        .lanes
        .iter()
        .map(|lane| {
            let mut entry = Map::new();
            entry.insert("lane_id".into(), Value::String(lane.lane_id.clone()));
            entry.insert("max_gib".into(), json::to_value(&lane.max_gib)?);
            entry.insert("allocated_gib".into(), json::to_value(&lane.allocated_gib)?);
            entry.insert("available_gib".into(), json::to_value(&lane.available_gib)?);
            Ok(Value::Object(entry))
        })
        .collect::<Result<Vec<_>, json::Error>>()?;
    root.insert("lanes".into(), Value::Array(lanes));

    let orders = usage
        .outstanding_orders
        .iter()
        .map(|order| {
            let mut entry = Map::new();
            entry.insert(
                "order_id_hex".into(),
                Value::String(hex::encode(order.order_id)),
            );
            entry.insert("slice_gib".into(), json::to_value(&order.slice_gib)?);
            entry.insert(
                "chunker_handle".into(),
                Value::String(order.chunker_handle.clone()),
            );
            entry.insert(
                "lane".into(),
                order
                    .lane
                    .as_ref()
                    .map(|lane| Value::String(lane.clone()))
                    .unwrap_or(Value::Null),
            );
            entry.insert("deadline_at".into(), json::to_value(&order.deadline_at)?);
            Ok(Value::Object(entry))
        })
        .collect::<Result<Vec<_>, json::Error>>()?;
    root.insert("outstanding_orders".into(), Value::Array(orders));

    let mut metadata_map = Map::new();
    for (key, value) in usage.metadata.iter() {
        metadata_map.insert(key.to_string(), Value::String(value.to_string()));
    }
    root.insert("metadata".into(), Value::Object(metadata_map));

    root.insert("metering".into(), metering_to_json(metering)?);
    if let Some(projection) = fee_projection {
        root.insert("fee_projection".into(), fee_projection_to_json(projection)?);
    }
    if let Some(preview) = telemetry_preview {
        root.insert("telemetry_preview".into(), preview);
    }

    let mut window = Map::new();
    window.insert(
        "registered_epoch".into(),
        json::to_value(&usage.declaration_window.registered_epoch)?,
    );
    window.insert(
        "valid_from_epoch".into(),
        json::to_value(&usage.declaration_window.valid_from_epoch)?,
    );
    window.insert(
        "valid_until_epoch".into(),
        json::to_value(&usage.declaration_window.valid_until_epoch)?,
    );
    root.insert("declaration_window".into(), Value::Object(window));

    Ok(Value::Object(root))
}

fn metering_to_json(snapshot: &MeteringSnapshot) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "window_start_epoch".into(),
        json::to_value(&snapshot.window_start_epoch)?,
    );
    map.insert(
        "window_end_epoch".into(),
        json::to_value(&snapshot.window_end_epoch)?,
    );
    map.insert(
        "observed_at_epoch".into(),
        json::to_value(&snapshot.observed_at_epoch)?,
    );
    map.insert(
        "declared_gib".into(),
        json::to_value(&snapshot.declared_gib)?,
    );
    map.insert(
        "effective_gib".into(),
        json::to_value(&snapshot.effective_gib)?,
    );
    map.insert(
        "utilised_gib".into(),
        json::to_value(&snapshot.utilised_gib)?,
    );
    map.insert(
        "outstanding_total_gib".into(),
        json::to_value(&snapshot.outstanding_total_gib)?,
    );
    map.insert(
        "outstanding_orders".into(),
        json::to_value(&snapshot.outstanding_orders)?,
    );
    map.insert(
        "orders_issued".into(),
        json::to_value(&snapshot.orders_issued)?,
    );
    map.insert(
        "orders_completed".into(),
        json::to_value(&snapshot.orders_completed)?,
    );
    map.insert(
        "orders_failed".into(),
        json::to_value(&snapshot.orders_failed)?,
    );
    map.insert(
        "accumulated_gib_seconds".into(),
        Value::String(snapshot.accumulated_gib_seconds.to_string()),
    );
    map.insert(
        "accumulated_gib_hours".into(),
        json::to_value(&snapshot.accumulated_gib_hours)?,
    );
    if let Some(value) = snapshot.smoothed_gib_hours {
        map.insert("smoothed_gib_hours".into(), json::to_value(&value)?);
    }
    map.insert(
        "uptime_samples_success".into(),
        json::to_value(&snapshot.uptime_samples_success)?,
    );
    map.insert(
        "uptime_samples_total".into(),
        json::to_value(&snapshot.uptime_samples_total)?,
    );
    map.insert("uptime_bps".into(), json::to_value(&snapshot.uptime_bps)?);
    map.insert(
        "por_samples_success".into(),
        json::to_value(&snapshot.por_samples_success)?,
    );
    map.insert(
        "por_samples_total".into(),
        json::to_value(&snapshot.por_samples_total)?,
    );
    map.insert(
        "por_success_bps".into(),
        json::to_value(&snapshot.por_success_bps)?,
    );
    if let Some(value) = snapshot.smoothed_por_success_bps {
        map.insert("smoothed_por_success_bps".into(), json::to_value(&value)?);
    }
    map.insert("notes".into(), json::to_value(&snapshot.notes)?);
    Ok(Value::Object(map))
}

fn fee_projection_to_json(projection: &FeeProjection) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "provider_id_hex".into(),
        Value::String(hex::encode(projection.provider_id)),
    );
    map.insert(
        "window_start_epoch".into(),
        json::to_value(&projection.window_start_epoch)?,
    );
    map.insert(
        "window_end_epoch".into(),
        json::to_value(&projection.window_end_epoch)?,
    );
    map.insert(
        "window_duration_secs".into(),
        json::to_value(&projection.window_duration_secs)?,
    );
    map.insert(
        "declared_gib".into(),
        json::to_value(&projection.declared_gib)?,
    );
    map.insert(
        "effective_gib".into(),
        json::to_value(&projection.effective_gib)?,
    );
    map.insert(
        "utilised_gib".into(),
        json::to_value(&projection.utilised_gib)?,
    );
    map.insert(
        "accumulated_gib_seconds".into(),
        Value::String(projection.accumulated_gib_seconds.to_string()),
    );
    map.insert(
        "accumulated_gib_hours".into(),
        json::to_value(&projection.accumulated_gib_hours())?,
    );
    map.insert("uptime_bps".into(), json::to_value(&projection.uptime_bps)?);
    map.insert(
        "por_success_bps".into(),
        json::to_value(&projection.por_success_bps)?,
    );
    map.insert(
        "fee_nanos".into(),
        Value::String(projection.fee_nanos.to_string()),
    );
    Ok(Value::Object(map))
}

fn build_telemetry_preview_json(
    preview: Option<Result<CapacityTelemetryV1, TelemetryError>>,
) -> Option<Value> {
    match preview {
        Some(Ok(payload)) => match capacity_telemetry_to_json(&payload) {
            Ok(value) => Some(value),
            Err(err) => {
                warn!(?err, "failed to serialize SoraFS telemetry preview to JSON");
                None
            }
        },
        Some(Err(err)) => {
            warn!(?err, "failed to build SoraFS telemetry preview");
            None
        }
        None => None,
    }
}

fn capacity_telemetry_to_json(snapshot: &CapacityTelemetryV1) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("version".into(), json::to_value(&snapshot.version)?);
    map.insert(
        "provider_id_hex".into(),
        Value::String(snapshot.provider_id.encode_hex::<String>()),
    );
    map.insert("epoch_start".into(), json::to_value(&snapshot.epoch_start)?);
    map.insert("epoch_end".into(), json::to_value(&snapshot.epoch_end)?);
    map.insert(
        "declared_capacity_gib".into(),
        json::to_value(&snapshot.declared_capacity_gib)?,
    );
    map.insert(
        "utilised_capacity_gib".into(),
        json::to_value(&snapshot.utilised_capacity_gib)?,
    );
    map.insert(
        "successful_replications".into(),
        json::to_value(&snapshot.successful_replications)?,
    );
    map.insert(
        "failed_replications".into(),
        json::to_value(&snapshot.failed_replications)?,
    );
    map.insert(
        "uptime_percent_milli".into(),
        json::to_value(&snapshot.uptime_percent_milli)?,
    );
    map.insert(
        "por_success_percent_milli".into(),
        json::to_value(&snapshot.por_success_percent_milli)?,
    );
    map.insert("notes".into(), json::to_value(&snapshot.notes)?);
    Ok(Value::Object(map))
}

fn pin_snapshot_with_attestation(
    state: &SharedAppState,
) -> ApiResult<(Value, PinRegistrySnapshot)> {
    let view = state.state.view();
    let world = view.world();
    let block_hash = iroha_core::state::StateReadOnly::latest_block_hash(&view);
    let height = view.height() as u64;
    let chain_id = view.chain_id();

    let attestation = match build_attestation_value(height, block_hash.as_ref(), chain_id) {
        Ok(value) => value,
        Err(err) => {
            error!(?err, "failed to serialize registry attestation metadata");
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialize registry attestation metadata",
            )
            .into());
        }
    };

    match collect_pin_registry(world) {
        Ok(snapshot) => {
            let telemetry = state.telemetry_handle();
            record_pin_registry_metrics(&telemetry, &snapshot);
            Ok((attestation, snapshot))
        }
        Err(err) => {
            error!(?err, "failed to collect pin registry snapshot");
            Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to collect pin registry snapshot",
            )
            .into())
        }
    }
}

fn parse_manifest_digest_hex(hex_str: &str) -> ApiResult<[u8; 32]> {
    match hex::decode(hex_str) {
        Ok(bytes) => bytes.try_into().map_err(|_| {
            ResponseError::from(json_error(
                StatusCode::BAD_REQUEST,
                "expected 32-byte manifest digest",
            ))
        }),
        Err(err) => Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("invalid manifest digest hex: {err}"),
        )
        .into()),
    }
}

fn normalize_limit(limit: Option<u32>) -> usize {
    let requested = limit.unwrap_or(DEFAULT_LIST_LIMIT as u32);
    let clamped = requested.clamp(1, MAX_LIST_LIMIT as u32);
    clamped as usize
}

fn normalize_offset(offset: Option<u32>, total: usize) -> usize {
    let requested = offset.unwrap_or(0);
    requested.min(total as u32) as usize
}

impl PinStatusFilter {
    fn matches(self, status: &str) -> bool {
        match self {
            PinStatusFilter::Pending => status.eq_ignore_ascii_case("pending"),
            PinStatusFilter::Approved => status.eq_ignore_ascii_case("approved"),
            PinStatusFilter::Retired => status.eq_ignore_ascii_case("retired"),
        }
    }
}

fn parse_pin_status_filter(value: &str) -> Option<PinStatusFilter> {
    match value.to_ascii_lowercase().as_str() {
        "pending" => Some(PinStatusFilter::Pending),
        "approved" => Some(PinStatusFilter::Approved),
        "retired" => Some(PinStatusFilter::Retired),
        _ => None,
    }
}

impl ReplicationStatusFilter {
    fn matches(self, status: &str) -> bool {
        match self {
            ReplicationStatusFilter::Pending => status.eq_ignore_ascii_case("pending"),
            ReplicationStatusFilter::Completed => status.eq_ignore_ascii_case("completed"),
            ReplicationStatusFilter::Expired => status.eq_ignore_ascii_case("expired"),
        }
    }
}

fn parse_replication_status_filter(value: &str) -> Option<ReplicationStatusFilter> {
    match value.to_ascii_lowercase().as_str() {
        "pending" => Some(ReplicationStatusFilter::Pending),
        "completed" => Some(ReplicationStatusFilter::Completed),
        "expired" => Some(ReplicationStatusFilter::Expired),
        _ => None,
    }
}

fn build_attestation_value(
    height: u64,
    block_hash: Option<&HashOf<BlockHeader>>,
    chain_id: &ChainId,
) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("block_height".into(), json::to_value(&height)?);
    map.insert(
        "block_hash_hex".into(),
        block_hash
            .map(|hash| Value::String(hex::encode(hash.as_ref().as_ref())))
            .unwrap_or(Value::Null),
    );
    map.insert("chain_id".into(), Value::String(chain_id.to_string()));
    Ok(Value::Object(map))
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    let response = json_object(vec![json_entry("error", message.into())]);
    (status, JsonBody(response)).into_response()
}

fn pin_auth_error_response(err: PinAuthError) -> Response {
    match err {
        PinAuthError::TokenRequired => json_error(StatusCode::UNAUTHORIZED, "pin token required"),
        PinAuthError::InvalidToken => json_error(StatusCode::FORBIDDEN, "invalid pin token"),
        PinAuthError::IpNotAllowed => {
            json_error(StatusCode::FORBIDDEN, "client not allowed for storage pin")
        }
        PinAuthError::RateLimited { retry_after } => {
            let mut response = json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "storage pin rate limit exceeded",
            );
            let secs = retry_after.as_secs().max(1);
            if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
            response
        }
    }
}

fn storage_pin_quota_response(err: QuotaExceeded) -> Response {
    let mut response = json_error(
        StatusCode::TOO_MANY_REQUESTS,
        format!(
            "storage pin quota exceeded ({} requests per {}s)",
            err.max_events(),
            err.window().as_secs().max(1)
        ),
    );
    if let Ok(value) = HeaderValue::from_str(&err.window().as_secs().max(1).to_string()) {
        response.headers_mut().insert(RETRY_AFTER, value);
    }
    response
}

fn feature_disabled(message: &str) -> Response {
    json_error(StatusCode::NOT_FOUND, message)
}

fn storage_disabled_response() -> Response {
    feature_disabled("sorafs storage API is not enabled on this node")
}

fn decode_nonce(input: &str) -> Result<[u8; 16], String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("nonce_b64 must not be empty".to_string());
    }
    let decoded = BASE64_STANDARD
        .decode(trimmed.as_bytes())
        .map_err(|err| format!("invalid nonce_b64: {err}"))?;
    if decoded.len() != 16 {
        return Err(format!(
            "nonce_b64 must decode to 16 bytes (found {})",
            decoded.len()
        ));
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&decoded);
    Ok(out)
}

fn parse_hex_fixed<const N: usize>(input: &str, field: &str) -> Result<[u8; N], String> {
    let trimmed = input.trim();
    if trimmed.len() != N * 2 {
        return Err(format!("{field} must be {} hexadecimal characters", N * 2));
    }
    let mut out = [0u8; N];
    hex::decode_to_slice(trimmed, &mut out).map_err(|err| format!("invalid {field}: {err}"))?;
    Ok(out)
}

fn record_storage_metrics(state: &SharedAppState) {
    #[cfg(feature = "telemetry")]
    {
        if let Some(provider_id) = state.sorafs_node.capacity_usage().provider_id {
            let provider_hex = hex::encode(provider_id);
            let schedulers = state.sorafs_node.schedulers();
            let snapshot = schedulers.telemetry_snapshot();
            state.telemetry.with_metrics(|metrics| {
                metrics.record_sorafs_storage(
                    &provider_hex,
                    snapshot.bytes_used,
                    snapshot.bytes_capacity,
                    snapshot.pin_queue_depth as u64,
                    snapshot.fetch_inflight as u64,
                    snapshot.fetch_bytes_per_sec,
                    snapshot.por_inflight as u64,
                    snapshot.por_samples_success,
                    snapshot.por_samples_failed,
                );
                for status in state.sorafs_node.por_ingestion_overview() {
                    let manifest_hex = hex::encode(status.manifest_digest);
                    let entry_provider_hex = hex::encode(status.provider_id);
                    metrics.record_sorafs_por_ingestion_backlog(
                        &entry_provider_hex,
                        &manifest_hex,
                        status.pending_challenges,
                    );
                    metrics.record_sorafs_por_ingestion_failures(
                        &entry_provider_hex,
                        &manifest_hex,
                        status.failures_total,
                    );
                }
            });
        }
    }
    #[cfg(not(feature = "telemetry"))]
    let _ = state;
}

fn parse_proof_kind(label: &str) -> Option<ProofStreamKind> {
    match label.trim().to_ascii_lowercase().as_str() {
        "por" => Some(ProofStreamKind::Por),
        "pdp" => Some(ProofStreamKind::Pdp),
        "potr" => Some(ProofStreamKind::Potr),
        _ => None,
    }
}

fn parse_tier(label: Option<&str>) -> Result<Option<ProofStreamTier>, String> {
    label.map_or(Ok(None), |value| {
        match value.trim().to_ascii_lowercase().as_str() {
            "hot" => Ok(Some(ProofStreamTier::Hot)),
            "warm" => Ok(Some(ProofStreamTier::Warm)),
            "archive" => Ok(Some(ProofStreamTier::Archive)),
            _ => Err("tier must be hot, warm, or archive".to_string()),
        }
    })
}

fn request_error_message(error: ProofStreamRequestError) -> &'static str {
    match error {
        ProofStreamRequestError::InvalidManifestDigest => "manifest_digest_hex must be non-zero",
        ProofStreamRequestError::InvalidProviderId => "provider_id_hex must be non-zero",
        ProofStreamRequestError::InvalidNonce => "nonce must be non-zero",
        ProofStreamRequestError::MissingSampleCount => {
            "sample_count is required for PoR/PDP proof kinds"
        }
        ProofStreamRequestError::MissingDeadlineMs => {
            "deadline_ms is required when requesting PoTR proofs"
        }
        ProofStreamRequestError::ZeroSampleCount => "sample_count must be greater than zero",
        ProofStreamRequestError::ZeroDeadlineMs => {
            "deadline_ms must be greater than zero milliseconds"
        }
    }
}

fn node_storage_error_response(err: NodeStorageError) -> Response {
    match err {
        NodeStorageError::Disabled => storage_disabled_response(),
        NodeStorageError::Storage(inner) => storage_backend_error(inner),
    }
}

fn storage_backend_error(err: StorageBackendError) -> Response {
    match err {
        StorageBackendError::ManifestExists { manifest_id } => json_error(
            StatusCode::CONFLICT,
            format!("manifest {manifest_id} already stored"),
        ),
        StorageBackendError::CapacityExceeded {
            required,
            available,
        } => json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "storage capacity exceeded (required {required} GiB, available {available} GiB)"
            ),
        ),
        StorageBackendError::PinLimitReached { limit } => json_error(
            StatusCode::BAD_REQUEST,
            format!("storage pin limit of {limit} manifests reached"),
        ),
        StorageBackendError::ManifestNotFound { manifest_id } => json_error(
            StatusCode::NOT_FOUND,
            format!("manifest {manifest_id} not found"),
        ),
        StorageBackendError::ChunkNotFound {
            manifest_id,
            digest_hex,
        } => json_error(
            StatusCode::NOT_FOUND,
            format!("chunk {digest_hex} not found in manifest {manifest_id}"),
        ),
        StorageBackendError::RangeOutOfBounds {
            offset,
            len,
            content_length,
        } => json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "requested range offset {offset} length {len} exceeds payload length {content_length}"
            ),
        ),
        StorageBackendError::ChunkDigestMismatch { chunk_index } => json_error(
            StatusCode::BAD_REQUEST,
            format!("chunk digest mismatch for chunk {chunk_index}"),
        ),
        StorageBackendError::PayloadLengthMismatch { expected, actual } => json_error(
            StatusCode::BAD_REQUEST,
            format!("payload length mismatch: expected {expected}, read {actual}"),
        ),
        StorageBackendError::UnsupportedManifestVersion { version } => json_error(
            StatusCode::BAD_REQUEST,
            format!("unsupported manifest version {version}"),
        ),
        StorageBackendError::ChunkProfileMismatch => json_error(
            StatusCode::BAD_REQUEST,
            "chunk profile mismatch between manifest and payload plan",
        ),
        StorageBackendError::ChunkRoleLengthMismatch { expected, actual } => json_error(
            StatusCode::BAD_REQUEST,
            format!("chunk_roles length {actual} does not match chunk count {expected}"),
        ),
        StorageBackendError::Io(err) => {
            error!(?err, "SoraFS storage backend I/O error");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage backend I/O error",
            )
        }
        StorageBackendError::Norito(err) => {
            error!(?err, "SoraFS storage backend serialization error");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage backend serialization error",
            )
        }
        StorageBackendError::ChunkStore(err) => {
            error!(?err, "SoraFS PoR store error");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage backend PoR error",
            )
        }
    }
}

fn car_verification_refusal(
    state: &SharedAppState,
    profile: &str,
    provider_id: Option<&[u8; 32]>,
    scope: &'static str,
    err: CarVerifyError,
) -> Response {
    let (detail_code, reason) = match err {
        CarVerifyError::ManifestCarSizeMismatch { .. } => (
            "manifest_car_size_mismatch",
            "manifest CAR size mismatch detected during verification",
        ),
        CarVerifyError::ManifestCarDigestMismatch => (
            "manifest_car_digest_mismatch",
            "manifest CAR digest mismatch detected during verification",
        ),
        CarVerifyError::ManifestContentLengthMismatch { .. } => (
            "manifest_content_length_mismatch",
            "manifest content length mismatch detected during verification",
        ),
        CarVerifyError::ManifestRootMismatch => (
            "manifest_root_mismatch",
            "manifest root CID mismatch detected during verification",
        ),
        CarVerifyError::ManifestMultihashMismatch(_) => (
            "manifest_multihash_mismatch",
            "manifest multihash mismatch detected during verification",
        ),
        CarVerifyError::ChunkProfileMismatch => (
            "chunk_profile_mismatch",
            "manifest chunk profile is inconsistent with proof bundle",
        ),
        CarVerifyError::ExpectedRangeMismatch { .. } => (
            "range_expected_mismatch",
            "range verification failed: expected range mismatch",
        ),
        CarVerifyError::RangeExceedsContentLength { .. } => (
            "range_exceeds_content_length",
            "range verification failed: range exceeds manifest content length",
        ),
        CarVerifyError::ChunkLengthMismatch { .. } => (
            "chunk_length_mismatch",
            "chunk length mismatch detected in proof bundle",
        ),
        CarVerifyError::ChunkSizeExceeded { .. } => (
            "chunk_size_exceeds_limit",
            "chunk size exceeds configured verification limit",
        ),
        CarVerifyError::ChunkDigestMismatch { .. } => (
            "chunk_digest_mismatch",
            "chunk digest mismatch detected in proof bundle",
        ),
        CarVerifyError::ChunkOffsetMismatch { .. } => (
            "chunk_offset_mismatch",
            "chunk offset mismatch detected in proof bundle",
        ),
        CarVerifyError::PlanChunkCountMismatch { .. } => (
            "plan_chunk_count_mismatch",
            "chunk count mismatch detected in proof bundle",
        ),
        CarVerifyError::PlanContentLengthMismatch { .. } => (
            "plan_content_length_mismatch",
            "plan content length mismatch detected during verification",
        ),
        CarVerifyError::UnknownChunkDigest { .. } => (
            "unknown_chunk_digest",
            "unknown chunk digest encountered during verification",
        ),
        CarVerifyError::NodeDigestMismatch { .. } => (
            "node_digest_mismatch",
            "node digest mismatch detected during verification",
        ),
        CarVerifyError::NonContiguousChunkRange { .. } => (
            "non_contiguous_chunk_range",
            "non-contiguous chunk range detected in proof bundle",
        ),
        CarVerifyError::EmptyRange => (
            "empty_range",
            "empty range detected during proof verification",
        ),
        CarVerifyError::PlanChunkIndexOutOfRange { .. } => (
            "plan_chunk_index_out_of_range",
            "chunk index outside expected range detected",
        ),
        CarVerifyError::Plan(_) => ("plan_corrupted", "chunk plan failed integrity validation"),
        CarVerifyError::UnsupportedDigestLength { .. } => (
            "unsupported_digest_length",
            "unsupported digest length encountered during verification",
        ),
        CarVerifyError::UnsupportedMultihash { .. } => (
            "unsupported_multihash",
            "unsupported multihash encountered during verification",
        ),
        CarVerifyError::UnsupportedSectionCodec { .. } => (
            "unsupported_section_codec",
            "unsupported CAR section codec encountered",
        ),
        CarVerifyError::InvalidIndexOffset => (
            "invalid_index_offset",
            "invalid CAR index offset encountered",
        ),
        CarVerifyError::TruncatedSection { .. }
        | CarVerifyError::TruncatedCid { .. }
        | CarVerifyError::Truncated
        | CarVerifyError::InvalidPragma
        | CarVerifyError::InvalidHeader
        | CarVerifyError::InvalidCarv1Header(_)
        | CarVerifyError::VarintOverflow
        | CarVerifyError::HeaderTruncated
        | CarVerifyError::InternalInvariant(_) => (
            "car_parse_error",
            "CAR verification failed due to malformed payload",
        ),
    };

    let mut details = vec![("error_kind", Value::from(detail_code))];
    if let CarVerifyError::ExpectedRangeMismatch {
        expected_start,
        expected_end,
        actual_start,
        actual_end,
    } = err
    {
        details.push(("expected_start", Value::from(expected_start)));
        details.push(("expected_end", Value::from(expected_end)));
        details.push(("actual_start", Value::from(actual_start)));
        details.push(("actual_end", Value::from(actual_end)));
    }

    gateway_refusal_response(
        state,
        StatusCode::UNPROCESSABLE_ENTITY,
        "proof_mismatch",
        reason,
        Some(profile),
        provider_id,
        scope,
        details,
    )
}

fn chunk_profile_for_manifest(manifest: &ManifestV1) -> ApiResult<ChunkProfile> {
    if let Some(descriptor) = chunker_registry::lookup(manifest.chunking.profile_id) {
        if descriptor.multihash_code != manifest.chunking.multihash_code {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                format!(
                    "manifest multihash code {} does not match registry descriptor {}",
                    manifest.chunking.multihash_code, descriptor.multihash_code
                ),
            )
            .into());
        }
        Ok(descriptor.profile)
    } else {
        let min_size = usize::try_from(manifest.chunking.min_size).map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "manifest min_size exceeds supported range",
            )
        })?;
        let target_size = usize::try_from(manifest.chunking.target_size).map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "manifest target_size exceeds supported range",
            )
        })?;
        let max_size = usize::try_from(manifest.chunking.max_size).map_err(|_| {
            json_error(
                StatusCode::BAD_REQUEST,
                "manifest max_size exceeds supported range",
            )
        })?;
        let profile = ChunkProfile {
            min_size,
            target_size,
            max_size,
            break_mask: u64::from(manifest.chunking.break_mask),
        };
        if profile.min_size == 0
            || profile.target_size == 0
            || profile.max_size == 0
            || profile.break_mask == 0
        {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "manifest chunking profile parameters must be non-zero",
            )
            .into());
        }
        if profile.min_size > profile.target_size || profile.target_size > profile.max_size {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "manifest chunking profile sizes must satisfy min <= target <= max",
            )
            .into());
        }
        Ok(profile)
    }
}

#[cfg(test)]
mod chunk_profile_tests {
    use super::*;

    use blake3;
    use sorafs_manifest::{BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy};

    #[test]
    fn chunk_profile_for_manifest_rejects_out_of_order_sizes() {
        let payload = b"chunk-profile-fixture";
        let content_length = payload.len() as u64;
        let mut manifest = ManifestBuilder::new()
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");
        manifest.chunking.profile_id = sorafs_manifest::ProfileId(u32::MAX);
        manifest.chunking.min_size = 1024;
        manifest.chunking.target_size = 512;
        manifest.chunking.max_size = 2048;
        manifest.chunking.break_mask = 1;

        let err = chunk_profile_for_manifest(&manifest).expect_err("invalid profile should fail");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }
}

fn advert_to_json(
    fingerprint: &[u8; 32],
    advert: &ProviderAdvertV1,
    known_capabilities: &[sorafs_manifest::CapabilityType],
    warnings: &[AdvertWarning],
) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "provider_id_hex".into(),
        Value::String(advert.body.provider_id.encode_hex::<String>()),
    );
    map.insert(
        "fingerprint_hex".into(),
        Value::String(fingerprint.encode_hex::<String>()),
    );
    map.insert("issued_at".into(), json::to_value(&advert.issued_at)?);
    map.insert("expires_at".into(), json::to_value(&advert.expires_at)?);
    map.insert(
        "allow_unknown_capabilities".into(),
        json::to_value(&advert.allow_unknown_capabilities)?,
    );
    let known_caps = known_capabilities
        .iter()
        .map(|cap| capability_name(*cap).to_string())
        .collect::<Vec<_>>();
    map.insert("known_capabilities".into(), json::to_value(&known_caps)?);
    map.insert(
        "signature_strict".into(),
        json::to_value(&advert.signature_strict)?,
    );
    map.insert(
        "profile_id".into(),
        json::to_value(&advert.body.profile_id)?,
    );
    map.insert(
        "profile_aliases".into(),
        json::to_value(&advert.body.profile_aliases)?,
    );
    map.insert("stake".into(), stake_to_json(&advert.body.stake)?);
    map.insert("qos".into(), qos_to_json(advert.body.qos)?);
    map.insert(
        "capabilities".into(),
        Value::Array(
            advert
                .body
                .capabilities
                .iter()
                .map(capability_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "endpoints".into(),
        Value::Array(
            advert
                .body
                .endpoints
                .iter()
                .map(endpoint_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "rendezvous_topics".into(),
        Value::Array(
            advert
                .body
                .rendezvous_topics
                .iter()
                .map(rendezvous_topic_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "path_policy".into(),
        path_policy_to_json(advert.body.path_policy)?,
    );
    map.insert("notes".into(), json::to_value(&advert.body.notes)?);
    if let Some(budget) = &advert.body.stream_budget {
        map.insert("stream_budget".into(), stream_budget_to_json(budget)?);
    }
    if let Some(hints) = advert
        .body
        .transport_hints
        .as_ref()
        .filter(|h| !h.is_empty())
    {
        map.insert("transport_hints".into(), transport_hints_to_json(hints)?);
    }
    map.insert(
        "norito_hex".into(),
        Value::String(
            to_bytes(advert)
                .map_err(|err| json::Error::Message(err.to_string()))?
                .encode_hex::<String>(),
        ),
    );
    map.insert(
        "warnings".into(),
        Value::Array(
            warnings
                .iter()
                .map(|warning| Value::String(warning.as_str().to_string()))
                .collect(),
        ),
    );
    Ok(Value::Object(map))
}

fn capability_to_json(capability: &CapabilityTlv) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "type".into(),
        Value::String(capability_name(capability.cap_type).to_string()),
    );
    map.insert(
        "type_id".into(),
        json::to_value(&(capability.cap_type as u16))?,
    );
    map.insert(
        "payload_hex".into(),
        Value::String(hex::encode(&capability.payload)),
    );
    if capability.cap_type == CapabilityType::ChunkRangeFetch {
        match ProviderCapabilityRangeV1::from_bytes(&capability.payload) {
            Ok(range) => {
                map.insert("range".into(), range_to_json(&range));
            }
            Err(err) => {
                map.insert("range_decode_error".into(), Value::String(err.to_string()));
            }
        }
    }
    Ok(Value::Object(map))
}

fn stake_to_json(stake: &StakePointer) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "pool_id_hex".into(),
        Value::String(stake.pool_id.encode_hex::<String>()),
    );
    map.insert("stake_amount".into(), json::to_value(&stake.stake_amount)?);
    Ok(Value::Object(map))
}

fn qos_to_json(qos: QosHints) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "availability".into(),
        Value::String(availability_tier_name(qos.availability).to_string()),
    );
    map.insert(
        "max_retrieval_latency_ms".into(),
        json::to_value(&qos.max_retrieval_latency_ms)?,
    );
    map.insert(
        "max_concurrent_streams".into(),
        json::to_value(&qos.max_concurrent_streams)?,
    );
    Ok(Value::Object(map))
}

fn availability_tier_name(tier: sorafs_manifest::AvailabilityTier) -> &'static str {
    match tier {
        sorafs_manifest::AvailabilityTier::Hot => "hot",
        sorafs_manifest::AvailabilityTier::Warm => "warm",
        sorafs_manifest::AvailabilityTier::Cold => "cold",
    }
}

fn endpoint_to_json(endpoint: &AdvertEndpoint) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "kind".into(),
        Value::String(endpoint_kind_name(endpoint.kind).to_string()),
    );
    map.insert(
        "host_pattern".into(),
        json::to_value(&endpoint.host_pattern)?,
    );
    map.insert(
        "metadata".into(),
        Value::Array(
            endpoint
                .metadata
                .iter()
                .map(endpoint_metadata_to_json)
                .collect(),
        ),
    );
    Ok(Value::Object(map))
}

fn endpoint_metadata_to_json(metadata: &EndpointMetadata) -> Value {
    let mut map = Map::new();
    map.insert(
        "key".into(),
        Value::String(endpoint_metadata_key_name(metadata.key).to_string()),
    );
    map.insert(
        "value_hex".into(),
        Value::String(hex::encode(&metadata.value)),
    );
    Value::Object(map)
}

fn endpoint_kind_name(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Torii => "torii",
        EndpointKind::Quic => "quic",
        EndpointKind::NoritoRpc => "norito_rpc",
    }
}

fn endpoint_metadata_key_name(key: EndpointMetadataKey) -> &'static str {
    match key {
        EndpointMetadataKey::TlsFingerprint => "tls_fingerprint",
        EndpointMetadataKey::Alpn => "alpn",
        EndpointMetadataKey::Region => "region",
    }
}

fn rendezvous_topic_to_json(topic: &RendezvousTopic) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("topic".into(), json::to_value(&topic.topic)?);
    map.insert("region".into(), json::to_value(&topic.region)?);
    Ok(Value::Object(map))
}

fn path_policy_to_json(policy: PathDiversityPolicy) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "min_guard_weight".into(),
        json::to_value(&policy.min_guard_weight)?,
    );
    map.insert(
        "max_same_asn_per_path".into(),
        json::to_value(&policy.max_same_asn_per_path)?,
    );
    map.insert(
        "max_same_pool_per_path".into(),
        json::to_value(&policy.max_same_pool_per_path)?,
    );
    Ok(Value::Object(map))
}

fn format_advert_error(err: AdvertError) -> String {
    match err {
        AdvertError::UnknownCapabilities { capabilities } => format!(
            "unknown capabilities rejected: {:?}",
            capabilities
                .into_iter()
                .map(capability_name)
                .collect::<Vec<_>>()
        ),
        AdvertError::AdmissionMissing { provider_id } => format!(
            "no admission envelope registered for provider {}",
            hex::encode(provider_id)
        ),
        AdvertError::AdmissionFailed { provider_id, error } => format!(
            "provider {} failed admission checks: {}",
            hex::encode(provider_id),
            error
        ),
        other => other.to_string(),
    }
}

fn range_to_json(range: &ProviderCapabilityRangeV1) -> Value {
    let mut obj = Map::new();
    obj.insert("max_chunk_span".into(), Value::from(range.max_chunk_span));
    obj.insert("min_granularity".into(), Value::from(range.min_granularity));
    obj.insert(
        "supports_sparse_offsets".into(),
        Value::from(range.supports_sparse_offsets),
    );
    obj.insert(
        "requires_alignment".into(),
        Value::from(range.requires_alignment),
    );
    obj.insert(
        "supports_merkle_proof".into(),
        Value::from(range.supports_merkle_proof),
    );
    Value::Object(obj)
}

fn stream_budget_to_json(budget: &StreamBudgetV1) -> Result<Value, json::Error> {
    let mut obj = Map::new();
    obj.insert(
        "max_in_flight".into(),
        json::to_value(&(budget.max_in_flight as u64))?,
    );
    obj.insert(
        "max_bytes_per_sec".into(),
        json::to_value(&budget.max_bytes_per_sec)?,
    );
    obj.insert(
        "burst_bytes".into(),
        match budget.burst_bytes {
            Some(burst) => json::to_value(&burst)?,
            None => Value::Null,
        },
    );
    Ok(Value::Object(obj))
}

fn transport_hints_to_json(hints: &[TransportHintV1]) -> Result<Value, json::Error> {
    let entries = hints
        .iter()
        .map(|hint| {
            let mut obj = Map::new();
            obj.insert(
                "protocol".into(),
                Value::String(transport_protocol_name(hint.protocol).to_string()),
            );
            obj.insert(
                "protocol_id".into(),
                json::to_value(&(hint.protocol as u8))?,
            );
            obj.insert("priority".into(), json::to_value(&(hint.priority as u64))?);
            Ok(Value::Object(obj))
        })
        .collect::<Result<Vec<_>, json::Error>>()?;
    Ok(Value::Array(entries))
}

fn transport_protocol_name(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::ToriiHttpRange => "torii_http_range",
        TransportProtocol::QuicStream => "quic_stream",
        TransportProtocol::SoraNetRelay => "soranet_relay",
        TransportProtocol::VendorReserved => "vendor_reserved",
    }
}

fn to_bytes(advert: &ProviderAdvertV1) -> Result<Vec<u8>, AdvertError> {
    Ok(norito::to_bytes(advert)?)
}

#[cfg(feature = "telemetry")]
fn record_range_capability_metrics(cache: &ProviderAdvertCache, telemetry: &MaybeTelemetry) {
    if !telemetry.is_enabled() {
        return;
    }

    let mut providers = 0_i64;
    let mut sparse = 0_i64;
    let mut alignment = 0_i64;
    let mut merkle = 0_i64;
    let mut stream_budget = 0_i64;
    let mut transport_hints = 0_i64;

    for record in cache.records() {
        if !record
            .known_capabilities()
            .contains(&CapabilityType::ChunkRangeFetch)
        {
            continue;
        }

        let advert = record.advert();
        let Some(range_tlv) = advert
            .body
            .capabilities
            .iter()
            .find(|tlv| tlv.cap_type == CapabilityType::ChunkRangeFetch)
        else {
            continue;
        };

        let range = match ProviderCapabilityRangeV1::from_bytes(&range_tlv.payload) {
            Ok(range) => range,
            Err(_) => continue,
        };

        providers += 1;
        if range.supports_sparse_offsets {
            sparse += 1;
        }
        if range.requires_alignment {
            alignment += 1;
        }
        if range.supports_merkle_proof {
            merkle += 1;
        }
        if advert.body.stream_budget.is_some() {
            stream_budget += 1;
        }
        if advert
            .body
            .transport_hints
            .as_ref()
            .map_or(false, |hints| !hints.is_empty())
        {
            transport_hints += 1;
        }
    }

    telemetry.with_metrics(|metrics| {
        metrics.set_sorafs_provider_range_capability(RANGE_CAPABILITY_FEATURE_PROVIDERS, providers);
        metrics.set_sorafs_provider_range_capability(RANGE_CAPABILITY_FEATURE_SPARSE, sparse);
        metrics.set_sorafs_provider_range_capability(RANGE_CAPABILITY_FEATURE_ALIGNMENT, alignment);
        metrics.set_sorafs_provider_range_capability(RANGE_CAPABILITY_FEATURE_MERKLE, merkle);
        metrics.set_sorafs_provider_range_capability(
            RANGE_CAPABILITY_FEATURE_STREAM_BUDGET,
            stream_budget,
        );
        metrics.set_sorafs_provider_range_capability(
            RANGE_CAPABILITY_FEATURE_TRANSPORT_HINTS,
            transport_hints,
        );
    });
}

#[cfg(not(feature = "telemetry"))]
fn record_range_capability_metrics(_cache: &ProviderAdvertCache, _telemetry: &MaybeTelemetry) {}

fn admission_error_reason(err: &AdvertError) -> &'static str {
    match err {
        AdvertError::Decode(_) => "decode",
        AdvertError::Validation(AdvertValidationError::Expired { .. }) => "stale",
        AdvertError::Validation(_) => "policy_violation",
        AdvertError::UnsupportedSignature(_) => "unsupported_signature",
        AdvertError::Signature(_) => "signature",
        AdvertError::UnknownCapabilities { .. } => "unknown_capabilities",
        AdvertError::AdmissionMissing { .. } => "admission_missing",
        AdvertError::AdmissionFailed { error, .. } => match error {
            crate::sorafs::AdmissionCheckError::Digest(_) => "digest_error",
            crate::sorafs::AdmissionCheckError::BodyMismatch => "body_mismatch",
            crate::sorafs::AdmissionCheckError::BodyDigestMismatch { .. } => "body_digest_mismatch",
            crate::sorafs::AdmissionCheckError::AdvertKeyMismatch => "advert_key_mismatch",
            crate::sorafs::AdmissionCheckError::ExpiryAfterRetention { .. } => "retention_expired",
        },
    }
}

pub(crate) fn init_cache(
    enabled: bool,
    capabilities: Vec<sorafs_manifest::CapabilityType>,
    admission: std::sync::Arc<AdmissionRegistry>,
) -> Option<std::sync::Arc<RwLock<ProviderAdvertCache>>> {
    if !enabled {
        return None;
    }
    let cache = ProviderAdvertCache::new(capabilities, admission);
    Some(std::sync::Arc::new(RwLock::new(cache)))
}

#[cfg(test)]
mod advert_tests {
    use std::{io::Write, str::FromStr, sync::Arc};

    use axum::body::{self, Bytes};
    use base64::Engine as _;
    use blake3;
    use ed25519_dalek::{Signer, SigningKey};
    use http_body_util::BodyExt;
    use iroha_config::parameters::actual::SorafsTokenConfig;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        metadata::Metadata,
        sorafs::{
            capacity::{CapacityDeclarationRecord, ProviderId},
            pin_registry::{
                ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasRecord, ManifestDigest,
                PinManifestRecord, PinPolicy as RegistryPinPolicy, PinStatus, ReplicationOrderId,
                ReplicationOrderRecord, ReplicationOrderStatus,
            },
        },
    };
    use nonzero_ext::nonzero;
    use norito::to_bytes;
    use sorafs_manifest::{
        AdvertEndpoint, AdvertSignature, AliasClaim, AvailabilityTier, CapabilityTlv,
        CapabilityType, CouncilSignature, DagCodecId, ENDPOINT_ATTESTATION_VERSION_V1,
        EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1, EndpointKind,
        EndpointMetadata, EndpointMetadataKey, MAX_ADVERT_TTL_SECS, ManifestBuilder,
        PROVIDER_ADVERT_VERSION_V1, PathDiversityPolicy, PinPolicy, ProviderAdmissionEnvelopeV1,
        ProviderAdmissionProposalV1, ProviderAdvertBodyV1, ProviderAdvertV1, QosHints,
        RendezvousTopic, SignatureAlgorithm, StakePointer,
        capacity::{CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, ChunkerCommitmentV1},
        chunker_registry, compute_advert_body_digest, compute_proposal_digest,
        por::{
            AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1, POR_CHALLENGE_VERSION_V1,
            POR_PROOF_VERSION_V1, PorChallengeV1, PorProofSampleV1, PorProofV1,
            derive_challenge_id, derive_challenge_seed,
        },
        potr::{POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrStatus},
        proof_stream::ProofStreamTier,
    };
    use sorafs_node::config::StorageConfig;
    use tempfile::{NamedTempFile, TempDir};

    use super::*;
    use crate::{
        build_sorafs_gateway_security, mk_app_state_for_tests, sorafs,
        sorafs::{
            StreamTokenIssuer,
            registry::{RegistryCreditLedgerEntry, RegistryDeclaration, RegistryFeeLedgerEntry},
        },
        utils::extractors::JsonOnly,
    };

    #[tokio::test]
    async fn proof_stream_potr_streams_recorded_receipts() {
        let mut state = mk_app_state_for_tests();
        let (node, _dir) = sorafs_node_with_temp_storage();
        Arc::get_mut(&mut state)
            .expect("unique app state required")
            .sorafs_node = node;
        let provider_id = [0xBB; 32];
        let trace_id = [0x44; 16];
        let state = state;

        let payload = b"proof stream manifest payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");
        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&state.sorafs_node, provider_id, &chunker_handle);
        let mut reader = payload.as_slice();
        let manifest_id_hex = state
            .sorafs_node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest: [u8; 32] = manifest
            .digest()
            .expect("compute manifest digest for receipts")
            .into();
        let manifest_digest_hex = hex::encode(manifest_digest);
        let manifest_root_cid_hex = hex::encode(&manifest.root_cid);
        assert_eq!(
            manifest_id_hex, manifest_root_cid_hex,
            "manifest id should use manifest CID"
        );
        let provider_id_hex = hex::encode(provider_id);
        let trace_id_hex = hex::encode(trace_id);

        let success_receipt = PotrReceiptV1 {
            version: POTR_RECEIPT_VERSION_V1,
            manifest_digest,
            provider_id,
            tier: ProofStreamTier::Hot,
            deadline_ms: 90_000,
            latency_ms: 45,
            status: PotrStatus::Success,
            requested_at_ms: 1_700_000_000_000,
            responded_at_ms: 1_700_000_000_045,
            recorded_at_ms: 1_700_000_000_050,
            range_start: 0,
            range_end: 4_194_303,
            request_id: None,
            trace_id: Some(trace_id),
            note: None,
            gateway_signature: None,
            provider_signature: None,
        };
        state
            .sorafs_node
            .record_potr_receipt(success_receipt)
            .expect("record PoTR receipt");

        let failure_receipt = PotrReceiptV1 {
            version: POTR_RECEIPT_VERSION_V1,
            manifest_digest,
            provider_id,
            tier: ProofStreamTier::Hot,
            deadline_ms: 90_000,
            latency_ms: 120,
            status: PotrStatus::MissedDeadline,
            requested_at_ms: 1_700_000_100_000,
            responded_at_ms: 1_700_000_100_120,
            recorded_at_ms: 1_700_000_500_000,
            range_start: 0,
            range_end: 4_194_303,
            request_id: None,
            trace_id: None,
            note: None,
            gateway_signature: None,
            provider_signature: None,
        };
        state
            .sorafs_node
            .record_potr_receipt(failure_receipt)
            .expect("record PoTR receipt");

        let request = ProofStreamRequestDto {
            manifest_digest_hex: manifest_digest_hex.clone(),
            provider_id_hex: provider_id_hex.clone(),
            proof_kind: "potr".to_string(),
            sample_count: None,
            deadline_ms: Some(90_000),
            sample_seed: None,
            nonce_b64: BASE64_STANDARD.encode([0x12u8; 16]),
            orchestrator_job_id_hex: None,
            tier: None,
        };

        let response =
            handle_post_sorafs_proof_stream(State(state.clone()), JsonOnly(request)).await;
        let status = response.status();
        let body_bytes = BodyExt::collect(response.into_body())
            .await
            .expect("collect response body")
            .to_bytes();
        let body_text = String::from_utf8(body_bytes.to_vec()).expect("utf8");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected status {status} body: {body_text}"
        );
        let lines: Vec<&str> = body_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        assert_eq!(lines.len(), 2, "expected two streamed receipts");

        let first: json::Value = json::from_str(lines[0]).expect("parse first item");
        assert_eq!(
            first.get("proof_kind").and_then(json::Value::as_str),
            Some("potr")
        );
        assert_eq!(
            first.get("result").and_then(json::Value::as_str),
            Some("success")
        );
        assert_eq!(
            first.get("deadline_ms").and_then(json::Value::as_u64),
            Some(90_000)
        );
        assert_eq!(
            first.get("recorded_at_ms").and_then(json::Value::as_u64),
            Some(1_700_000_000_050)
        );
        assert!(
            matches!(first.get("failure_reason"), Some(json::Value::Null) | None),
            "success item should not include failure reason"
        );
        assert_eq!(
            first
                .get("manifest_digest_hex")
                .and_then(json::Value::as_str),
            Some(manifest_digest_hex.as_str())
        );
        assert_eq!(
            first.get("manifest_cid_hex").and_then(json::Value::as_str),
            Some(manifest_root_cid_hex.as_str())
        );
        assert_eq!(
            first
                .get("manifest_digest_hex")
                .and_then(json::Value::as_str),
            Some(manifest_digest_hex.as_str())
        );
        assert_eq!(
            first.get("manifest_cid_hex").and_then(json::Value::as_str),
            Some(manifest_root_cid_hex.as_str())
        );
        assert_eq!(
            first
                .get("trace_id")
                .and_then(json::Value::as_str)
                .map(str::to_ascii_lowercase),
            Some(trace_id_hex.clone())
        );

        let second: json::Value = json::from_str(lines[1]).expect("parse second item");
        assert_eq!(
            second.get("result").and_then(json::Value::as_str),
            Some("failure")
        );
        assert_eq!(
            second.get("failure_reason").and_then(json::Value::as_str),
            Some("missed_deadline")
        );
        assert_eq!(
            second
                .get("manifest_digest_hex")
                .and_then(json::Value::as_str),
            Some(manifest_digest_hex.as_str())
        );
        assert_eq!(
            second.get("manifest_cid_hex").and_then(json::Value::as_str),
            Some(manifest_root_cid_hex.as_str())
        );
        assert_eq!(
            second
                .get("manifest_digest_hex")
                .and_then(json::Value::as_str),
            Some(manifest_digest_hex.as_str())
        );
        assert_eq!(
            second.get("manifest_cid_hex").and_then(json::Value::as_str),
            Some(manifest_root_cid_hex.as_str())
        );
    }

    #[tokio::test]
    async fn pin_registry_metrics_summary_tracks_counts() {
        let state = make_state();
        let mut block = state.block(default_block_header());
        let mut tx = block.transaction();

        let manifest_digest = ManifestDigest::new([0x11; 32]);
        let chunker_handle = default_chunker_handle();
        let chunk_digest = [0xAA; 32];
        let issuer = test_account();

        let mut manifest_record = PinManifestRecord::new(
            manifest_digest.clone(),
            chunker_handle.clone(),
            chunk_digest,
            RegistryPinPolicy::default(),
            issuer.clone(),
            5,
            None,
            None,
            Metadata::default(),
        );
        manifest_record.approve(7, None);
        tx.world_mut_for_testing()
            .pin_manifests_mut_for_testing()
            .insert(manifest_digest.clone(), manifest_record);

        let alias_binding = ManifestAliasBinding {
            namespace: "sora".into(),
            name: "docs".into(),
            proof: vec![0x01],
        };
        let alias_record = ManifestAliasRecord::new(
            alias_binding,
            manifest_digest.clone(),
            issuer.clone(),
            9,
            40,
        );
        let alias_id = alias_record.alias_id();
        tx.world_mut_for_testing()
            .manifest_aliases_mut_for_testing()
            .insert(alias_id, alias_record);

        let completed_met_id = ReplicationOrderId::new([0x21; 32]);
        tx.world_mut_for_testing()
            .replication_orders_mut_for_testing()
            .insert(
                completed_met_id,
                ReplicationOrderRecord {
                    order_id: completed_met_id,
                    manifest_digest: manifest_digest.clone(),
                    issued_by: issuer.clone(),
                    issued_epoch: 10,
                    deadline_epoch: 16,
                    canonical_order: encode_replication_order_bytes(
                        &completed_met_id,
                        &manifest_digest,
                        16,
                    ),
                    status: ReplicationOrderStatus::Completed(13),
                },
            );

        let completed_missed_id = ReplicationOrderId::new([0x22; 32]);
        tx.world_mut_for_testing()
            .replication_orders_mut_for_testing()
            .insert(
                completed_missed_id,
                ReplicationOrderRecord {
                    order_id: completed_missed_id,
                    manifest_digest: manifest_digest.clone(),
                    issued_by: issuer.clone(),
                    issued_epoch: 20,
                    deadline_epoch: 25,
                    canonical_order: encode_replication_order_bytes(
                        &completed_missed_id,
                        &manifest_digest,
                        25,
                    ),
                    status: ReplicationOrderStatus::Completed(32),
                },
            );

        let pending_id = ReplicationOrderId::new([0x23; 32]);
        tx.world_mut_for_testing()
            .replication_orders_mut_for_testing()
            .insert(
                pending_id,
                ReplicationOrderRecord {
                    order_id: pending_id,
                    manifest_digest: manifest_digest.clone(),
                    issued_by: issuer.clone(),
                    issued_epoch: 40,
                    deadline_epoch: 55,
                    canonical_order: encode_replication_order_bytes(
                        &pending_id,
                        &manifest_digest,
                        55,
                    ),
                    status: ReplicationOrderStatus::Pending,
                },
            );

        let expired_id = ReplicationOrderId::new([0x24; 32]);
        tx.world_mut_for_testing()
            .replication_orders_mut_for_testing()
            .insert(
                expired_id,
                ReplicationOrderRecord {
                    order_id: expired_id,
                    manifest_digest,
                    issued_by: issuer,
                    issued_epoch: 50,
                    deadline_epoch: 60,
                    canonical_order: encode_replication_order_bytes(
                        &expired_id,
                        &manifest_digest,
                        60,
                    ),
                    status: ReplicationOrderStatus::Expired(62),
                },
            );

        tx.apply();
        block.commit().expect("commit block");

        let view = state.view();
        let world = view.world();
        let snapshot = collect_pin_registry(world).expect("collect pin registry snapshot");
        let summary = PinRegistryMetricsSummary::from_snapshot(&snapshot);

        assert_eq!(summary.manifests_pending, 0);
        assert_eq!(summary.manifests_approved, 1);
        assert_eq!(summary.manifests_retired, 0);
        assert_eq!(summary.alias_total, 1);
        assert_eq!(summary.orders_pending, 1);
        assert_eq!(summary.orders_completed, 2);
        assert_eq!(summary.orders_expired, 1);
        assert_eq!(summary.sla_met, 1);
        assert_eq!(summary.sla_missed, 2);

        let mut latencies = summary.completion_latencies.clone();
        latencies.sort_by(f64::total_cmp);
        assert_eq!(latencies, vec![3.0, 12.0]);
        assert_eq!(summary.deadline_slack_epochs, vec![15.0]);

        #[cfg(feature = "telemetry")]
        {
            let telemetry = crate::routing::MaybeTelemetry::for_tests();
            record_pin_registry_metrics(&telemetry, &snapshot);
            let metrics_text = telemetry
                .telemetry()
                .expect("telemetry handle")
                .metrics()
                .await
                .try_to_string()
                .expect("serialize metrics");
            assert!(
                metrics_text.contains("torii_sorafs_registry_aliases_total 1"),
                "alias gauge missing in metrics output: {metrics_text}"
            );
            assert!(
                metrics_text.contains("torii_sorafs_replication_sla_total{outcome=\"met\"} 1"),
                "SLA met gauge missing in metrics output: {metrics_text}"
            );
            assert!(
                metrics_text.contains("torii_sorafs_replication_sla_total{outcome=\"missed\"} 2"),
                "SLA missed gauge missing in metrics output: {metrics_text}"
            );
        }
    }

    fn encode_replication_order_bytes(
        order_id: &ReplicationOrderId,
        manifest_digest: &ManifestDigest,
        deadline_epoch: u64,
    ) -> Vec<u8> {
        let id_bytes = *order_id.as_bytes();
        let providers = vec![[id_bytes[0]; 32], [id_bytes[0].wrapping_add(1); 32]];
        let order = sorafs_manifest::pin_registry::ReplicationOrderV1 {
            order_id: id_bytes,
            manifest_cid: manifest_digest.as_bytes().to_vec(),
            providers,
            redundancy: 1,
            deadline: deadline_epoch,
            policy_hash: [0x51; 32],
        };
        norito::to_bytes(&order).expect("encode replication order")
    }

    fn make_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::new(), kura, query)
    }

    fn default_block_header() -> BlockHeader {
        BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)
    }

    fn default_chunker_handle() -> ChunkerProfileHandle {
        let descriptor = chunker_registry::default_descriptor();
        ChunkerProfileHandle {
            profile_id: descriptor.id.0,
            namespace: descriptor.namespace.to_owned(),
            name: descriptor.name.to_owned(),
            semver: descriptor.semver.to_owned(),
            multihash_code: descriptor.multihash_code,
        }
    }

    fn test_account() -> AccountId {
        AccountId::from_str(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@sora",
        )
        .expect("parse account id")
    }

    #[test]
    fn advert_to_json_includes_range_budget_and_hints() {
        let range_payload = ProviderCapabilityRangeV1 {
            max_chunk_span: 32,
            min_granularity: 8,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        }
        .to_bytes()
        .expect("encode range capability");

        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: 42,
            expires_at: 99,
            body: ProviderAdvertBodyV1 {
                provider_id: [0x11; 32],
                profile_id: "sorafs.sf1@1.0.0".to_owned(),
                profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned()]),
                stake: StakePointer {
                    pool_id: [0x22; 32],
                    stake_amount: 123,
                },
                qos: QosHints {
                    availability: AvailabilityTier::Hot,
                    max_retrieval_latency_ms: 1_000,
                    max_concurrent_streams: 10,
                },
                capabilities: vec![
                    CapabilityTlv {
                        cap_type: CapabilityType::ToriiGateway,
                        payload: Vec::new(),
                    },
                    CapabilityTlv {
                        cap_type: CapabilityType::ChunkRangeFetch,
                        payload: range_payload,
                    },
                ],
                endpoints: vec![AdvertEndpoint {
                    kind: EndpointKind::Torii,
                    host_pattern: "torii.example.com".to_owned(),
                    metadata: Vec::new(),
                }],
                rendezvous_topics: vec![RendezvousTopic {
                    topic: "sorafs.sf1.primary".to_owned(),
                    region: "global".to_owned(),
                }],
                path_policy: PathDiversityPolicy {
                    min_guard_weight: 5,
                    max_same_asn_per_path: 1,
                    max_same_pool_per_path: 1,
                },
                notes: None,
                stream_budget: Some(StreamBudgetV1 {
                    max_in_flight: 4,
                    max_bytes_per_sec: 5_000_000,
                    burst_bytes: Some(2_500_000),
                }),
                transport_hints: Some(vec![TransportHintV1 {
                    protocol: TransportProtocol::ToriiHttpRange,
                    priority: 0,
                }]),
            },
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0u8; 32],
                signature: vec![0u8; 64],
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };

        let json = advert_to_json(
            &[0xAA; 32],
            &advert,
            &[
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            &[AdvertWarning::SignatureNotStrict],
        )
        .expect("advert serialises");

        let obj = json.as_object().expect("top-level object");
        let stream_budget = obj
            .get("stream_budget")
            .and_then(Value::as_object)
            .expect("stream_budget object");
        assert_eq!(
            stream_budget.get("max_in_flight").and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            stream_budget
                .get("max_bytes_per_sec")
                .and_then(Value::as_u64),
            Some(5_000_000)
        );
        assert_eq!(
            stream_budget.get("burst_bytes").and_then(Value::as_u64),
            Some(2_500_000)
        );

        let transport_hints = obj
            .get("transport_hints")
            .and_then(Value::as_array)
            .expect("transport_hints array");
        assert_eq!(transport_hints.len(), 1);
        let hint = transport_hints[0]
            .as_object()
            .expect("transport hint object");
        assert_eq!(
            hint.get("protocol").and_then(Value::as_str),
            Some("torii_http_range")
        );
        assert_eq!(hint.get("priority").and_then(Value::as_u64), Some(0));

        let capabilities = obj
            .get("capabilities")
            .and_then(Value::as_array)
            .expect("capabilities array");
        let range_cap = capabilities
            .iter()
            .find_map(|cap| {
                let cap_obj = cap.as_object()?;
                (cap_obj.get("type").and_then(Value::as_str) == Some("chunk_range_fetch"))
                    .then_some(cap_obj)
            })
            .expect("range capability present");
        let range_obj = range_cap
            .get("range")
            .and_then(Value::as_object)
            .expect("range metadata present");
        assert_eq!(
            range_obj.get("max_chunk_span").and_then(Value::as_u64),
            Some(32)
        );
        assert_eq!(
            range_obj.get("min_granularity").and_then(Value::as_u64),
            Some(8)
        );
    }
    use tokio::sync::RwLock;

    fn sorafs_node_with_temp_storage() -> (sorafs_node::NodeHandle, TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .build();
        (sorafs_node::NodeHandle::new(cfg), temp_dir)
    }

    fn stream_token_issuer_for_tests(dir: &TempDir) -> StreamTokenIssuer {
        use std::fs;

        let key_path = dir.path().join("token.sk");
        fs::write(&key_path, [0xAB; 32]).expect("write signing key");
        let cfg = iroha_config::parameters::actual::SorafsTokenConfig {
            enabled: true,
            signing_key_path: Some(key_path),
            default_requests_per_minute: 3,
            ..iroha_config::parameters::actual::SorafsTokenConfig::default()
        };
        StreamTokenIssuer::from_config(&cfg)
            .expect("valid config")
            .expect("issuer enabled")
    }

    fn token_enabled_state() -> (SharedAppState, TempDir, String) {
        let app = mk_app_state_for_tests();
        let mut inner =
            Arc::try_unwrap(app).unwrap_or_else(|_| panic!("no other references to app state"));
        let (node, temp_dir) = sorafs_node_with_temp_storage();

        let payload = vec![0x11; 128];
        let manifest = manifest_for_payload(0x42, &payload);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, sorafs_chunker::ChunkProfile::DEFAULT)
                .expect("plan");
        let mut reader = payload.as_slice();
        let manifest_id = node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        let issuer = stream_token_issuer_for_tests(&temp_dir);
        inner.sorafs_node = node;
        inner.stream_token_issuer = Some(Arc::new(issuer));

        (Arc::new(inner), temp_dir, manifest_id)
    }

    fn car_range_headers(
        chunker_handle: &str,
        content_length: u64,
        token_base64: &str,
        nonce: &str,
    ) -> HeaderMap {
        let end = content_length.saturating_sub(1);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(chunker_handle, "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            header_value(nonce, "X-SoraFS-Nonce"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
            HeaderValue::from_static("dummy-envelope"),
        );
        headers
    }

    fn chunk_range_headers(
        chunker_handle: &str,
        chunk_digest_hex: &str,
        token_base64: &str,
        nonce: &str,
    ) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(chunker_handle, "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            header_value(nonce, "X-SoraFS-Nonce"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
            HeaderValue::from_static("dummy-envelope"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNK_DIGEST),
            header_value(chunk_digest_hex, "X-SoraFS-Chunk-Digest"),
        );
        headers
    }

    fn capability_token_context(fixture: &ProviderFixture, payload: Vec<u8>) -> TokenTestContext {
        let admission =
            AdmissionRegistry::from_envelopes([fixture.envelope.clone()]).expect("valid envelope");
        let mut cache = ProviderAdvertCache::new(
            vec![
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            Arc::new(admission),
        );
        cache
            .ingest(fixture.advert.clone(), fixture.advert.issued_at + 1)
            .expect("cache ingest");

        let (node, storage_dir) = sorafs_node_with_temp_storage();
        let manifest = manifest_for_payload(0x9B, &payload);
        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&node, fixture.provider_id(), &chunker_handle);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let mut reader = payload.as_slice();
        let manifest_id_hex = node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        let mut app = Arc::try_unwrap(mk_app_state_for_tests())
            .unwrap_or_else(|_| panic!("unique app state required"));
        app.sorafs_cache = Some(Arc::new(RwLock::new(cache)));
        app.sorafs_node = node;
        let mut gateway_config = app.sorafs_gateway_config.clone();
        gateway_config.enforce_capabilities = true;
        gateway_config.enforce_admission = false;
        app.sorafs_gateway_config = gateway_config;
        let components =
            build_sorafs_gateway_security(&app.sorafs_gateway_config, app.sorafs_admission.clone());
        app.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));

        let issuer = stream_token_issuer_for_tests(&storage_dir);
        let verifying_key_hex = hex::encode(issuer.verifying_key_bytes());
        app.stream_token_issuer = Some(Arc::new(issuer));

        TokenTestContext {
            app: Arc::new(app),
            manifest_id_hex,
            provider_id_hex: hex::encode(fixture.provider_id()),
            verifying_key_hex,
            client_id: "capability-gateway".to_string(),
            _storage_dir: storage_dir,
        }
    }

    fn manifest_for_payload(seed: u8, payload: &[u8]) -> ManifestV1 {
        let mut manifest = ManifestBuilder::new()
            .root_cid(vec![seed; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .push_alias(AliasClaim {
                name: "test".into(),
                namespace: "alias".into(),
                proof: vec![0xAA; 16],
            })
            .build()
            .expect("manifest");
        manifest.chunking.aliases.push("alias/test".to_string());
        manifest
    }

    fn seed_capacity_declaration(
        node: &sorafs_node::NodeHandle,
        provider_id: [u8; 32],
        chunker_handle: &str,
    ) {
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id,
            stake: StakePointer {
                pool_id: [0x5A; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 1,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: chunker_handle.to_string(),
                profile_aliases: Some(vec![chunker_handle.to_string()]),
                committed_gib: 1,
                capability_refs: vec![
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: 0,
            valid_until: 10,
            metadata: Vec::new(),
        };
        let declaration_bytes =
            to_bytes(&declaration).expect("encode capacity declaration fixture");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(provider_id),
            declaration_bytes,
            declaration.committed_capacity_gib,
            0,
            0,
            10,
            Metadata::default(),
        );
        node.record_capacity_declaration(&record)
            .expect("record capacity declaration");
    }

    struct TokenTestContext {
        app: SharedAppState,
        manifest_id_hex: String,
        provider_id_hex: String,
        verifying_key_hex: String,
        client_id: String,
        _storage_dir: TempDir,
    }

    fn token_test_context() -> TokenTestContext {
        token_test_context_with_payload(b"stream token payload fixture".to_vec())
    }

    fn token_test_context_with_payload(payload: Vec<u8>) -> TokenTestContext {
        let mut app = Arc::try_unwrap(mk_app_state_for_tests())
            .unwrap_or_else(|_| panic!("exclusive app state required"));
        let (node, storage_dir) = sorafs_node_with_temp_storage();

        let manifest = manifest_for_payload(0x42, &payload);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let mut reader = payload.as_slice();
        let manifest_id_hex = node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        app.sorafs_node = node;
        let provider_id = [0xAB; 32];
        let provider_id_hex = hex::encode(provider_id);

        // Tests exercise manifest envelope gating explicitly; disable by default here
        // so individual cases can opt in to stricter enforcement.
        app.sorafs_gateway_config.require_manifest_envelope = false;
        let mut key_file = NamedTempFile::new().expect("create signing key file");
        key_file
            .write_all(&[0x51; 32])
            .expect("write signing key bytes");
        key_file.flush().expect("flush signing key file");

        let token_config = SorafsTokenConfig {
            enabled: true,
            signing_key_path: Some(key_file.path().to_path_buf()),
            key_version: 7,
            default_requests_per_minute: 3,
            ..SorafsTokenConfig::default()
        };

        let issuer = StreamTokenIssuer::from_config(&token_config)
            .expect("token config valid")
            .expect("stream token issuer enabled");
        let issuer = Arc::new(issuer);
        let verifying_key_hex = hex::encode(issuer.verifying_key_bytes());
        app.stream_token_issuer = Some(issuer);

        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&app.sorafs_node, provider_id, &chunker_handle);

        app.sorafs_gateway_config.enforce_admission = false;
        let components =
            build_sorafs_gateway_security(&app.sorafs_gateway_config, app.sorafs_admission.clone());
        app.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));

        let client_id = "gateway-beta".to_string();
        let app = Arc::new(app);

        TokenTestContext {
            app,
            manifest_id_hex,
            provider_id_hex,
            verifying_key_hex,
            client_id,
            _storage_dir: storage_dir,
        }
    }

    async fn issue_token_base64(context: &TokenTestContext, overrides: TokenOverrides) -> String {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            header_value(&context.client_id, "X-SoraFS-Client"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("issuer-nonce"),
        );

        let request = StreamTokenRequestDto {
            manifest_id_hex: context.manifest_id_hex.clone(),
            provider_id_hex: context.provider_id_hex.clone(),
            ttl_secs: overrides.ttl_secs,
            max_streams: overrides.max_streams,
            rate_limit_bytes: overrides.rate_limit_bytes,
            requests_per_minute: overrides.requests_per_minute,
        };

        let response = handle_post_sorafs_storage_token(
            State(context.app.clone()),
            headers,
            JsonOnly(request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let (_, body) = response.into_parts();
        let body_bytes = body::to_bytes(body, usize::MAX)
            .await
            .expect("collect token body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode token response");
        value
            .get("token_base64")
            .and_then(Value::as_str)
            .expect("token base64 present")
            .to_string()
    }

    fn sample_por_artifacts() -> (PorChallengeV1, PorProofV1, AuditVerdictV1) {
        let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
        let chunker_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let manifest_digest = [0xBB; 32];
        let provider_id = [0xCC; 32];
        let epoch_id = 444;
        let drand_round = 555;
        let drand_randomness = [0xED; 32];
        let vrf_output = [0xEE; 32];
        let seed = derive_challenge_seed(
            &drand_randomness,
            Some(&vrf_output),
            &manifest_digest,
            epoch_id,
        );
        let challenge_id =
            derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);

        let challenge = PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id,
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: vec![0xF1; 96],
            vrf_output: Some(vrf_output),
            vrf_proof: Some(vec![0xF2; 80]),
            forced: false,
            chunking_profile: chunker_handle,
            seed,
            sample_tier: 1,
            sample_count: 1,
            sample_indices: vec![0],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_600,
        };

        let proof = PorProofV1 {
            version: POR_PROOF_VERSION_V1,
            challenge_id: challenge.challenge_id,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            samples: vec![PorProofSampleV1 {
                sample_index: 0,
                chunk_offset: 0,
                chunk_size: 65_536,
                chunk_digest: [0x11; 32],
                leaf_digest: [0x22; 32],
            }],
            auth_path: vec![[0x33; 32]],
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0x44; 32],
                signature: vec![0x55; 64],
            },
            submitted_at: 1_700_000_100,
        };

        let verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            challenge_id: challenge.challenge_id,
            proof_digest: Some(proof.proof_digest()),
            outcome: AuditOutcomeV1::Success,
            failure_reason: None,
            decided_at: 1_700_000_200,
            auditor_signatures: vec![AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0x66; 32],
                signature: vec![0x77; 64],
            }],
            metadata: Vec::new(),
        };

        (challenge, proof, verdict)
    }

    #[test]
    fn formats_admission_error_reasons() {
        let provider_id = [0u8; 32];
        assert_eq!(
            admission_error_reason(&AdvertError::AdmissionMissing { provider_id }),
            "admission_missing"
        );
        let expired = AdvertError::Validation(AdvertValidationError::Expired {
            now: 11,
            expires_at: 10,
        });
        assert_eq!(admission_error_reason(&expired), "stale",);
    }

    #[test]
    fn snapshot_to_json_renders_counts() {
        let declaration = RegistryDeclaration {
            provider_id_hex: "aa".into(),
            committed_capacity_gib: 10,
            registered_epoch: 1,
            valid_from_epoch: 2,
            valid_until_epoch: 3,
            declaration_json: json_object(vec![json_entry("version", 1u64)]),
            metadata_json: Value::Object(Map::new()),
        };
        let ledger = RegistryFeeLedgerEntry {
            provider_id_hex: "aa".into(),
            total_declared_gib: 100,
            total_utilised_gib: 80,
            storage_fee_nano: 30,
            egress_fee_nano: 12,
            accrued_fee_nano: 42,
            expected_settlement_nano: 84,
            penalty_slashed_nano: 0,
            penalty_events: 0,
            last_updated_epoch: 4,
        };
        let credit = RegistryCreditLedgerEntry {
            provider_id_hex: "aa".into(),
            available_credit_nano: 1_000,
            bonded_nano: 500,
            required_bond_nano: 400,
            expected_settlement_nano: 300,
            onboarding_epoch: 1,
            last_settlement_epoch: 2,
            low_balance_since_epoch: None,
            slashed_nano: 0,
            under_delivery_strikes: 0,
            last_penalty_epoch: None,
            metadata_json: Value::Object(Map::new()),
        };
        let snapshot = CapacitySnapshot {
            declarations: vec![declaration],
            fee_ledger: vec![ledger],
            credit_ledger: vec![credit],
            disputes: Vec::new(),
        };
        let metering = MeteringSnapshot::default();
        let json = snapshot_to_json(
            snapshot,
            &CapacityUsageSnapshot::default(),
            &metering,
            None,
            None,
        )
        .expect("serialize snapshot");
        let map = json.as_object().expect("json object");
        assert_eq!(
            map.get("declaration_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(map.get("ledger_count").and_then(Value::as_u64), Some(1));
        assert_eq!(
            map.get("credit_ledger_count").and_then(Value::as_u64),
            Some(1)
        );
        assert!(map.get("local_usage").is_some());
        assert!(map.get("disputes").is_some());
        assert!(map.get("credit_ledger").is_some());
    }

    #[tokio::test]
    async fn post_provider_advert_accepts_valid_payload() {
        let fixture = make_signed_advert();
        let app = app_state_with_cache(&fixture);

        let bytes = Bytes::from(norito::to_bytes(fixture.advert()).expect("encode advert"));
        let response = handle_post_sorafs_provider_advert(State(app.clone()), bytes).await;
        let status = response.status();
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected status {status} with body: {}",
            String::from_utf8_lossy(&body_bytes)
        );
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode JSON");
        assert_eq!(value.get("status").and_then(Value::as_str), Some("stored"));

        let cache = app
            .sorafs_cache
            .as_ref()
            .expect("cache enabled")
            .read()
            .await;
        assert_eq!(cache.len(), 1);
        let record = cache
            .record_by_provider(&fixture.advert().body.provider_id)
            .expect("record stored");
        assert!(record.warnings().is_empty());
    }

    #[tokio::test]
    async fn post_provider_advert_rejects_invalid_payload() {
        let fixture = make_signed_advert();
        let app = app_state_with_cache(&fixture);

        let response = handle_post_sorafs_provider_advert(
            State(app.clone()),
            Bytes::from_static(b"not a valid advert payload"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let cache = app
            .sorafs_cache
            .as_ref()
            .expect("cache enabled")
            .read()
            .await;
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn post_provider_advert_returns_not_found_when_disabled() {
        let app = mk_app_state_for_tests();
        let response =
            handle_post_sorafs_provider_advert(State(app), Bytes::from_static(b"anything")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn capacity_state_returns_not_found_when_disabled() {
        let app = mk_app_state_for_tests();
        let response = handle_get_sorafs_capacity_state(State(app)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn pin_registry_respects_norito_accept_header() {
        let app = mk_app_state_for_tests();
        let mut inner =
            Arc::try_unwrap(app).unwrap_or_else(|_| panic!("no other references to app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let app = Arc::new(inner);

        let response = handle_get_sorafs_pin_registry(
            State(app),
            axum::extract::RawQuery(None),
            Some(ExtractAccept(axum::http::HeaderValue::from_static(
                crate::utils::NORITO_MIME_TYPE,
            ))),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .expect("content-type header");
        assert_eq!(content_type, crate::utils::NORITO_MIME_TYPE);

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        assert!(
            !body_bytes.is_empty(),
            "Norito response body should not be empty"
        );
    }

    #[tokio::test]
    async fn capacity_state_returns_empty_snapshot() {
        let app = mk_app_state_for_tests();
        let mut inner =
            Arc::try_unwrap(app).unwrap_or_else(|_| panic!("no other references to app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let app = Arc::new(inner);

        let response = handle_get_sorafs_capacity_state(State(app)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode JSON");
        assert_eq!(
            value.get("declaration_count").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(value.get("ledger_count").and_then(Value::as_u64), Some(0));
        assert!(value.get("local_usage").map_or(false, Value::is_null));
    }

    #[tokio::test]
    async fn capacity_state_reports_local_metering_snapshot() {
        use sorafs_manifest::capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, ChunkerCommitmentV1,
        };

        let app = mk_app_state_for_tests();
        let mut inner =
            Arc::try_unwrap(app).unwrap_or_else(|_| panic!("no other references to app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x21; 32],
            stake: StakePointer {
                pool_id: [0xDD; 32],
                stake_amount: 5_000,
            },
            committed_capacity_gib: 128,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 128,
                capability_refs: Vec::new(),
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: 10,
            valid_until: 20,
            metadata: Vec::new(),
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            5,
            declaration.valid_from,
            declaration.valid_until,
            Metadata::default(),
        );
        node.record_capacity_declaration(&record)
            .expect("record declaration");
        node.record_uptime_observation(600, 600);
        node.record_por_observation(true);

        inner.sorafs_node = node;
        let app = Arc::new(inner);

        let response = handle_get_sorafs_capacity_state(State(app)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode JSON");
        let local_usage = value
            .get("local_usage")
            .and_then(Value::as_object)
            .expect("local usage present");
        let metering = local_usage
            .get("metering")
            .and_then(Value::as_object)
            .expect("metering snapshot");
        assert_eq!(
            metering.get("declared_gib").and_then(Value::as_u64),
            Some(128)
        );
        assert_eq!(
            metering.get("uptime_bps").and_then(Value::as_u64),
            Some(10_000)
        );
        assert_eq!(
            metering
                .get("accumulated_gib_seconds")
                .and_then(Value::as_str),
            Some("0")
        );
        assert!(
            local_usage
                .get("telemetry_preview")
                .and_then(Value::as_object)
                .is_some()
        );
        let fee_projection = local_usage
            .get("fee_projection")
            .and_then(Value::as_object)
            .expect("fee projection present");
        assert!(
            fee_projection
                .get("fee_nanos")
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn storage_pin_updates_storage_metrics() {
        use sorafs_manifest::capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, ChunkerCommitmentV1,
        };

        let app = mk_app_state_for_tests();
        let mut inner =
            Arc::try_unwrap(app).unwrap_or_else(|_| panic!("exclusive app state required"));
        let (node, _dir) = sorafs_node_with_temp_storage();

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x33; 32],
            stake: StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 10_000,
            },
            committed_capacity_gib: 64,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 64,
                capability_refs: Vec::new(),
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: 1,
            valid_until: 100,
            metadata: Vec::new(),
        };
        let declaration_bytes = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            declaration_bytes,
            declaration.committed_capacity_gib,
            0,
            declaration.valid_from,
            declaration.valid_until,
            Metadata::default(),
        );
        node.record_capacity_declaration(&record)
            .expect("record capacity declaration");

        let payload = vec![0xAB; 512];
        let manifest = manifest_for_payload(0x11, &payload);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload.as_slice());

        inner.sorafs_node = node;
        let app = Arc::new(inner);

        let request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };
        let response = handle_post_sorafs_storage_pin(
            State(app.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let provider_hex = hex::encode(declaration.provider_id);
        let metrics = app.telemetry.metrics().await;
        let gauge = metrics
            .torii_sorafs_storage_bytes_used
            .with_label_values(&[provider_hex.as_str()])
            .get();
        assert_eq!(
            gauge,
            payload.len() as u64,
            "storage bytes used gauge should track pinned payload size"
        );
    }

    #[tokio::test]
    async fn storage_pin_requires_token_and_respects_allowlist_and_rate_limit() {
        use std::num::NonZeroU32;

        use iroha_config::parameters::actual::{
            SorafsGatewayRateLimit as GatewayRateLimitCfg, SorafsStoragePin as PinCfg,
        };

        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app)
            .unwrap_or_else(|_| panic!("unique app state for pin policy mutation"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;

        let mut pin_cfg = PinCfg {
            require_token: true,
            ..Default::default()
        };
        pin_cfg.tokens.insert("secret-token".to_string());
        pin_cfg.allow_cidrs = vec!["10.0.0.0/8".to_string(), "127.0.0.0/8".to_string()];
        pin_cfg.rate_limit = GatewayRateLimitCfg {
            max_requests: Some(NonZeroU32::new(1).expect("non-zero max_requests")),
            window: Duration::from_secs(60),
            ban: None,
        };
        inner.sorafs_pin_policy =
            PinSubmissionPolicy::from_config(&pin_cfg).expect("valid pin policy");
        // Build a reusable pin request.
        let payload = vec![0xCD; 128];
        let manifest = manifest_for_payload(0x33, &payload);
        let request = StoragePinRequestDto {
            manifest_b64: base64::engine::general_purpose::STANDARD
                .encode(norito::to_bytes(&manifest).expect("encode manifest")),
            payload_b64: base64::engine::general_purpose::STANDARD.encode(payload),
            ..Default::default()
        };
        let app = Arc::new(inner);
        let loopback = ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080)));
        let denied_ip = ConnectInfo(SocketAddr::from(([192, 0, 2, 1], 8081)));

        // Missing token → 401
        let response = handle_post_sorafs_storage_pin(
            State(app.clone()),
            HeaderMap::new(),
            loopback.clone(),
            JsonOnly(request.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Wrong token → 403
        let mut wrong_headers = HeaderMap::new();
        wrong_headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer wrong-token"),
        );
        let response = handle_post_sorafs_storage_pin(
            State(app.clone()),
            wrong_headers,
            loopback.clone(),
            JsonOnly(request.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // Valid token but disallowed IP → 403
        let mut allowed_headers = HeaderMap::new();
        allowed_headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret-token"),
        );
        let response = handle_post_sorafs_storage_pin(
            State(app.clone()),
            allowed_headers.clone(),
            denied_ip,
            JsonOnly(request.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // First allowed request succeeds.
        let response = handle_post_sorafs_storage_pin(
            State(app.clone()),
            allowed_headers.clone(),
            loopback.clone(),
            JsonOnly(request.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Second request from same client hits rate limit → 429 with Retry-After.
        let response = handle_post_sorafs_storage_pin(
            State(app.clone()),
            allowed_headers,
            loopback,
            JsonOnly(request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok());
        assert!(
            retry_after.is_some(),
            "retry-after header should be present when rate limited"
        );
    }

    #[tokio::test]
    async fn storage_pin_and_fetch_roundtrip() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        inner.sorafs_gateway_config.require_manifest_envelope = false;
        inner.sorafs_gateway_config.enforce_admission = false;
        let components = build_sorafs_gateway_security(
            &inner.sorafs_gateway_config,
            inner.sorafs_admission.clone(),
        );
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        let state = Arc::new(inner);

        let payload = b"sorafs storage roundtrip payload";
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);

        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode response");
        let manifest_id_hex = value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        let mut fetch_headers = HeaderMap::new();
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("fetch-nonce"),
        );
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("fetch-client"),
        );
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            HeaderValue::from_static("alice@wonderland"),
        );
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alice@wonderland"),
        );

        let fetch_request = StorageFetchRequestDto {
            manifest_id_hex: manifest_id_hex.clone(),
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: Some(hex::encode([0x45; 32])),
        };
        let fetch_response = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            fetch_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8088))),
            JsonOnly(fetch_request),
        )
        .await;
        assert_eq!(fetch_response.status(), StatusCode::OK);
        let fetch_bytes = body::to_bytes(fetch_response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let fetch_value: Value =
            norito::json::from_slice(&fetch_bytes).expect("decode fetch response");
        let data_b64 = fetch_value
            .get("data_b64")
            .and_then(Value::as_str)
            .expect("data");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data_b64.as_bytes())
            .expect("decode payload");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn storage_manifest_endpoint_returns_manifest_payload() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();

        let payload = b"sorafs manifest export payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xAB; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let manifest_bytes = norito::to_bytes(&manifest).expect("encode manifest");
        let mut reader = payload.as_slice();
        let manifest_id = node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        inner.sorafs_node = node;
        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&inner.sorafs_node, [0xAA; 32], &chunker_handle);
        let state = Arc::new(inner);

        let response =
            handle_get_sorafs_storage_manifest(State(state.clone()), Path(manifest_id.clone()))
                .await;
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode manifest response");

        assert_eq!(
            value
                .get("manifest_id_hex")
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some(manifest_id.clone())
        );
        assert_eq!(
            value.get("content_length").and_then(Value::as_u64),
            Some(plan.content_length)
        );
        let manifest_b64 = value
            .get("manifest_b64")
            .and_then(Value::as_str)
            .expect("manifest_b64 present");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(manifest_b64.as_bytes())
            .expect("decode manifest");
        assert_eq!(decoded, manifest_bytes);
    }

    #[tokio::test]
    async fn storage_plan_endpoint_returns_chunk_plan() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();

        let payload = b"sorafs chunk plan export payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xCD; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&node, [0xAC; 32], &chunker_handle);

        let mut reader = payload.as_slice();
        let manifest_id = node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        inner.sorafs_node = node;
        let state = Arc::new(inner);

        let response =
            handle_get_sorafs_storage_plan(State(state.clone()), Path(manifest_id.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode plan response");
        assert_eq!(
            value
                .get("manifest_id_hex")
                .and_then(Value::as_str)
                .map(str::to_owned),
            Some(manifest_id.clone())
        );

        let plan_value = value
            .get("plan")
            .and_then(Value::as_object)
            .expect("plan object");
        assert_eq!(
            plan_value.get("chunk_count").and_then(Value::as_u64),
            Some(plan.chunks.len() as u64)
        );
        assert_eq!(
            plan_value.get("content_length").and_then(Value::as_u64),
            Some(plan.content_length)
        );
        let chunks = plan_value
            .get("chunks")
            .and_then(Value::as_array)
            .expect("chunks array");
        assert_eq!(chunks.len(), plan.chunks.len());
    }

    #[tokio::test]
    async fn storage_por_sample_endpoint_returns_proofs() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let state = Arc::new(inner);

        let payload = b"sorafs storage por sampling payload";
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xBB; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&state.sorafs_node, [0xAD; 32], &chunker_handle);

        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };
        let response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode response");
        let manifest_id_hex = value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        let sample_request = StoragePorSampleRequestDto {
            manifest_id_hex,
            count: 2,
            seed: Some(42),
        };
        let sample_response =
            handle_post_sorafs_storage_por_sample(State(state.clone()), JsonOnly(sample_request))
                .await;
        assert_eq!(sample_response.status(), StatusCode::OK);
        let sample_bytes = body::to_bytes(sample_response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let sample_value: Value =
            norito::json::from_slice(&sample_bytes).expect("decode sample response");
        let samples = sample_value
            .get("samples")
            .and_then(Value::as_array)
            .expect("samples array");
        assert_eq!(samples.len(), 1);
    }

    #[tokio::test]
    async fn storage_por_pipeline_records_success() {
        let (challenge, proof, verdict) = sample_por_artifacts();

        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        let node_view = node.clone();
        inner.sorafs_node = node;
        let state = Arc::new(inner);

        seed_capacity_declaration(
            &state.sorafs_node,
            challenge.provider_id,
            &challenge.chunking_profile,
        );

        let challenge_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&challenge).expect("encode challenge"));
        let challenge_req = StoragePorChallengeDto { challenge_b64 };
        let challenge_resp =
            handle_post_sorafs_storage_por_challenge(State(state.clone()), JsonOnly(challenge_req))
                .await;
        assert_eq!(challenge_resp.status(), StatusCode::OK);
        let challenge_body = body::to_bytes(challenge_resp.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let challenge_json: Value =
            norito::json::from_slice(&challenge_body).expect("challenge response");
        assert_eq!(
            challenge_json.get("status").and_then(Value::as_str),
            Some("accepted")
        );

        let proof_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&proof).expect("encode proof"));
        let proof_req = StoragePorProofDto { proof_b64 };
        let proof_resp =
            handle_post_sorafs_storage_por_proof(State(state.clone()), JsonOnly(proof_req)).await;
        assert_eq!(proof_resp.status(), StatusCode::OK);
        let proof_body = body::to_bytes(proof_resp.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let proof_json: Value = norito::json::from_slice(&proof_body).expect("proof response");
        assert_eq!(
            proof_json.get("status").and_then(Value::as_str),
            Some("accepted")
        );

        let verdict_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&verdict).expect("encode verdict"));
        let verdict_req = StoragePorVerdictDto { verdict_b64 };
        let verdict_resp =
            handle_post_sorafs_storage_por_verdict(State(state.clone()), JsonOnly(verdict_req))
                .await;
        assert_eq!(verdict_resp.status(), StatusCode::OK);
        let verdict_body = body::to_bytes(verdict_resp.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let verdict_json: Value =
            norito::json::from_slice(&verdict_body).expect("verdict response");
        assert_eq!(
            verdict_json.get("status").and_then(Value::as_str),
            Some("accepted")
        );

        let snapshot = node_view.metering_snapshot();
        assert_eq!(snapshot.por_samples_success, 1);
        assert_eq!(snapshot.por_samples_total, 1);
    }

    #[tokio::test]
    async fn storage_por_challenge_rejects_invalid_base64() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let state = Arc::new(inner);

        let request = StoragePorChallengeDto {
            challenge_b64: "!!not_base64!!".to_owned(),
        };
        let response =
            handle_post_sorafs_storage_por_challenge(State(state), JsonOnly(request)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error response");
        let error_msg = value
            .get("error")
            .and_then(Value::as_str)
            .expect("error string");
        assert!(
            error_msg.contains("invalid base64"),
            "unexpected error message: {error_msg}"
        );
    }

    #[tokio::test]
    async fn storage_manifest_refuses_unsupported_chunker() {
        use sorafs_manifest::{
            capacity::{
                CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, ChunkerCommitmentV1,
            },
            provider_advert::StakePointer,
        };

        let mut context = token_test_context();
        {
            let mut app_inner = Arc::try_unwrap(context.app)
                .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
            app_inner.sorafs_gateway_config.enforce_capabilities = true;
            let components = build_sorafs_gateway_security(
                &app_inner.sorafs_gateway_config,
                app_inner.sorafs_admission.clone(),
            );
            app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
            app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
            app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
            context.app = Arc::new(app_inner);
        }
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x41; 32],
            stake: StakePointer {
                pool_id: [0x52; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 512,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf2@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 512,
                capability_refs: Vec::new(),
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: Vec::new(),
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            declaration.valid_from,
            declaration.valid_until,
            Metadata::default(),
        );
        context
            .app
            .sorafs_node
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let response = handle_get_sorafs_storage_manifest(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode refusal");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("unsupported_chunker".into()))
        );
    }

    #[tokio::test]
    async fn storage_por_sample_refuses_unsupported_chunker() {
        use sorafs_manifest::{
            capacity::{
                CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, ChunkerCommitmentV1,
            },
            provider_advert::StakePointer,
        };

        let mut context = token_test_context();
        {
            let mut app_inner = Arc::try_unwrap(context.app)
                .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
            app_inner.sorafs_gateway_config.enforce_capabilities = true;
            let components = build_sorafs_gateway_security(
                &app_inner.sorafs_gateway_config,
                app_inner.sorafs_admission.clone(),
            );
            app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
            app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
            app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
            context.app = Arc::new(app_inner);
        }
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x31; 32],
            stake: StakePointer {
                pool_id: [0x26; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 128,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf2@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 128,
                capability_refs: Vec::new(),
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: 1,
            valid_until: 5,
            metadata: Vec::new(),
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            declaration.valid_from,
            declaration.valid_until,
            Metadata::default(),
        );
        context
            .app
            .sorafs_node
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let request = StoragePorSampleRequestDto {
            manifest_id_hex: context.manifest_id_hex.clone(),
            count: 4,
            seed: None,
        };
        let response =
            handle_post_sorafs_storage_por_sample(State(context.app.clone()), JsonOnly(request))
                .await;
        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode refusal");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("unsupported_chunker".into()))
        );
    }

    #[tokio::test]
    async fn storage_restart_recovers_manifest_metadata() {
        use sorafs_node::config::StorageConfig;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let storage_root = temp_dir.path().join("storage");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(storage_root.clone())
            .build();
        let cfg_restart = cfg.clone();

        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        inner.sorafs_node = sorafs_node::NodeHandle::new(cfg);
        inner.sorafs_gateway_config.require_manifest_envelope = false;
        inner.sorafs_gateway_config.enforce_admission = false;
        let components = build_sorafs_gateway_security(
            &inner.sorafs_gateway_config,
            inner.sorafs_admission.clone(),
        );
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        let state = Arc::new(inner);

        let payload = b"sorafs restart persistence check payload".to_vec();
        let manifest = manifest_for_payload(0xAB, &payload);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);
        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let pin_response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(pin_response.status(), StatusCode::OK);
        let pin_bytes = body::to_bytes(pin_response.into_body(), usize::MAX)
            .await
            .expect("collect pin body");
        let pin_value: Value = norito::json::from_slice(&pin_bytes).expect("decode pin response");
        let manifest_id_hex = pin_value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        drop(state);

        let app_after = mk_app_state_for_tests();
        let mut inner_after =
            Arc::try_unwrap(app_after).unwrap_or_else(|_| panic!("unique app state"));
        inner_after.sorafs_node = sorafs_node::NodeHandle::new(cfg_restart);
        inner_after.sorafs_gateway_config.require_manifest_envelope = false;
        inner_after.sorafs_gateway_config.enforce_admission = false;
        let components_after = build_sorafs_gateway_security(
            &inner_after.sorafs_gateway_config,
            inner_after.sorafs_admission.clone(),
        );
        inner_after.sorafs_gateway_policy = Some(Arc::clone(&components_after.policy));
        inner_after.sorafs_gateway_denylist = Some(Arc::clone(&components_after.denylist));
        inner_after.sorafs_gateway_tls_state = Some(Arc::clone(&components_after.tls_state));
        let state_after = Arc::new(inner_after);

        let mut fetch_headers = HeaderMap::new();
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("restart-fetch"),
        );
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("restart-client"),
        );
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            HeaderValue::from_static("alice@wonderland"),
        );
        fetch_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alice@wonderland"),
        );

        let fetch_request = StorageFetchRequestDto {
            manifest_id_hex: manifest_id_hex.clone(),
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: Some(hex::encode([0x59; 32])),
        };
        let fetch_response = handle_post_sorafs_storage_fetch(
            State(state_after.clone()),
            fetch_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8089))),
            JsonOnly(fetch_request),
        )
        .await;
        assert_eq!(fetch_response.status(), StatusCode::OK);
        let fetch_bytes = body::to_bytes(fetch_response.into_body(), usize::MAX)
            .await
            .expect("collect fetch body");
        let fetch_value: Value =
            norito::json::from_slice(&fetch_bytes).expect("decode fetch response");
        let data_b64 = fetch_value
            .get("data_b64")
            .and_then(Value::as_str)
            .expect("payload data");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data_b64.as_bytes())
            .expect("decode persisted payload");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn storage_token_issues_signed_response() {
        let context = token_test_context();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-token-1"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            header_value(&context.client_id, "X-SoraFS-Client"),
        );

        let request = StreamTokenRequestDto {
            manifest_id_hex: context.manifest_id_hex.clone(),
            provider_id_hex: context.provider_id_hex.clone(),
            ttl_secs: None,
            max_streams: None,
            rate_limit_bytes: None,
            requests_per_minute: None,
        };

        let response = handle_post_sorafs_storage_token(
            State(context.app.clone()),
            headers,
            JsonOnly(request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers().clone();
        assert_eq!(
            headers
                .get(HEADER_SORA_NONCE)
                .and_then(|value| value.to_str().ok()),
            Some("nonce-token-1")
        );
        assert_eq!(
            headers
                .get(HEADER_SORA_CLIENT)
                .and_then(|value| value.to_str().ok()),
            Some(context.client_id.as_str())
        );
        assert_eq!(
            headers
                .get(HEADER_SORA_VERIFYING_KEY)
                .and_then(|value| value.to_str().ok()),
            Some(context.verifying_key_hex.as_str())
        );
        let token_id = headers
            .get(HEADER_SORA_TOKEN_ID)
            .and_then(|value| value.to_str().ok())
            .expect("token id header");
        assert!(!token_id.is_empty());
        assert_eq!(
            headers
                .get(HEADER_SORA_CLIENT_QUOTA_REMAINING)
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(
            headers
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect token body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode token response");
        let token_obj = value
            .get("token")
            .and_then(Value::as_object)
            .expect("token object");
        let signature_hex = token_obj
            .get("signature_hex")
            .and_then(Value::as_str)
            .expect("signature hex");
        assert_eq!(signature_hex.len(), 64 * 2);
        assert!(signature_hex.chars().all(|c| c.is_ascii_hexdigit()));
        let body_obj = token_obj
            .get("body")
            .and_then(Value::as_object)
            .expect("token body");
        assert!(
            body_obj.get("token_id").and_then(Value::as_str).is_some(),
            "token body must include token_id"
        );
        let token_base64 = value
            .get("token_base64")
            .and_then(Value::as_str)
            .expect("token base64 present");
        assert!(!token_base64.is_empty());
        let decoded = decode_token_base64(token_base64).expect("decode token base64");
        assert_eq!(
            decoded.body.token_id,
            body_obj
                .get("token_id")
                .and_then(Value::as_str)
                .expect("token id"),
        );
    }

    #[tokio::test]
    async fn storage_token_requires_nonce_header() {
        let context = token_test_context();
        let request = StreamTokenRequestDto {
            manifest_id_hex: context.manifest_id_hex.clone(),
            provider_id_hex: context.provider_id_hex.clone(),
            ttl_secs: None,
            max_streams: None,
            rate_limit_bytes: None,
            requests_per_minute: None,
        };

        let response = handle_post_sorafs_storage_token(
            State(context.app.clone()),
            {
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::HeaderName::from_static(HEADER_SORA_CLIENT),
                    header_value(&context.client_id, "X-SoraFS-Client"),
                );
                headers
            },
            JsonOnly(request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect error body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error payload");
        let error_message = value
            .get("error")
            .and_then(Value::as_str)
            .expect("error string");
        assert!(
            error_message.contains("missing X-SoraFS-Nonce"),
            "unexpected error message: {error_message}"
        );
    }

    #[tokio::test]
    async fn storage_token_enforces_client_quota() {
        let context = token_test_context();

        let request_builder = || StreamTokenRequestDto {
            manifest_id_hex: context.manifest_id_hex.clone(),
            provider_id_hex: context.provider_id_hex.clone(),
            ttl_secs: None,
            max_streams: None,
            rate_limit_bytes: None,
            requests_per_minute: None,
        };

        let expected = ["2", "1", "0"];
        for (idx, quota_remaining) in expected.into_iter().enumerate() {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_NONCE),
                header_value(format!("nonce-quota-{idx}"), "X-SoraFS-Nonce"),
            );
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_CLIENT),
                HeaderValue::from_static("gateway-alpha"),
            );

            let response = handle_post_sorafs_storage_token(
                State(context.app.clone()),
                headers,
                JsonOnly(request_builder()),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let remaining = response
                .headers()
                .get(HEADER_SORA_CLIENT_QUOTA_REMAINING)
                .and_then(|value| value.to_str().ok())
                .expect("quota header");
            assert_eq!(remaining, quota_remaining);
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            header_value("nonce-quota-3", "X-SoraFS-Nonce"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("gateway-alpha"),
        );

        let response = handle_post_sorafs_storage_token(
            State(context.app.clone()),
            headers,
            JsonOnly(request_builder()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .expect("retry-after header");
        assert!(retry_after.parse::<u64>().unwrap_or(0) > 0);
        let quota_header = response
            .headers()
            .get(HEADER_SORA_CLIENT_QUOTA_REMAINING)
            .and_then(|value| value.to_str().ok())
            .expect("quota header on 429");
        assert_eq!(quota_header, "0");
    }

    #[tokio::test]
    async fn storage_token_returns_not_found_when_disabled() {
        let mut app = Arc::try_unwrap(mk_app_state_for_tests())
            .unwrap_or_else(|_| panic!("exclusive app state required"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        let payload = b"disabled issuer payload";
        let manifest = manifest_for_payload(0x17, payload);
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut reader = &payload[..];
        let manifest_id_hex = node
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        app.sorafs_node = node;
        // Leave stream_token_issuer unset to emulate disabled tokens.
        let state = Arc::new(app);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-disabled"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("gateway-disabled"),
        );
        let request = StreamTokenRequestDto {
            manifest_id_hex,
            provider_id_hex: hex::encode([0xEF; 32]),
            ttl_secs: None,
            max_streams: None,
            rate_limit_bytes: None,
            requests_per_minute: None,
        };

        let response =
            handle_post_sorafs_storage_token(State(state), headers, JsonOnly(request)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect error body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error response");
        let error_message = value
            .get("error")
            .and_then(Value::as_str)
            .expect("error string");
        assert!(
            error_message.contains("stream token issuance is not enabled"),
            "unexpected error: {error_message}"
        );
    }

    #[tokio::test]
    async fn storage_pin_persists_stripe_layout_and_roles() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let state = Arc::new(inner);

        let payload = vec![0x41; 32];
        let profile = ChunkProfile {
            min_size: 8,
            target_size: 8,
            max_size: 8,
            break_mask: 1,
        };
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(profile, sorafs_manifest::BLAKE3_256_MULTIHASH_CODE)
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(&payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("plan");
        let stripe_layout = DaStripeLayout {
            total_stripes: 2,
            shards_per_stripe: plan.chunks.len() as u32,
            row_parity_stripes: 1,
        };
        let chunk_roles: Vec<ChunkRoleDto> = plan
            .chunks
            .iter()
            .enumerate()
            .map(|(idx, _)| ChunkRoleDto {
                role: Some(if idx % 2 == 0 {
                    ChunkRole::Data
                } else {
                    ChunkRole::StripeParity
                }),
                group_id: Some(idx as u32),
            })
            .collect();

        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);

        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            stripe_layout: Some(stripe_layout),
            chunk_roles: Some(chunk_roles.clone()),
        };

        let response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode response");
        let manifest_id_hex = value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        let stored = state
            .sorafs_node
            .manifest_metadata(&manifest_id_hex)
            .expect("stored manifest");
        assert_eq!(stored.stripe_layout(), Some(&stripe_layout));
        for (idx, role) in chunk_roles.iter().enumerate() {
            let chunk = stored.chunk(idx).expect("chunk metadata");
            assert_eq!(chunk.role, role.role);
            assert_eq!(chunk.group_id, Some(role.group_id.unwrap_or(idx as u32)));
        }
    }

    #[tokio::test]
    async fn storage_pin_rejects_mismatched_chunk_roles() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let state = Arc::new(inner);

        let payload = vec![0x24; 8];
        let manifest = manifest_for_payload(0x33, &payload);
        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);

        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            stripe_layout: Some(DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            }),
            chunk_roles: Some(Vec::new()),
        };

        let response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error");
        let message = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            message.contains("chunk_roles length"),
            "expected chunk_roles length error, got: {message}"
        );
    }

    #[tokio::test]
    async fn storage_pin_rejects_when_capacity_exceeded() {
        use iroha_config::base::util::Bytes;
        use sorafs_node::config::StorageConfig;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let storage_root = temp_dir.path().join("storage");
        let payload = b"capacity-checked manifest payload".to_vec();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(storage_root)
            .max_capacity_bytes(Bytes(payload.len() as u64))
            .build();

        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        inner.sorafs_node = sorafs_node::NodeHandle::new(cfg);
        let state = Arc::new(inner);

        let manifest = manifest_for_payload(0x11, &payload);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);
        let first_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };
        let first_response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(first_request),
        )
        .await;
        assert_eq!(first_response.status(), StatusCode::OK);

        let second_payload = vec![0x42; payload.len()];
        let second_manifest = manifest_for_payload(0x22, &second_payload);
        let second_request = StoragePinRequestDto {
            manifest_b64: base64::engine::general_purpose::STANDARD
                .encode(norito::to_bytes(&second_manifest).unwrap()),
            payload_b64: base64::engine::general_purpose::STANDARD.encode(&second_payload),
            ..Default::default()
        };
        let second_response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(second_request),
        )
        .await;
        assert_eq!(second_response.status(), StatusCode::BAD_REQUEST);
        let error_bytes = body::to_bytes(second_response.into_body(), usize::MAX)
            .await
            .expect("collect error body");
        let error_value: Value =
            norito::json::from_slice(&error_bytes).expect("decode error payload");
        let error_message = error_value
            .get("error")
            .and_then(Value::as_str)
            .expect("error field present");
        assert!(
            error_message.contains("capacity exceeded"),
            "unexpected error message: {error_message}"
        );
    }

    #[tokio::test]
    async fn storage_pin_returns_not_found_when_disabled() {
        let app = mk_app_state_for_tests();
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xCC; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(4)
            .car_digest(blake3::hash(b"noop").into())
            .car_size(4)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");
        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(b"noop");

        let request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let response = handle_post_sorafs_storage_pin(
            State(app),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn storage_pin_requires_token_when_policy_enabled() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let mut pin_cfg = iroha_config::parameters::actual::SorafsStoragePin {
            require_token: true,
            ..Default::default()
        };
        pin_cfg.tokens.insert("sorafs-secret".to_owned());
        pin_cfg.allow_cidrs.push("127.0.0.0/8".to_owned());
        inner.sorafs_pin_policy =
            sorafs::PinSubmissionPolicy::from_config(&pin_cfg).expect("pin policy");
        let state = Arc::new(inner);

        let payload = b"pin auth payload".to_vec();
        let manifest = manifest_for_payload(0x44, &payload);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);

        let request = StoragePinRequestDto {
            manifest_b64: manifest_b64.clone(),
            payload_b64: payload_b64.clone(),
            ..Default::default()
        };

        let missing_token = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8081))),
            JsonOnly(request),
        )
        .await;
        assert_eq!(missing_token.status(), StatusCode::UNAUTHORIZED);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer sorafs-secret"),
        );
        let valid_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };
        let ok_response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8081))),
            JsonOnly(valid_request),
        )
        .await;
        assert_eq!(ok_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn storage_pin_rate_limits_repeated_requests() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        let mut pin_cfg = iroha_config::parameters::actual::SorafsStoragePin::default();
        pin_cfg.rate_limit.max_requests = Some(NonZeroU32::new(1).expect("nonzero"));
        pin_cfg.rate_limit.window = Duration::from_millis(200);
        pin_cfg.allow_cidrs.push("127.0.0.0/8".to_owned());
        inner.sorafs_pin_policy =
            sorafs::PinSubmissionPolicy::from_config(&pin_cfg).expect("pin policy");
        let state = Arc::new(inner);

        let payload = b"pin rate limit payload".to_vec();
        let manifest = manifest_for_payload(0x55, &payload);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);
        let request = StoragePinRequestDto {
            manifest_b64: manifest_b64.clone(),
            payload_b64: payload_b64.clone(),
            ..Default::default()
        };

        let first = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8082))),
            JsonOnly(request),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);

        let second_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };
        let second = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8082))),
            JsonOnly(second_request),
        )
        .await;
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = second
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or_default();
        assert!(
            retry_after > 0,
            "retry-after header should be set on rate limit responses"
        );
    }

    #[tokio::test]
    async fn storage_pin_enforces_quota_limits() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();

        let payload = b"quota pin payload".to_vec();
        let manifest = manifest_for_payload(0x77, &payload);
        let chunker_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        seed_capacity_declaration(&node, [0xCE; 32], &chunker_handle);
        inner.sorafs_node = node;

        let mut quotas = sorafs::SorafsQuotaConfig::unlimited();
        quotas.storage_pin = sorafs::SorafsQuotaWindow {
            max_events: Some(1),
            window: Duration::from_secs(300),
        };
        inner.sorafs_limits = Arc::new(sorafs::SorafsQuotaEnforcer::from_config(&quotas));

        let state = Arc::new(inner);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);

        let request = StoragePinRequestDto {
            manifest_b64: manifest_b64.clone(),
            payload_b64: payload_b64.clone(),
            ..Default::default()
        };

        let first = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8083))),
            JsonOnly(request),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);

        let second_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };
        let second = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8083))),
            JsonOnly(second_request),
        )
        .await;
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = body::to_bytes(second.into_body(), usize::MAX)
            .await
            .expect("collect response");
        let value: Value = norito::json::from_slice(&body).expect("decode error body");
        let message = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            message.contains("quota"),
            "quota error message should mention quota: {message}"
        );
    }

    #[tokio::test]
    async fn storage_token_requires_client_header() {
        let (app, _dir, manifest_id) = token_enabled_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-test"),
        );
        let request = StreamTokenRequestDto {
            manifest_id_hex: manifest_id,
            provider_id_hex: hex::encode([0x55; 32]),
            ttl_secs: None,
            max_streams: None,
            rate_limit_bytes: None,
            requests_per_minute: None,
        };

        let response =
            handle_post_sorafs_storage_token(State(app), headers, JsonOnly(request)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn storage_token_emits_expected_headers() {
        let (app, _dir, manifest_id) = token_enabled_state();
        let verifying_hex = {
            let issuer = app
                .stream_token_issuer
                .as_ref()
                .expect("issuer configured for tests");
            hex::encode(issuer.verifying_key_bytes())
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-123"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("gateway-alpha"),
        );

        let request = StreamTokenRequestDto {
            manifest_id_hex: manifest_id,
            provider_id_hex: hex::encode([0x66; 32]),
            ttl_secs: Some(900),
            max_streams: Some(7),
            rate_limit_bytes: Some(1_048_576),
            requests_per_minute: Some(60),
        };

        let response =
            handle_post_sorafs_storage_token(State(app.clone()), headers, JsonOnly(request)).await;

        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers().clone();
        assert_eq!(
            headers
                .get(HEADER_SORA_NONCE)
                .and_then(|value| value.to_str().ok()),
            Some("nonce-123")
        );
        assert_eq!(
            headers
                .get(HEADER_SORA_CLIENT)
                .and_then(|value| value.to_str().ok()),
            Some("gateway-alpha")
        );
        assert!(
            headers
                .get(HEADER_SORA_TOKEN_ID)
                .and_then(|value| value.to_str().ok())
                .is_some(),
            "token id header must be present"
        );
        assert_eq!(
            headers
                .get(HEADER_SORA_VERIFYING_KEY)
                .and_then(|value| value.to_str().ok()),
            Some(verifying_hex.as_str())
        );
        assert_eq!(
            headers
                .get(HEADER_SORA_CLIENT_QUOTA_REMAINING)
                .and_then(|value| value.to_str().ok()),
            Some("59")
        );
        assert_eq!(
            headers
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("token JSON");
        assert!(
            value.get("token").is_some(),
            "response should contain token payload"
        );
    }

    #[tokio::test]
    async fn car_range_requires_stream_token() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn car_range_missing_dag_scope_returns_structured_refusal() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8082))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);

        let request_id_header = response
            .headers()
            .get(HeaderName::from_static(HEADER_SORA_REQUEST_ID))
            .expect("request id header present");
        let request_id = request_id_header
            .to_str()
            .expect("request id header must be ASCII");
        assert!(
            !request_id.is_empty(),
            "request id header should not be empty"
        );

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect refusal body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("missing_header".into()))
        );
        let reason = value
            .get("reason")
            .and_then(Value::as_str)
            .expect("reason field present");
        assert!(
            reason.contains("dag-scope"),
            "reason should mention dag-scope"
        );
        let header_value = value
            .get("details")
            .and_then(Value::as_object)
            .and_then(|map| map.get("header"))
            .and_then(Value::as_str)
            .expect("details.header present");
        assert_eq!(header_value, "Sora-Dag-Scope");
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn car_range_missing_dag_scope_increments_refusal_metric() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata");
        let end = manifest.content_length().saturating_sub(1);
        let label_values = [
            "missing_header",
            manifest.chunk_profile_handle(),
            "",
            TELEMETRY_ENDPOINT_CAR_RANGE,
        ];

        let metrics_before = context.app.telemetry.metrics().await;
        let counter_before = metrics_before
            .torii_sorafs_gateway_refusals_total
            .with_label_values(&label_values)
            .get();

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8090))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);

        let metrics_after = context.app.telemetry.metrics().await;
        let counter_after = metrics_after
            .torii_sorafs_gateway_refusals_total
            .with_label_values(&label_values)
            .get();
        assert_eq!(
            counter_after,
            counter_before + 1,
            "gateway refusal counter should increment for missing dag-scope header"
        );
    }

    #[tokio::test]
    async fn car_range_accepts_valid_stream_token() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;
        let expected_length = manifest.content_length();

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8081))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let (parts, body) = response.into_parts();
        let headers = parts.headers;
        let body_bytes = body::to_bytes(body, usize::MAX)
            .await
            .expect("collect CAR body");
        let car_bytes = body_bytes.to_vec();

        assert_eq!(
            headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some(MIME_CAR)
        );
        let expected_length_header = car_bytes.len().to_string();
        assert_eq!(
            headers
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok()),
            Some(expected_length_header.as_str())
        );
        let tls_state = headers
            .get(HeaderName::from_static(SORA_TLS_STATE_HEADER))
            .and_then(|value| value.to_str().ok())
            .expect("TLS state header present");
        assert!(
            tls_state.starts_with("ech-"),
            "unexpected TLS state header: {tls_state}"
        );
        let chunk_range_header = headers
            .get(header::HeaderName::from_static(HEADER_SORA_CHUNK_RANGE))
            .and_then(|value| value.to_str().ok())
            .expect("chunk range header present");

        let manifest_v1 = manifest.load_manifest().expect("load manifest");
        let chunk_profile =
            chunk_profile_for_manifest(&manifest_v1).expect("resolve chunk profile");
        let taikai_hint = sorafs_car::taikai_segment_hint_from_sorafs_manifest(&manifest_v1)
            .expect("taikai hint");
        let full_plan = manifest.to_car_plan_with_hint(chunk_profile, taikai_hint);
        let expected_range = 0..=end;
        let report = CarVerifier::verify_block_car(
            &manifest_v1,
            &full_plan,
            &car_bytes,
            Some(expected_range.clone()),
        )
        .expect("CAR verification");

        assert_eq!(
            report.payload_bytes, expected_length,
            "verified payload length must match manifest length"
        );
        assert_eq!(
            report.payload_range, expected_range,
            "verified payload range should equal requested range"
        );
        assert_eq!(
            report.chunk_indices.len(),
            manifest.chunk_count(),
            "full range should contain every manifest chunk"
        );
        assert!(
            chunk_range_header.contains(&format!("chunks={}", report.chunk_indices.len())),
            "chunk range header must report the verified chunk count"
        );
    }

    #[tokio::test]
    async fn car_range_emits_gateway_headers() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8082))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);

        let (parts, _) = response.into_parts();
        let headers = parts.headers;

        let tls_state = headers
            .get(HeaderName::from_static(SORA_TLS_STATE_HEADER))
            .and_then(|value| value.to_str().ok())
            .expect("TLS state header present");
        assert!(
            tls_state.starts_with("ech-"),
            "unexpected TLS state header: {tls_state}"
        );

        assert_eq!(
            headers
                .get(HeaderName::from_static(HEADER_SORA_CONTENT_CID))
                .and_then(|value| value.to_str().ok()),
            Some(context.manifest_id_hex.as_str()),
            "Sora-Content-CID must match manifest id"
        );
    }

    #[tokio::test]
    async fn car_range_enforces_request_quota() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(
            &context,
            TokenOverrides {
                requests_per_minute: Some(1),
                ..TokenOverrides::default()
            },
        )
        .await;

        let mut first_headers = HeaderMap::new();
        first_headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        first_headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        first_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        first_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("quota-nonce"),
        );
        first_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        first_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        first_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let first_response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            first_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8083))),
        )
        .await;
        assert_eq!(first_response.status(), StatusCode::PARTIAL_CONTENT);

        let mut second_headers = HeaderMap::new();
        second_headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        second_headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        second_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        second_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("quota-nonce"),
        );
        second_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        second_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        second_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let second_response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            second_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8084))),
        )
        .await;
        assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = second_response
            .headers()
            .get(RETRY_AFTER)
            .expect("retry-after header present")
            .to_str()
            .expect("retry-after header must be ASCII")
            .parse::<u32>()
            .expect("retry-after header numeric");
        assert!(
            (1..=60).contains(&retry_after),
            "retry-after must be between 1 and 60 seconds (got {retry_after})"
        );
        let body_bytes = body::to_bytes(second_response.into_body(), usize::MAX)
            .await
            .expect("collect quota response body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode quota response");
        assert_eq!(
            value.get("error"),
            Some(&Value::String(
                "stream token request quota exceeded".to_string()
            ))
        );
    }

    #[tokio::test]
    async fn car_range_requires_manifest_envelope_when_policy_enabled() {
        let mut context = token_test_context();
        let mut app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        app_inner.sorafs_gateway_config.require_manifest_envelope = true;
        let components = build_sorafs_gateway_security(
            &app_inner.sorafs_gateway_config,
            app_inner.sorafs_admission.clone(),
        );
        app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8085))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body bytes");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error JSON");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("manifest_envelope_required".into()))
        );
    }

    #[tokio::test]
    async fn storage_fetch_requires_manifest_envelope_when_policy_enabled() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        inner.sorafs_gateway_config.enforce_admission = false;
        inner.sorafs_gateway_config.require_manifest_envelope = true;
        let components = build_sorafs_gateway_security(
            &inner.sorafs_gateway_config,
            inner.sorafs_admission.clone(),
        );
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        let state = Arc::new(inner);

        let payload = b"sorafs fetch manifest envelope enforcement payload";
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xCC; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode response");
        let manifest_id_hex = value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("fetch-nonce"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("fetch-client"),
        );

        let fetch_request = StorageFetchRequestDto {
            manifest_id_hex,
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: Some(hex::encode([0x77; 32])),
        };

        let response = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8093))),
            JsonOnly(fetch_request),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect fetch body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode fetch response");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("manifest_envelope_required".into()))
        );
    }

    #[tokio::test]
    async fn storage_fetch_requires_provider_id_when_gar_enforced() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        inner.sorafs_gateway_config.require_manifest_envelope = false;
        inner.sorafs_gateway_config.enforce_admission = true;
        inner.sorafs_gateway_config.enforce_capabilities = true;
        inner.sorafs_admission = Some(Arc::new(AdmissionRegistry::empty()));
        let components = build_sorafs_gateway_security(
            &inner.sorafs_gateway_config,
            inner.sorafs_admission.clone(),
        );
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        let state = Arc::new(inner);

        let payload = b"sorafs fetch provider enforcement payload";
        let manifest = manifest_for_payload(0x23, payload);
        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let pin_response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(pin_response.status(), StatusCode::OK);
        let pin_bytes = body::to_bytes(pin_response.into_body(), usize::MAX)
            .await
            .expect("collect pin body");
        let pin_value: Value = norito::json::from_slice(&pin_bytes).expect("decode pin response");
        let manifest_id_hex = pin_value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        assert!(
            state.sorafs_node.capacity_usage().provider_id.is_none(),
            "fixture should not advertise a provider id"
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("missing-provider"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("provider-check"),
        );

        let fetch_request = StorageFetchRequestDto {
            manifest_id_hex,
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: None,
        };

        let response = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8100))),
            JsonOnly(fetch_request),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect response body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode provider response");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("provider_id_missing".into()))
        );
    }

    #[tokio::test]
    async fn storage_chunk_requires_manifest_envelope_when_policy_enabled() {
        let mut context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let chunk_digest_hex =
            hex::encode(manifest.chunk(0).expect("chunk metadata present").digest);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("chunk-nonce"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );

        // Enable manifest envelope enforcement for this test and rebuild policy handles.
        let mut app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        app_inner.sorafs_gateway_config.require_manifest_envelope = true;
        let components = build_sorafs_gateway_security(
            &app_inner.sorafs_gateway_config,
            app_inner.sorafs_admission.clone(),
        );
        app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(app_inner);

        let response = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8096))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect chunk body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode chunk response");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("manifest_envelope_required".into()))
        );
    }

    #[tokio::test]
    async fn storage_fetch_allows_missing_manifest_envelope_when_policy_disabled() {
        let app = mk_app_state_for_tests();
        let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
        let (node, _dir) = sorafs_node_with_temp_storage();
        inner.sorafs_node = node;
        inner.sorafs_gateway_config.enforce_admission = false;
        inner.sorafs_gateway_config.require_manifest_envelope = false;
        let components = build_sorafs_gateway_security(
            &inner.sorafs_gateway_config,
            inner.sorafs_admission.clone(),
        );
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        let state = Arc::new(inner);

        let payload = b"sorafs fetch manifest envelope optional payload";
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xCC; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let manifest_b64 = base64::engine::general_purpose::STANDARD
            .encode(norito::to_bytes(&manifest).expect("encode manifest"));
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode response");
        let manifest_id_hex = value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("fetch-nonce-optional"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("fetch-client-optional"),
        );

        let fetch_request = StorageFetchRequestDto {
            manifest_id_hex,
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: Some(hex::encode([0x77; 32])),
        };
        let expected_manifest_id = fetch_request.manifest_id_hex.clone();

        let response = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8097))),
            JsonOnly(fetch_request),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect fetch body");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode fetch response");
        assert_eq!(
            value.get("manifest_id_hex"),
            Some(&Value::String(expected_manifest_id))
        );
    }

    #[tokio::test]
    async fn car_range_allows_missing_manifest_envelope_when_policy_disabled() {
        let mut context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        let mut gateway_config = inner.sorafs_gateway_config.clone();
        gateway_config.require_manifest_envelope = false;
        let components =
            build_sorafs_gateway_security(&gateway_config, inner.sorafs_admission.clone());
        inner.sorafs_gateway_config = gateway_config;
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(inner);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-optional-envelope"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8094))),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    }

    #[tokio::test]
    async fn storage_fetch_requires_capability_when_enforced() {
        let fixture = make_signed_advert();
        let mut inner = Arc::try_unwrap(app_state_with_seeded_cache(&fixture))
            .unwrap_or_else(|_| panic!("unique app state"));
        inner.sorafs_gateway_config.enforce_admission = false;
        inner.sorafs_gateway_config.enforce_capabilities = true;
        let components = build_sorafs_gateway_security(
            &inner.sorafs_gateway_config,
            inner.sorafs_admission.clone(),
        );
        inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        let state = Arc::new(inner);

        let payload = b"sorafs capability enforcement payload";
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xDC; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(payload.len() as u64)
            .car_digest(blake3::hash(payload).into())
            .car_size(payload.len() as u64)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let manifest_b64 =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&manifest).unwrap());
        let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        let pin_request = StoragePinRequestDto {
            manifest_b64,
            payload_b64,
            ..Default::default()
        };

        let pin_response = handle_post_sorafs_storage_pin(
            State(state.clone()),
            HeaderMap::new(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
            JsonOnly(pin_request),
        )
        .await;
        assert_eq!(pin_response.status(), StatusCode::OK);
        let pin_bytes = body::to_bytes(pin_response.into_body(), usize::MAX)
            .await
            .expect("collect pin body");
        let pin_value: Value = norito::json::from_slice(&pin_bytes).expect("decode pin response");
        let manifest_id_hex = pin_value
            .get("manifest_id_hex")
            .and_then(Value::as_str)
            .expect("manifest id")
            .to_string();

        let provider_bytes = fixture.provider_id();
        let provider_id_hex = hex::encode(provider_bytes);

        let make_headers = |nonce: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_NONCE),
                HeaderValue::from_str(nonce).expect("nonce header must be ASCII"),
            );
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_CLIENT),
                HeaderValue::from_static("capability-client"),
            );
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_NAME),
                HeaderValue::from_static("alias@capability"),
            );
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_PROOF),
                alias_proof_header("alias@capability"),
            );
            headers.insert(
                header::HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
                HeaderValue::from_static("dummy-envelope"),
            );
            headers
        };

        let make_request = || StorageFetchRequestDto {
            manifest_id_hex: manifest_id_hex.clone(),
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: Some(provider_id_hex.clone()),
        };

        // Capability advertised via discovery should allow fetch.
        let response_success = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            make_headers("capability-allowed"),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8094))),
            JsonOnly(make_request()),
        )
        .await;
        assert_eq!(response_success.status(), StatusCode::OK);

        // Capability override denies chunk-range fetch (missing capability branch).
        state
            .sorafs_chunk_range_overrides
            .insert(provider_bytes, false);
        let response_missing = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            make_headers("capability-missing"),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8095))),
            JsonOnly(make_request()),
        )
        .await;
        assert_eq!(response_missing.status(), StatusCode::PRECONDITION_FAILED);
        let missing_body = body::to_bytes(response_missing.into_body(), usize::MAX)
            .await
            .expect("collect missing capability body");
        let missing_value: Value =
            norito::json::from_slice(&missing_body).expect("decode missing capability response");
        assert_eq!(
            missing_value.get("error"),
            Some(&Value::String("capability_missing".into()))
        );
        let missing_details = missing_value
            .get("details")
            .and_then(Value::as_object)
            .expect("missing capability details");
        assert_eq!(
            missing_details
                .get("provider_id_hex")
                .and_then(Value::as_str),
            Some(provider_id_hex.as_str())
        );
        assert_eq!(
            missing_details.get("capability").and_then(Value::as_str),
            Some("chunk_range_fetch")
        );

        // Drop override so capability state becomes unknown.
        state.sorafs_chunk_range_overrides.remove(&provider_bytes);
        if let Some(cache) = &state.sorafs_cache {
            let mut guard = cache
                .try_write()
                .expect("expire cached provider advert entry");
            guard.prune_stale(fixture.expires_at().saturating_add(TTL_SECS));
        }

        let response_unknown = handle_post_sorafs_storage_fetch(
            State(state.clone()),
            make_headers("capability-unknown"),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8096))),
            JsonOnly(make_request()),
        )
        .await;
        assert_eq!(response_unknown.status(), StatusCode::PRECONDITION_FAILED);
        let unknown_body = body::to_bytes(response_unknown.into_body(), usize::MAX)
            .await
            .expect("collect unknown capability body");
        let unknown_value: Value =
            norito::json::from_slice(&unknown_body).expect("decode unknown capability response");
        assert_eq!(
            unknown_value.get("error"),
            Some(&Value::String("capability_state_unknown".into()))
        );
        let unknown_details = unknown_value
            .get("details")
            .and_then(Value::as_object)
            .expect("unknown capability details");
        assert_eq!(
            unknown_details
                .get("provider_id_hex")
                .and_then(Value::as_str),
            Some(provider_id_hex.as_str())
        );
        assert_eq!(
            unknown_details.get("capability").and_then(Value::as_str),
            Some("chunk_range_fetch")
        );

        // Advertise capability (override) and ensure the fetch succeeds.
        state
            .sorafs_chunk_range_overrides
            .insert(provider_bytes, true);
        let response_restored = handle_post_sorafs_storage_fetch(
            State(state),
            make_headers("capability-restored"),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8097))),
            JsonOnly(make_request()),
        )
        .await;
        assert_eq!(response_restored.status(), StatusCode::OK);
        let restored_body = body::to_bytes(response_restored.into_body(), usize::MAX)
            .await
            .expect("collect restored fetch body");
        let restored_value: Value =
            norito::json::from_slice(&restored_body).expect("decode restored fetch response");
        let data_b64 = restored_value
            .get("data_b64")
            .and_then(Value::as_str)
            .expect("payload base64");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data_b64.as_bytes())
            .expect("decode permitted payload");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn storage_fetch_perceptual_denylist_preempts_capability_override() {
        let fixture = make_signed_advert();
        let payload = b"perceptual capability fetch payload".to_vec();
        let mut context = capability_token_context(&fixture, payload.clone());

        let app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        if let Some(denylist) = &app_inner.sorafs_gateway_denylist {
            let metadata = crate::sorafs::gateway::DenylistEntryBuilder::default()
                .reason("perceptual capability block")
                .build();
            let canonical_hash = [0x99; 32];
            denylist.upsert_perceptual(
                crate::sorafs::gateway::PerceptualFamilyEntry::new([0xDD; 16], metadata)
                    .with_perceptual_hash(Some(canonical_hash), 4),
            );
        } else {
            panic!("gateway denylist must be configured");
        }
        app_inner
            .sorafs_chunk_range_overrides
            .insert(fixture.provider_id(), false);
        context.app = Arc::new(app_inner);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("perceptual-fetch-nonce"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("perceptual-fetch-client"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );
        let mut observed_hash = [0x99; 32];
        observed_hash[0] ^= 0x01;
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PERCEPTUAL_HASH),
            header_value(&hex::encode(observed_hash), "X-SoraFS-Perceptual-Hash"),
        );

        let request = StorageFetchRequestDto {
            manifest_id_hex: context.manifest_id_hex.clone(),
            offset: 0,
            length: payload.len() as u64,
            provider_id_hex: Some(context.provider_id_hex.clone()),
        };

        let response = handle_post_sorafs_storage_fetch(
            State(context.app.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8108))),
            JsonOnly(request),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect perceptual denylist body");
        let value: Value =
            norito::json::from_slice(&body_bytes).expect("decode perceptual denylist response");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("denylisted".into()))
        );
        assert_eq!(
            value
                .get("perceptual_hamming_distance")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            value.get("provider_id_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn car_range_enforces_capability_when_enabled() {
        let fixture = make_signed_advert();
        let context =
            capability_token_context(&fixture, b"capability enforcement car payload".to_vec());
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata present");
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let provider_id_hex = hex::encode(fixture.provider_id());
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let headers_success = car_range_headers(
            &chunker_handle,
            manifest.content_length(),
            &token_base64,
            "capability-car",
        );
        let response_success = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers_success,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8088))),
        )
        .await;
        assert_eq!(response_success.status(), StatusCode::PARTIAL_CONTENT);

        context
            .app
            .sorafs_chunk_range_overrides
            .insert(fixture.provider_id(), false);
        let headers_missing = car_range_headers(
            &chunker_handle,
            manifest.content_length(),
            &token_base64,
            "capability-car-missing",
        );
        let response_missing = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers_missing,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8089))),
        )
        .await;
        assert_eq!(response_missing.status(), StatusCode::PRECONDITION_FAILED);
        let missing_body = body::to_bytes(response_missing.into_body(), usize::MAX)
            .await
            .expect("collect missing capability body");
        let missing_value: Value =
            norito::json::from_slice(&missing_body).expect("decode missing capability response");
        assert_eq!(
            missing_value.get("error"),
            Some(&Value::String("capability_missing".into()))
        );
        let missing_details = missing_value
            .get("details")
            .and_then(Value::as_object)
            .expect("missing capability details");
        assert_eq!(
            missing_details
                .get("provider_id_hex")
                .and_then(Value::as_str),
            Some(provider_id_hex.as_str())
        );

        context
            .app
            .sorafs_chunk_range_overrides
            .remove(&fixture.provider_id());
        if let Some(cache) = &context.app.sorafs_cache {
            let mut guard = cache
                .try_write()
                .expect("expire cached provider advert entry");
            guard.prune_stale(fixture.expires_at().saturating_add(TTL_SECS));
        }

        let headers_unknown = car_range_headers(
            &chunker_handle,
            manifest.content_length(),
            &token_base64,
            "capability-car-unknown",
        );
        let response_unknown = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers_unknown,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8090))),
        )
        .await;
        assert_eq!(response_unknown.status(), StatusCode::PRECONDITION_FAILED);
        let unknown_body = body::to_bytes(response_unknown.into_body(), usize::MAX)
            .await
            .expect("collect unknown capability body");
        let unknown_value: Value =
            norito::json::from_slice(&unknown_body).expect("decode unknown capability response");
        assert_eq!(
            unknown_value.get("error"),
            Some(&Value::String("capability_state_unknown".into()))
        );
        let unknown_details = unknown_value
            .get("details")
            .and_then(Value::as_object)
            .expect("unknown capability details");
        assert_eq!(
            unknown_details
                .get("provider_id_hex")
                .and_then(Value::as_str),
            Some(provider_id_hex.as_str())
        );
    }

    #[tokio::test]
    async fn chunk_range_enforces_capability_when_enabled() {
        let fixture = make_signed_advert();
        let context =
            capability_token_context(&fixture, b"capability enforcement chunk payload".to_vec());
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata present");
        let chunk_digest_hex =
            hex::encode(manifest.chunk(0).expect("chunk metadata present").digest);
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let provider_id_hex = hex::encode(fixture.provider_id());
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let headers_success = chunk_range_headers(
            &chunker_handle,
            &chunk_digest_hex,
            &token_base64,
            "capability-chunk",
        );
        let response_success = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex.clone())),
            headers_success,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8091))),
        )
        .await;
        assert_eq!(response_success.status(), StatusCode::OK);

        context
            .app
            .sorafs_chunk_range_overrides
            .insert(fixture.provider_id(), false);
        let headers_missing = chunk_range_headers(
            &chunker_handle,
            &chunk_digest_hex,
            &token_base64,
            "capability-chunk-missing",
        );
        let response_missing = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex.clone())),
            headers_missing,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8092))),
        )
        .await;
        assert_eq!(response_missing.status(), StatusCode::PRECONDITION_FAILED);
        let missing_body = body::to_bytes(response_missing.into_body(), usize::MAX)
            .await
            .expect("collect missing capability body");
        let missing_value: Value =
            norito::json::from_slice(&missing_body).expect("decode missing capability response");
        assert_eq!(
            missing_value.get("error"),
            Some(&Value::String("capability_missing".into()))
        );
        let missing_details = missing_value
            .get("details")
            .and_then(Value::as_object)
            .expect("missing capability details");
        assert_eq!(
            missing_details
                .get("provider_id_hex")
                .and_then(Value::as_str),
            Some(provider_id_hex.as_str())
        );

        context
            .app
            .sorafs_chunk_range_overrides
            .remove(&fixture.provider_id());
        if let Some(cache) = &context.app.sorafs_cache {
            let mut guard = cache
                .try_write()
                .expect("expire cached provider advert entry");
            guard.prune_stale(fixture.expires_at().saturating_add(TTL_SECS));
        }

        let headers_unknown = chunk_range_headers(
            &chunker_handle,
            &chunk_digest_hex,
            &token_base64,
            "capability-chunk-unknown",
        );
        let response_unknown = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            headers_unknown,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8093))),
        )
        .await;
        assert_eq!(response_unknown.status(), StatusCode::PRECONDITION_FAILED);
        let unknown_body = body::to_bytes(response_unknown.into_body(), usize::MAX)
            .await
            .expect("collect unknown capability body");
        let unknown_value: Value =
            norito::json::from_slice(&unknown_body).expect("decode unknown capability response");
        assert_eq!(
            unknown_value.get("error"),
            Some(&Value::String("capability_state_unknown".into()))
        );
        let unknown_details = unknown_value
            .get("details")
            .and_then(Value::as_object)
            .expect("unknown capability details");
        assert_eq!(
            unknown_details
                .get("provider_id_hex")
                .and_then(Value::as_str),
            Some(provider_id_hex.as_str())
        );
    }

    #[tokio::test]
    async fn chunk_range_perceptual_denylist_reports_match() {
        let mut context = token_test_context();
        let app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));

        if let Some(denylist) = &app_inner.sorafs_gateway_denylist {
            let metadata = crate::sorafs::gateway::DenylistEntryBuilder::default()
                .reason("perceptual deny")
                .build();
            let canonical_hash = [0xAA; 32];
            let family_id = [0xEF; 16];
            denylist.upsert_perceptual(
                crate::sorafs::gateway::PerceptualFamilyEntry::new(family_id, metadata)
                    .with_perceptual_hash(Some(canonical_hash), 4),
            );
        } else {
            panic!("gateway denylist must be configured");
        }

        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata present");
        let chunk_digest_hex =
            hex::encode(manifest.chunk(0).expect("chunk metadata present").digest);
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = chunk_range_headers(
            &chunker_handle,
            &chunk_digest_hex,
            &token_base64,
            "perceptual-deny",
        );
        let mut observed_hash = [0xAA; 32];
        observed_hash[0] ^= 0x01;
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PERCEPTUAL_HASH),
            header_value(&hex::encode(observed_hash), "X-SoraFS-Perceptual-Hash"),
        );

        let response = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8098))),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect perceptual denylist body");
        let body: Value =
            norito::json::from_slice(&body_bytes).expect("decode perceptual denylist response");
        assert_eq!(body.get("error"), Some(&Value::String("denylisted".into())));
        assert_eq!(
            body.get("perceptual_hamming_distance")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.get("provider_id_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn car_range_perceptual_denylist_reports_match() {
        let mut context = token_test_context();
        let app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));

        if let Some(denylist) = &app_inner.sorafs_gateway_denylist {
            let metadata = crate::sorafs::gateway::DenylistEntryBuilder::default()
                .reason("perceptual deny car")
                .build();
            let canonical_hash = [0xBE; 32];
            let family_id = [0xDE; 16];
            denylist.upsert_perceptual(
                crate::sorafs::gateway::PerceptualFamilyEntry::new(family_id, metadata)
                    .with_perceptual_hash(Some(canonical_hash), 4),
            );
        } else {
            panic!("gateway denylist must be configured");
        }

        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata present");
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = car_range_headers(
            &chunker_handle,
            manifest.content_length(),
            &token_base64,
            "perceptual-car",
        );
        let mut observed_hash = [0xBE; 32];
        observed_hash[1] ^= 0x01;
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PERCEPTUAL_HASH),
            header_value(&hex::encode(observed_hash), "X-SoraFS-Perceptual-Hash"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8099))),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect perceptual car denylist body");
        let body: Value =
            norito::json::from_slice(&body_bytes).expect("decode perceptual car denylist response");
        assert_eq!(body.get("error"), Some(&Value::String("denylisted".into())));
        assert_eq!(
            body.get("perceptual_hamming_distance")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.get("provider_id_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn chunk_range_perceptual_denylist_preempts_capability_override() {
        let fixture = make_signed_advert();
        let mut context =
            capability_token_context(&fixture, b"perceptual capability chunk payload".to_vec());

        let app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        if let Some(denylist) = &app_inner.sorafs_gateway_denylist {
            let metadata = crate::sorafs::gateway::DenylistEntryBuilder::default()
                .reason("perceptual capability block")
                .build();
            let canonical_hash = [0xBA; 32];
            denylist.upsert_perceptual(
                crate::sorafs::gateway::PerceptualFamilyEntry::new([0xCC; 16], metadata)
                    .with_perceptual_hash(Some(canonical_hash), 4),
            );
        } else {
            panic!("gateway denylist must be configured");
        }
        app_inner
            .sorafs_chunk_range_overrides
            .insert(fixture.provider_id(), false);
        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata present");
        let chunk_record = manifest.chunk(0).expect("chunk record");
        let chunk_digest_hex = hex::encode(chunk_record.digest);
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = chunk_range_headers(
            &chunker_handle,
            &chunk_digest_hex,
            &token_base64,
            "perceptual-capability",
        );
        let mut observed_hash = [0xBA; 32];
        observed_hash[0] ^= 0x01;
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PERCEPTUAL_HASH),
            header_value(&hex::encode(observed_hash), "X-SoraFS-Perceptual-Hash"),
        );

        let response = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8109))),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect perceptual denylist body");
        let body: Value =
            norito::json::from_slice(&body_bytes).expect("decode perceptual denylist response");
        assert_eq!(body.get("error"), Some(&Value::String("denylisted".into())));
        assert_eq!(
            body.get("perceptual_hamming_distance")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.get("provider_id_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn car_range_perceptual_denylist_preempts_capability_override() {
        let fixture = make_signed_advert();
        let mut context =
            capability_token_context(&fixture, b"perceptual capability car payload".to_vec());

        let app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        if let Some(denylist) = &app_inner.sorafs_gateway_denylist {
            let metadata = crate::sorafs::gateway::DenylistEntryBuilder::default()
                .reason("perceptual capability block (car)")
                .build();
            let canonical_hash = [0xAB; 32];
            denylist.upsert_perceptual(
                crate::sorafs::gateway::PerceptualFamilyEntry::new([0xEE; 16], metadata)
                    .with_perceptual_hash(Some(canonical_hash), 4),
            );
        } else {
            panic!("gateway denylist must be configured");
        }
        app_inner
            .sorafs_chunk_range_overrides
            .insert(fixture.provider_id(), false);
        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest metadata present");
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = car_range_headers(
            &chunker_handle,
            manifest.content_length(),
            &token_base64,
            "perceptual-capability-car",
        );
        let mut observed_hash = [0xAB; 32];
        observed_hash[0] ^= 0x01;
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PERCEPTUAL_HASH),
            header_value(&hex::encode(observed_hash), "X-SoraFS-Perceptual-Hash"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8110))),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect perceptual car denylist body");
        let body: Value =
            norito::json::from_slice(&body_bytes).expect("decode perceptual car denylist response");
        assert_eq!(body.get("error"), Some(&Value::String("denylisted".into())));
        assert_eq!(
            body.get("perceptual_hamming_distance")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            body.get("provider_id_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn car_range_rejects_provider_without_admission() {
        let mut context = token_test_context();
        let mut app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        app_inner.sorafs_admission = Some(Arc::new(AdmissionRegistry::empty()));
        let mut gateway_config = app_inner.sorafs_gateway_config.clone();
        gateway_config.enforce_admission = true;
        let components =
            build_sorafs_gateway_security(&gateway_config, app_inner.sorafs_admission.clone());
        app_inner.sorafs_gateway_config = gateway_config;
        app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-admission"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
            HeaderValue::from_static("dummy"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8087))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body bytes");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error JSON");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("provider_not_admitted".into()))
        );
        assert_eq!(
            value.get("provider_id_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn car_range_blocked_for_denylisted_provider() {
        let mut context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let denylist_dir = tempfile::tempdir().expect("create denylist dir");
        let denylist_path = denylist_dir.path().join("denylist.json");
        let denylist_json = format!(
            "[{{\"kind\":\"provider\",\"provider_id_hex\":\"{}\",\"reason\":\"blocked\"}}]",
            context.provider_id_hex
        );
        fs::write(&denylist_path, denylist_json).expect("write denylist file");

        let mut app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        let mut gateway_config = app_inner.sorafs_gateway_config.clone();
        gateway_config.denylist.path = Some(denylist_path.clone());
        let components =
            build_sorafs_gateway_security(&gateway_config, app_inner.sorafs_admission.clone());
        app_inner.sorafs_gateway_config = gateway_config;
        app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(app_inner);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
            HeaderValue::from_static("dummy"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8086))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body bytes");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error JSON");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("denylisted".into()))
        );
    }

    #[tokio::test]
    async fn car_range_rate_limits_repeated_clients() {
        let mut context = token_test_context();
        let mut app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        let mut gateway_config = app_inner.sorafs_gateway_config.clone();
        gateway_config.enforce_admission = false;
        gateway_config.rate_limit.max_requests = Some(NonZeroU32::new(1).expect("non-zero"));
        gateway_config.rate_limit.window = Duration::from_mins(1);
        gateway_config.rate_limit.ban = None;
        let components =
            build_sorafs_gateway_security(&gateway_config, app_inner.sorafs_admission.clone());
        app_inner.sorafs_gateway_config = gateway_config;
        app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(app_inner);

        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-rate-0"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
            HeaderValue::from_static("dummy"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CLIENT),
            HeaderValue::from_static("gateway-alpha"),
        );

        let success_response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers.clone(),
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8092))),
        )
        .await;
        assert_eq!(success_response.status(), StatusCode::PARTIAL_CONTENT);
        let tls_header = success_response
            .headers()
            .get(HeaderName::from_static(SORA_TLS_STATE_HEADER))
            .and_then(|value| value.to_str().ok())
            .expect("tls state header present");
        assert!(
            tls_header.starts_with("ech-"),
            "header value should advertise ECH state"
        );

        let mut throttled_headers = headers;
        throttled_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-rate-1"),
        );

        let throttled = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            throttled_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8092))),
        )
        .await;
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        let body_bytes = body::to_bytes(throttled.into_body(), usize::MAX)
            .await
            .expect("collect body bytes");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error JSON");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("rate_limited".into()))
        );
    }

    #[tokio::test]
    async fn chunk_range_blocked_for_denylisted_provider() {
        let mut context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let chunk_record = manifest.chunk(0).expect("chunk record");
        let chunk_digest_hex = hex::encode(chunk_record.digest);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let denylist_dir = tempfile::tempdir().expect("create denylist dir");
        let denylist_path = denylist_dir.path().join("denylist.json");
        let denylist_json = format!(
            "[{{\"kind\":\"provider\",\"provider_id_hex\":\"{}\",\"reason\":\"blocked\"}}]",
            context.provider_id_hex
        );
        fs::write(&denylist_path, denylist_json).expect("write denylist file");

        let mut app_inner = Arc::try_unwrap(context.app)
            .unwrap_or_else(|_| panic!("token test context should hold unique app state"));
        let mut gateway_config = app_inner.sorafs_gateway_config.clone();
        gateway_config.denylist.path = Some(denylist_path.clone());
        let components =
            build_sorafs_gateway_security(&gateway_config, app_inner.sorafs_admission.clone());
        app_inner.sorafs_gateway_config = gateway_config;
        app_inner.sorafs_gateway_policy = Some(Arc::clone(&components.policy));
        app_inner.sorafs_gateway_denylist = Some(Arc::clone(&components.denylist));
        app_inner.sorafs_gateway_tls_state = Some(Arc::clone(&components.tls_state));
        context.app = Arc::new(app_inner);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-chunk"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8090))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body bytes");
        let value: Value = norito::json::from_slice(&body_bytes).expect("decode error JSON");
        assert_eq!(
            value.get("error"),
            Some(&Value::String("denylisted".into()))
        );
        assert_eq!(value.get("kind"), Some(&Value::String("provider".into())));
        assert_eq!(
            value.get("value_hex"),
            Some(&Value::String(context.provider_id_hex.clone()))
        );
    }

    #[tokio::test]
    async fn car_range_rejects_corrupted_payload() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let chunker_handle = manifest.chunk_profile_handle().to_string();
        let chunk_path = manifest.chunk(0).expect("chunk metadata").path.clone();
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut chunk_bytes = std::fs::read(&chunk_path).expect("read chunk bytes");
        if let Some(first) = chunk_bytes.first_mut() {
            *first ^= 0xAA;
        }
        std::fs::write(&chunk_path, &chunk_bytes).expect("write corrupted chunk");

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(&chunker_handle, "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8084))),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::UNPROCESSABLE_ENTITY);
        let body_bytes = body::to_bytes(body, usize::MAX)
            .await
            .expect("collect error body bytes");
        let value: Value =
            norito::json::from_slice(&body_bytes).expect("decode error response JSON");
        let error_str = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            error_str.contains("car verification failed"),
            "unexpected error message: {error_str}"
        );
    }

    #[tokio::test]
    async fn car_range_enforces_rate_limit_bytes() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let end = manifest.content_length().saturating_sub(1);
        let limited_bytes = manifest.content_length().saturating_sub(1);
        let overrides = TokenOverrides {
            rate_limit_bytes: Some(limited_bytes),
            ..TokenOverrides::default()
        };
        let token_base64 = issue_token_base64(&context, overrides).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8085))),
        )
        .await;
        let (parts, body) = response.into_parts();
        let status = parts.status;
        let body_bytes = body::to_bytes(body, usize::MAX)
            .await
            .expect("collect rate-limit response body");
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let value: Value =
            norito::json::from_slice(&body_bytes).expect("decode rate-limit response");
        assert_eq!(
            value.get("error").and_then(Value::as_str),
            Some("stream token rate limit exceeded")
        );
    }

    #[tokio::test]
    async fn car_range_accepts_multi_chunk_requests_with_small_stream_budget() {
        let payload = vec![0xDD; 2 * 1024 * 1024];
        let context = token_test_context_with_payload(payload);
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        assert!(
            manifest.chunk_count() > 4,
            "expected multi-chunk manifest for test"
        );
        let full_slice = manifest
            .chunk_slice(0, manifest.content_length() as usize)
            .expect("derive full chunk slice");
        assert_eq!(
            full_slice.start_index, 0,
            "chunk slice should start at the first chunk"
        );
        assert_eq!(
            full_slice.chunk_count(),
            manifest.chunk_count(),
            "slice should cover all chunks for full-range request"
        );
        let end = manifest.content_length().saturating_sub(1);
        let overrides = TokenOverrides {
            max_streams: Some(4),
            ..TokenOverrides::default()
        };
        let token_base64 = issue_token_base64(&context, overrides).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );

        let response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await;
        let status = response.status();
        if status != StatusCode::PARTIAL_CONTENT {
            let (_, resp_body) = response.into_parts();
            let body = body::to_bytes(resp_body, usize::MAX)
                .await
                .expect("collect failure body");
            panic!(
                "expected 206 PARTIAL_CONTENT, got {} with body {}",
                status,
                String::from_utf8_lossy(&body)
            );
        }
    }

    #[tokio::test]
    async fn chunk_range_accepts_valid_stream_token() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let chunk_record = manifest.chunk(0).expect("chunk record");
        let chunk_digest_hex = hex::encode(chunk_record.digest);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-chunk"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let response = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8082))),
        )
        .await;
        let (parts, _) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        let tls_state = parts
            .headers
            .get(HeaderName::from_static(SORA_TLS_STATE_HEADER))
            .and_then(|value| value.to_str().ok())
            .expect("TLS state header present");
        assert!(
            tls_state.starts_with("ech-"),
            "unexpected TLS state header: {tls_state}"
        );
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn chunk_range_records_metrics() {
        let context = token_test_context();
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let chunk_record = manifest.chunk(0).expect("chunk record");
        let chunk_digest_hex = hex::encode(chunk_record.digest);
        let token_base64 = issue_token_base64(&context, TokenOverrides::default()).await;

        let (car_requests_before, car_bytes_before, chunk_requests_before, chunk_bytes_before) = {
            let metrics = context.app.telemetry.metrics().await;
            let car_requests = metrics
                .torii_sorafs_chunk_range_requests_total
                .with_label_values(&[TELEMETRY_ENDPOINT_CAR_RANGE, "206"])
                .get();
            let car_bytes = metrics
                .torii_sorafs_chunk_range_bytes_total
                .with_label_values(&[TELEMETRY_ENDPOINT_CAR_RANGE])
                .get();
            let chunk_requests = metrics
                .torii_sorafs_chunk_range_requests_total
                .with_label_values(&[TELEMETRY_ENDPOINT_CHUNK, "200"])
                .get();
            let chunk_bytes = metrics
                .torii_sorafs_chunk_range_bytes_total
                .with_label_values(&[TELEMETRY_ENDPOINT_CHUNK])
                .get();
            (car_requests, car_bytes, chunk_requests, chunk_bytes)
        };

        let mut range_headers = HeaderMap::new();
        let end = manifest.content_length().saturating_sub(1);
        range_headers.insert(
            header::RANGE,
            HeaderValue::from_str(&format!("bytes=0-{end}")).expect("range header"),
        );
        range_headers.insert(
            header::HeaderName::from_static(HEADER_DAG_SCOPE),
            HeaderValue::from_static("block"),
        );
        range_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        range_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-download"),
        );
        range_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        range_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        range_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let car_response = handle_get_sorafs_storage_car_range(
            State(context.app.clone()),
            Path(context.manifest_id_hex.clone()),
            range_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))),
        )
        .await;
        assert_eq!(car_response.status(), StatusCode::PARTIAL_CONTENT);
        let car_body_bytes = body::to_bytes(car_response.into_body(), usize::MAX)
            .await
            .expect("car range body");
        let car_payload_len = car_body_bytes.len() as u64;

        let mut chunk_headers = HeaderMap::new();
        chunk_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NONCE),
            HeaderValue::from_static("nonce-chunk"),
        );
        chunk_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );
        chunk_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_CHUNKER),
            header_value(manifest.chunk_profile_handle(), "X-SoraFS-Chunker"),
        );
        chunk_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_NAME),
            header_value("alias/test", "Sora-Name"),
        );
        chunk_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_PROOF),
            alias_proof_header("alias/test"),
        );

        let chunk_response = handle_get_sorafs_storage_chunk(
            State(context.app.clone()),
            Path((context.manifest_id_hex.clone(), chunk_digest_hex)),
            chunk_headers,
            ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8083))),
        )
        .await;
        assert_eq!(chunk_response.status(), StatusCode::OK);
        let chunk_body_bytes = body::to_bytes(chunk_response.into_body(), usize::MAX)
            .await
            .expect("chunk body");
        let chunk_payload_len = chunk_body_bytes.len() as u64;

        let metrics_after = context.app.telemetry.metrics().await;
        let car_requests_after = metrics_after
            .torii_sorafs_chunk_range_requests_total
            .with_label_values(&[TELEMETRY_ENDPOINT_CAR_RANGE, "206"])
            .get();
        let car_bytes_after = metrics_after
            .torii_sorafs_chunk_range_bytes_total
            .with_label_values(&[TELEMETRY_ENDPOINT_CAR_RANGE])
            .get();
        let chunk_requests_after = metrics_after
            .torii_sorafs_chunk_range_requests_total
            .with_label_values(&[TELEMETRY_ENDPOINT_CHUNK, "200"])
            .get();
        let chunk_bytes_after = metrics_after
            .torii_sorafs_chunk_range_bytes_total
            .with_label_values(&[TELEMETRY_ENDPOINT_CHUNK])
            .get();

        assert_eq!(car_requests_after, car_requests_before + 1);
        assert_eq!(car_bytes_after, car_bytes_before + car_payload_len);
        assert_eq!(chunk_requests_after, chunk_requests_before + 1);
        assert_eq!(chunk_bytes_after, chunk_bytes_before + chunk_payload_len);
    }

    #[tokio::test]
    async fn stream_token_enforces_concurrency_budget() {
        let payload = vec![0xEE; 128 * 1024];
        let context = token_test_context_with_payload(payload);
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let overrides = TokenOverrides {
            max_streams: Some(1),
            ..TokenOverrides::default()
        };
        let token_base64 = issue_token_base64(&context, overrides).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_base64, "X-SoraFS-Stream-Token"),
        );

        let requested_bytes = manifest.content_length();
        let (first_guard, _) =
            enforce_stream_token_for_request(&context.app, &headers, &manifest, requested_bytes)
                .expect("first request permitted");
        assert!(
            first_guard.has_permit(),
            "finite limit should produce a concurrency permit"
        );

        let second =
            enforce_stream_token_for_request(&context.app, &headers, &manifest, requested_bytes)
                .expect_err("second request must be rejected while guard held");
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);

        drop(first_guard);

        let (retry_guard, _) =
            enforce_stream_token_for_request(&context.app, &headers, &manifest, requested_bytes)
                .expect("request should succeed after guard dropped");
        assert!(
            retry_guard.has_permit(),
            "finite limit should produce a concurrency permit"
        );
        drop(retry_guard);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn provider_advert_updates_range_capability_gauge() {
        let fixture = make_signed_advert();
        let app = app_state_with_cache(&fixture);
        let advert_bytes =
            Bytes::from(norito::to_bytes(fixture.advert()).expect("serialize provider advert"));
        let response = handle_post_sorafs_provider_advert(State(app.clone()), advert_bytes).await;
        assert_eq!(response.status(), StatusCode::OK);

        let metrics = app.telemetry.metrics().await;
        assert_eq!(
            metrics
                .torii_sorafs_provider_range_capability_total
                .with_label_values(&[RANGE_CAPABILITY_FEATURE_PROVIDERS])
                .get(),
            1,
            "provider capability gauge reflects stored advert count"
        );
        assert_eq!(
            metrics
                .torii_sorafs_provider_range_capability_total
                .with_label_values(&[RANGE_CAPABILITY_FEATURE_SPARSE])
                .get(),
            1,
            "sparse offsets support is recorded"
        );
        assert_eq!(
            metrics
                .torii_sorafs_provider_range_capability_total
                .with_label_values(&[RANGE_CAPABILITY_FEATURE_ALIGNMENT])
                .get(),
            0,
            "alignment requirement gauge remains zero when not requested"
        );
        assert_eq!(
            metrics
                .torii_sorafs_provider_range_capability_total
                .with_label_values(&[RANGE_CAPABILITY_FEATURE_MERKLE])
                .get(),
            1,
            "merkle proof support is recorded"
        );
        assert_eq!(
            metrics
                .torii_sorafs_provider_range_capability_total
                .with_label_values(&[RANGE_CAPABILITY_FEATURE_STREAM_BUDGET])
                .get(),
            1,
            "stream budget availability is recorded"
        );
        assert_eq!(
            metrics
                .torii_sorafs_provider_range_capability_total
                .with_label_values(&[RANGE_CAPABILITY_FEATURE_TRANSPORT_HINTS])
                .get(),
            1,
            "transport hints presence is recorded"
        );
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn stream_token_records_range_fetch_throttle_metrics() {
        let payload = vec![0xDD; 64 * 1024];
        let context = token_test_context_with_payload(payload);
        let manifest = context
            .app
            .sorafs_node
            .manifest_metadata(&context.manifest_id_hex)
            .expect("manifest");
        let requested_bytes = manifest.content_length();

        let baseline_metrics = context.app.telemetry.metrics().await;
        let baseline_concurrency = baseline_metrics
            .torii_sorafs_range_fetch_concurrency_current
            .get();
        let baseline_quota = baseline_metrics
            .torii_sorafs_range_fetch_throttle_events_total
            .with_label_values(&[RANGE_THROTTLE_REASON_QUOTA])
            .get();
        let baseline_byte_rate = baseline_metrics
            .torii_sorafs_range_fetch_throttle_events_total
            .with_label_values(&[RANGE_THROTTLE_REASON_BYTE_RATE])
            .get();

        assert_eq!(baseline_concurrency, 0, "no active streams on baseline");

        // Quota throttle
        let overrides_quota = TokenOverrides {
            requests_per_minute: Some(1),
            max_streams: Some(4),
            ..TokenOverrides::default()
        };
        let token_quota = issue_token_base64(&context, overrides_quota).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_quota, "X-SoraFS-Stream-Token"),
        );

        let (quota_guard, _) =
            enforce_stream_token_for_request(&context.app, &headers, &manifest, requested_bytes)
                .expect("first quota request permitted");
        drop(quota_guard);

        let quota_retry =
            enforce_stream_token_for_request(&context.app, &headers, &manifest, requested_bytes)
                .expect_err("quota enforcement should reject second request");
        assert_eq!(quota_retry.status(), StatusCode::TOO_MANY_REQUESTS);

        let quota_metrics = context.app.telemetry.metrics().await;
        assert_eq!(
            quota_metrics
                .torii_sorafs_range_fetch_throttle_events_total
                .with_label_values(&[RANGE_THROTTLE_REASON_QUOTA])
                .get(),
            baseline_quota + 1,
            "quota throttle counter increments"
        );

        // Byte-rate throttle
        let overrides_rate = TokenOverrides {
            rate_limit_bytes: Some(requested_bytes.saturating_sub(1)),
            max_streams: Some(4),
            ..TokenOverrides::default()
        };
        let token_rate = issue_token_base64(&context, overrides_rate).await;
        let mut rate_headers = HeaderMap::new();
        rate_headers.insert(
            header::HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            header_value(&token_rate, "X-SoraFS-Stream-Token"),
        );

        let rate_error = enforce_stream_token_for_request(
            &context.app,
            &rate_headers,
            &manifest,
            requested_bytes,
        )
        .expect_err("rate limit should reject oversized fetch");
        assert_eq!(rate_error.status(), StatusCode::TOO_MANY_REQUESTS);

        let final_metrics = context.app.telemetry.metrics().await;
        assert_eq!(
            final_metrics
                .torii_sorafs_range_fetch_throttle_events_total
                .with_label_values(&[RANGE_THROTTLE_REASON_BYTE_RATE])
                .get(),
            baseline_byte_rate + 1,
            "byte-rate throttle counter increments"
        );
        assert_eq!(
            final_metrics
                .torii_sorafs_range_fetch_concurrency_current
                .get(),
            0,
            "concurrency gauge returns to zero after throttled requests"
        );
    }
    const TTL_SECS: u64 = 3_600;

    #[derive(Clone)]
    struct ProviderFixture {
        advert: ProviderAdvertV1,
        envelope: ProviderAdmissionEnvelopeV1,
    }

    impl ProviderFixture {
        fn advert(&self) -> &ProviderAdvertV1 {
            &self.advert
        }

        fn provider_id(&self) -> [u8; 32] {
            self.advert.body.provider_id
        }

        fn issued_at(&self) -> u64 {
            self.advert.issued_at
        }

        fn expires_at(&self) -> u64 {
            self.advert.expires_at
        }
    }

    fn app_state_with_cache(fixture: &ProviderFixture) -> SharedAppState {
        app_state_with_cache_inner(fixture, false)
    }

    fn app_state_with_seeded_cache(fixture: &ProviderFixture) -> SharedAppState {
        app_state_with_cache_inner(fixture, true)
    }

    fn app_state_with_cache_inner(fixture: &ProviderFixture, seed_fixture: bool) -> SharedAppState {
        let admission = AdmissionRegistry::from_envelopes([fixture.envelope.clone()])
            .expect("fixture envelope must validate");
        let mut cache = ProviderAdvertCache::new(
            vec![
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            Arc::new(admission),
        );
        if seed_fixture {
            cache
                .ingest(fixture.advert.clone(), fixture.issued_at())
                .expect("ingest fixture advert into cache");
        }

        let mut app = Arc::try_unwrap(mk_app_state_for_tests())
            .unwrap_or_else(|_| panic!("unique app state"));
        let cache_arc = Arc::new(RwLock::new(cache));
        app.sorafs_cache = Some(cache_arc);
        let (node, _dir) = sorafs_node_with_temp_storage();
        seed_capacity_declaration(
            &node,
            fixture.provider_id(),
            &fixture.advert.body.profile_id,
        );
        app.sorafs_node = node;
        Arc::new(app)
    }

    fn make_signed_advert() -> ProviderFixture {
        let signing_key = SigningKey::from_bytes(&[0xA5; 32]);
        let provider_id = [0x11; 32];
        let stake_pool_id = [0x21; 32];
        let capabilities = vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: ProviderCapabilityRangeV1 {
                    max_chunk_span: 32,
                    min_granularity: 8,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: true,
                }
                .to_bytes()
                .expect("encode range capability"),
            },
        ];

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();
        let issued_at = now.saturating_sub(300);
        let expires_at = issued_at + TTL_SECS;

        let body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
            stake: StakePointer {
                pool_id: stake_pool_id,
                stake_amount: 1_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1_000,
                max_concurrent_streams: 8,
            },
            capabilities: capabilities.clone(),
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "storage.example.test".to_owned(),
                metadata: vec![EndpointMetadata {
                    key: EndpointMetadataKey::Region,
                    value: b"global".to_vec(),
                }],
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".to_owned(),
                region: "global".to_owned(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 5,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(StreamBudgetV1 {
                max_in_flight: 8,
                max_bytes_per_sec: 8_388_608,
                burst_bytes: Some(1_048_576),
            }),
            transport_hints: Some(vec![TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 0,
            }]),
        };
        body.validate().expect("fixture body must validate");
        let body_clone = body.clone();

        let body_bytes = norito::to_bytes(&body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at,
            body,
            signature: sorafs_manifest::AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };

        let attestation = EndpointAttestationV1 {
            version: ENDPOINT_ATTESTATION_VERSION_V1,
            kind: EndpointAttestationKind::Mtls,
            attested_at: issued_at.saturating_sub(60),
            expires_at: expires_at + 60,
            leaf_certificate: vec![0xAA],
            intermediate_certificates: Vec::new(),
            alpn_ids: vec!["h2".to_owned()],
            report: Vec::new(),
        };

        let proposal = ProviderAdmissionProposalV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id,
            profile_id: body_clone.profile_id.clone(),
            profile_aliases: body_clone.profile_aliases.clone(),
            stake: body_clone.stake.clone(),
            capabilities: body_clone.capabilities.clone(),
            endpoints: vec![EndpointAdmissionV1 {
                endpoint: body_clone.endpoints.first().expect("endpoint").clone(),
                attestation,
            }],
            advert_key: signing_key.verifying_key().to_bytes(),
            jurisdiction_code: "US".to_owned(),
            contact_uri: Some("mailto:ops@example.test".to_owned()),
            stream_budget: Some(StreamBudgetV1 {
                max_in_flight: 8,
                max_bytes_per_sec: 8_388_608,
                burst_bytes: Some(1_048_576),
            }),
            transport_hints: Some(vec![TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 0,
            }]),
        };

        let proposal_digest = compute_proposal_digest(&proposal).expect("proposal digest");
        let advert_body_digest =
            compute_advert_body_digest(&body_clone).expect("advert body digest");

        let council_key = SigningKey::from_bytes(&[0x42; 32]);
        let council_signature = council_key.sign(&proposal_digest);
        let envelope = ProviderAdmissionEnvelopeV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal,
            proposal_digest,
            advert_body: body_clone,
            advert_body_digest,
            issued_at,
            retention_epoch: expires_at + 600,
            council_signatures: vec![CouncilSignature {
                signer: council_key.verifying_key().to_bytes(),
                signature: council_signature.to_bytes().to_vec(),
            }],
            notes: None,
        };

        ProviderFixture { advert, envelope }
    }
}
