//! Canonical request signing helpers for app-facing HTTP endpoints.
//!
//! Clients may optionally attach:
//! - `X-Iroha-Account`: exact canonical I105 account id or active canonical
//!   ASCII account alias that authorises the request.
//! - `X-Iroha-Signature`: base64 signature over the canonical request bytes plus
//!   freshness metadata.
//! - `X-Iroha-Timestamp-Ms`: unix timestamp in milliseconds included in the
//!   signed payload.
//! - `X-Iroha-Nonce`: caller-chosen nonce included in the signed payload.
//! - `X-Iroha-Witness`: base64 Norito witness for multisig-controlled accounts.
//!
//! The canonical request bytes are:
//! ```text
//! <UPPERCASE_METHOD>\n
//! <path>\n
//! <sorted_query_string>\n
//! <hex_sha256(body)>\n
//! <timestamp_ms>\n
//! <nonce>
//! ```
//! - Query parameters are parsed, percent-decoded (treating `+` as space), sorted
//!   by `(key, value)`, then re-encoded using `application/x-www-form-urlencoded`
//!   rules.
//! - The body hash is computed over the raw request body bytes.
//! - Freshness validation rejects stale timestamps and replayed nonces.
//! - Nonce retention must exceed the full timestamp-skew window. A saturated
//!   cache rejects new requests and never evicts live replay evidence.
//! - `X-Iroha-Account` only identifies a caller when paired with a valid
//!   signature or witness; bare account headers are rejected on caller-scoped
//!   read paths.
//!
//! Some endpoints carry the same auth envelope inside a JSON body instead of
//! HTTP headers. Those callers provide `account_id`, `timestamp_ms`, `nonce`,
//! and exactly one proof field in the body, while the canonical message hashes
//! the endpoint-defined unsigned body bytes.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_config::parameters::{actual::AppApi as AppApiConfig, defaults};
use iroha_core::{
    sns::resolve_active_account_alias,
    state::{State as CoreState, WorldReadOnly},
};
use iroha_crypto::{Algorithm, Hash, PublicKey, Signature};
use iroha_data_model::{
    NetworkId, ValidationFail,
    account::{AccountController, AccountId, rekey::AccountAlias},
    query::{
        ErasedIterQuery, Query, QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
        dsl::{CompoundPredicate, HasProjection, PredicateMarker, SelectorMarker, SelectorTuple},
        error::{FindError, QueryExecutionFail},
        parameters::QueryParams,
    },
    soracloud::{
        CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
        CanonicalRequestWitnessV1,
    },
};
#[cfg(feature = "app_api")]
use iroha_torii_shared::FeeQuoteRequest;
use norito::codec::Encode;
use sha2::{Digest as _, Sha256};

use crate::bounded_replay_cache::{InsertError as ReplayInsertError, ReplayCache};

/// Header carrying the authorising account id.
pub const HEADER_ACCOUNT: &str = "X-Iroha-Account";
/// Header carrying the base64-encoded signature over the canonical request bytes.
pub const HEADER_SIGNATURE: &str = "X-Iroha-Signature";
/// Header carrying the unix timestamp in milliseconds for freshness checks.
pub const HEADER_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
/// Header carrying the caller-chosen replay nonce.
pub const HEADER_NONCE: &str = "X-Iroha-Nonce";
/// Header carrying the base64 Norito-encoded multisig witness.
pub const HEADER_WITNESS: &str = "X-Iroha-Witness";
const ACCOUNT_BODY_CONTEXT: &str = "account_id";
/// HTTP request types used for canonical signing.
pub use axum::http::{Method, Uri};

/// Canonical request freshness configuration.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalRequestAuthConfig {
    /// Maximum allowed clock skew for signed requests.
    pub max_clock_skew: Duration,
    /// TTL for nonces retained for replay detection; must exceed twice
    /// `max_clock_skew`.
    pub nonce_ttl: Duration,
    /// Maximum number of nonce entries held in memory for replay detection.
    pub replay_cache_capacity: NonZeroUsize,
}

/// Invalid relationship between canonical-request freshness and replay retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalRequestAuthConfigError;

impl fmt::Display for CanonicalRequestAuthConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "canonical request nonce TTL must be greater than twice the maximum clock skew",
        )
    }
}

impl std::error::Error for CanonicalRequestAuthConfigError {}

impl CanonicalRequestAuthConfig {
    /// Validate that a nonce cannot expire while its signed timestamp remains admissible.
    pub fn validate(self) -> Result<(), CanonicalRequestAuthConfigError> {
        let replay_window = self
            .max_clock_skew
            .checked_mul(2)
            .ok_or(CanonicalRequestAuthConfigError)?;
        if self.nonce_ttl <= replay_window {
            return Err(CanonicalRequestAuthConfigError);
        }
        Ok(())
    }
}

impl Default for CanonicalRequestAuthConfig {
    fn default() -> Self {
        Self {
            max_clock_skew: Duration::from_secs(defaults::torii::app_auth::MAX_CLOCK_SKEW_SECS),
            nonce_ttl: Duration::from_secs(defaults::torii::app_auth::NONCE_TTL_SECS),
            replay_cache_capacity: NonZeroUsize::new(
                defaults::torii::app_auth::REPLAY_CACHE_CAPACITY,
            )
            .expect("default app-auth replay cache capacity must be non-zero"),
        }
    }
}

impl From<&AppApiConfig> for CanonicalRequestAuthConfig {
    fn from(value: &AppApiConfig) -> Self {
        Self {
            max_clock_skew: value.request_signature_max_clock_skew,
            nonce_ttl: value.request_signature_nonce_ttl,
            replay_cache_capacity: value.request_signature_replay_cache_capacity,
        }
    }
}

#[derive(Debug)]
struct CanonicalRequestAuthRuntime {
    config: CanonicalRequestAuthConfig,
    replay_cache: Arc<ReplayCache>,
}

impl CanonicalRequestAuthRuntime {
    fn new(config: CanonicalRequestAuthConfig) -> Self {
        debug_assert!(config.validate().is_ok());
        Self {
            config,
            replay_cache: Arc::new(ReplayCache::new(
                config.nonce_ttl,
                config.replay_cache_capacity,
            )),
        }
    }
}

fn auth_runtime() -> &'static RwLock<CanonicalRequestAuthRuntime> {
    static STATE: OnceLock<RwLock<CanonicalRequestAuthRuntime>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(CanonicalRequestAuthRuntime::new(Default::default())))
}

fn auth_runtime_snapshot() -> (CanonicalRequestAuthConfig, Arc<ReplayCache>) {
    let guard = auth_runtime()
        .read()
        .expect("canonical request auth config lock");
    (guard.config, guard.replay_cache.clone())
}

/// Configure app-facing canonical request freshness enforcement.
///
/// # Errors
///
/// Returns an error when nonce retention is too short to cover the complete
/// accepted timestamp-skew window.
pub fn configure(
    config: CanonicalRequestAuthConfig,
) -> Result<(), CanonicalRequestAuthConfigError> {
    config.validate()?;
    let mut guard = auth_runtime()
        .write()
        .expect("canonical request auth config lock");
    guard.config = config;
    guard
        .replay_cache
        .configure(config.nonce_ttl, config.replay_cache_capacity);
    Ok(())
}

/// Authenticated canonical request identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCanonicalRequest {
    /// Account declared in the canonical request authentication material.
    pub account: AccountId,
    /// Exact account controller key that verified the request signature.
    pub signer: PublicKey,
    /// Full signer set that satisfied the request authorisation.
    pub verified_signers: Vec<PublicKey>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CanonicalRequestBodyProof<'a> {
    SignatureBase64(&'a str),
    WitnessBase64(&'a str),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CanonicalRequestBodyAuth<'a> {
    pub(crate) account_id: &'a str,
    pub(crate) timestamp_ms: u64,
    pub(crate) nonce: &'a str,
    pub(crate) proof: CanonicalRequestBodyProof<'a>,
}

/// Canonicalise a raw query string by decoding, sorting, and re-encoding.
#[must_use]
pub fn canonical_query_string(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return String::new();
    };
    if raw.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(raw.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        serializer.append_pair(&k, &v);
    }
    serializer.finish()
}

/// Construct canonical request bytes for signing.
#[must_use]
pub fn canonical_request_message(method: &Method, uri: &Uri, body: &[u8]) -> Vec<u8> {
    let query = canonical_query_string(uri.query());
    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash = hasher.finalize();
    format!(
        "{}\n{}\n{}\n{}",
        method.as_str().to_ascii_uppercase(),
        uri.path(),
        query,
        hex::encode(body_hash)
    )
    .into_bytes()
}

/// Construct exact-network canonical request bytes for signing.
#[must_use]
pub fn canonical_network_request_message(
    network_id: &NetworkId,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Vec<u8> {
    const DOMAIN: &[u8] = b"iroha.app.request.network.v1\0";
    let request = canonical_request_message(method, uri, body);
    let mut message =
        Vec::with_capacity(DOMAIN.len() + network_id.as_bytes().len() + request.len());
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(network_id.as_bytes());
    message.extend_from_slice(&request);
    message
}

/// Hash the canonical request bytes used by witness verification.
#[must_use]
pub fn canonical_request_hash(method: &Method, uri: &Uri, body: &[u8]) -> Hash {
    Hash::new(canonical_request_message(method, uri, body))
}

/// Hash an exact-network canonical request for a multisig witness.
#[must_use]
pub fn canonical_network_request_hash(
    network_id: &NetworkId,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Hash {
    Hash::new(canonical_network_request_message(
        network_id, method, uri, body,
    ))
}

/// Construct canonical request bytes for signature verification with freshness metadata.
#[must_use]
pub fn canonical_request_signature_message(
    method: &Method,
    uri: &Uri,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Vec<u8> {
    let mut msg = canonical_request_message(method, uri, body);
    msg.push(b'\n');
    msg.extend_from_slice(timestamp_ms.to_string().as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(nonce.as_bytes());
    msg
}

/// Construct exact-network canonical request bytes with freshness metadata.
#[must_use]
pub fn canonical_network_request_signature_message(
    network_id: &NetworkId,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Vec<u8> {
    let mut msg = canonical_network_request_message(network_id, method, uri, body);
    msg.push(b'\n');
    msg.extend_from_slice(timestamp_ms.to_string().as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(nonce.as_bytes());
    msg
}

fn request_hash_for_network(
    network_id: Option<&NetworkId>,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Hash {
    network_id.map_or_else(
        || canonical_request_hash(method, uri, body),
        |network_id| canonical_network_request_hash(network_id, method, uri, body),
    )
}

fn request_signature_message_for_network(
    network_id: Option<&NetworkId>,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Vec<u8> {
    network_id.map_or_else(
        || canonical_request_signature_message(method, uri, body, timestamp_ms, nonce),
        |network_id| {
            canonical_network_request_signature_message(
                network_id,
                method,
                uri,
                body,
                timestamp_ms,
                nonce,
            )
        },
    )
}

/// Encode a signature payload for use in `X-Iroha-Signature` headers.
#[must_use]
pub fn signature_header_value(signature: &Signature) -> String {
    BASE64_STANDARD.encode(signature.payload())
}

#[derive(Encode)]
struct CanonicalRequestWitnessPayloadV1 {
    schema_version: u16,
    subject_account: AccountId,
    timestamp_ms: u64,
    nonce: String,
    canonical_request_hash: Hash,
}

/// Construct the signed payload for a canonical request witness.
///
/// The payload binds the witness to the subject account, freshness fields, and
/// reconstructed canonical request hash. Individual signatures are supplied
/// separately in [`CanonicalRequestWitnessV1::signatures`].
///
/// # Errors
/// Returns [`norito::Error`] when witness encoding fails.
pub fn canonical_request_witness_message(
    witness: &CanonicalRequestWitnessV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::to_bytes(&CanonicalRequestWitnessPayloadV1 {
        schema_version: witness.schema_version,
        subject_account: witness.subject_account.clone(),
        timestamp_ms: witness.timestamp_ms,
        nonce: witness.nonce.clone(),
        canonical_request_hash: witness.canonical_request_hash,
    })
}

/// Encode a multisig witness payload for use in `X-Iroha-Witness` headers.
///
/// # Errors
/// Returns [`norito::Error`] when witness encoding fails.
pub fn witness_header_value(witness: &CanonicalRequestWitnessV1) -> Result<String, norito::Error> {
    norito::to_bytes(witness).map(|bytes| BASE64_STANDARD.encode(bytes))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn parse_required_header_text(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<String, crate::Error> {
    let value = headers.get(name).ok_or_else(|| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "missing required canonical request header `{name}`"
        )))
    })?;
    let value = std::str::from_utf8(value.as_bytes())
        .map(str::trim)
        .map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(format!(
                "invalid canonical request header `{name}`"
            )))
        })?;
    if value.is_empty() {
        Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid canonical request header `{name}`"
        ))))
    } else {
        Ok(value.to_owned())
    }
}

fn parse_required_header_exact_text(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<String, crate::Error> {
    let value = headers.get(name).ok_or_else(|| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "missing required canonical request header `{name}`"
        )))
    })?;
    let value = std::str::from_utf8(value.as_bytes()).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid canonical request header `{name}`"
        )))
    })?;
    if value.is_empty() {
        Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid canonical request header `{name}`"
        ))))
    } else {
        Ok(value.to_owned())
    }
}

fn parse_account_header_value(
    state: &Arc<CoreState>,
    account_literal: &str,
) -> Result<AccountId, crate::Error> {
    fn invalid_account_header() -> crate::Error {
        crate::Error::Query(ValidationFail::NotPermitted(
            "invalid X-Iroha-Account value".to_owned(),
        ))
    }

    if account_literal.is_empty() || account_literal.trim() != account_literal {
        return Err(invalid_account_header());
    }

    if let Ok(parsed) = AccountId::parse_encoded(account_literal) {
        let (account_id, canonical, _) = parsed.into_parts();
        if canonical != account_literal {
            return Err(invalid_account_header());
        }
        return Ok(account_id);
    }

    // Browser Fetch cannot transport the Kana-bearing I105 spelling in a
    // header. App authentication therefore has one deliberately narrower
    // alias exception: resolve the exact active ASCII alias only to establish
    // which controller must verify the request signature. User-directed alias
    // lookup remains permissioned independently.
    if !account_literal.is_ascii() {
        return Err(invalid_account_header());
    }
    let nexus = state.nexus_snapshot();
    let alias = AccountAlias::from_literal(account_literal, &nexus.dataspace_catalog)
        .map_err(|_| invalid_account_header())?;
    let canonical = alias
        .to_literal(&nexus.dataspace_catalog)
        .map_err(|_| invalid_account_header())?;
    if canonical != account_literal {
        return Err(invalid_account_header());
    }
    let now_ms = state
        .latest_block_header_fast()
        .map(|header| header.creation_time_ms)
        .unwrap_or(0);
    let world = state.world_view();
    resolve_active_account_alias(&world, &nexus.dataspace_catalog, &alias, now_ms)
        .ok_or_else(invalid_account_header)
}

fn parse_account_body_value(
    state: &Arc<CoreState>,
    account_literal: &str,
) -> Result<AccountId, crate::Error> {
    parse_account_literal_value(
        state,
        account_literal,
        ACCOUNT_BODY_CONTEXT,
        "invalid account_id value",
    )
}

fn parse_account_literal_value(
    state: &Arc<CoreState>,
    account_literal: &str,
    context: &'static str,
    invalid_message: &'static str,
) -> Result<AccountId, crate::Error> {
    crate::routing::parse_account_literal_with_state(
        state.as_ref(),
        account_literal.trim(),
        &crate::routing::MaybeTelemetry::disabled(),
        context,
    )
    .map(|(account_id, _)| account_id)
    .map_err(|_| crate::Error::Query(ValidationFail::NotPermitted(invalid_message.to_owned())))
}

fn validate_freshness(
    config: &CanonicalRequestAuthConfig,
    timestamp_ms: u64,
    nonce: &str,
    nonce_context: &'static str,
) -> Result<(), crate::Error> {
    let delta_ms = now_unix_ms().abs_diff(timestamp_ms);
    let max_skew_ms: u64 = config
        .max_clock_skew
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    if delta_ms > max_skew_ms {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "request timestamp outside allowed skew window".to_owned(),
        )));
    }
    if nonce.len() > 256
        || !nonce.is_ascii()
        || nonce.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid {nonce_context} value"
        ))));
    }
    Ok(())
}

fn decode_signature_bytes_value(
    signature_b64: &str,
    context: &'static str,
) -> Result<Vec<u8>, crate::Error> {
    let signature_bytes = BASE64_STANDARD.decode(signature_b64).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid base64 in {context}"
        )))
    })?;
    if BASE64_STANDARD.encode(&signature_bytes) != signature_b64 {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
            "noncanonical base64 in {context}"
        ))));
    }
    Ok(signature_bytes)
}

fn checked_app_auth_signature_from_bytes(
    signature_bytes: &[u8],
    signer: &PublicKey,
    signature_context: &'static str,
) -> Result<Signature, crate::Error> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature_bytes).map_err(|err| {
                crate::Error::Query(ValidationFail::NotPermitted(format!(
                    "invalid {signature_context}: Ed25519 signature failed admission: {err}"
                )))
            })
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature_bytes).map_err(|err| {
                crate::Error::Query(ValidationFail::NotPermitted(format!(
                    "invalid {signature_context}: ML-DSA signature failed admission: {err}"
                )))
            })
        }
        _ => Signature::try_from_bytes(signature_bytes).map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(format!(
                "invalid {signature_context}"
            )))
        }),
    }
}

fn verify_app_auth_signature(
    signature: &Signature,
    signer: &PublicKey,
    message: &[u8],
    signature_context: &'static str,
) -> Result<(), crate::Error> {
    validate_app_auth_signature_for_signer(signature, signer, signature_context)?;
    signature.verify(signer, message).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(
            "query signature failed verification".to_owned(),
        ))
    })
}

fn validate_app_auth_signature_for_signer(
    signature: &Signature,
    signer: &PublicKey,
    signature_context: &'static str,
) -> Result<(), crate::Error> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => iroha_crypto::ed25519_parse_signature(signature.payload())
            .map(|_| ())
            .map_err(|err| {
                crate::Error::Query(ValidationFail::NotPermitted(format!(
                    "invalid {signature_context}: Ed25519 signature failed admission: {err}"
                )))
            }),
        Ok(Algorithm::MlDsa) => iroha_crypto::mldsa65_parse_signature(signature.payload())
            .map(|_| ())
            .map_err(|err| {
                crate::Error::Query(ValidationFail::NotPermitted(format!(
                    "invalid {signature_context}: ML-DSA signature failed admission: {err}"
                )))
            }),
        _ => Ok(()),
    }
}

fn decode_witness_value(
    witness_b64: &str,
    context: &'static str,
) -> Result<CanonicalRequestWitnessV1, crate::Error> {
    let witness_bytes = BASE64_STANDARD.decode(witness_b64.trim()).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid base64 in {context}"
        )))
    })?;
    let witness: CanonicalRequestWitnessV1 =
        norito::decode_from_bytes(&witness_bytes).map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(format!(
                "invalid {context} payload"
            )))
        })?;
    if witness.schema_version != CANONICAL_REQUEST_WITNESS_VERSION_V1 {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
            "unsupported {context} schema_version `{}`",
            witness.schema_version
        ))));
    }
    Ok(witness)
}

fn verify_single_signature_authorization(
    state: &Arc<CoreState>,
    account: &AccountId,
    signature_bytes: &[u8],
    message: &[u8],
    signature_context: &'static str,
) -> Result<PublicKey, crate::Error> {
    let world = state.world_view();
    let account_entry = world.account(account).map_err(|_| {
        crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Account(account.clone()),
        )))
    })?;

    match account_entry.id.controller() {
        AccountController::Single(pk) => {
            let signature =
                checked_app_auth_signature_from_bytes(signature_bytes, pk, signature_context)?;
            verify_app_auth_signature(&signature, pk, message, signature_context)?;
            Ok(pk.clone())
        }
        AccountController::Multisig(_) => Err(crate::Error::Query(ValidationFail::NotPermitted(
            "multisig accounts must use X-Iroha-Witness".to_owned(),
        ))),
    }
}

fn verify_multisig_witness_authorization(
    state: &Arc<CoreState>,
    account: &AccountId,
    witness: &CanonicalRequestWitnessV1,
    witness_context: &'static str,
) -> Result<Vec<PublicKey>, crate::Error> {
    let message = canonical_request_witness_message(witness).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid {witness_context} payload"
        )))
    })?;

    let world = state.world_view();
    let account_entry = world.account(account).map_err(|_| {
        crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Account(account.clone()),
        )))
    })?;
    match account_entry.id.controller() {
        AccountController::Single(_) => Err(crate::Error::Query(ValidationFail::NotPermitted(
            "single-signature accounts must use X-Iroha-Signature".to_owned(),
        ))),
        AccountController::Multisig(policy) => {
            if witness.signatures.is_empty() {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
                    "{witness_context} must include at least one signature"
                ))));
            }

            let member_weights: BTreeMap<PublicKey, u16> = policy
                .members()
                .iter()
                .map(|member| (member.public_key().clone(), member.weight()))
                .collect();
            let mut seen = BTreeSet::new();
            let mut total_weight = 0_u32;
            let mut verified_signers = Vec::with_capacity(witness.signatures.len());

            for CanonicalRequestSignatureWitnessV1 { signer, signature } in &witness.signatures {
                if !seen.insert(signer.clone()) {
                    return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
                        "{witness_context} contains duplicate signer keys"
                    ))));
                }
                let Some(weight) = member_weights.get(signer) else {
                    return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
                        "{witness_context} includes a signer outside the account multisig policy"
                    ))));
                };
                verify_app_auth_signature(
                    signature,
                    signer,
                    &message,
                    "canonical body witness signature payload",
                )?;
                total_weight = total_weight.saturating_add(u32::from(*weight));
                verified_signers.push(signer.clone());
            }
            if total_weight < u32::from(policy.threshold()) {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
                    "{witness_context} signatures do not satisfy multisig threshold"
                ))));
            }
            Ok(verified_signers)
        }
    }
}

fn validate_expected_account(
    expected_account: Option<&AccountId>,
    account: &AccountId,
) -> Result<(), crate::Error> {
    if let Some(expected) = expected_account
        && expected != account
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "signed account does not match request path".to_owned(),
        )));
    }
    Ok(())
}

fn check_replay(
    account: &AccountId,
    nonce: &str,
    replay_cache: &ReplayCache,
) -> Result<(), crate::Error> {
    let replay_key = format!("{account}:{nonce}");
    match replay_cache.check_and_insert(replay_key) {
        Ok(()) => Ok(()),
        Err(ReplayInsertError::Replay) => Err(crate::Error::Query(ValidationFail::NotPermitted(
            "request nonce already used".to_owned(),
        ))),
        Err(ReplayInsertError::Capacity | ReplayInsertError::LifetimeOverflow) => {
            Err(crate::Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )))
        }
    }
}

/// Validate an iterable query against the executor on behalf of `authority`.
pub fn validate_iter_query_for_authority<Q>(
    state: &Arc<CoreState>,
    authority: &AccountId,
    query: Q,
) -> Result<(), crate::Error>
where
    Q: Query + 'static,
    Q::Item:
        HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
    Q: norito::codec::Encode,
{
    use iroha_core::smartcontracts::isi::query::{
        QueryLimits, validate_fresh_query_for_client_world_parts,
    };

    let payload = norito::codec::Encode::encode(&query);
    let qbox: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Q::Item>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        payload,
    ));
    let iter = QueryWithParams::new(&qbox, QueryParams::default());
    let request = QueryRequest::Start(iter);
    let limits = QueryLimits::new(crate::routing::app_query_limits().max_fetch_size);
    let world = state.world_view();
    let latest_block = state.latest_block_header_fast();
    validate_fresh_query_for_client_world_parts(request, authority, &world, latest_block, limits)
        .map_err(crate::Error::Query)
}

pub(crate) fn verify_canonical_body_request(
    state: &Arc<CoreState>,
    auth: CanonicalRequestBodyAuth<'_>,
    method: &Method,
    uri: &Uri,
    unsigned_body: &[u8],
    expected_account: Option<&AccountId>,
) -> Result<VerifiedCanonicalRequest, crate::Error> {
    let account = parse_account_body_value(state, auth.account_id)?;
    validate_expected_account(expected_account, &account)?;

    let (auth_config, replay_cache) = auth_runtime_snapshot();
    validate_freshness(&auth_config, auth.timestamp_ms, auth.nonce, "nonce")?;

    match auth.proof {
        CanonicalRequestBodyProof::SignatureBase64(signature_b64) => {
            let signature_bytes = decode_signature_bytes_value(signature_b64, "signature_base64")?;
            let message = canonical_request_signature_message(
                method,
                uri,
                unsigned_body,
                auth.timestamp_ms,
                auth.nonce,
            );
            let signer = verify_single_signature_authorization(
                state,
                &account,
                &signature_bytes,
                &message,
                "signature_base64 payload",
            )?;
            check_replay(&account, auth.nonce, &replay_cache)?;
            Ok(VerifiedCanonicalRequest {
                account,
                signer: signer.clone(),
                verified_signers: vec![signer],
            })
        }
        CanonicalRequestBodyProof::WitnessBase64(witness_b64) => {
            let witness = decode_witness_value(witness_b64, "witness_base64")?;
            if witness.subject_account != account {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "account_id does not match witness_base64 subject_account".to_owned(),
                )));
            }
            if witness.timestamp_ms != auth.timestamp_ms {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "timestamp_ms does not match witness_base64 timestamp_ms".to_owned(),
                )));
            }
            if witness.nonce != auth.nonce {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "nonce does not match witness_base64 nonce".to_owned(),
                )));
            }

            let expected_hash = canonical_request_hash(method, uri, unsigned_body);
            if witness.canonical_request_hash != expected_hash {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "witness_base64 canonical request hash mismatch".to_owned(),
                )));
            }

            let verified_signers =
                verify_multisig_witness_authorization(state, &account, &witness, "witness_base64")?;
            check_replay(&account, auth.nonce, &replay_cache)?;
            let signer = verified_signers
                .first()
                .cloned()
                .expect("non-empty witness signer set");
            Ok(VerifiedCanonicalRequest {
                account,
                signer,
                verified_signers,
            })
        }
    }
}

/// Verify optional canonical request headers.
///
/// Returns `Ok(Some(identity))` when a signature is present and valid, `Ok(None)` when
/// no signing headers are provided, and an error when headers are malformed or verification fails.
pub fn verify_canonical_request(
    state: &Arc<CoreState>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    expected_account: Option<&AccountId>,
) -> Result<Option<VerifiedCanonicalRequest>, crate::Error> {
    verify_canonical_request_for_network(state, headers, method, uri, body, expected_account, None)
}

/// Verify required canonical request headers against an exact network identity.
///
/// A signature produced for a different genesis-derived [`NetworkId`] cannot
/// authenticate on this network even when every HTTP field and body byte is identical.
pub fn verify_canonical_network_request(
    state: &Arc<CoreState>,
    network_id: &NetworkId,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    expected_account: Option<&AccountId>,
) -> Result<Option<VerifiedCanonicalRequest>, crate::Error> {
    verify_canonical_request_for_network(
        state,
        headers,
        method,
        uri,
        body,
        expected_account,
        Some(network_id),
    )
}

fn verify_canonical_request_for_network(
    state: &Arc<CoreState>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    expected_account: Option<&AccountId>,
    network_id: Option<&NetworkId>,
) -> Result<Option<VerifiedCanonicalRequest>, crate::Error> {
    let account_hdr = headers.get(HEADER_ACCOUNT);
    let signature_hdr = headers.get(HEADER_SIGNATURE);
    let timestamp_hdr = headers.get(HEADER_TIMESTAMP_MS);
    let nonce_hdr = headers.get(HEADER_NONCE);
    let witness_hdr = headers.get(HEADER_WITNESS);
    let all_missing = account_hdr.is_none()
        && signature_hdr.is_none()
        && timestamp_hdr.is_none()
        && nonce_hdr.is_none()
        && witness_hdr.is_none();
    if all_missing {
        return Ok(None);
    }
    if witness_hdr.is_some() {
        if signature_hdr.is_some() || timestamp_hdr.is_some() || nonce_hdr.is_some() {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "X-Iroha-Witness must not be combined with X-Iroha-Signature, X-Iroha-Timestamp-Ms, or X-Iroha-Nonce".to_owned(),
            )));
        }

        let witness_b64 = parse_required_header_text(headers, HEADER_WITNESS)?;
        let witness_bytes = BASE64_STANDARD.decode(witness_b64.trim()).map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid base64 in X-Iroha-Witness".to_owned(),
            ))
        })?;
        let witness: CanonicalRequestWitnessV1 = norito::decode_from_bytes(&witness_bytes)
            .map_err(|_| {
                crate::Error::Query(ValidationFail::NotPermitted(
                    "invalid X-Iroha-Witness payload".to_owned(),
                ))
            })?;
        if witness.schema_version != CANONICAL_REQUEST_WITNESS_VERSION_V1 {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
                "unsupported X-Iroha-Witness schema_version `{}`",
                witness.schema_version
            ))));
        }
        let account = if account_hdr.is_some() {
            let account_literal = parse_required_header_exact_text(headers, HEADER_ACCOUNT)?;
            let account = parse_account_header_value(state, &account_literal)?;
            if account != witness.subject_account {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "X-Iroha-Account does not match X-Iroha-Witness subject_account".to_owned(),
                )));
            }
            account
        } else {
            witness.subject_account.clone()
        };

        if let Some(expected) = expected_account
            && expected != &account
        {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "signed account does not match request path".to_owned(),
            )));
        }

        let (auth_config, replay_cache) = auth_runtime_snapshot();
        validate_freshness(
            &auth_config,
            witness.timestamp_ms,
            &witness.nonce,
            "X-Iroha-Nonce",
        )?;

        let expected_hash = request_hash_for_network(network_id, method, uri, body);
        if witness.canonical_request_hash != expected_hash {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "X-Iroha-Witness canonical request hash mismatch".to_owned(),
            )));
        }
        let message = canonical_request_witness_message(&witness).map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid X-Iroha-Witness payload".to_owned(),
            ))
        })?;

        let world = state.world_view();
        let account_entry = world.account(&account).map_err(|_| {
            crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                FindError::Account(account.clone()),
            )))
        })?;
        let verified_signers = match account_entry.id.controller() {
            AccountController::Single(_) => {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "single-signature accounts must use X-Iroha-Signature".to_owned(),
                )));
            }
            AccountController::Multisig(policy) => {
                if witness.signatures.is_empty() {
                    return Err(crate::Error::Query(ValidationFail::NotPermitted(
                        "X-Iroha-Witness must include at least one signature".to_owned(),
                    )));
                }

                let member_weights: BTreeMap<PublicKey, u16> = policy
                    .members()
                    .iter()
                    .map(|member| (member.public_key().clone(), member.weight()))
                    .collect();
                let mut seen = BTreeSet::new();
                let mut total_weight = 0_u32;
                let mut verified_signers = Vec::with_capacity(witness.signatures.len());

                for CanonicalRequestSignatureWitnessV1 { signer, signature } in &witness.signatures
                {
                    if !seen.insert(signer.clone()) {
                        return Err(crate::Error::Query(ValidationFail::NotPermitted(
                            "X-Iroha-Witness contains duplicate signer keys".to_owned(),
                        )));
                    }
                    let Some(weight) = member_weights.get(signer) else {
                        return Err(crate::Error::Query(ValidationFail::NotPermitted(
                            "X-Iroha-Witness includes a signer outside the account multisig policy"
                                .to_owned(),
                        )));
                    };
                    verify_app_auth_signature(
                        signature,
                        signer,
                        &message,
                        "X-Iroha-Witness signature payload",
                    )?;
                    total_weight = total_weight.saturating_add(u32::from(*weight));
                    verified_signers.push(signer.clone());
                }
                if total_weight < u32::from(policy.threshold()) {
                    return Err(crate::Error::Query(ValidationFail::NotPermitted(
                        "X-Iroha-Witness signatures do not satisfy multisig threshold".to_owned(),
                    )));
                }
                verified_signers
            }
        };

        check_replay(&account, &witness.nonce, &replay_cache)?;
        let signer = verified_signers
            .first()
            .cloned()
            .expect("non-empty witness signer set");
        return Ok(Some(VerifiedCanonicalRequest {
            account,
            signer,
            verified_signers,
        }));
    }

    if account_hdr.is_none()
        || signature_hdr.is_none()
        || timestamp_hdr.is_none()
        || nonce_hdr.is_none()
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "X-Iroha-Account, X-Iroha-Signature, X-Iroha-Timestamp-Ms, and X-Iroha-Nonce must be set together".to_owned(),
        )));
    };

    let account_literal = parse_required_header_exact_text(headers, HEADER_ACCOUNT)?;
    let account = parse_account_header_value(state, &account_literal)?;

    if let Some(expected) = expected_account
        && expected != &account
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "signed account does not match request path".to_owned(),
        )));
    }

    let timestamp_ms = parse_required_header_text(headers, HEADER_TIMESTAMP_MS)?
        .parse::<u64>()
        .map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid X-Iroha-Timestamp-Ms value".to_owned(),
            ))
        })?;
    let nonce = parse_required_header_text(headers, HEADER_NONCE)?;
    let (auth_config, replay_cache) = auth_runtime_snapshot();
    validate_freshness(&auth_config, timestamp_ms, &nonce, "X-Iroha-Nonce")?;

    let signature_b64 = parse_required_header_exact_text(headers, HEADER_SIGNATURE)?;
    let signature_bytes = decode_signature_bytes_value(&signature_b64, "X-Iroha-Signature")?;
    let message =
        request_signature_message_for_network(network_id, method, uri, body, timestamp_ms, &nonce);

    let world = state.world_view();
    let account_entry = world.account(&account).map_err(|_| {
        crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Account(account.clone()),
        )))
    })?;

    let signer = match account_entry.id.controller() {
        AccountController::Single(pk) => {
            let signature = checked_app_auth_signature_from_bytes(
                &signature_bytes,
                pk,
                "X-Iroha-Signature payload",
            )?;
            verify_app_auth_signature(&signature, pk, &message, "X-Iroha-Signature payload")?;
            pk.clone()
        }
        AccountController::Multisig(_) => {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "multisig accounts must use X-Iroha-Witness".to_owned(),
            )));
        }
    };
    check_replay(&account, &nonce, &replay_cache)?;

    Ok(Some(VerifiedCanonicalRequest {
        account,
        signer: signer.clone(),
        verified_signers: vec![signer],
    }))
}

/// Verify canonical request headers for the fee-quote endpoint.
///
/// Normal world-state-backed authentication always takes precedence. When the exact canonical
/// single-key account named by `X-Iroha-Account` is not registered yet, this endpoint alone may
/// authenticate it from the key embedded in its account id, provided the quoted payload's first
/// instruction self-registers that same authority. Aliases, multisig controllers, and other
/// endpoints never enter this fallback.
#[cfg(feature = "app_api")]
pub(crate) fn verify_fee_quote_canonical_request(
    state: &Arc<CoreState>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Result<Option<VerifiedCanonicalRequest>, crate::Error> {
    let normal_error = match verify_canonical_request(state, headers, method, uri, body, None) {
        Ok(verified) => return Ok(verified),
        Err(error) => error,
    };

    if method != Method::POST || uri.path() != "/v1/fees/quote" || uri.query().is_some() {
        return Err(normal_error);
    }
    if headers.get(HEADER_WITNESS).is_some() {
        return Err(normal_error);
    }

    let account_literal = match parse_required_header_exact_text(headers, HEADER_ACCOUNT) {
        Ok(account_literal) => account_literal,
        Err(_) => return Err(normal_error),
    };
    let parsed = match AccountId::parse_encoded(&account_literal) {
        Ok(parsed) => parsed,
        Err(_) => return Err(normal_error),
    };
    let (account, canonical, _) = parsed.into_parts();
    if canonical != account_literal {
        return Err(normal_error);
    }
    let signer = match account.controller() {
        AccountController::Single(signer) => signer.clone(),
        AccountController::Multisig(_) => return Err(normal_error),
    };

    // A materialised account must always use its world-state controller and normal account
    // authentication, even when the request body happens to contain a registration instruction.
    if state.world_view().account(&account).is_ok() {
        return Err(normal_error);
    }

    let timestamp_ms = parse_required_header_text(headers, HEADER_TIMESTAMP_MS)?
        .parse::<u64>()
        .map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid X-Iroha-Timestamp-Ms value".to_owned(),
            ))
        })?;
    let nonce = parse_required_header_text(headers, HEADER_NONCE)?;
    let (auth_config, replay_cache) = auth_runtime_snapshot();
    validate_freshness(&auth_config, timestamp_ms, &nonce, "X-Iroha-Nonce")?;

    let signature_b64 = parse_required_header_exact_text(headers, HEADER_SIGNATURE)?;
    let signature_bytes = decode_signature_bytes_value(&signature_b64, "X-Iroha-Signature")?;
    let signature = checked_app_auth_signature_from_bytes(
        &signature_bytes,
        &signer,
        "X-Iroha-Signature payload",
    )?;
    let message = canonical_request_signature_message(method, uri, body, timestamp_ms, &nonce);
    verify_app_auth_signature(&signature, &signer, &message, "X-Iroha-Signature payload")?;

    // Decode only after proving the raw request bytes. Malformed bodies therefore cannot bypass
    // authentication, and they do not qualify an absent authority for this endpoint exception.
    let request: FeeQuoteRequest = match norito::json::from_slice(body) {
        Ok(request) => request,
        Err(_) => return Err(normal_error),
    };
    if request.payload.authority != account
        || !iroha_core::tx::executable_self_registers_authority(
            &request.payload.instructions,
            &account,
        )
    {
        return Err(normal_error);
    }

    check_replay(&account, &nonce, &replay_cache)?;
    Ok(Some(VerifiedCanonicalRequest {
        account,
        signer: signer.clone(),
        verified_signers: vec![signer],
    }))
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use axum::http::Uri;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute as _,
        state::{State, StateReadOnly, World},
        sumeragi::network_topology::Topology,
    };
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::{Account, AccountAddress, MultisigMember, MultisigPolicy},
        block::BlockHeader,
        domain::Domain,
        isi::Register,
        prelude::DomainId,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;

    use super::*;

    const TEST_ACCOUNT_I105: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
    const ED25519_SMALL_ORDER_POINT: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const ED25519_NONCANONICAL_IDENTITY: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn minimal_state_with_account(account: &AccountId) -> Arc<State> {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(account);
        let account_value = Account::new(account.clone()).build(account);
        Arc::new(State::new_for_testing(
            World::with([domain], [account_value], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ))
    }

    fn minimal_state_without_accounts() -> Arc<State> {
        Arc::new(State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ))
    }

    fn fee_quote_body(authority: &AccountId, account_to_register: &AccountId) -> Vec<u8> {
        let payload = TransactionBuilder::new(
            test_network_id(0x30),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Register::account(Account::new(account_to_register.clone()))])
        .into_payload()
        .expect("build self-registering fee quote payload");
        norito::json::to_vec(&FeeQuoteRequest { payload }).expect("encode fee quote request")
    }

    fn signed_headers_for_test(
        account: &AccountId,
        key_pair: &KeyPair,
        method: &Method,
        uri: &Uri,
        body: &[u8],
        nonce: &'static str,
    ) -> HeaderMap {
        let timestamp_ms = now_unix_ms();
        let message = canonical_request_signature_message(method, uri, body, timestamp_ms, nonce);
        let signature = checked_signature(key_pair.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            account
                .canonical_i105()
                .expect("canonical account header")
                .parse()
                .expect("valid account header"),
        );
        headers.insert(
            HEADER_SIGNATURE,
            signature_header_value(&signature)
                .parse()
                .expect("valid signature header"),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            timestamp_ms
                .to_string()
                .parse()
                .expect("valid timestamp header"),
        );
        headers.insert(HEADER_NONCE, nonce.parse().expect("valid nonce header"));
        headers
    }

    fn test_network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }

    fn signed_network_headers_for_test(
        network_id: &NetworkId,
        account: &AccountId,
        key_pair: &KeyPair,
        method: &Method,
        uri: &Uri,
        body: &[u8],
        nonce: &'static str,
    ) -> HeaderMap {
        let timestamp_ms = now_unix_ms();
        let message = canonical_network_request_signature_message(
            network_id,
            method,
            uri,
            body,
            timestamp_ms,
            nonce,
        );
        let signature = checked_signature(key_pair.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            account
                .canonical_i105()
                .expect("canonical account header")
                .parse()
                .expect("valid account header"),
        );
        headers.insert(
            HEADER_SIGNATURE,
            signature_header_value(&signature)
                .parse()
                .expect("valid signature header"),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            timestamp_ms
                .to_string()
                .parse()
                .expect("valid timestamp header"),
        );
        headers.insert(HEADER_NONCE, nonce.parse().expect("valid nonce header"));
        headers
    }

    fn assert_missing_account_rejection(error: crate::Error, expected: &AccountId) {
        match error {
            crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                FindError::Account(actual),
            ))) => assert_eq!(&actual, expected),
            other => panic!("expected missing-account authentication rejection, got {other:?}"),
        }
    }

    fn bind_account_alias_for_test(state: &Arc<State>, account_id: &AccountId, alias: &str) {
        let dataspace_catalog = state.nexus_snapshot().dataspace_catalog.clone();
        let label =
            iroha_data_model::account::rekey::AccountAlias::from_literal(alias, &dataspace_catalog)
                .expect("valid account alias");
        let selector = iroha_core::sns::selector_for_account_alias(&label, &dataspace_catalog)
            .expect("account alias selector");
        let account_address =
            AccountAddress::from_account_id(account_id).expect("address from account id");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            account_id.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(
                &account_address,
            )],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_data_model::metadata::Metadata::default(),
        );
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let world = tx.world_mut_for_testing();
        world
            .account_aliases_mut_for_testing()
            .insert(label.clone(), account_id.clone());
        let mut labels = world
            .account_aliases_by_account_mut_for_testing()
            .get(account_id)
            .cloned()
            .unwrap_or_default();
        labels.insert(label.clone());
        world
            .account_aliases_by_account_mut_for_testing()
            .insert(account_id.clone(), labels);
        world.account_rekey_records_mut_for_testing().insert(
            label.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(label, account_id.clone()),
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
        tx.apply();
        block.commit().expect("commit account alias for test");
    }

    #[cfg(test)]
    fn test_guard(config: CanonicalRequestAuthConfig) -> impl Drop {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        struct Guard(std::sync::MutexGuard<'static, ()>);
        impl Drop for Guard {
            fn drop(&mut self) {
                configure(CanonicalRequestAuthConfig::default()).expect("default app-auth config");
            }
        }
        let guard = TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        configure(config).expect("valid app-auth test config");
        Guard(guard)
    }

    #[cfg(test)]
    fn checked_signature(private_key: &iroha_crypto::PrivateKey, payload: &[u8]) -> Signature {
        Signature::try_new(private_key, payload).expect("test fixture signing should succeed")
    }

    fn noncanonical_standard_base64_pad_bit_alias(encoded: &str) -> String {
        assert!(
            encoded.ends_with("=="),
            "64-byte signatures encode with == padding"
        );
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut bytes = encoded.as_bytes().to_vec();
        let index = bytes.len() - 3;
        let value = ALPHABET
            .iter()
            .position(|byte| *byte == bytes[index])
            .expect("standard base64 alphabet");
        bytes[index] = ALPHABET[value ^ 0x01];
        String::from_utf8(bytes).expect("base64 alias remains ASCII")
    }

    fn checked_app_auth_key_fixture() -> KeyPair {
        KeyPair::try_random().expect("generate checked app auth fixture key")
    }

    #[test]
    fn app_auth_fixture_uses_checked_default_key_generation() {
        let key_pair = checked_app_auth_key_fixture();
        let actual = key_pair
            .public_key()
            .try_algorithm()
            .expect("app auth fixture key advertises a valid algorithm");

        assert_eq!(actual, Algorithm::default());
    }

    #[test]
    fn checked_app_auth_signature_from_bytes_rejects_malformed_ed25519_r_before_wrapping() {
        let valid_signature = checked_signature(
            ALICE_KEYPAIR.private_key(),
            b"app-auth checked signature admission helper",
        );

        for (label, replacement_r) in [
            ("small-order", ED25519_SMALL_ORDER_POINT),
            ("noncanonical", ED25519_NONCANONICAL_IDENTITY),
        ] {
            let mut payload = valid_signature.payload().to_vec();
            payload[..ed25519_dalek::PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);

            let err = checked_app_auth_signature_from_bytes(
                &payload,
                ALICE_KEYPAIR.public_key(),
                "helper signature payload",
            )
            .expect_err("malformed Ed25519 R must fail before opaque signature wrapping");

            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("helper signature payload")
                            && msg.contains("Ed25519 signature failed admission"),
                        "{label} signature failed with unexpected error: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn checked_app_auth_signature_from_bytes_rejects_malformed_mldsa_signature_lengths_before_wrapping()
     {
        let key_pair = KeyPair::try_random_with_algorithm(Algorithm::MlDsa)
            .expect("generate checked app auth ML-DSA fixture key");
        let valid_signature = checked_signature(
            key_pair.private_key(),
            b"app-auth checked ML-DSA signature admission helper",
        );

        let mut extended = valid_signature.payload().to_vec();
        extended.push(0);
        for (label, payload) in [
            (
                "truncated",
                valid_signature.payload()[..valid_signature.payload().len() - 1].to_vec(),
            ),
            ("extended", extended),
        ] {
            let err = checked_app_auth_signature_from_bytes(
                &payload,
                key_pair.public_key(),
                "helper ML-DSA signature payload",
            )
            .expect_err("malformed ML-DSA length must fail before opaque signature wrapping");

            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("helper ML-DSA signature payload")
                            && msg.contains("ML-DSA signature failed admission"),
                        "{label} signature failed with unexpected error: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn validate_app_auth_signature_for_signer_rejects_malformed_mldsa_signature_lengths() {
        let key_pair = KeyPair::try_random_with_algorithm(Algorithm::MlDsa)
            .expect("generate checked app auth ML-DSA validation fixture key");
        let valid_signature = checked_signature(
            key_pair.private_key(),
            b"app-auth wrapped ML-DSA signature admission helper",
        );

        let mut extended = valid_signature.payload().to_vec();
        extended.push(0);
        for (label, payload) in [
            (
                "truncated",
                valid_signature.payload()[..valid_signature.payload().len() - 1].to_vec(),
            ),
            ("extended", extended),
        ] {
            let signature = Signature::from_bytes(&payload);
            let err = validate_app_auth_signature_for_signer(
                &signature,
                key_pair.public_key(),
                "wrapped ML-DSA signature payload",
            )
            .expect_err("malformed wrapped ML-DSA length must fail admission");

            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("wrapped ML-DSA signature payload")
                            && msg.contains("ML-DSA signature failed admission"),
                        "{label} signature failed with unexpected error: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn canonical_query_sorting_is_stable() {
        let raw = "b=2&a=3&b=1&space=a+b";
        let canonical = canonical_query_string(Some(raw));
        assert_eq!(canonical, "a=3&b=1&b=2&space=a+b");
    }

    #[test]
    fn canonical_message_includes_body_hash() {
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=5")
            .parse()
            .expect("uri");
        let msg = canonical_request_message(&Method::GET, &uri, b"{\"foo\":1}");
        let rendered = String::from_utf8(msg).expect("utf8");
        assert!(rendered.contains(&format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets")));
        assert!(rendered.contains("limit=5"));
        assert!(
            rendered.ends_with("37a76343c8e3c695feeaadfe52329673ff129c65f99f55ae6056c9254f4c481d")
        );
    }

    #[test]
    fn exact_network_auth_rejects_wrong_network_and_replay() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let network_id = test_network_id(0x31);
        let wrong_network_id = test_network_id(0x32);
        let method = Method::POST;
        let uri: Uri = "/v1/da/ingest".parse().expect("DA ingest URI");
        let body = br#"{"payload_public_key":"self-declared-only"}"#;
        let headers = signed_network_headers_for_test(
            &network_id,
            &account,
            &ALICE_KEYPAIR,
            &method,
            &uri,
            body,
            "exact-network-replay",
        );

        verify_canonical_network_request(
            &state,
            &wrong_network_id,
            &headers,
            &method,
            &uri,
            body,
            None,
        )
        .expect_err("a request signed for a different genesis lineage must fail");

        let verified = verify_canonical_network_request(
            &state,
            &network_id,
            &headers,
            &method,
            &uri,
            body,
            None,
        )
        .expect("exact-network request verification")
        .expect("signed identity");
        assert_eq!(verified.account, account);

        let replay = verify_canonical_network_request(
            &state,
            &network_id,
            &headers,
            &method,
            &uri,
            body,
            None,
        )
        .expect_err("an accepted exact-network nonce must be one-shot");
        assert!(matches!(
            replay,
            crate::Error::Query(ValidationFail::NotPermitted(message))
                if message == "request nonce already used"
        ));
    }

    #[test]
    fn exact_network_auth_rejects_self_declared_unregistered_principal() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let key_pair = checked_app_auth_key_fixture();
        let self_declared = AccountId::new(key_pair.public_key().clone());
        let state = minimal_state_without_accounts();
        let network_id = test_network_id(0x41);
        let method = Method::POST;
        let uri: Uri = "/v1/da/ingest".parse().expect("DA ingest URI");
        let body = br#"{"payload_signature":"validity-does-not-establish-eligibility"}"#;
        let headers = signed_network_headers_for_test(
            &network_id,
            &self_declared,
            &key_pair,
            &method,
            &uri,
            body,
            "unregistered-self-declared-principal",
        );

        let error = verify_canonical_network_request(
            &state,
            &network_id,
            &headers,
            &method,
            &uri,
            body,
            None,
        )
        .expect_err("a payload-declared key is not an eligible on-ledger principal");
        assert_missing_account_rejection(error, &self_declared);
    }

    #[test]
    fn fee_quote_auth_accepts_exact_absent_self_registration_and_rejects_replay() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let key_pair = checked_app_auth_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let state = minimal_state_without_accounts();
        let method = Method::POST;
        let uri: Uri = "/v1/fees/quote".parse().expect("fee quote uri");
        let body = fee_quote_body(&authority, &authority);
        let headers = signed_headers_for_test(
            &authority,
            &key_pair,
            &method,
            &uri,
            &body,
            "fee-quote-self-register-success",
        );

        let verified = verify_fee_quote_canonical_request(&state, &headers, &method, &uri, &body)
            .expect("self-registering authority should authenticate")
            .expect("signed request identity");
        assert_eq!(verified.account, authority);
        assert_eq!(verified.signer, key_pair.public_key().clone());
        assert_eq!(
            verified.verified_signers,
            vec![key_pair.public_key().clone()]
        );

        let replay = verify_fee_quote_canonical_request(&state, &headers, &method, &uri, &body)
            .expect_err("accepted fallback must use the canonical replay cache");
        assert!(matches!(
            replay,
            crate::Error::Query(ValidationFail::NotPermitted(message))
                if message == "request nonce already used"
        ));
    }

    #[test]
    fn fee_quote_auth_fallback_is_limited_to_the_exact_self_registering_request() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let key_pair = checked_app_auth_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let other_key_pair = checked_app_auth_key_fixture();
        let other = AccountId::new(other_key_pair.public_key().clone());
        let state = minimal_state_without_accounts();
        let method = Method::POST;
        let quote_uri: Uri = "/v1/fees/quote".parse().expect("fee quote uri");

        let registers_other = fee_quote_body(&authority, &other);
        let headers = signed_headers_for_test(
            &authority,
            &key_pair,
            &method,
            &quote_uri,
            &registers_other,
            "fee-quote-registers-other",
        );
        let error = verify_fee_quote_canonical_request(
            &state,
            &headers,
            &method,
            &quote_uri,
            &registers_other,
        )
        .expect_err("registration of another account must not qualify");
        assert_missing_account_rejection(error, &authority);

        let mismatched_authority = fee_quote_body(&other, &other);
        let headers = signed_headers_for_test(
            &authority,
            &key_pair,
            &method,
            &quote_uri,
            &mismatched_authority,
            "fee-quote-mismatched-payload-authority",
        );
        let error = verify_fee_quote_canonical_request(
            &state,
            &headers,
            &method,
            &quote_uri,
            &mismatched_authority,
        )
        .expect_err("header and payload authorities must match");
        assert_missing_account_rejection(error, &authority);

        let multisig_policy = MultisigPolicy::new(
            1,
            vec![MultisigMember::new(key_pair.public_key().clone(), 1).expect("multisig member")],
        )
        .expect("multisig policy");
        let multisig_authority = AccountId::new_multisig(multisig_policy);
        let multisig_body = fee_quote_body(&multisig_authority, &multisig_authority);
        let headers = signed_headers_for_test(
            &multisig_authority,
            &key_pair,
            &method,
            &quote_uri,
            &multisig_body,
            "fee-quote-absent-multisig",
        );
        let error = verify_fee_quote_canonical_request(
            &state,
            &headers,
            &method,
            &quote_uri,
            &multisig_body,
        )
        .expect_err("absent multisig authority must require a materialised WSV policy");
        assert_missing_account_rejection(error, &multisig_authority);

        let correct_body = fee_quote_body(&authority, &authority);
        let wrong_key_headers = signed_headers_for_test(
            &authority,
            &other_key_pair,
            &method,
            &quote_uri,
            &correct_body,
            "fee-quote-wrong-embedded-controller",
        );
        let error = verify_fee_quote_canonical_request(
            &state,
            &wrong_key_headers,
            &method,
            &quote_uri,
            &correct_body,
        )
        .expect_err("embedded authority controller must verify the request");
        assert!(matches!(
            error,
            crate::Error::Query(ValidationFail::NotPermitted(message))
                if message == "query signature failed verification"
        ));

        let other_uri: Uri = "/v1/fee-sponsor-programs/by-id"
            .parse()
            .expect("other signed endpoint uri");
        let headers = signed_headers_for_test(
            &authority,
            &key_pair,
            &method,
            &other_uri,
            &correct_body,
            "fee-quote-fallback-other-endpoint",
        );
        let error = verify_fee_quote_canonical_request(
            &state,
            &headers,
            &method,
            &other_uri,
            &correct_body,
        )
        .expect_err("fallback must not broaden another endpoint");
        assert_missing_account_rejection(error, &authority);

        let malformed_body = b"{";
        let headers = signed_headers_for_test(
            &authority,
            &key_pair,
            &method,
            &quote_uri,
            malformed_body,
            "fee-quote-malformed-body-auth-first",
        );
        let error = verify_fee_quote_canonical_request(
            &state,
            &headers,
            &method,
            &quote_uri,
            malformed_body,
        )
        .expect_err("malformed body must not qualify an absent account");
        assert_missing_account_rejection(error, &authority);
    }

    #[test]
    fn fee_quote_auth_never_bypasses_a_registered_account_controller() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let key_pair = checked_app_auth_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let wrong_key_pair = checked_app_auth_key_fixture();
        let state = minimal_state_with_account(&authority);
        let method = Method::POST;
        let uri: Uri = "/v1/fees/quote".parse().expect("fee quote uri");
        let body = fee_quote_body(&authority, &authority);
        let headers = signed_headers_for_test(
            &authority,
            &wrong_key_pair,
            &method,
            &uri,
            &body,
            "fee-quote-registered-controller-wins",
        );

        let error = verify_fee_quote_canonical_request(&state, &headers, &method, &uri, &body)
            .expect_err("registered controller verification must not fall back");
        assert!(matches!(
            error,
            crate::Error::Query(ValidationFail::NotPermitted(message))
                if message == "query signature failed verification"
        ));
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=10")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "accept-valid-signature";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&account))
                .expect("verify");
        assert_eq!(
            verified,
            Some(VerifiedCanonicalRequest {
                account,
                signer: ALICE_KEYPAIR.public_key().clone(),
                verified_signers: vec![ALICE_KEYPAIR.public_key().clone()],
            })
        );
    }

    #[test]
    fn verify_rejects_noncanonical_signature_header_base64_text() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=10")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "reject-noncanonical-signature-header";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");

        for signature_b64 in [
            format!(" {} ", BASE64_STANDARD.encode(signature.payload())),
            noncanonical_standard_base64_pad_bit_alias(
                &BASE64_STANDARD.encode(signature.payload()),
            ),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                HEADER_ACCOUNT,
                axum::http::HeaderValue::from_str(&account_literal).unwrap(),
            );
            headers.insert(
                HEADER_SIGNATURE,
                axum::http::HeaderValue::from_str(&signature_b64).unwrap(),
            );
            headers.insert(
                HEADER_TIMESTAMP_MS,
                axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
            );
            headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

            let err =
                verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&account))
                    .expect_err("noncanonical signature header text must fail before verification");
            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("base64") && msg.contains("X-Iroha-Signature"),
                        "unexpected noncanonical signature header rejection: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn verify_accepts_alias_account_header_and_returns_canonical_i105_account() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        bind_account_alias_for_test(&state, &account, "wallet@universal");
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=10")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "accept-alias-account-header";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_static("wallet@universal"),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&account))
                .expect("verify");
        assert_eq!(
            verified,
            Some(VerifiedCanonicalRequest {
                account,
                signer: ALICE_KEYPAIR.public_key().clone(),
                verified_signers: vec![ALICE_KEYPAIR.public_key().clone()],
            })
        );
    }

    #[test]
    fn account_header_alias_requires_an_exact_active_ascii_binding() {
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        bind_account_alias_for_test(&state, &account, "wallet@universal");

        assert_eq!(
            parse_account_header_value(&state, "wallet@universal").expect("active alias"),
            account
        );
        for invalid in [
            " wallet@universal",
            "wallet@universal ",
            "Wallet@universal",
            "wallet@UNIVERSAL",
            "wallet",
            "wállét@universal",
            "missing@universal",
        ] {
            parse_account_header_value(&state, invalid)
                .expect_err("noncanonical or inactive account header alias must fail closed");
        }
    }

    #[test]
    fn verify_rejects_wrong_signature() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "wrong-signature";
        let bad_sig = checked_signature(checked_app_auth_key_fixture().private_key(), b"forged");
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(bad_sig.payload())).unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("signature"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_all_zero_signature_payload_before_backend() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "all-zero-signature";
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode([0u8; 64])).unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("inert signature payload must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("X-Iroha-Signature payload"),
                    "unexpected all-zero signature rejection: {msg}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_malformed_ed25519_signature_payload_before_backend() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let message =
            canonical_request_signature_message(&method, &uri, &[], timestamp_ms, "invalid-r");
        let valid_signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");

        for (label, replacement_r, expected) in [
            (
                "small-order",
                ED25519_SMALL_ORDER_POINT,
                "Ed25519 signature failed admission",
            ),
            (
                "noncanonical",
                ED25519_NONCANONICAL_IDENTITY,
                "Ed25519 signature failed admission",
            ),
        ] {
            let nonce = format!("invalid-r-{label}");
            let mut payload = valid_signature.payload().to_vec();
            payload[..ed25519_dalek::PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
            let mut headers = HeaderMap::new();
            headers.insert(
                HEADER_ACCOUNT,
                axum::http::HeaderValue::from_str(&account_literal).unwrap(),
            );
            headers.insert(
                HEADER_SIGNATURE,
                axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(&payload)).unwrap(),
            );
            headers.insert(
                HEADER_TIMESTAMP_MS,
                axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
            );
            headers.insert(
                HEADER_NONCE,
                axum::http::HeaderValue::from_str(&nonce).unwrap(),
            );

            let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
                .expect_err("malformed Ed25519 signature R must fail before backend verify");
            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("X-Iroha-Signature payload") && msg.contains(expected),
                        "{label} signature failed with unexpected error: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn body_auth_rejects_noncanonical_signature_base64_text() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/transactions".parse().expect("uri");
        let unsigned_body = br#"{"request":"refill"}"#;
        let timestamp_ms = now_unix_ms();
        let nonce = "body-auth-noncanonical-base64";
        let message =
            canonical_request_signature_message(&method, &uri, unsigned_body, timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");

        for signature_b64 in [
            format!(" {} ", BASE64_STANDARD.encode(signature.payload())),
            noncanonical_standard_base64_pad_bit_alias(
                &BASE64_STANDARD.encode(signature.payload()),
            ),
        ] {
            let auth = CanonicalRequestBodyAuth {
                account_id: &account_literal,
                timestamp_ms,
                nonce,
                proof: CanonicalRequestBodyProof::SignatureBase64(&signature_b64),
            };

            let err = verify_canonical_body_request(
                &state,
                auth,
                &method,
                &uri,
                unsigned_body,
                Some(&account),
            )
            .expect_err("noncanonical body signature base64 must fail before verification");
            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("base64") && msg.contains("signature_base64"),
                        "unexpected noncanonical body signature rejection: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn body_auth_rejects_malformed_ed25519_signature_payload_before_backend() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/transactions".parse().expect("uri");
        let unsigned_body = br#"{"request":"refill"}"#;
        let timestamp_ms = now_unix_ms();
        let nonce = "body-auth-invalid-r";
        let message =
            canonical_request_signature_message(&method, &uri, unsigned_body, timestamp_ms, nonce);
        let valid_signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");

        for (label, replacement_r, expected) in [
            (
                "small-order",
                ED25519_SMALL_ORDER_POINT,
                "Ed25519 signature failed admission",
            ),
            (
                "noncanonical",
                ED25519_NONCANONICAL_IDENTITY,
                "Ed25519 signature failed admission",
            ),
        ] {
            let mut payload = valid_signature.payload().to_vec();
            payload[..ed25519_dalek::PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
            let signature_b64 = BASE64_STANDARD.encode(&payload);
            let auth = CanonicalRequestBodyAuth {
                account_id: &account_literal,
                timestamp_ms,
                nonce,
                proof: CanonicalRequestBodyProof::SignatureBase64(&signature_b64),
            };

            let err = verify_canonical_body_request(
                &state,
                auth,
                &method,
                &uri,
                unsigned_body,
                Some(&account),
            )
            .expect_err("body malformed Ed25519 signature R must fail before backend verify");
            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("signature_base64 payload") && msg.contains(expected),
                        "{label} body signature failed with unexpected error: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn verify_rejects_mismatched_path_account() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let other: AccountId = AccountId::new(checked_app_auth_key_fixture().public_key().clone());
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "mismatched-path-account";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&other))
            .unwrap_err();
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("signed account does not match request path"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_missing_freshness_headers() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let message = canonical_request_message(&method, &uri, &[]);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("freshness headers must be required");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("must be set together"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_replayed_nonce() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "replayed-nonce";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect("first request must pass");
        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("replay must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("nonce already used"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn replay_cache_capacity_fails_closed_without_evicting_live_nonces() {
        let cache = ReplayCache::new(Duration::from_secs(300), nonzero!(2_usize));
        let account = ALICE_ID.clone();

        check_replay(&account, "protected-a", &cache).expect("first nonce");
        check_replay(&account, "protected-b", &cache).expect("second nonce");
        let saturated =
            check_replay(&account, "overflow", &cache).expect_err("full cache must reject");
        assert!(matches!(
            saturated,
            crate::Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
            ))
        ));

        let replay = check_replay(&account, "protected-a", &cache)
            .expect_err("capacity pressure must preserve the first nonce");
        match replay {
            crate::Error::Query(ValidationFail::NotPermitted(message)) => {
                assert!(message.contains("nonce already used"));
            }
            other => panic!("unexpected replay error: {other:?}"),
        }
    }

    #[test]
    fn app_auth_config_rejects_short_nonce_retention() {
        let config = CanonicalRequestAuthConfig {
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(120),
            replay_cache_capacity: nonzero!(8_usize),
        };

        assert_eq!(config.validate(), Err(CanonicalRequestAuthConfigError));
    }

    #[test]
    fn configure_preserves_replay_cache_entries() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = format!("configure-preserves-replay-cache-{timestamp_ms}");
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, &nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(
            HEADER_NONCE,
            axum::http::HeaderValue::from_str(&nonce).unwrap(),
        );

        verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect("first request must pass");
        configure(CanonicalRequestAuthConfig {
            max_clock_skew: Duration::from_secs(120),
            ..CanonicalRequestAuthConfig::default()
        })
        .expect("valid reconfigured app-auth window");
        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("replay must still fail after reconfigure");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("nonce already used"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_stale_timestamp() {
        let _guard = test_guard(CanonicalRequestAuthConfig {
            max_clock_skew: Duration::from_secs(1),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: nonzero!(128usize),
        });
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = 1;
        let nonce = "stale-timestamp";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_static("1"),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("stale request must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("timestamp outside allowed skew"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_multisig_account_signature() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = checked_app_auth_key_fixture();
        let signer_two = checked_app_auth_key_fixture();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "multisig-http-auth";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = checked_signature(signer_one.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("multisig app-auth must fail closed");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("X-Iroha-Witness"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn multisig_witness(
        account: &AccountId,
        method: &Method,
        uri: &Uri,
        body: &[u8],
        timestamp_ms: u64,
        nonce: &str,
        signers: &[&KeyPair],
    ) -> CanonicalRequestWitnessV1 {
        let mut witness = CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: account.clone(),
            timestamp_ms,
            nonce: nonce.to_owned(),
            canonical_request_hash: canonical_request_hash(method, uri, body),
            signatures: Vec::new(),
        };
        let message = canonical_request_witness_message(&witness).expect("witness payload");
        witness.signatures = signers
            .iter()
            .map(|signer| CanonicalRequestSignatureWitnessV1 {
                signer: signer.public_key().clone(),
                signature: checked_signature(signer.private_key(), &message),
            })
            .collect();
        witness
    }

    fn witness_headers(account: &AccountId, witness: &CanonicalRequestWitnessV1) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account.canonical_i105().expect("i105 account"))
                .expect("valid account header"),
        );
        headers.insert(
            HEADER_WITNESS,
            axum::http::HeaderValue::from_str(
                &witness_header_value(witness).expect("encode witness header"),
            )
            .expect("valid witness header"),
        );
        headers
    }

    #[test]
    fn verify_accepts_valid_multisig_witness() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = checked_app_auth_key_fixture();
        let signer_two = checked_app_auth_key_fixture();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy?view=full".parse().expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "valid-multisig-witness";
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{\"deploy\":true}",
            timestamp_ms,
            nonce,
            &[&signer_one, &signer_two],
        );
        let headers = witness_headers(&account, &witness);

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, b"{\"deploy\":true}", None)
                .expect("verify")
                .expect("witness auth must be present");
        assert_eq!(verified.account, account);
        assert_eq!(verified.signer, signer_one.public_key().clone());
        assert_eq!(
            verified.verified_signers,
            vec![
                signer_one.public_key().clone(),
                signer_two.public_key().clone()
            ]
        );
    }

    #[test]
    fn verify_rejects_malformed_multisig_witness_signature_r_before_backend() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = checked_app_auth_key_fixture();
        let policy = MultisigPolicy::new(
            1,
            vec![MultisigMember::new(signer_one.public_key().clone(), 1).expect("member")],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy?view=full".parse().expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "invalid-r-multisig-witness";

        for (label, replacement_r, expected) in [
            (
                "small-order",
                ED25519_SMALL_ORDER_POINT,
                "Ed25519 signature failed admission",
            ),
            (
                "noncanonical",
                ED25519_NONCANONICAL_IDENTITY,
                "Ed25519 signature failed admission",
            ),
        ] {
            let mut witness = multisig_witness(
                &account,
                &method,
                &uri,
                b"{\"deploy\":true}",
                timestamp_ms,
                nonce,
                &[&signer_one],
            );
            let mut payload = witness.signatures[0].signature.payload().to_vec();
            payload[..ed25519_dalek::PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
            witness.signatures[0].signature = Signature::from_bytes(&payload);
            let headers = witness_headers(&account, &witness);

            let err = verify_canonical_request(
                &state,
                &headers,
                &method,
                &uri,
                b"{\"deploy\":true}",
                None,
            )
            .expect_err("malformed witness signature R must fail before backend verify");
            match err {
                crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("X-Iroha-Witness signature payload") && msg.contains(expected),
                        "{label} witness signature failed with unexpected error: {msg}"
                    );
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn verify_rejects_duplicate_multisig_witness_signers() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = checked_app_auth_key_fixture();
        let signer_two = checked_app_auth_key_fixture();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy".parse().expect("uri");
        let timestamp_ms = now_unix_ms();
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{}",
            timestamp_ms,
            "duplicate-multisig-witness",
            &[&signer_one],
        );
        let mut duplicate = witness.clone();
        duplicate.signatures.push(duplicate.signatures[0].clone());
        let headers = witness_headers(&account, &duplicate);

        let err = verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect_err("duplicate witness signers must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("duplicate signer"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_multisig_witness_below_threshold() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = checked_app_auth_key_fixture();
        let signer_two = checked_app_auth_key_fixture();
        let signer_three = checked_app_auth_key_fixture();
        let policy = MultisigPolicy::new(
            3,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_three.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy".parse().expect("uri");
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{}",
            now_unix_ms(),
            "threshold-multisig-witness",
            &[&signer_one, &signer_two],
        );
        let headers = witness_headers(&account, &witness);

        let err = verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect_err("threshold failure must reject witness");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("threshold"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_replayed_multisig_witness_nonce() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = checked_app_auth_key_fixture();
        let signer_two = checked_app_auth_key_fixture();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy".parse().expect("uri");
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{}",
            now_unix_ms(),
            "replayed-multisig-witness",
            &[&signer_one, &signer_two],
        );
        let headers = witness_headers(&account, &witness);

        verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect("first multisig witness must pass");
        let err = verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect_err("replayed multisig witness must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("nonce already used"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
