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
use iroha_crypto::{PublicKey, Signature};

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
        public_key: &PublicKey,
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

        self.ensure_freshness(timestamp_ms, nonce, &public_key)?;

        let msg = Self::operator_request_message(method, uri, body, timestamp_ms, nonce);
        signature
            .verify(&public_key, &msg)
            .map_err(|_| OperatorSignatureError::bad_signature())?;

        Ok(())
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

    fn signed_headers(
        key_pair: &KeyPair,
        method: &crate::Method,
        uri: &crate::Uri,
        body: &[u8],
        timestamp_ms: u64,
        nonce: &str,
    ) -> HeaderMap {
        let msg =
            OperatorSignatures::operator_request_message(method, uri, body, timestamp_ms, nonce);
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
            timestamp_ms.to_string().parse().expect("timestamp header"),
        );
        headers.insert(HEADER_OPERATOR_NONCE, nonce.parse().expect("nonce header"));
        headers.insert(
            HEADER_OPERATOR_SIGNATURE,
            BASE64_STANDARD
                .encode(signature.payload())
                .parse()
                .expect("signature header"),
        );
        headers
    }

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
        let headers = signed_headers(&key_pair, &crate::Method::POST, &uri, body, ts, nonce);

        auth.authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect("first use ok");
        let err = auth
            .authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .err()
            .expect("second use rejected");
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
        let ts = OperatorSignatures::now_unix_ms();
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);
        let headers = signed_headers(&key_pair, &crate::Method::POST, &uri, body, ts, &nonce);

        auth.authorize_bytes(&headers, &crate::Method::POST, &uri, body)
            .expect("valid signature");
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
