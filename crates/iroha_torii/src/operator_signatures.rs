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
use iroha_crypto::{KeyPair, PublicKey, Signature};
use rand::RngCore as _;

use crate::{JsonBody, SharedAppState, canonical_request_message, json_entry, json_object};

const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";

#[derive(Debug, Clone)]
struct OperatorSignatureError {
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

    fn replay_cache_full() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "operator_signature_replay_cache_full",
            "operator nonce replay cache is full; retry later",
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

/// Outcome of recording a nonce in the replay cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NonceCheck {
    Fresh,
    Replayed,
    CacheFull,
}

#[derive(Debug)]
struct ReplayCache {
    ttl: Duration,
    capacity: NonZeroUsize,
    // key -> expiry
    entries: DashMap<String, Instant>,
    // FIFO for expiry pruning; duplicates allowed.
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

    fn check_and_insert(&self, key: String) -> NonceCheck {
        let now = Instant::now();
        let expires_at = now + self.ttl;
        let mut order = self
            .order
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.prune_locked(&mut order, now);

        match self.entries.entry(key.clone()) {
            Entry::Occupied(mut occ) => {
                if *occ.get() > now {
                    return NonceCheck::Replayed;
                }
                occ.insert(expires_at);
            }
            Entry::Vacant(vac) => {
                // Never evict a live nonce to make room: forgetting it would let the
                // request that used it be replayed while its timestamp is still fresh.
                if order.len() >= self.capacity.get() {
                    return NonceCheck::CacheFull;
                }
                vac.insert(expires_at);
            }
        }
        order.push_back((key, expires_at));

        NonceCheck::Fresh
    }

    fn prune_locked(&self, order: &mut VecDeque<(String, Instant)>, now: Instant) {
        while let Some((_key, expiry)) = order.front() {
            if *expiry > now {
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

    fn ensure_freshness(
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

    /// Record a nonce whose request signature has already been verified.
    fn consume_nonce(
        &self,
        nonce: &str,
        public_key: &PublicKey,
    ) -> Result<(), OperatorSignatureError> {
        let replay_key = format!("{public_key}:{nonce}");
        match self.replay_cache.check_and_insert(replay_key) {
            NonceCheck::Fresh => Ok(()),
            NonceCheck::Replayed => Err(OperatorSignatureError::replay()),
            NonceCheck::CacheFull => Err(OperatorSignatureError::replay_cache_full()),
        }
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

    fn authorize_bytes(
        &self,
        headers: &HeaderMap,
        method: &crate::Method,
        uri: &crate::Uri,
        body: &[u8],
    ) -> Result<(), OperatorSignatureError> {
        let public_key_str = Self::parse_required_header(headers, HEADER_OPERATOR_PUBLIC_KEY)?;
        let public_key = public_key_str
            .parse::<PublicKey>()
            .map_err(|_| OperatorSignatureError::invalid_header(HEADER_OPERATOR_PUBLIC_KEY))?;
        if !self.is_key_allowed(&public_key) {
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
        let signature = Signature::from_bytes(&signature_bytes);

        self.ensure_freshness(timestamp_ms, nonce)?;

        let msg = Self::operator_request_message(method, uri, body, timestamp_ms, nonce);
        signature
            .verify(&public_key, &msg)
            .map_err(|_| OperatorSignatureError::bad_signature())?;

        // Only signed requests may occupy replay-cache slots.
        self.consume_nonce(nonce, &public_key)
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
}

/// Build operator signature headers for an internal Torii request.
pub(crate) fn signed_request_headers(
    key_pair: &KeyPair,
    method: &crate::Method,
    uri: &crate::Uri,
    body: &[u8],
) -> HeaderMap {
    let timestamp_ms = OperatorSignatures::now_unix_ms();
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);

    let msg = OperatorSignatures::operator_request_message(method, uri, body, timestamp_ms, &nonce);
    let signature = Signature::new(key_pair.private_key(), &msg);

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use axum::routing::get;
    use iroha_crypto::KeyPair;
    use rand::RngCore as _;
    use tower::ServiceExt as _;

    #[test]
    fn operator_signatures_rejects_replay() {
        let key_pair = KeyPair::random();
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
        let signature = Signature::new(key_pair.private_key(), &msg);
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

    fn auth_with_capacity(key_pair: &KeyPair, capacity: usize) -> OperatorSignatures {
        let cfg = ToriiOperatorSignatures {
            enabled: true,
            allow_node_key: true,
            allowed_public_keys: Vec::new(),
            max_clock_skew: Duration::from_secs(60),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: NonZeroUsize::new(capacity).unwrap(),
        };
        OperatorSignatures::new(
            cfg,
            key_pair.public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        )
    }

    fn headers_with_nonce(
        key_pair: &KeyPair,
        uri: &crate::Uri,
        body: &[u8],
        nonce: &str,
        valid_signature: bool,
    ) -> HeaderMap {
        let ts = OperatorSignatures::now_unix_ms();
        let msg = OperatorSignatures::operator_request_message(
            &crate::Method::POST,
            uri,
            body,
            ts,
            nonce,
        );
        let signer = if valid_signature {
            key_pair.clone()
        } else {
            KeyPair::random()
        };
        let signature = Signature::new(signer.private_key(), &msg);
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_OPERATOR_PUBLIC_KEY,
            key_pair.public_key().to_string().parse().unwrap(),
        );
        headers.insert(
            HEADER_OPERATOR_TIMESTAMP_MS,
            ts.to_string().parse().unwrap(),
        );
        headers.insert(HEADER_OPERATOR_NONCE, nonce.parse().unwrap());
        headers.insert(
            HEADER_OPERATOR_SIGNATURE,
            BASE64_STANDARD.encode(signature.payload()).parse().unwrap(),
        );
        headers
    }

    #[test]
    fn operator_signatures_unsigned_flood_cannot_evict_nonce() {
        let key_pair = KeyPair::random();
        let auth = auth_with_capacity(&key_pair, 2);
        let uri: crate::Uri = "/v1/configuration".parse().unwrap();
        let body = b"{}";
        let operator = headers_with_nonce(&key_pair, &uri, body, "operator-nonce", true);
        auth.authorize_bytes(&operator, &crate::Method::POST, &uri, body)
            .expect("operator request accepted");

        // Anyone who knows the node key can send these; they must not touch the cache.
        for idx in 0..8 {
            let forged = headers_with_nonce(&key_pair, &uri, body, &format!("forged-{idx}"), false);
            let err = auth
                .authorize_bytes(&forged, &crate::Method::POST, &uri, body)
                .err()
                .expect("forged signature rejected");
            assert_eq!(err.code, "operator_signature_bad");
        }

        let err = auth
            .authorize_bytes(&operator, &crate::Method::POST, &uri, body)
            .err()
            .expect("replayed operator request rejected");
        assert_eq!(err.code, "operator_signature_replay");
    }

    #[test]
    fn operator_signatures_full_cache_keeps_live_nonces() {
        let key_pair = KeyPair::random();
        let auth = auth_with_capacity(&key_pair, 1);
        let uri: crate::Uri = "/v1/configuration".parse().unwrap();
        let body = b"{}";
        let first = headers_with_nonce(&key_pair, &uri, body, "nonce-a", true);
        auth.authorize_bytes(&first, &crate::Method::POST, &uri, body)
            .expect("first request accepted");

        let second = headers_with_nonce(&key_pair, &uri, body, "nonce-b", true);
        let err = auth
            .authorize_bytes(&second, &crate::Method::POST, &uri, body)
            .err()
            .expect("full cache rejects new nonce");
        assert_eq!(err.code, "operator_signature_replay_cache_full");

        let err = auth
            .authorize_bytes(&first, &crate::Method::POST, &uri, body)
            .err()
            .expect("live nonce still remembered");
        assert_eq!(err.code, "operator_signature_replay");
    }

    #[test]
    fn operator_signatures_accepts_valid_signature() {
        let key_pair = KeyPair::random();
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
            KeyPair::random().public_key().clone(),
            1024,
            crate::routing::MaybeTelemetry::disabled(),
        );

        let uri: crate::Uri = "/v1/configuration?b=2&a=1".parse().unwrap();
        let body = b"{\"foo\":1}";
        let headers = signed_request_headers(&key_pair, &crate::Method::POST, &uri, body);

        auth.authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect("valid signature");
    }

    #[test]
    fn signed_request_headers_authorize_successfully() {
        let key_pair = KeyPair::random();
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
        let headers = signed_request_headers(&key_pair, &crate::Method::GET, &uri, &[]);

        auth.authorize_bytes(&headers, &crate::Method::GET, &uri, &[])
            .expect("generated headers should verify");
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
