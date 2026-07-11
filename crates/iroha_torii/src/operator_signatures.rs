//! Signature-based operator authentication for Torii operator endpoints.
//!
//! This middleware is intended for internet-exposed deployments where operator endpoints are
//! reachable but must be authenticated. Requests must include the following headers:
//! - `x-iroha-operator-public-key`: operator public key (Iroha multihash string).
//! - `x-iroha-operator-timestamp-ms`: unix timestamp in milliseconds.
//! - `x-iroha-operator-nonce`: caller-chosen nonce (unique per request).
//! - `x-iroha-operator-signature`: base64 signature over the canonical request bytes plus
//!   `timestamp-ms` and `nonce`.
//!
//! Canonical request bytes follow `crate::canonical_request_message`:
//! ```text
//! <UPPERCASE_METHOD>\n
//! <path>\n
//! <sorted_query_string>\n
//! <hex_sha256(body)>\n
//! <timestamp_ms>\n
//! <nonce>
//! ```
//!
//! Replay protection is enforced via a bounded in-memory nonce cache.

use std::{
    collections::{HashSet, VecDeque},
    fmt,
    num::NonZeroUsize,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use dashmap::{DashMap, mapref::entry::Entry};
use iroha_config::parameters::actual::ToriiOperatorSignatures;
use iroha_crypto::{Algorithm, KeyPair, PublicKey, Signature};
use rand::{
    rand_core::{TryCryptoRng, TryRngCore},
    rngs::OsRng,
};

use crate::{JsonBody, SharedAppState, canonical_request_message, json_entry, json_object};

const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";

fn validate_operator_signature_for_public_key(
    signature: &Signature,
    public_key: &PublicKey,
) -> Result<(), OperatorSignatureError> {
    if !matches!(public_key.try_algorithm(), Ok(Algorithm::Ed25519)) {
        return Ok(());
    }
    validate_operator_ed25519_signature_payload(signature.payload())
}

fn validate_operator_ed25519_signature_payload(
    signature: &[u8],
) -> Result<(), OperatorSignatureError> {
    if signature.len() != ed25519_dalek::SIGNATURE_LENGTH {
        return Err(OperatorSignatureError::invalid_header(
            HEADER_OPERATOR_SIGNATURE,
        ));
    }
    let r_bytes: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = signature
        .get(..ed25519_dalek::PUBLIC_KEY_LENGTH)
        .ok_or_else(|| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE))?
        .try_into()
        .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE))?;
    if !operator_ed25519_compressed_y_is_canonical(&r_bytes) {
        return Err(OperatorSignatureError::invalid_header(
            HEADER_OPERATOR_SIGNATURE,
        ));
    }
    let r_point = ed25519_dalek::VerifyingKey::from_bytes(&r_bytes)
        .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE))?;
    if r_point.is_weak() {
        return Err(OperatorSignatureError::invalid_header(
            HEADER_OPERATOR_SIGNATURE,
        ));
    }
    Ok(())
}

fn parse_operator_signature_for_public_key(
    signature_bytes: &[u8],
    public_key: &PublicKey,
) -> Result<Signature, OperatorSignatureError> {
    let algorithm = public_key
        .try_algorithm()
        .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_PUBLIC_KEY))?;
    match algorithm {
        Algorithm::Ed25519 => iroha_crypto::ed25519_parse_signature(signature_bytes)
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE)),
        Algorithm::MlDsa => iroha_crypto::mldsa65_parse_signature(signature_bytes)
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE)),
        _ => Signature::try_from_bytes(signature_bytes)
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE)),
    }
}

fn operator_ed25519_compressed_y_is_canonical(
    bytes: &[u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
) -> bool {
    const ED25519_FIELD_MODULUS_LE: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    let mut y = *bytes;
    y[ed25519_dalek::PUBLIC_KEY_LENGTH - 1] &= 0x7f;
    for idx in (0..ed25519_dalek::PUBLIC_KEY_LENGTH).rev() {
        match y[idx].cmp(&ED25519_FIELD_MODULUS_LE[idx]) {
            std::cmp::Ordering::Less => return true,
            std::cmp::Ordering::Greater => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    false
}

#[derive(Debug, Clone)]
pub(crate) struct OperatorSignatureError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl OperatorSignatureError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn missing_header(name: &'static str) -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_signature_missing",
            format!("missing required operator signature header `{name}`"),
        )
    }

    fn invalid_header(name: &'static str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "operator_signature_invalid",
            format!("invalid operator signature header `{name}`"),
        )
    }

    fn key_not_allowed() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "operator_key_not_allowed",
            "operator public key is not allow-listed",
        )
    }

    fn skew_exceeded() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_signature_skew",
            "operator request timestamp outside allowed skew window",
        )
    }

    fn replay() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_signature_replay",
            "operator request nonce was already used",
        )
    }

    fn bad_signature() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_signature_bad",
            "operator signature failed verification",
        )
    }

    fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "operator_signature_body_too_large",
            "operator request body exceeds configured maximum",
        )
    }

    fn random_nonce(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operator_signature_nonce_rng",
            format!("operator signature nonce RNG failed: {}", message.into()),
        )
    }

    fn signing(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operator_signature_signing",
            format!("operator signature signing failed: {}", message.into()),
        )
    }
}

impl fmt::Display for OperatorSignatureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl IntoResponse for OperatorSignatureError {
    fn into_response(self) -> Response {
        let payload = json_object(vec![
            json_entry("code", self.code),
            json_entry("message", self.message),
        ]);
        let mut resp = JsonBody(payload).into_response();
        *resp.status_mut() = self.status;
        resp
    }
}

#[derive(Debug)]
struct ReplayCache {
    ttl: Duration,
    capacity: NonZeroUsize,
    // key -> expiry
    entries: DashMap<String, Instant>,
    // FIFO for bounded pruning; duplicates allowed.
    order: Mutex<VecDeque<(String, Instant)>>,
}

impl ReplayCache {
    fn new(ttl: Duration, capacity: NonZeroUsize) -> Self {
        Self {
            ttl: ttl.max(Duration::from_secs(1)),
            capacity,
            entries: DashMap::new(),
            order: Mutex::new(VecDeque::new()),
        }
    }

    fn check_and_insert(&self, key: String) -> bool {
        let now = Instant::now();
        let expires_at = now + self.ttl;

        match self.entries.entry(key.clone()) {
            Entry::Occupied(mut occ) => {
                if *occ.get() > now {
                    return false;
                }
                occ.insert(expires_at);
            }
            Entry::Vacant(vac) => {
                vac.insert(expires_at);
            }
        }

        if let Ok(mut guard) = self.order.lock() {
            guard.push_back((key, expires_at));
            self.prune_locked(&mut guard, now);
        }

        true
    }

    fn prune_locked(&self, order: &mut VecDeque<(String, Instant)>, now: Instant) {
        let cap = self.capacity.get();
        while let Some((_key, expiry)) = order.front() {
            if *expiry > now && order.len() <= cap {
                break;
            }
            let (key, expiry) = order
                .pop_front()
                .expect("front is Some so pop_front must succeed");
            // Avoid `DashMap` shard deadlocks: don't hold a read guard (`get`) while attempting
            // a write lock (`remove`) on the same shard.
            let _ = self
                .entries
                .remove_if(&key, |_k, existing| *existing == expiry);
        }
    }
}

/// Signature-based operator authentication state.
pub struct OperatorSignatures {
    enabled: bool,
    allow_node_key: bool,
    allowed_public_keys: HashSet<PublicKey>,
    node_public_key: PublicKey,
    max_clock_skew: Duration,
    replay_cache: ReplayCache,
    max_body_bytes: usize,
}

impl OperatorSignatures {
    pub fn new(
        config: ToriiOperatorSignatures,
        node_public_key: PublicKey,
        max_body_bytes: u64,
        _telemetry: crate::routing::MaybeTelemetry,
    ) -> Self {
        let max_body_bytes = usize::try_from(max_body_bytes).unwrap_or(usize::MAX);
        let allowed_public_keys = config.allowed_public_keys.into_iter().collect();
        Self {
            enabled: config.enabled,
            allow_node_key: config.allow_node_key,
            allowed_public_keys,
            node_public_key,
            max_clock_skew: config.max_clock_skew,
            replay_cache: ReplayCache::new(config.nonce_ttl, config.replay_cache_capacity),
            max_body_bytes,
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Return the canonical Ed25519 key payloads trusted by operator policy.
    ///
    /// PoR verdict authentication reuses this configured trust root rather
    /// than trusting keys embedded by the submitter. Non-Ed25519 operator keys
    /// are intentionally excluded because first-release PoR artefacts require
    /// Ed25519 signatures.
    pub(crate) fn trusted_ed25519_key_bytes(&self) -> Vec<Vec<u8>> {
        let mut keys = self
            .allowed_public_keys
            .iter()
            .chain(self.allow_node_key.then_some(&self.node_public_key))
            .filter_map(|key| {
                let (algorithm, bytes) = key.to_bytes();
                (algorithm == iroha_crypto::Algorithm::Ed25519).then(|| bytes.to_vec())
            })
            .collect::<Vec<_>>();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    fn is_key_allowed(&self, public_key: &PublicKey) -> bool {
        if self.allow_node_key && public_key == &self.node_public_key {
            return true;
        }
        self.allowed_public_keys.contains(public_key)
    }

    fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn parse_required_header<'a>(
        headers: &'a HeaderMap,
        name: &'static str,
    ) -> Result<&'a str, OperatorSignatureError> {
        headers
            .get(name)
            .ok_or_else(|| OperatorSignatureError::missing_header(name))?
            .to_str()
            .map_err(|_| OperatorSignatureError::invalid_header(name))
            .map(|s| s.trim())
            .and_then(|s| {
                if s.is_empty() {
                    Err(OperatorSignatureError::invalid_header(name))
                } else {
                    Ok(s)
                }
            })
    }

    fn validate_freshness(
        &self,
        timestamp_ms: u64,
        nonce: &str,
    ) -> Result<(), OperatorSignatureError> {
        let now_ms = Self::now_unix_ms();
        let delta_ms = now_ms.abs_diff(timestamp_ms);
        let max_skew_ms: u64 = self
            .max_clock_skew
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        if delta_ms > max_skew_ms {
            return Err(OperatorSignatureError::skew_exceeded());
        }

        if nonce.len() > 256 || !nonce.is_ascii() || nonce.bytes().any(|b| b.is_ascii_whitespace())
        {
            return Err(OperatorSignatureError::invalid_header(
                HEADER_OPERATOR_NONCE,
            ));
        }

        Ok(())
    }

    fn admit_nonce(
        &self,
        nonce: &str,
        public_key: &PublicKey,
    ) -> Result<(), OperatorSignatureError> {
        let replay_key = format!("{public_key}:{nonce}");
        if !self.replay_cache.check_and_insert(replay_key) {
            return Err(OperatorSignatureError::replay());
        }
        Ok(())
    }

    fn operator_request_message(
        method: &crate::Method,
        uri: &crate::Uri,
        body: &[u8],
        timestamp_ms: u64,
        nonce: &str,
    ) -> Vec<u8> {
        let mut msg = canonical_request_message(method, uri, body);
        msg.extend_from_slice(b"\n");
        msg.extend_from_slice(timestamp_ms.to_string().as_bytes());
        msg.extend_from_slice(b"\n");
        msg.extend_from_slice(nonce.as_bytes());
        msg
    }

    fn authorize_bytes_with_policy(
        &self,
        headers: &HeaderMap,
        method: &crate::Method,
        uri: &crate::Uri,
        body: &[u8],
        require_allowlisted_key: bool,
    ) -> Result<(), OperatorSignatureError> {
        let public_key_str = Self::parse_required_header(headers, HEADER_OPERATOR_PUBLIC_KEY)?;
        let public_key = public_key_str
            .parse::<PublicKey>()
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_PUBLIC_KEY))?;
        if require_allowlisted_key && !self.is_key_allowed(&public_key) {
            return Err(OperatorSignatureError::key_not_allowed());
        }

        let timestamp_str = Self::parse_required_header(headers, HEADER_OPERATOR_TIMESTAMP_MS)?;
        let timestamp_ms = timestamp_str
            .parse::<u64>()
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_TIMESTAMP_MS))?;

        let nonce = Self::parse_required_header(headers, HEADER_OPERATOR_NONCE)?;

        let signature_str = Self::parse_required_header(headers, HEADER_OPERATOR_SIGNATURE)?;
        let signature_bytes = BASE64_STANDARD
            .decode(signature_str)
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_SIGNATURE))?;
        let signature = parse_operator_signature_for_public_key(&signature_bytes, &public_key)?;
        validate_operator_signature_for_public_key(&signature, &public_key)?;

        self.validate_freshness(timestamp_ms, nonce)?;

        let msg = Self::operator_request_message(method, uri, body, timestamp_ms, nonce);
        signature
            .verify(&public_key, &msg)
            .map_err(|_| OperatorSignatureError::bad_signature())?;

        // Admit the nonce only after authenticating the request. `check_and_insert` is atomic for
        // a replay key, so concurrently verified requests using the same nonce still have exactly
        // one winner without letting unauthenticated traffic consume or evict cache entries.
        self.admit_nonce(nonce, &public_key)?;

        Ok(())
    }

    fn authorize_bytes(
        &self,
        headers: &HeaderMap,
        method: &crate::Method,
        uri: &crate::Uri,
        body: &[u8],
    ) -> Result<(), OperatorSignatureError> {
        self.authorize_bytes_with_policy(headers, method, uri, body, true)
    }

    fn authorize_request(
        &self,
        req: &axum::http::Request<Body>,
        body_bytes: &[u8],
    ) -> Result<(), OperatorSignatureError> {
        if body_bytes.len() > self.max_body_bytes {
            return Err(OperatorSignatureError::payload_too_large());
        }
        self.authorize_bytes(req.headers(), req.method(), req.uri(), body_bytes)
    }

    fn authorize_request_for_bound_key(
        &self,
        req: &axum::http::Request<Body>,
        body_bytes: &[u8],
    ) -> Result<(), OperatorSignatureError> {
        if body_bytes.len() > self.max_body_bytes {
            return Err(OperatorSignatureError::payload_too_large());
        }
        self.authorize_bytes_with_policy(req.headers(), req.method(), req.uri(), body_bytes, false)
    }
}

/// Build operator signature headers for an internal Torii request.
pub(crate) fn signed_request_headers(
    key_pair: &KeyPair,
    method: &crate::Method,
    uri: &crate::Uri,
    body: &[u8],
) -> Result<HeaderMap, OperatorSignatureError> {
    signed_request_headers_with_rng(key_pair, method, uri, body, &mut OsRng)
}

fn signed_request_headers_with_rng<R: TryCryptoRng>(
    key_pair: &KeyPair,
    method: &crate::Method,
    uri: &crate::Uri,
    body: &[u8],
    rng: &mut R,
) -> Result<HeaderMap, OperatorSignatureError> {
    let timestamp_ms = OperatorSignatures::now_unix_ms();
    let nonce = operator_signature_nonce_with_rng(rng)?;

    let msg = OperatorSignatures::operator_request_message(method, uri, body, timestamp_ms, &nonce);
    let signature = Signature::try_new(key_pair.private_key(), &msg)
        .map_err(|error| OperatorSignatureError::signing(error.to_string()))?;

    Ok(operator_signature_headers(
        key_pair,
        timestamp_ms,
        &nonce,
        &signature,
    ))
}

fn operator_signature_nonce_with_rng<R: TryCryptoRng>(
    rng: &mut R,
) -> Result<String, OperatorSignatureError> {
    let mut nonce_bytes = [0u8; 12];
    rng.try_fill_bytes(&mut nonce_bytes)
        .map_err(|error| OperatorSignatureError::random_nonce(error.to_string()))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes))
}

fn operator_signature_headers(
    key_pair: &KeyPair,
    timestamp_ms: u64,
    nonce: &str,
    signature: &Signature,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HEADER_OPERATOR_PUBLIC_KEY,
        key_pair
            .public_key()
            .to_string()
            .parse()
            .expect("operator public key header"),
    );
    headers.insert(
        HEADER_OPERATOR_TIMESTAMP_MS,
        timestamp_ms
            .to_string()
            .parse()
            .expect("operator timestamp header"),
    );
    headers.insert(
        HEADER_OPERATOR_NONCE,
        nonce.parse().expect("operator nonce header"),
    );
    headers.insert(
        HEADER_OPERATOR_SIGNATURE,
        BASE64_STANDARD
            .encode(signature.payload())
            .parse()
            .expect("operator signature header"),
    );
    headers
}

pub async fn enforce_operator_access(
    State(app): State<SharedAppState>,
    req: Request,
    next: Next,
) -> Response {
    if app.operator_signatures.is_enabled() {
        let (parts, body) = req.into_parts();
        let body_bytes =
            match axum::body::to_bytes(body, app.operator_signatures.max_body_bytes).await {
                Ok(bytes) => bytes,
                Err(_) => return OperatorSignatureError::payload_too_large().into_response(),
            };
        let req = axum::http::Request::from_parts(parts, Body::from(body_bytes.clone()));
        if let Err(err) = app.operator_signatures.authorize_request(&req, &body_bytes) {
            return err.into_response();
        }
        return next.run(req).await;
    }

    if app.operator_auth.is_enabled() {
        if let Err(err) = app
            .operator_auth
            .authorize_operator_endpoint(req.headers(), None)
            .await
        {
            return err.into_response();
        }
        return next.run(req).await;
    }

    OperatorSignatureError::new(
        StatusCode::FORBIDDEN,
        "operator_access_disabled",
        "operator endpoints are disabled without authentication",
    )
    .into_response()
}

/// Verify a canonical signed request without applying the operator allow-list.
///
/// Route handlers using this middleware must bind the authenticated header key to an
/// authoritative identity before mutating state. This is used for SoraFS provider submissions,
/// whose admitted advert key is dynamic and therefore cannot be copied into the static operator
/// allow-list. Freshness, signature verification, body binding, and replay protection are still
/// enforced here before the request reaches the handler.
pub async fn enforce_identity_bound_signature(
    State(app): State<SharedAppState>,
    req: Request,
    next: Next,
) -> Response {
    let (parts, body) = req.into_parts();
    let body_bytes = match axum::body::to_bytes(body, app.operator_signatures.max_body_bytes).await
    {
        Ok(bytes) => bytes,
        Err(_) => return OperatorSignatureError::payload_too_large().into_response(),
    };
    let req = axum::http::Request::from_parts(parts, Body::from(body_bytes.clone()));
    if let Err(error) = app
        .operator_signatures
        .authorize_request_for_bound_key(&req, &body_bytes)
    {
        return error.into_response();
    }
    next.run(req).await
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;
    use axum::routing::get;
    use iroha_crypto::{Algorithm, KeyPair};
    use rand::rand_core::{TryCryptoRng, TryRngCore};
    use tower::ServiceExt as _;

    struct FailingOperatorNonceRng;

    #[derive(Debug)]
    struct FailingOperatorNonceRngError;

    impl fmt::Display for FailingOperatorNonceRngError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("failing operator nonce RNG")
        }
    }

    impl TryRngCore for FailingOperatorNonceRng {
        type Error = FailingOperatorNonceRngError;

        fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
            Err(FailingOperatorNonceRngError)
        }

        fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
            Err(FailingOperatorNonceRngError)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> std::result::Result<(), Self::Error> {
            Err(FailingOperatorNonceRngError)
        }
    }

    impl TryCryptoRng for FailingOperatorNonceRng {}

    fn checked_ed25519_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked operator signature fixture keypair")
    }

    fn checked_mldsa_keypair() -> KeyPair {
        KeyPair::try_from_seed(b"torii-operator-signature-mldsa".to_vec(), Algorithm::MlDsa)
            .expect("generate checked ML-DSA operator signature fixture keypair")
    }

    fn operator_signatures_with_capacity(
        key_pair: &KeyPair,
        capacity: usize,
    ) -> OperatorSignatures {
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: true,
            allowed_public_keys: Vec::new(),
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(capacity).expect("non-zero test capacity"),
        };
        OperatorSignatures::new(
            cfg,
            key_pair.public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        )
    }

    fn signed_headers_with_nonce(
        key_pair: &KeyPair,
        method: &crate::Method,
        uri: &crate::Uri,
        body: &[u8],
        timestamp_ms: u64,
        nonce: &str,
    ) -> HeaderMap {
        let message =
            OperatorSignatures::operator_request_message(method, uri, body, timestamp_ms, nonce);
        let signature = Signature::try_new(key_pair.private_key(), &message)
            .expect("checked operator signature fixture");
        operator_signature_headers(key_pair, timestamp_ms, nonce, &signature)
    }

    const ED25519_SMALL_ORDER_POINT: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const ED25519_NONCANONICAL_IDENTITY: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    #[test]
    fn operator_signatures_rejects_replay() {
        let key_pair = checked_ed25519_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: true,
            allowed_public_keys: Vec::new(),
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            key_pair.public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );
        let uri: crate::Uri = "/v1/configuration".parse().unwrap();
        let body = b"{}";
        let ts = OperatorSignatures::now_unix_ms();
        let nonce = "nonce-1";
        let msg = OperatorSignatures::operator_request_message(
            &crate::Method::POST,
            &uri,
            body,
            ts,
            nonce,
        );
        let signature = Signature::try_new(key_pair.private_key(), &msg)
            .expect("checked operator replay fixture signature");
        signature
            .verify(key_pair.public_key(), &msg)
            .expect("checked operator replay fixture signature verifies");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_OPERATOR_PUBLIC_KEY,
            key_pair
                .public_key()
                .to_string()
                .parse()
                .expect("public key header"),
        );
        headers.insert(
            HEADER_OPERATOR_TIMESTAMP_MS,
            ts.to_string().parse().expect("timestamp header"),
        );
        headers.insert(HEADER_OPERATOR_NONCE, nonce.parse().expect("nonce header"));
        headers.insert(
            HEADER_OPERATOR_SIGNATURE,
            BASE64_STANDARD
                .encode(signature.payload())
                .parse()
                .expect("signature header"),
        );

        auth.authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect("first use ok");
        let err = auth
            .authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .err()
            .expect("second use rejected");
        assert_eq!(err.code, "operator_signature_replay");
    }

    #[test]
    fn identity_bound_signatures_verify_unlisted_keys_and_retain_replay_protection() {
        let operator = checked_ed25519_keypair();
        let provider = checked_ed25519_keypair();
        let auth = operator_signatures_with_capacity(&operator, 8);
        let uri: crate::Uri = "/v1/sorafs/deal/usage".parse().expect("deal URI");
        let body = br#"{"deal_id_hex":"00"}"#;
        let timestamp_ms = OperatorSignatures::now_unix_ms();
        let headers = signed_headers_with_nonce(
            &provider,
            &crate::Method::POST,
            &uri,
            body,
            timestamp_ms,
            "provider-deal-usage-1",
        );

        let allowlist_error = auth
            .authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect_err("provider key is not a static operator");
        assert_eq!(allowlist_error.code, "operator_key_not_allowed");
        auth.authorize_bytes_with_policy(&headers, &crate::Method::POST, &uri, body, false)
            .expect("handler will bind the verified provider key to the admitted advert");
        let replay = auth
            .authorize_bytes_with_policy(&headers, &crate::Method::POST, &uri, body, false)
            .expect_err("provider nonce remains replay protected");
        assert_eq!(replay.code, "operator_signature_replay");

        let tampered_headers = signed_headers_with_nonce(
            &provider,
            &crate::Method::POST,
            &uri,
            body,
            timestamp_ms,
            "provider-deal-usage-2",
        );
        let tampered = auth
            .authorize_bytes_with_policy(
                &tampered_headers,
                &crate::Method::POST,
                &uri,
                br#"{"deal_id_hex":"ff"}"#,
                false,
            )
            .expect_err("signature must bind the exact request body");
        assert_eq!(tampered.code, "operator_signature_bad");

        let route_bound_headers = signed_headers_with_nonce(
            &provider,
            &crate::Method::POST,
            &uri,
            body,
            timestamp_ms,
            "provider-deal-usage-3",
        );
        let funding_uri: crate::Uri = "/v1/sorafs/deal/fund-provider"
            .parse()
            .expect("funding URI");
        let wrong_route = auth
            .authorize_bytes_with_policy(
                &route_bound_headers,
                &crate::Method::POST,
                &funding_uri,
                body,
                false,
            )
            .expect_err("identity-bound signatures must bind the exact lifecycle route");
        assert_eq!(wrong_route.code, "operator_signature_bad");
    }

    #[test]
    fn bad_signature_does_not_consume_its_claimed_nonce() {
        let key_pair = checked_ed25519_keypair();
        let auth = operator_signatures_with_capacity(&key_pair, 1);
        let uri: crate::Uri = "/v1/configuration".parse().unwrap();
        let body = b"{}";
        let timestamp_ms = OperatorSignatures::now_unix_ms();
        let nonce = "nonce-after-bad-signature";
        let valid_headers = signed_headers_with_nonce(
            &key_pair,
            &crate::Method::POST,
            &uri,
            body,
            timestamp_ms,
            nonce,
        );

        let mut invalid_headers = signed_headers_with_nonce(
            &key_pair,
            &crate::Method::POST,
            &uri,
            body,
            timestamp_ms,
            "nonce-that-was-actually-signed",
        );
        invalid_headers.insert(
            HEADER_OPERATOR_NONCE,
            nonce.parse().expect("claimed nonce header"),
        );

        let error = auth
            .authorize_bytes(&invalid_headers, &crate::Method::POST, &uri, body)
            .expect_err("mismatched signature must fail");
        assert_eq!(error.code, "operator_signature_bad");

        auth.authorize_bytes(&valid_headers, &crate::Method::POST, &uri, body)
            .expect("bad signature must not consume the nonce");
        let replay = auth
            .authorize_bytes(&valid_headers, &crate::Method::POST, &uri, body)
            .expect_err("authenticated nonce must remain replay-protected");
        assert_eq!(replay.code, "operator_signature_replay");
    }

    #[test]
    fn bad_signatures_cannot_poison_replay_cache_capacity() {
        let key_pair = checked_ed25519_keypair();
        let auth = operator_signatures_with_capacity(&key_pair, 2);
        let uri: crate::Uri = "/v1/configuration".parse().unwrap();
        let body = b"{}";
        let timestamp_ms = OperatorSignatures::now_unix_ms();
        let protected_headers: Vec<_> = ["protected-nonce-a", "protected-nonce-b"]
            .into_iter()
            .map(|nonce| {
                signed_headers_with_nonce(
                    &key_pair,
                    &crate::Method::POST,
                    &uri,
                    body,
                    timestamp_ms,
                    nonce,
                )
            })
            .collect();

        for headers in &protected_headers {
            auth.authorize_bytes(headers, &crate::Method::POST, &uri, body)
                .expect("initial authenticated nonce use");
        }

        for index in 0..32 {
            let mut poison_headers = signed_headers_with_nonce(
                &key_pair,
                &crate::Method::POST,
                &uri,
                body,
                timestamp_ms,
                "different-signed-nonce",
            );
            poison_headers.insert(
                HEADER_OPERATOR_NONCE,
                format!("poison-nonce-{index}")
                    .parse()
                    .expect("poison nonce header"),
            );

            let error = auth
                .authorize_bytes(&poison_headers, &crate::Method::POST, &uri, body)
                .expect_err("poisoning request must fail signature verification");
            assert_eq!(error.code, "operator_signature_bad");
        }

        for headers in &protected_headers {
            let replay = auth
                .authorize_bytes(headers, &crate::Method::POST, &uri, body)
                .expect_err("invalid traffic must not evict authenticated replay protection");
            assert_eq!(replay.code, "operator_signature_replay");
        }
    }

    #[test]
    fn concurrent_valid_requests_with_same_nonce_have_one_winner() {
        const WORKERS: usize = 16;

        let key_pair = checked_ed25519_keypair();
        let auth = operator_signatures_with_capacity(&key_pair, 64);
        let uri: crate::Uri = "/v1/configuration".parse().unwrap();
        let body = b"{}";
        let headers = signed_headers_with_nonce(
            &key_pair,
            &crate::Method::POST,
            &uri,
            body,
            OperatorSignatures::now_unix_ms(),
            "concurrent-shared-nonce",
        );
        let barrier = Arc::new(Barrier::new(WORKERS));

        let results = thread::scope(|scope| {
            let mut handles = Vec::with_capacity(WORKERS);
            for _ in 0..WORKERS {
                let barrier = Arc::clone(&barrier);
                let headers = headers.clone();
                let auth = &auth;
                let uri = &uri;
                handles.push(scope.spawn(move || {
                    barrier.wait();
                    auth.authorize_bytes(&headers, &crate::Method::POST, uri, body)
                        .map_err(|error| error.code)
                }));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().expect("authorization worker"))
                .collect::<Vec<_>>()
        });

        let mut accepted = 0;
        let mut replayed = 0;
        for result in results {
            match result {
                Ok(()) => accepted += 1,
                Err("operator_signature_replay") => replayed += 1,
                Err(code) => panic!("unexpected concurrent authorization error: {code}"),
            }
        }
        assert_eq!(accepted, 1);
        assert_eq!(replayed, WORKERS - 1);
    }

    #[test]
    fn operator_signatures_accepts_valid_signature() {
        let key_pair = checked_ed25519_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: false,
            allowed_public_keys: vec![key_pair.public_key().clone()],
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            checked_ed25519_keypair().public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );

        let uri: crate::Uri = "/v1/configuration?b=2&a=1".parse().unwrap();
        let body = b"{\"foo\":1}";
        let headers = signed_request_headers(&key_pair, &crate::Method::POST, &uri, body)
            .expect("operator signature headers");

        auth.authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect("valid signature");
    }

    #[test]
    fn operator_signatures_accepts_valid_mldsa_signature() {
        let key_pair = checked_mldsa_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: false,
            allowed_public_keys: vec![key_pair.public_key().clone()],
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            checked_ed25519_keypair().public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );

        let uri: crate::Uri = "/v1/configuration?b=2&a=1".parse().unwrap();
        let body = b"{\"foo\":1}";
        let headers = signed_request_headers(&key_pair, &crate::Method::POST, &uri, body)
            .expect("ML-DSA operator signature headers");

        auth.authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect("valid ML-DSA signature");
    }

    #[test]
    fn operator_signatures_reject_all_zero_signature_header() {
        let key_pair = checked_ed25519_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: false,
            allowed_public_keys: vec![key_pair.public_key().clone()],
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            checked_ed25519_keypair().public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );
        let uri: crate::Uri = "/v1/configuration?b=2&a=1".parse().unwrap();
        let body = b"{\"foo\":1}";
        let mut headers = signed_request_headers(&key_pair, &crate::Method::POST, &uri, body)
            .expect("operator signature headers");
        headers.insert(
            HEADER_OPERATOR_SIGNATURE,
            BASE64_STANDARD
                .encode([0u8; 64])
                .parse()
                .expect("all-zero signature header"),
        );

        let error = auth
            .authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect_err("all-zero signature header must fail");

        assert_eq!(error.code, "operator_signature_invalid");
    }

    #[test]
    fn operator_signatures_reject_malformed_ed25519_signature_r_before_backend() {
        let key_pair = checked_ed25519_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: false,
            allowed_public_keys: vec![key_pair.public_key().clone()],
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            checked_ed25519_keypair().public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );
        let uri: crate::Uri = "/v1/configuration?b=2&a=1".parse().unwrap();
        let body = b"{\"foo\":1}";

        for (label, replacement_r) in [
            ("small-order", ED25519_SMALL_ORDER_POINT),
            ("noncanonical", ED25519_NONCANONICAL_IDENTITY),
        ] {
            let mut headers = signed_request_headers(&key_pair, &crate::Method::POST, &uri, body)
                .expect("operator signature headers");
            let signature_str = headers
                .get(HEADER_OPERATOR_SIGNATURE)
                .expect("signature header")
                .to_str()
                .expect("signature header is text");
            let mut signature_bytes = BASE64_STANDARD
                .decode(signature_str)
                .expect("decode generated signature");
            signature_bytes[..ed25519_dalek::PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
            headers.insert(
                HEADER_OPERATOR_SIGNATURE,
                BASE64_STANDARD
                    .encode(signature_bytes)
                    .parse()
                    .expect("malformed signature header"),
            );

            let error = auth
                .authorize_bytes(&headers, &crate::Method::POST, &uri, body)
                .expect_err("malformed signature header must fail");

            assert_eq!(
                error.code, "operator_signature_invalid",
                "{label} signature R must fail at header admission"
            );
        }
    }

    #[test]
    fn operator_signatures_reject_malformed_mldsa_signature_lengths() {
        let key_pair = checked_mldsa_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: false,
            allowed_public_keys: vec![key_pair.public_key().clone()],
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            checked_ed25519_keypair().public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );
        let uri: crate::Uri = "/v1/configuration?b=2&a=1".parse().unwrap();
        let body = b"{\"foo\":1}";

        for label in ["short", "overlong"] {
            let mut headers = signed_request_headers(&key_pair, &crate::Method::POST, &uri, body)
                .expect("ML-DSA operator signature headers");
            let signature_str = headers
                .get(HEADER_OPERATOR_SIGNATURE)
                .expect("signature header")
                .to_str()
                .expect("signature header is text");
            let mut signature_bytes = BASE64_STANDARD
                .decode(signature_str)
                .expect("decode generated ML-DSA signature");
            match label {
                "short" => {
                    signature_bytes
                        .pop()
                        .expect("ML-DSA fixture signature is non-empty");
                }
                "overlong" => signature_bytes.push(0xA5),
                _ => unreachable!("covered labels"),
            }
            headers.insert(
                HEADER_OPERATOR_SIGNATURE,
                BASE64_STANDARD
                    .encode(signature_bytes)
                    .parse()
                    .expect("malformed ML-DSA signature header"),
            );

            let error = auth
                .authorize_bytes(&headers, &crate::Method::POST, &uri, body)
                .expect_err("malformed ML-DSA operator signature header must fail");

            assert_eq!(
                error.code, "operator_signature_invalid",
                "{label} ML-DSA signature length must fail at header admission"
            );
        }
    }

    #[test]
    fn signed_request_headers_authorize_successfully() {
        let key_pair = checked_ed25519_keypair();
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: true,
            allowed_public_keys: Vec::new(),
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(64).unwrap(),
        };
        let auth = OperatorSignatures::new(
            cfg,
            key_pair.public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );
        let uri: crate::Uri = iroha_torii_shared::uri::CONFIGURATION.parse().unwrap();
        let headers = signed_request_headers(&key_pair, &crate::Method::GET, &uri, &[])
            .expect("operator signature headers");

        auth.authorize_bytes(&headers, &crate::Method::GET, &uri, &[])
            .expect("generated headers should verify");
    }

    #[test]
    fn signed_request_headers_reports_nonce_rng_failure() {
        let key_pair = checked_ed25519_keypair();
        let uri: crate::Uri = iroha_torii_shared::uri::CONFIGURATION
            .parse()
            .expect("configuration URI");
        let mut rng = FailingOperatorNonceRng;

        let error =
            signed_request_headers_with_rng(&key_pair, &crate::Method::GET, &uri, &[], &mut rng)
                .expect_err("RNG failure must be reported");

        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.code, "operator_signature_nonce_rng");
        assert!(
            error
                .message
                .contains("operator signature nonce RNG failed")
        );
        assert!(error.message.contains("failing operator nonce RNG"));
    }

    #[test]
    fn operator_signature_signing_error_is_internal() {
        let error = OperatorSignatureError::signing("backend rejected message");

        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.code, "operator_signature_signing");
        assert!(error.message.contains("backend rejected message"));
    }

    #[tokio::test]
    async fn operator_middleware_forbids_when_all_operator_auth_is_disabled() {
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        assert!(app.operator_signatures.is_enabled());
        assert!(!app.operator_auth.is_enabled());

        let node_public_key = app.da_receipt_signer.public_key().clone();
        let telemetry = app.telemetry.clone();
        let mut cfg = ToriiOperatorSignatures::default();
        cfg.enabled = false;
        Arc::get_mut(&mut app)
            .expect("unique app state required")
            .operator_signatures = Arc::new(OperatorSignatures::new(
            cfg,
            node_public_key,
            iroha_config::parameters::defaults::torii::MAX_CONTENT_LEN.get(),
            telemetry,
        ));
        assert!(!app.operator_signatures.is_enabled());

        let operator_layer = axum::middleware::from_fn_with_state::<
            _,
            _,
            (axum::extract::State<SharedAppState>, axum::extract::Request),
        >(app.clone(), enforce_operator_access);

        let router = axum::Router::new()
            .route("/status", get(|| async { "ok" }))
            .route_layer(operator_layer);

        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
