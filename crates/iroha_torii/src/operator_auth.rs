//! WebAuthn/mTLS operator authentication for Torii operator endpoints.

use std::{
    collections::HashSet,
    fs,
    io::Write as _,
    net::IpAddr,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ciborium::{de::from_reader, value::Value as CborValue};
use dashmap::DashMap;
use ed25519_dalek::{Signature as Ed25519Signature, Verifier as _, VerifyingKey as Ed25519Key};
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256Key, signature::Verifier as _};
use rand::TryRngCore as _;
use sha2::{Digest as _, Sha256};
use url::Url;

use iroha_config::parameters::actual::{
    OperatorAuthLockout, OperatorTokenFallback, OperatorTokenSource, OperatorWebAuthnAlgorithm,
    OperatorWebAuthnConfig, ToriiOperatorAuth,
};

use crate::{
    JsonBody, JsonOnly, SharedAppState, json_entry, json_object, json_value, limits,
    routing::MaybeTelemetry,
};

const HEADER_OPERATOR_SESSION: &str = "x-iroha-operator-session";
const HEADER_OPERATOR_TOKEN: &str = "x-iroha-operator-token";
const HEADER_MTLS_FORWARD: &str = "x-forwarded-client-cert";
const HEADER_API_TOKEN: &str = "x-api-token";
const CREDENTIALS_FILENAME: &str = "operator_webauthn.json";
const CHALLENGE_BYTES: usize = 32;
const SESSION_TOKEN_BYTES: usize = 32;

const ACTION_GATE: &str = "gate";
const ACTION_REGISTER_OPTIONS: &str = "register_options";
const ACTION_REGISTER_VERIFY: &str = "register_verify";
const ACTION_LOGIN_OPTIONS: &str = "login_options";
const ACTION_LOGIN_VERIFY: &str = "login_verify";

const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_USER_VERIFIED: u8 = 0x04;
const FLAG_ATTESTED_CREDENTIAL_DATA: u8 = 0x40;

#[derive(Clone, Debug)]
pub struct AuthContext {
    key: String,
}

#[derive(Debug, Clone)]
pub struct OperatorAuthError {
    status: StatusCode,
    code: &'static str,
    message: String,
    metric_label: &'static str,
}

impl OperatorAuthError {
    fn new(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
        metric_label: &'static str,
    ) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            metric_label,
        }
    }

    fn metric_label(&self) -> &'static str {
        self.metric_label
    }

    fn disabled() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "operator_auth_disabled",
            "operator authentication is disabled",
            "disabled",
        )
    }

    fn missing_mtls() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "operator_mtls_required",
            "operator endpoints require mTLS at ingress",
            "missing_mtls",
        )
    }

    fn rate_limited() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "operator_auth_rate_limited",
            "operator auth rate limit exceeded",
            "rate_limited",
        )
    }

    fn locked_out() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "operator_auth_locked",
            "operator auth temporarily locked out",
            "locked_out",
        )
    }

    fn missing_session() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_session_missing",
            "missing operator session token",
            "missing_session",
        )
    }

    fn invalid_session() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_session_invalid",
            "operator session token is invalid or expired",
            "invalid_session",
        )
    }

    fn missing_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_token_missing",
            "missing operator bootstrap token",
            "missing_token",
        )
    }

    fn invalid_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_token_invalid",
            "operator bootstrap token is invalid",
            "invalid_token",
        )
    }

    fn webauthn_disabled() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "operator_webauthn_disabled",
            "WebAuthn operator auth is disabled",
            "webauthn_disabled",
        )
    }

    fn no_credentials() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "operator_webauthn_no_credentials",
            "no operator credentials are enrolled",
            "no_credentials",
        )
    }

    fn invalid_payload(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "operator_webauthn_payload_invalid",
            message,
            "invalid_payload",
        )
    }

    fn challenge_invalid() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_challenge_invalid",
            "webauthn challenge is invalid or expired",
            "challenge_invalid",
        )
    }

    fn origin_denied() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_origin_denied",
            "webauthn origin is not allowed",
            "origin_denied",
        )
    }

    fn credential_unknown() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_credential_unknown",
            "webauthn credential is not registered",
            "credential_unknown",
        )
    }

    fn signature_invalid() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_signature_invalid",
            "webauthn assertion signature failed verification",
            "signature_invalid",
        )
    }

    fn credential_not_allowed() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "operator_webauthn_credential_not_allowed",
            "webauthn credential algorithm is not allowed",
            "credential_not_allowed",
        )
    }

    fn rp_id_mismatch() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_rp_id_mismatch",
            "webauthn rpId hash mismatch",
            "rp_id_mismatch",
        )
    }

    fn user_verification_required() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_user_verification_required",
            "webauthn user verification is required",
            "user_verification_required",
        )
    }

    fn user_presence_required() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_user_presence_required",
            "webauthn user presence is required",
            "user_presence_required",
        )
    }

    fn persistence_failure(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operator_webauthn_persist_failed",
            message,
            "persist_failed",
        )
    }
}

impl IntoResponse for OperatorAuthError {
    fn into_response(self) -> Response {
        operator_auth_error_response(self.status, self.code, &self.message)
    }
}

fn operator_auth_error_response(status: StatusCode, code: &'static str, message: &str) -> Response {
    let payload = json_object(vec![
        json_entry("code", code),
        json_entry("message", message),
    ]);
    let mut resp = JsonBody(payload).into_response();
    *resp.status_mut() = status;
    resp
}

#[derive(Debug)]
pub enum OperatorAuthInitError {
    MissingWebAuthn,
    InvalidWebAuthn(String),
    CredentialLoad(String),
}

impl std::fmt::Display for OperatorAuthInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingWebAuthn => write!(f, "torii.operator_auth.webauthn is required"),
            Self::InvalidWebAuthn(msg) => write!(f, "{msg}"),
            Self::CredentialLoad(msg) => write!(f, "failed to load operator credentials: {msg}"),
        }
    }
}

#[derive(Clone)]
struct WebAuthnPolicy {
    rp_id: String,
    rp_name: String,
    origins: Vec<Url>,
    user_id: Vec<u8>,
    user_name: String,
    user_display_name: String,
    challenge_ttl: Duration,
    session_ttl: Duration,
    require_user_verification: bool,
    allowed_algorithms: Vec<OperatorWebAuthnAlgorithm>,
    rp_id_hash: [u8; 32],
}

impl WebAuthnPolicy {
    fn from_config(config: OperatorWebAuthnConfig) -> Result<Self, OperatorAuthInitError> {
        if config.allowed_algorithms.is_empty() {
            return Err(OperatorAuthInitError::InvalidWebAuthn(
                "torii.operator_auth.webauthn.allowed_algorithms must not be empty".to_owned(),
            ));
        }
        let mut hasher = Sha256::new();
        hasher.update(config.rp_id.as_bytes());
        let rp_id_hash = hasher.finalize().into();
        Ok(Self {
            rp_id: config.rp_id,
            rp_name: config.rp_name,
            origins: config.origins,
            user_id: config.user_id,
            user_name: config.user_name,
            user_display_name: config.user_display_name,
            challenge_ttl: config.challenge_ttl,
            session_ttl: config.session_ttl,
            require_user_verification: config.require_user_verification,
            allowed_algorithms: config.allowed_algorithms,
            rp_id_hash,
        })
    }

    fn challenge_timeout_ms(&self) -> u64 {
        self.challenge_ttl
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

#[derive(Clone, Debug)]
struct StoredCredential {
    id: Vec<u8>,
    public_key: Vec<u8>,
    alg: OperatorWebAuthnAlgorithm,
    sign_count: u32,
    created_at_ms: u64,
}

#[derive(Clone, Debug)]
struct SessionEntry {
    expires_at: Instant,
    credential_id: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ChallengeKind {
    Registration,
    Authentication,
}

#[derive(Clone, Debug)]
struct ChallengeEntry {
    kind: ChallengeKind,
    expires_at: Instant,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct LockoutTracker {
    config: OperatorAuthLockout,
    entries: DashMap<String, FailureEntry>,
}

#[derive(Clone, Debug)]
struct FailureEntry {
    failures: u32,
    window_start: Instant,
    locked_until: Option<Instant>,
}

impl LockoutTracker {
    fn new(config: OperatorAuthLockout) -> Self {
        Self {
            config,
            entries: DashMap::new(),
        }
    }

    fn is_locked(&self, key: &str) -> bool {
        let Some(mut entry) = self.entries.get_mut(key) else {
            return false;
        };
        let Some(until) = entry.locked_until else {
            return false;
        };
        if Instant::now() >= until {
            entry.locked_until = None;
            entry.failures = 0;
            entry.window_start = Instant::now();
            return false;
        }
        true
    }

    fn record_failure(&self, key: &str) -> bool {
        let Some(limit) = self.config.failures else {
            return false;
        };
        let now = Instant::now();
        let mut entry = self.entries.entry(key.to_string()).or_insert(FailureEntry {
            failures: 0,
            window_start: now,
            locked_until: None,
        });
        if entry.locked_until.is_some() {
            return true;
        }
        if now.duration_since(entry.window_start) > self.config.window {
            entry.window_start = now;
            entry.failures = 0;
        }
        entry.failures = entry.failures.saturating_add(1);
        if entry.failures >= limit.get() {
            entry.locked_until = Some(now + self.config.duration);
            return true;
        }
        false
    }

    fn clear(&self, key: &str) {
        self.entries.remove(key);
    }
}

pub struct OperatorAuth {
    enabled: bool,
    require_mtls: bool,
    mtls_trusted_proxy_nets: Vec<limits::IpNet>,
    token_fallback: OperatorTokenFallback,
    token_source: OperatorTokenSource,
    operator_tokens: HashSet<String>,
    api_tokens: Arc<HashSet<String>>,
    webauthn: Option<WebAuthnPolicy>,
    credentials: Arc<RwLock<Vec<StoredCredential>>>,
    sessions: DashMap<String, SessionEntry>,
    challenges: DashMap<String, ChallengeEntry>,
    limiter: limits::RateLimiter,
    lockout: LockoutTracker,
    telemetry: MaybeTelemetry,
    credentials_path: PathBuf,
}

fn rate_per_minute_to_per_sec(rate: NonZeroU32) -> u32 {
    rate.div_ceil(NonZeroU32::new(60).expect("non-zero")).get()
}

impl OperatorAuth {
    pub(crate) fn new(
        config: ToriiOperatorAuth,
        api_tokens: Arc<HashSet<String>>,
        data_dir: PathBuf,
        telemetry: MaybeTelemetry,
    ) -> Result<Self, OperatorAuthInitError> {
        let webauthn = if config.enabled {
            let Some(cfg) = config.webauthn.clone() else {
                return Err(OperatorAuthInitError::MissingWebAuthn);
            };
            Some(WebAuthnPolicy::from_config(cfg)?)
        } else {
            None
        };
        let credentials_path = operator_credentials_path(&data_dir);
        let credentials = if config.enabled {
            load_credentials(&credentials_path).map_err(OperatorAuthInitError::CredentialLoad)?
        } else {
            Vec::new()
        };
        let operator_tokens = config
            .tokens
            .into_iter()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
            .collect();
        let rate_per_sec = config.rate_per_minute.map(rate_per_minute_to_per_sec);
        let burst = config.burst.map(std::num::NonZeroU32::get);
        let limiter = limits::RateLimiter::new(rate_per_sec, burst);
        Ok(Self {
            enabled: config.enabled,
            require_mtls: config.require_mtls,
            mtls_trusted_proxy_nets: limits::parse_cidrs(&config.mtls_trusted_proxy_cidrs),
            token_fallback: config.token_fallback,
            token_source: config.token_source,
            operator_tokens,
            api_tokens,
            webauthn,
            credentials: Arc::new(RwLock::new(credentials)),
            sessions: DashMap::new(),
            challenges: DashMap::new(),
            limiter,
            lockout: LockoutTracker::new(config.lockout),
            telemetry,
            credentials_path,
        })
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn webauthn_policy(&self) -> Result<&WebAuthnPolicy, OperatorAuthError> {
        self.webauthn
            .as_ref()
            .ok_or_else(OperatorAuthError::webauthn_disabled)
    }

    fn credentials_read(&self) -> RwLockReadGuard<'_, Vec<StoredCredential>> {
        match self.credentials.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                iroha_logger::warn!("operator credentials lock poisoned; using last known values");
                poisoned.into_inner()
            }
        }
    }

    fn credentials_write(&self) -> RwLockWriteGuard<'_, Vec<StoredCredential>> {
        match self.credentials.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                iroha_logger::warn!("operator credentials lock poisoned; using last known values");
                poisoned.into_inner()
            }
        }
    }

    fn has_credentials(&self) -> bool {
        !self.credentials_read().is_empty()
    }

    fn token_allowed_for_operator(&self) -> bool {
        matches!(self.token_fallback, OperatorTokenFallback::Always)
    }

    fn token_allowed_for_bootstrap(&self) -> bool {
        match self.token_fallback {
            OperatorTokenFallback::Always => true,
            OperatorTokenFallback::Bootstrap => !self.has_credentials(),
            OperatorTokenFallback::Disabled => false,
        }
    }

    async fn check_common(
        &self,
        headers: &HeaderMap,
        remote_ip: Option<IpAddr>,
        action: &'static str,
    ) -> Result<AuthContext, OperatorAuthError> {
        if self.require_mtls && !mtls_present(headers, remote_ip, &self.mtls_trusted_proxy_nets) {
            let err = OperatorAuthError::missing_mtls();
            self.record_event(action, "denied", err.metric_label());
            return Err(err);
        }
        let key = auth_key(headers);
        if !self.limiter.allow(&key).await {
            let err = OperatorAuthError::rate_limited();
            self.record_event(action, "rate_limited", err.metric_label());
            return Err(err);
        }
        if self.lockout.is_locked(&key) {
            let err = OperatorAuthError::locked_out();
            self.record_event(action, "locked", err.metric_label());
            return Err(err);
        }
        Ok(AuthContext { key })
    }

    pub(crate) async fn authorize_operator_endpoint(
        &self,
        headers: &HeaderMap,
        remote_ip: Option<IpAddr>,
    ) -> Result<(), OperatorAuthError> {
        if !self.enabled {
            return Ok(());
        }
        let ctx = self.check_common(headers, remote_ip, ACTION_GATE).await?;
        if let Some(session) = session_from_headers(headers) {
            if self.session_valid(session) {
                self.record_success(&ctx, ACTION_GATE, "session");
                return Ok(());
            }
            if !self.token_allowed_for_operator() {
                let err = OperatorAuthError::invalid_session();
                self.record_failure(&ctx, ACTION_GATE, err.metric_label());
                return Err(err);
            }
        }

        if self.token_allowed_for_operator() {
            match self.check_token(headers) {
                TokenCheck::Valid(kind) => {
                    self.record_success(&ctx, ACTION_GATE, kind.label());
                    return Ok(());
                }
                TokenCheck::Missing => {
                    let err = OperatorAuthError::missing_token();
                    self.record_failure(&ctx, ACTION_GATE, err.metric_label());
                    return Err(err);
                }
                TokenCheck::Invalid => {
                    let err = OperatorAuthError::invalid_token();
                    self.record_failure(&ctx, ACTION_GATE, err.metric_label());
                    return Err(err);
                }
            }
        }

        let err = OperatorAuthError::missing_session();
        self.record_failure(&ctx, ACTION_GATE, err.metric_label());
        Err(err)
    }

    pub(crate) async fn authorize_bootstrap(
        &self,
        headers: &HeaderMap,
        remote_ip: Option<IpAddr>,
        action: &'static str,
    ) -> Result<AuthContext, OperatorAuthError> {
        if !self.enabled {
            let err = OperatorAuthError::disabled();
            self.record_event(action, "denied", err.metric_label());
            return Err(err);
        }
        let ctx = self.check_common(headers, remote_ip, action).await?;
        if let Some(session) = session_from_headers(headers) {
            if self.session_valid(session) {
                return Ok(ctx);
            }
        }

        if self.token_allowed_for_bootstrap() {
            match self.check_token(headers) {
                TokenCheck::Valid(_) => return Ok(ctx),
                TokenCheck::Missing => {
                    let err = OperatorAuthError::missing_token();
                    self.record_failure(&ctx, action, err.metric_label());
                    return Err(err);
                }
                TokenCheck::Invalid => {
                    let err = OperatorAuthError::invalid_token();
                    self.record_failure(&ctx, action, err.metric_label());
                    return Err(err);
                }
            }
        }

        let err = if session_from_headers(headers).is_some() {
            OperatorAuthError::invalid_session()
        } else {
            OperatorAuthError::missing_session()
        };
        self.record_failure(&ctx, action, err.metric_label());
        Err(err)
    }

    pub(crate) async fn authorize_login(
        &self,
        headers: &HeaderMap,
        remote_ip: Option<IpAddr>,
        action: &'static str,
    ) -> Result<AuthContext, OperatorAuthError> {
        if !self.enabled {
            let err = OperatorAuthError::disabled();
            self.record_event(action, "denied", err.metric_label());
            return Err(err);
        }
        self.check_common(headers, remote_ip, action).await
    }

    pub(crate) fn webauthn_registration_options(
        &self,
        ctx: &AuthContext,
    ) -> Result<norito::json::Value, OperatorAuthError> {
        let policy = self.webauthn_policy().inspect_err(|err| {
            self.record_failure(ctx, ACTION_REGISTER_OPTIONS, err.metric_label());
        })?;
        self.prune_challenges();
        let challenge_bytes = random_bytes(CHALLENGE_BYTES);
        let challenge_b64 = encode_b64url(&challenge_bytes);
        let expires_at = Instant::now() + policy.challenge_ttl;
        self.challenges.insert(
            challenge_b64.clone(),
            ChallengeEntry {
                kind: ChallengeKind::Registration,
                expires_at,
                bytes: challenge_bytes,
            },
        );
        let user_id_b64 = encode_b64url(&policy.user_id);

        let mut params = Vec::new();
        for alg in &policy.allowed_algorithms {
            params.push(json_object(vec![
                json_entry("type", "public-key"),
                json_entry("alg", alg.cose_alg()),
            ]));
        }
        let mut exclude_credentials = Vec::new();
        for credential in self.credentials_read().iter() {
            exclude_credentials.push(json_object(vec![
                json_entry("type", "public-key"),
                json_entry("id", encode_b64url(&credential.id)),
            ]));
        }

        let mut public_key = norito::json::Map::new();
        public_key.insert(
            "rp".into(),
            json_object(vec![
                json_entry("id", policy.rp_id.as_str()),
                json_entry("name", policy.rp_name.as_str()),
            ]),
        );
        public_key.insert(
            "user".into(),
            json_object(vec![
                json_entry("id", user_id_b64),
                json_entry("name", policy.user_name.as_str()),
                json_entry("displayName", policy.user_display_name.as_str()),
            ]),
        );
        public_key.insert("challenge".into(), json_value(&challenge_b64));
        public_key.insert("pubKeyCredParams".into(), json_value(&params));
        public_key.insert("timeout".into(), json_value(&policy.challenge_timeout_ms()));
        public_key.insert("attestation".into(), json_value(&"none"));
        public_key.insert(
            "authenticatorSelection".into(),
            json_object(vec![json_entry(
                "userVerification",
                if policy.require_user_verification {
                    "required"
                } else {
                    "preferred"
                },
            )]),
        );
        if !exclude_credentials.is_empty() {
            public_key.insert(
                "excludeCredentials".into(),
                json_value(&exclude_credentials),
            );
        }

        self.record_success(ctx, ACTION_REGISTER_OPTIONS, "ok");
        Ok(json_object(vec![json_entry(
            "publicKey",
            norito::json::Value::Object(public_key),
        )]))
    }

    fn webauthn_finish_registration(
        &self,
        ctx: &AuthContext,
        payload: &norito::json::Value,
    ) -> Result<RegistrationOutcome, OperatorAuthError> {
        let policy = self.webauthn_policy().inspect_err(|err| {
            self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
        })?;
        let input = parse_registration_payload(payload).inspect_err(|err| {
            self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
        })?;
        let client =
            parse_client_data(&input.client_data_json, "webauthn.create").inspect_err(|err| {
                self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
            })?;
        let _challenge_entry = self
            .take_challenge(&client.challenge, ChallengeKind::Registration)
            .inspect_err(|err| {
                self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
            })?;
        if !origin_allowed(&client.origin, &policy.origins) {
            let err = OperatorAuthError::origin_denied();
            self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
            return Err(err);
        }
        let attestation =
            parse_attestation_object(&input.attestation_object).inspect_err(|err| {
                self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
            })?;
        let auth_data =
            parse_auth_data_registration(&attestation.auth_data, policy).inspect_err(|err| {
                self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
            })?;
        if auth_data.credential_id != input.raw_id {
            let err = OperatorAuthError::invalid_payload("credential id mismatch");
            self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
            return Err(err);
        }
        let created_at_ms = now_ms();
        let credential = StoredCredential {
            id: auth_data.credential_id.clone(),
            public_key: auth_data.cose_key.public_key.clone(),
            alg: auth_data.cose_key.alg,
            sign_count: auth_data.sign_count,
            created_at_ms,
        };
        let total = self.upsert_credential(credential).inspect_err(|err| {
            self.record_failure(ctx, ACTION_REGISTER_VERIFY, err.metric_label());
        })?;
        self.record_success(ctx, ACTION_REGISTER_VERIFY, "ok");
        Ok(RegistrationOutcome {
            credential_id: encode_b64url(&auth_data.credential_id),
            credentials_total: total,
        })
    }

    pub(crate) fn webauthn_authentication_options(
        &self,
        ctx: &AuthContext,
    ) -> Result<norito::json::Value, OperatorAuthError> {
        let policy = self.webauthn_policy().inspect_err(|err| {
            self.record_failure(ctx, ACTION_LOGIN_OPTIONS, err.metric_label());
        })?;
        self.prune_challenges();
        let credentials = self.credentials_read();
        if credentials.is_empty() {
            let err = OperatorAuthError::no_credentials();
            self.record_failure(ctx, ACTION_LOGIN_OPTIONS, err.metric_label());
            return Err(err);
        }
        let challenge_bytes = random_bytes(CHALLENGE_BYTES);
        let challenge_b64 = encode_b64url(&challenge_bytes);
        let expires_at = Instant::now() + policy.challenge_ttl;
        self.challenges.insert(
            challenge_b64.clone(),
            ChallengeEntry {
                kind: ChallengeKind::Authentication,
                expires_at,
                bytes: challenge_bytes,
            },
        );
        let mut allow = Vec::new();
        for credential in credentials.iter() {
            allow.push(json_object(vec![
                json_entry("type", "public-key"),
                json_entry("id", encode_b64url(&credential.id)),
            ]));
        }
        let mut public_key = norito::json::Map::new();
        public_key.insert("challenge".into(), json_value(&challenge_b64));
        public_key.insert("timeout".into(), json_value(&policy.challenge_timeout_ms()));
        public_key.insert("rpId".into(), json_value(&policy.rp_id));
        public_key.insert("allowCredentials".into(), json_value(&allow));
        public_key.insert(
            "userVerification".into(),
            json_value(if policy.require_user_verification {
                "required"
            } else {
                "preferred"
            }),
        );
        self.record_success(ctx, ACTION_LOGIN_OPTIONS, "ok");
        Ok(json_object(vec![json_entry(
            "publicKey",
            norito::json::Value::Object(public_key),
        )]))
    }

    fn webauthn_finish_authentication(
        &self,
        ctx: &AuthContext,
        payload: &norito::json::Value,
    ) -> Result<SessionOutcome, OperatorAuthError> {
        let policy = self.webauthn_policy().inspect_err(|err| {
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
        })?;
        let input = parse_assertion_payload(payload).inspect_err(|err| {
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
        })?;
        let client =
            parse_client_data(&input.client_data_json, "webauthn.get").inspect_err(|err| {
                self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
            })?;
        let _challenge_entry = self
            .take_challenge(&client.challenge, ChallengeKind::Authentication)
            .inspect_err(|err| {
                self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
            })?;
        if !origin_allowed(&client.origin, &policy.origins) {
            let err = OperatorAuthError::origin_denied();
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
            return Err(err);
        }
        let auth_data =
            parse_auth_data_assertion(&input.authenticator_data, policy).inspect_err(|err| {
                self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
            })?;
        let mut credentials = self.credentials_write();
        let Some(pos) = credentials
            .iter()
            .position(|entry| entry.id == input.raw_id)
        else {
            let err = OperatorAuthError::credential_unknown();
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
            return Err(err);
        };
        let credential = credentials.get_mut(pos).expect("position valid");
        let client_hash = Sha256::digest(&input.client_data_json);
        let mut signed_bytes =
            Vec::with_capacity(input.authenticator_data.len() + client_hash.as_slice().len());
        signed_bytes.extend_from_slice(&input.authenticator_data);
        signed_bytes.extend_from_slice(&client_hash);
        verify_signature(
            credential.alg,
            &credential.public_key,
            &signed_bytes,
            &input.signature,
        )
        .inspect_err(|err| {
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
        })?;
        if credential.sign_count != 0
            && auth_data.sign_count != 0
            && auth_data.sign_count <= credential.sign_count
        {
            let err = OperatorAuthError::invalid_payload("webauthn signCount did not advance");
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
            return Err(err);
        }
        credential.sign_count = auth_data.sign_count;
        let updated = credentials.clone();
        drop(credentials);
        persist_credentials(&self.credentials_path, &updated).inspect_err(|err| {
            self.record_failure(ctx, ACTION_LOGIN_VERIFY, err.metric_label());
        })?;
        let outcome = self.issue_session(&input.raw_id, policy.session_ttl);
        self.record_success(ctx, ACTION_LOGIN_VERIFY, "ok");
        Ok(outcome)
    }

    fn issue_session(&self, credential_id: &[u8], ttl: Duration) -> SessionOutcome {
        self.prune_sessions();
        let token_bytes = random_bytes(SESSION_TOKEN_BYTES);
        let token = encode_b64url(&token_bytes);
        let expires_at = Instant::now() + ttl;
        self.sessions.insert(
            token.clone(),
            SessionEntry {
                expires_at,
                credential_id: credential_id.to_vec(),
            },
        );
        SessionOutcome {
            session_token: token,
            expires_in_secs: ttl.as_secs().max(1),
            credential_id: encode_b64url(credential_id),
        }
    }

    fn upsert_credential(&self, credential: StoredCredential) -> Result<usize, OperatorAuthError> {
        let mut credentials = self.credentials_write();
        let mut updated = credentials.clone();
        if let Some(pos) = updated.iter().position(|entry| entry.id == credential.id) {
            updated[pos] = credential;
        } else {
            updated.push(credential);
        }
        persist_credentials(&self.credentials_path, &updated)?;
        *credentials = updated;
        Ok(credentials.len())
    }

    fn take_challenge(
        &self,
        challenge: &str,
        kind: ChallengeKind,
    ) -> Result<ChallengeEntry, OperatorAuthError> {
        self.prune_challenges();
        match self.challenges.remove(challenge) {
            Some((_key, entry)) => {
                if entry.kind != kind {
                    return Err(OperatorAuthError::challenge_invalid());
                }
                if Instant::now() > entry.expires_at {
                    return Err(OperatorAuthError::challenge_invalid());
                }
                Ok(entry)
            }
            None => Err(OperatorAuthError::challenge_invalid()),
        }
    }

    fn check_token(&self, headers: &HeaderMap) -> TokenCheck {
        match self.token_source {
            OperatorTokenSource::OperatorTokens => operator_token(headers)
                .map(|token| {
                    if self.operator_tokens.contains(token) {
                        TokenCheck::Valid(TokenKind::Operator)
                    } else {
                        TokenCheck::Invalid
                    }
                })
                .unwrap_or(TokenCheck::Missing),
            OperatorTokenSource::ApiTokens => api_token(headers)
                .map(|token| {
                    if self.api_tokens.contains(token) {
                        TokenCheck::Valid(TokenKind::Api)
                    } else {
                        TokenCheck::Invalid
                    }
                })
                .unwrap_or(TokenCheck::Missing),
            OperatorTokenSource::Both => {
                if let Some(token) = operator_token(headers) {
                    if self.operator_tokens.contains(token) {
                        return TokenCheck::Valid(TokenKind::Operator);
                    }
                    return TokenCheck::Invalid;
                }
                api_token(headers)
                    .map(|token| {
                        if self.api_tokens.contains(token) {
                            TokenCheck::Valid(TokenKind::Api)
                        } else {
                            TokenCheck::Invalid
                        }
                    })
                    .unwrap_or(TokenCheck::Missing)
            }
        }
    }

    fn record_event(&self, action: &'static str, result: &'static str, reason: &'static str) {
        self.telemetry.with_metrics(|telemetry| {
            telemetry.inc_torii_operator_auth(action, result, reason);
        });
    }

    fn record_lockout(&self, action: &'static str, reason: &'static str) {
        self.telemetry.with_metrics(|telemetry| {
            telemetry.inc_torii_operator_auth_lockout(action, reason);
        });
    }

    fn record_failure(&self, ctx: &AuthContext, action: &'static str, reason: &'static str) {
        self.record_event(action, "denied", reason);
        if self.lockout.record_failure(&ctx.key) {
            self.record_lockout(action, reason);
        }
    }

    fn record_success(&self, ctx: &AuthContext, action: &'static str, reason: &'static str) {
        self.lockout.clear(&ctx.key);
        self.record_event(action, "allowed", reason);
    }

    fn prune_sessions(&self) {
        let now = Instant::now();
        let expired: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| entry.expires_at <= now)
            .map(|entry| entry.key().clone())
            .collect();
        for key in expired {
            self.sessions.remove(&key);
        }
    }

    fn prune_challenges(&self) {
        let now = Instant::now();
        let expired: Vec<String> = self
            .challenges
            .iter()
            .filter(|entry| entry.expires_at <= now)
            .map(|entry| entry.key().clone())
            .collect();
        for key in expired {
            self.challenges.remove(&key);
        }
    }

    fn session_valid(&self, token: &str) -> bool {
        self.prune_sessions();
        self.sessions
            .get(token)
            .is_some_and(|entry| entry.expires_at > Instant::now())
    }
}

/// Result of a successful WebAuthn registration ceremony.
pub struct RegistrationOutcome {
    credential_id: String,
    credentials_total: usize,
}

/// Result of a successful WebAuthn authentication ceremony.
pub struct SessionOutcome {
    session_token: String,
    expires_in_secs: u64,
    credential_id: String,
}

struct RegistrationInput {
    raw_id: Vec<u8>,
    client_data_json: Vec<u8>,
    attestation_object: Vec<u8>,
}

struct AssertionInput {
    raw_id: Vec<u8>,
    client_data_json: Vec<u8>,
    authenticator_data: Vec<u8>,
    signature: Vec<u8>,
}

struct ClientData {
    challenge: String,
    origin: String,
}

struct AttestationObject {
    auth_data: Vec<u8>,
}

struct CoseKey {
    alg: OperatorWebAuthnAlgorithm,
    public_key: Vec<u8>,
}

struct AuthDataRegistration {
    credential_id: Vec<u8>,
    cose_key: CoseKey,
    sign_count: u32,
}

struct AuthDataAssertion {
    sign_count: u32,
}

enum TokenCheck {
    Valid(TokenKind),
    Missing,
    Invalid,
}

#[derive(Clone, Copy)]
enum TokenKind {
    Operator,
    Api,
}

impl TokenKind {
    fn label(self) -> &'static str {
        match self {
            Self::Operator => "operator_token",
            Self::Api => "api_token",
        }
    }
}

fn operator_credentials_path(base: &Path) -> PathBuf {
    base.join("operator_auth").join(CREDENTIALS_FILENAME)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let mut rng = rand::rngs::OsRng;
    rng.try_fill_bytes(&mut buf)
        .expect("operating-system RNG should be available");
    buf
}

fn encode_b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn decode_b64url(label: &'static str, value: &str) -> Result<Vec<u8>, OperatorAuthError> {
    if value.trim().is_empty() {
        return Err(OperatorAuthError::invalid_payload(format!(
            "{label} must not be empty"
        )));
    }
    URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .map_err(|_| OperatorAuthError::invalid_payload(format!("{label} must be base64url")))
}

fn auth_key(headers: &HeaderMap) -> String {
    headers
        .get(limits::REMOTE_ADDR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<IpAddr>().ok())
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "anon".to_string())
}

fn mtls_present(
    headers: &HeaderMap,
    remote: Option<IpAddr>,
    trusted_proxies: &[limits::IpNet],
) -> bool {
    limits::has_trusted_forwarded_header(headers, remote, trusted_proxies, HEADER_MTLS_FORWARD)
}

fn session_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(HEADER_OPERATOR_SESSION)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
}

fn operator_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(HEADER_OPERATOR_TOKEN)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
}

fn api_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(HEADER_API_TOKEN)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
}

fn origin_allowed(origin: &str, allowed: &[Url]) -> bool {
    let Ok(parsed) = Url::parse(origin) else {
        return false;
    };
    let parsed_origin = parsed.origin();
    if matches!(parsed_origin, url::Origin::Opaque(_)) {
        return false;
    }
    allowed
        .iter()
        .any(|candidate| candidate.origin() == parsed_origin)
}

fn load_credentials(path: &Path) -> Result<Vec<StoredCredential>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.to_string()),
    };
    let value: norito::json::Value = norito::json::from_str(&raw).map_err(|err| err.to_string())?;
    let obj = value
        .as_object()
        .ok_or_else(|| "credentials payload must be a JSON object".to_string())?;
    let version = obj
        .get("version")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| "credentials payload missing version".to_string())?;
    if version != 1 {
        return Err(format!("unsupported credentials version {version}"));
    }
    let items = obj
        .get("credentials")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "credentials payload missing credentials array".to_string())?;
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let item_obj = item
            .as_object()
            .ok_or_else(|| "credential entry must be an object".to_string())?;
        let id_b64 = item_obj
            .get("id_b64")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "credential entry missing id_b64".to_string())?;
        let public_key_b64 = item_obj
            .get("public_key_b64")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "credential entry missing public_key_b64".to_string())?;
        let alg_label = item_obj
            .get("alg")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "credential entry missing alg".to_string())?;
        let sign_count = item_obj
            .get("sign_count")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| "credential entry missing sign_count".to_string())?;
        let created_at_ms = item_obj
            .get("created_at_ms")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| "credential entry missing created_at_ms".to_string())?;
        let id = URL_SAFE_NO_PAD
            .decode(id_b64.as_bytes())
            .map_err(|_| "invalid credential id_b64".to_string())?;
        let public_key = URL_SAFE_NO_PAD
            .decode(public_key_b64.as_bytes())
            .map_err(|_| "invalid credential public_key_b64".to_string())?;
        let alg = match alg_label {
            "es256" => OperatorWebAuthnAlgorithm::Es256,
            "ed25519" => OperatorWebAuthnAlgorithm::Ed25519,
            other => return Err(format!("unsupported credential alg {other}")),
        };
        result.push(StoredCredential {
            id,
            public_key,
            alg,
            sign_count: sign_count.min(u64::from(u32::MAX)) as u32,
            created_at_ms,
        });
    }
    Ok(result)
}

fn persist_credentials(
    path: &Path,
    credentials: &[StoredCredential],
) -> Result<(), OperatorAuthError> {
    let mut entries = Vec::with_capacity(credentials.len());
    for credential in credentials {
        let entry = json_object(vec![
            json_entry("id_b64", encode_b64url(&credential.id)),
            json_entry("public_key_b64", encode_b64url(&credential.public_key)),
            json_entry("alg", credential.alg.label()),
            json_entry("sign_count", credential.sign_count),
            json_entry("created_at_ms", credential.created_at_ms),
        ]);
        entries.push(entry);
    }
    let payload = json_object(vec![
        json_entry("version", 1_u64),
        json_entry("credentials", entries),
    ]);
    let body = norito::json::to_json_pretty(&payload).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to serialize credentials: {err}"))
    })?;
    let Some(parent) = path.parent() else {
        return Err(OperatorAuthError::persistence_failure(
            "credentials path has no parent directory",
        ));
    };
    fs::create_dir_all(parent).map_err(|err| {
        OperatorAuthError::persistence_failure(format!(
            "failed to create operator auth directory: {err}"
        ))
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to create temp file: {err}"))
    })?;
    tmp.write_all(body.as_bytes()).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to write credentials: {err}"))
    })?;
    tmp.flush().map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to flush credentials: {err}"))
    })?;
    tmp.persist(path).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to persist credentials: {err}"))
    })?;
    Ok(())
}

fn parse_registration_payload(
    payload: &norito::json::Value,
) -> Result<RegistrationInput, OperatorAuthError> {
    let obj = payload.as_object().ok_or_else(|| {
        OperatorAuthError::invalid_payload("credential payload must be an object")
    })?;
    let id = obj
        .get("rawId")
        .and_then(|value| value.as_str())
        .or_else(|| obj.get("id").and_then(|value| value.as_str()))
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential id missing"))?;
    let response = obj
        .get("response")
        .and_then(|value| value.as_object())
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential response missing"))?;
    let client_data = response
        .get("clientDataJSON")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("clientDataJSON missing"))?;
    let attestation = response
        .get("attestationObject")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("attestationObject missing"))?;
    Ok(RegistrationInput {
        raw_id: decode_b64url("rawId", id)?,
        client_data_json: decode_b64url("clientDataJSON", client_data)?,
        attestation_object: decode_b64url("attestationObject", attestation)?,
    })
}

fn parse_assertion_payload(
    payload: &norito::json::Value,
) -> Result<AssertionInput, OperatorAuthError> {
    let obj = payload.as_object().ok_or_else(|| {
        OperatorAuthError::invalid_payload("credential payload must be an object")
    })?;
    let id = obj
        .get("rawId")
        .and_then(|value| value.as_str())
        .or_else(|| obj.get("id").and_then(|value| value.as_str()))
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential id missing"))?;
    let response = obj
        .get("response")
        .and_then(|value| value.as_object())
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential response missing"))?;
    let client_data = response
        .get("clientDataJSON")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("clientDataJSON missing"))?;
    let authenticator_data = response
        .get("authenticatorData")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("authenticatorData missing"))?;
    let signature = response
        .get("signature")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("signature missing"))?;
    Ok(AssertionInput {
        raw_id: decode_b64url("rawId", id)?,
        client_data_json: decode_b64url("clientDataJSON", client_data)?,
        authenticator_data: decode_b64url("authenticatorData", authenticator_data)?,
        signature: decode_b64url("signature", signature)?,
    })
}

fn parse_client_data(bytes: &[u8], expected_type: &str) -> Result<ClientData, OperatorAuthError> {
    let value: norito::json::Value = norito::json::from_slice(bytes)
        .map_err(|_| OperatorAuthError::invalid_payload("clientDataJSON must be valid JSON"))?;
    let obj = value.as_object().ok_or_else(|| {
        OperatorAuthError::invalid_payload("clientDataJSON must be a JSON object")
    })?;
    let ty = obj
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("clientDataJSON type missing"))?;
    if ty != expected_type {
        return Err(OperatorAuthError::invalid_payload(format!(
            "clientDataJSON type must be {expected_type}"
        )));
    }
    let challenge = obj
        .get("challenge")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("clientDataJSON challenge missing"))?;
    let origin = obj
        .get("origin")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("clientDataJSON origin missing"))?;
    Ok(ClientData {
        challenge: challenge.to_string(),
        origin: origin.to_string(),
    })
}

fn parse_attestation_object(bytes: &[u8]) -> Result<AttestationObject, OperatorAuthError> {
    let value: CborValue = from_reader(bytes)
        .map_err(|_| OperatorAuthError::invalid_payload("attestationObject must be CBOR"))?;
    let map = match value {
        CborValue::Map(map) => map,
        _ => {
            return Err(OperatorAuthError::invalid_payload(
                "attestationObject must be a CBOR map",
            ));
        }
    };
    let auth_data = expect_cbor_bytes(&map, "authData")?;
    Ok(AttestationObject { auth_data })
}

fn parse_auth_data_registration(
    auth_data: &[u8],
    policy: &WebAuthnPolicy,
) -> Result<AuthDataRegistration, OperatorAuthError> {
    if auth_data.len() < 37 + 16 + 2 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData is too short",
        ));
    }
    let rp_id_hash: [u8; 32] = auth_data[0..32].try_into().expect("slice length verified");
    if rp_id_hash != policy.rp_id_hash {
        return Err(OperatorAuthError::rp_id_mismatch());
    }
    let flags = auth_data[32];
    if flags & FLAG_USER_PRESENT == 0 {
        return Err(OperatorAuthError::user_presence_required());
    }
    if policy.require_user_verification && flags & FLAG_USER_VERIFIED == 0 {
        return Err(OperatorAuthError::user_verification_required());
    }
    if flags & FLAG_ATTESTED_CREDENTIAL_DATA == 0 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData missing attested credential data",
        ));
    }
    let sign_count =
        u32::from_be_bytes(auth_data[33..37].try_into().expect("slice length verified"));
    let mut offset = 37 + 16;
    let credential_len = u16::from_be_bytes(
        auth_data[offset..offset + 2]
            .try_into()
            .expect("slice length verified"),
    ) as usize;
    offset += 2;
    if auth_data.len() < offset + credential_len {
        return Err(OperatorAuthError::invalid_payload(
            "credential id extends past authenticatorData",
        ));
    }
    let credential_id = auth_data[offset..offset + credential_len].to_vec();
    offset += credential_len;
    let cose_value: CborValue = from_reader(&auth_data[offset..])
        .map_err(|_| OperatorAuthError::invalid_payload("credential public key must be CBOR"))?;
    let cose_key = parse_cose_key(&cose_value, &policy.allowed_algorithms)?;
    Ok(AuthDataRegistration {
        credential_id,
        cose_key,
        sign_count,
    })
}

fn parse_auth_data_assertion(
    auth_data: &[u8],
    policy: &WebAuthnPolicy,
) -> Result<AuthDataAssertion, OperatorAuthError> {
    if auth_data.len() < 37 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData is too short",
        ));
    }
    let rp_id_hash: [u8; 32] = auth_data[0..32].try_into().expect("slice length verified");
    if rp_id_hash != policy.rp_id_hash {
        return Err(OperatorAuthError::rp_id_mismatch());
    }
    let flags = auth_data[32];
    if flags & FLAG_USER_PRESENT == 0 {
        return Err(OperatorAuthError::user_presence_required());
    }
    if policy.require_user_verification && flags & FLAG_USER_VERIFIED == 0 {
        return Err(OperatorAuthError::user_verification_required());
    }
    let sign_count =
        u32::from_be_bytes(auth_data[33..37].try_into().expect("slice length verified"));
    Ok(AuthDataAssertion { sign_count })
}

fn parse_cose_key(
    value: &CborValue,
    allowed: &[OperatorWebAuthnAlgorithm],
) -> Result<CoseKey, OperatorAuthError> {
    let map = match value {
        CborValue::Map(map) => map,
        _ => {
            return Err(OperatorAuthError::invalid_payload(
                "credential public key must be a CBOR map",
            ));
        }
    };
    let kty = cbor_int(expect_cbor_value(map, 1)?)?;
    let alg = cbor_int(expect_cbor_value(map, 3)?)?;
    let crv = cbor_int(expect_cbor_value(map, -1)?)?;
    let x = expect_cbor_bytes_i(map, -2)?;
    match (kty, alg, crv) {
        (2, -7, 1) => {
            let y = expect_cbor_bytes_i(map, -3)?;
            let mut public_key = Vec::with_capacity(65);
            public_key.push(0x04);
            public_key.extend_from_slice(&x);
            public_key.extend_from_slice(&y);
            if !allowed.contains(&OperatorWebAuthnAlgorithm::Es256) {
                return Err(OperatorAuthError::credential_not_allowed());
            }
            Ok(CoseKey {
                alg: OperatorWebAuthnAlgorithm::Es256,
                public_key,
            })
        }
        (1, -8, 6) => {
            if !allowed.contains(&OperatorWebAuthnAlgorithm::Ed25519) {
                return Err(OperatorAuthError::credential_not_allowed());
            }
            Ok(CoseKey {
                alg: OperatorWebAuthnAlgorithm::Ed25519,
                public_key: x,
            })
        }
        _ => Err(OperatorAuthError::invalid_payload(
            "unsupported COSE key parameters",
        )),
    }
}

fn verify_signature(
    alg: OperatorWebAuthnAlgorithm,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), OperatorAuthError> {
    match alg {
        OperatorWebAuthnAlgorithm::Es256 => {
            let encoded = p256::EncodedPoint::from_bytes(public_key).map_err(|_| {
                OperatorAuthError::invalid_payload("invalid ES256 public key encoding")
            })?;
            let verifying_key = P256Key::from_encoded_point(&encoded)
                .map_err(|_| OperatorAuthError::invalid_payload("invalid ES256 public key"))?;
            let sig = P256Signature::from_der(signature)
                .map_err(|_| OperatorAuthError::signature_invalid())?;
            verifying_key
                .verify(message, &sig)
                .map_err(|_| OperatorAuthError::signature_invalid())
        }
        OperatorWebAuthnAlgorithm::Ed25519 => {
            let pk: [u8; 32] = public_key
                .try_into()
                .map_err(|_| OperatorAuthError::invalid_payload("invalid Ed25519 key length"))?;
            let sig: [u8; 64] = signature
                .try_into()
                .map_err(|_| OperatorAuthError::signature_invalid())?;
            let verifying_key =
                Ed25519Key::from_bytes(&pk).map_err(|_| OperatorAuthError::signature_invalid())?;
            let signature = Ed25519Signature::from_bytes(&sig);
            verifying_key
                .verify_strict(message, &signature)
                .map_err(|_| OperatorAuthError::signature_invalid())
        }
    }
}

fn cbor_int(value: &CborValue) -> Result<i128, OperatorAuthError> {
    match value {
        CborValue::Integer(value) => Ok(i128::from(value.clone())),
        _ => Err(OperatorAuthError::invalid_payload(
            "COSE value must be an integer",
        )),
    }
}

fn expect_cbor_value(
    map: &[(CborValue, CborValue)],
    key: i128,
) -> Result<&CborValue, OperatorAuthError> {
    map.iter()
        .find(|(candidate, _)| match candidate {
            CborValue::Integer(value) => i128::from(value.clone()) == key,
            _ => false,
        })
        .map(|(_, value)| value)
        .ok_or_else(|| OperatorAuthError::invalid_payload("missing COSE key entry"))
}

fn expect_cbor_bytes(
    map: &[(CborValue, CborValue)],
    key: &str,
) -> Result<Vec<u8>, OperatorAuthError> {
    map.iter()
        .find(|(candidate, _)| matches!(candidate, CborValue::Text(text) if text == key))
        .and_then(|(_, value)| match value {
            CborValue::Bytes(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .ok_or_else(|| OperatorAuthError::invalid_payload("missing CBOR bytes entry"))
}

fn expect_cbor_bytes_i(
    map: &[(CborValue, CborValue)],
    key: i128,
) -> Result<Vec<u8>, OperatorAuthError> {
    match expect_cbor_value(map, key)? {
        CborValue::Bytes(bytes) => Ok(bytes.clone()),
        _ => Err(OperatorAuthError::invalid_payload(
            "COSE bytes entry must be a byte array",
        )),
    }
}

pub async fn enforce_operator_auth(
    State(app): State<SharedAppState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Result<Response, std::convert::Infallible> {
    let remote_ip = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip());
    if let Err(err) = app
        .operator_auth
        .authorize_operator_endpoint(req.headers(), remote_ip)
        .await
    {
        return Ok(err.into_response());
    }
    Ok(next.run(req).await)
}

pub async fn handle_operator_register_options(
    State(app): State<SharedAppState>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let ctx = app
        .operator_auth
        .authorize_bootstrap(&headers, Some(remote.ip()), ACTION_REGISTER_OPTIONS)
        .await?;
    let payload = app.operator_auth.webauthn_registration_options(&ctx)?;
    Ok(JsonBody(payload))
}

pub async fn handle_operator_register_verify(
    State(app): State<SharedAppState>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    JsonOnly(payload): JsonOnly<norito::json::Value>,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let ctx = app
        .operator_auth
        .authorize_bootstrap(&headers, Some(remote.ip()), ACTION_REGISTER_VERIFY)
        .await?;
    let outcome = app
        .operator_auth
        .webauthn_finish_registration(&ctx, &payload)?;
    let response = json_object(vec![
        json_entry("status", "ok"),
        json_entry("credential_id", outcome.credential_id),
        json_entry("credentials_total", outcome.credentials_total),
    ]);
    Ok(JsonBody(response))
}

pub async fn handle_operator_login_options(
    State(app): State<SharedAppState>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let ctx = app
        .operator_auth
        .authorize_login(&headers, Some(remote.ip()), ACTION_LOGIN_OPTIONS)
        .await?;
    let payload = app.operator_auth.webauthn_authentication_options(&ctx)?;
    Ok(JsonBody(payload))
}

pub async fn handle_operator_login_verify(
    State(app): State<SharedAppState>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    JsonOnly(payload): JsonOnly<norito::json::Value>,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let ctx = app
        .operator_auth
        .authorize_login(&headers, Some(remote.ip()), ACTION_LOGIN_VERIFY)
        .await?;
    let outcome = app
        .operator_auth
        .webauthn_finish_authentication(&ctx, &payload)?;
    let response = json_object(vec![
        json_entry("status", "ok"),
        json_entry("session_token", outcome.session_token),
        json_entry("expires_in_secs", outcome.expires_in_secs),
        json_entry("credential_id", outcome.credential_id),
    ]);
    Ok(JsonBody(response))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use axum::http::HeaderValue;
    use ciborium::ser::into_writer;
    use ed25519_dalek::Signer as _;
    use p256::{
        ecdsa::{SigningKey, signature::Signer as _},
        elliptic_curve::rand_core::OsRng,
    };

    use super::*;

    fn base_webauthn_config(algorithms: Vec<OperatorWebAuthnAlgorithm>) -> OperatorWebAuthnConfig {
        OperatorWebAuthnConfig {
            rp_id: "example.com".to_owned(),
            rp_name: "Iroha Operator".to_owned(),
            origins: vec![Url::parse("https://example.com").expect("origin")],
            user_id: b"operator".to_vec(),
            user_name: "operator".to_owned(),
            user_display_name: "Operator".to_owned(),
            challenge_ttl: Duration::from_secs(120),
            session_ttl: Duration::from_secs(600),
            require_user_verification: true,
            allowed_algorithms: algorithms,
        }
    }

    fn base_operator_auth_config(
        token_fallback: OperatorTokenFallback,
        token_source: OperatorTokenSource,
        tokens: Vec<String>,
        lockout: OperatorAuthLockout,
        algorithms: Vec<OperatorWebAuthnAlgorithm>,
    ) -> ToriiOperatorAuth {
        ToriiOperatorAuth {
            enabled: true,
            require_mtls: false,
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::operator_auth::mtls_trusted_proxy_cidrs(),
            token_fallback,
            token_source,
            tokens,
            rate_per_minute: None,
            burst: None,
            lockout,
            webauthn: Some(base_webauthn_config(algorithms)),
        }
    }

    fn build_operator_auth(
        config: ToriiOperatorAuth,
        api_tokens: HashSet<String>,
        data_dir: &Path,
    ) -> OperatorAuth {
        OperatorAuth::new(
            config,
            Arc::new(api_tokens),
            data_dir.to_path_buf(),
            MaybeTelemetry::disabled(),
        )
        .expect("operator auth")
    }

    fn base_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            limits::REMOTE_ADDR_HEADER,
            HeaderValue::from_static("127.0.0.1"),
        );
        headers
    }

    fn loopback_ip() -> Option<IpAddr> {
        Some("127.0.0.1".parse().expect("loopback ip"))
    }

    fn loopback_connect_info() -> ConnectInfo<std::net::SocketAddr> {
        ConnectInfo("127.0.0.1:8080".parse().expect("loopback socket"))
    }

    #[test]
    fn origin_allows_default_port_and_trailing_slash() {
        let allowed = vec![Url::parse("https://example.com").expect("origin")];
        assert!(origin_allowed("https://example.com/", &allowed));
        assert!(origin_allowed("https://example.com:443", &allowed));
        assert!(!origin_allowed("https://example.com:444", &allowed));
    }

    #[test]
    fn credentials_lock_recovers_from_poison() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            OperatorTokenFallback::Always,
            OperatorTokenSource::OperatorTokens,
            Vec::new(),
            OperatorAuthLockout {
                failures: None,
                window: Duration::from_secs(0),
                duration: Duration::from_secs(0),
            },
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, HashSet::new(), tempdir.path());
        {
            let mut creds = auth.credentials_write();
            creds.push(StoredCredential {
                id: vec![1, 2, 3],
                public_key: vec![4, 5, 6],
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 0,
                created_at_ms: 0,
            });
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = auth.credentials.write().expect("lock");
            panic!("poison");
        }));
        assert!(auth.has_credentials());
    }

    #[test]
    fn rate_per_minute_to_per_sec_rounds_up() {
        assert_eq!(
            rate_per_minute_to_per_sec(NonZeroU32::new(1).expect("non-zero")),
            1
        );
        assert_eq!(
            rate_per_minute_to_per_sec(NonZeroU32::new(60).expect("non-zero")),
            1
        );
        assert_eq!(
            rate_per_minute_to_per_sec(NonZeroU32::new(61).expect("non-zero")),
            2
        );
    }

    fn headers_with_operator_token(token: &str) -> HeaderMap {
        let mut headers = base_headers();
        headers.insert(
            HEADER_OPERATOR_TOKEN,
            HeaderValue::from_str(token).expect("token"),
        );
        headers
    }

    fn headers_with_api_token(token: &str) -> HeaderMap {
        let mut headers = base_headers();
        headers.insert(
            HEADER_API_TOKEN,
            HeaderValue::from_str(token).expect("api token"),
        );
        headers
    }

    fn extract_challenge(payload: &norito::json::Value) -> String {
        let obj = payload.as_object().expect("payload object");
        let public_key = obj
            .get("publicKey")
            .and_then(norito::json::Value::as_object)
            .expect("publicKey object");
        public_key
            .get("challenge")
            .and_then(norito::json::Value::as_str)
            .expect("challenge")
            .to_string()
    }

    fn build_client_data(challenge: &str, origin: &str, ty: &str) -> Vec<u8> {
        let payload = json_object(vec![
            json_entry("type", ty),
            json_entry("challenge", challenge),
            json_entry("origin", origin),
        ]);
        let json = norito::json::to_json(&payload).expect("clientDataJSON");
        json.into_bytes()
    }

    fn build_attestation_object(auth_data: Vec<u8>) -> Vec<u8> {
        let map = vec![(
            CborValue::Text("authData".to_owned()),
            CborValue::Bytes(auth_data),
        )];
        let mut bytes = Vec::new();
        into_writer(&CborValue::Map(map), &mut bytes).expect("attestationObject");
        bytes
    }

    fn build_cose_key_es256(signing_key: &SigningKey) -> Vec<u8> {
        let verifying_key = signing_key.verifying_key();
        let point = verifying_key.to_encoded_point(false);
        let x = point.x().expect("x coordinate").to_vec();
        let y = point.y().expect("y coordinate").to_vec();
        let map = vec![
            (CborValue::Integer(1.into()), CborValue::Integer(2.into())),
            (
                CborValue::Integer(3.into()),
                CborValue::Integer((-7).into()),
            ),
            (
                CborValue::Integer((-1).into()),
                CborValue::Integer(1.into()),
            ),
            (CborValue::Integer((-2).into()), CborValue::Bytes(x)),
            (CborValue::Integer((-3).into()), CborValue::Bytes(y)),
        ];
        let mut bytes = Vec::new();
        into_writer(&CborValue::Map(map), &mut bytes).expect("cose key");
        bytes
    }

    fn build_auth_data_registration(
        policy: &WebAuthnPolicy,
        credential_id: &[u8],
        cose_key: &[u8],
        sign_count: u32,
    ) -> Vec<u8> {
        let mut auth_data = Vec::new();
        auth_data.extend_from_slice(&policy.rp_id_hash);
        let mut flags = FLAG_USER_PRESENT | FLAG_ATTESTED_CREDENTIAL_DATA;
        if policy.require_user_verification {
            flags |= FLAG_USER_VERIFIED;
        }
        auth_data.push(flags);
        auth_data.extend_from_slice(&sign_count.to_be_bytes());
        auth_data.extend_from_slice(&[0u8; 16]);
        auth_data.extend_from_slice(&(credential_id.len() as u16).to_be_bytes());
        auth_data.extend_from_slice(credential_id);
        auth_data.extend_from_slice(cose_key);
        auth_data
    }

    fn build_auth_data_assertion(policy: &WebAuthnPolicy, sign_count: u32) -> Vec<u8> {
        let mut auth_data = Vec::new();
        auth_data.extend_from_slice(&policy.rp_id_hash);
        let mut flags = FLAG_USER_PRESENT;
        if policy.require_user_verification {
            flags |= FLAG_USER_VERIFIED;
        }
        auth_data.push(flags);
        auth_data.extend_from_slice(&sign_count.to_be_bytes());
        auth_data
    }

    fn build_registration_payload(
        credential_id: &[u8],
        client_data_json: &[u8],
        attestation_object: &[u8],
    ) -> norito::json::Value {
        let response = json_object(vec![
            json_entry("clientDataJSON", encode_b64url(client_data_json)),
            json_entry("attestationObject", encode_b64url(attestation_object)),
        ]);
        json_object(vec![
            json_entry("rawId", encode_b64url(credential_id)),
            json_entry("response", response),
        ])
    }

    fn build_assertion_payload(
        credential_id: &[u8],
        client_data_json: &[u8],
        authenticator_data: &[u8],
        signature: &[u8],
    ) -> norito::json::Value {
        let response = json_object(vec![
            json_entry("clientDataJSON", encode_b64url(client_data_json)),
            json_entry("authenticatorData", encode_b64url(authenticator_data)),
            json_entry("signature", encode_b64url(signature)),
        ]);
        json_object(vec![
            json_entry("rawId", encode_b64url(credential_id)),
            json_entry("response", response),
        ])
    }

    #[tokio::test]
    async fn operator_auth_registration_login_and_rollover_es256() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            OperatorTokenFallback::Bootstrap,
            OperatorTokenSource::OperatorTokens,
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, HashSet::new(), tempdir.path());

        let headers = headers_with_operator_token("bootstrap");
        let ctx = auth
            .authorize_bootstrap(&headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect("bootstrap allowed");
        let options = auth.webauthn_registration_options(&ctx).expect("options");
        let challenge = extract_challenge(&options);

        let signing_key = SigningKey::random(&mut OsRng);
        let credential_id = random_bytes(16);
        let policy = auth.webauthn_policy().expect("policy");
        let cose_key = build_cose_key_es256(&signing_key);
        let auth_data = build_auth_data_registration(policy, &credential_id, &cose_key, 1);
        let client_data_json =
            build_client_data(&challenge, "https://example.com", "webauthn.create");
        let attestation_object = build_attestation_object(auth_data);
        let payload =
            build_registration_payload(&credential_id, &client_data_json, &attestation_object);
        let outcome = auth
            .webauthn_finish_registration(&ctx, &payload)
            .expect("registration");
        assert_eq!(outcome.credentials_total, 1);

        let err = auth
            .authorize_bootstrap(&headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect_err("token bootstrap denied after enrollment");
        assert_eq!(err.code, "operator_session_missing");

        let login_ctx = auth
            .authorize_login(&base_headers(), loopback_ip(), ACTION_LOGIN_OPTIONS)
            .await
            .expect("login allowed");
        let login_options = auth
            .webauthn_authentication_options(&login_ctx)
            .expect("login options");
        let login_challenge = extract_challenge(&login_options);
        let assertion_auth_data = build_auth_data_assertion(policy, 2);
        let client_data_json =
            build_client_data(&login_challenge, "https://example.com", "webauthn.get");
        let client_hash = Sha256::digest(&client_data_json);
        let mut signed_bytes =
            Vec::with_capacity(assertion_auth_data.len() + client_hash.as_slice().len());
        signed_bytes.extend_from_slice(&assertion_auth_data);
        signed_bytes.extend_from_slice(&client_hash);
        let signature: p256::ecdsa::Signature = signing_key.sign(&signed_bytes);
        let payload = build_assertion_payload(
            &credential_id,
            &client_data_json,
            &assertion_auth_data,
            signature.to_der().as_bytes(),
        );
        let session = auth
            .webauthn_finish_authentication(&login_ctx, &payload)
            .expect("login verify");
        assert!(auth.session_valid(&session.session_token));

        let mut session_headers = base_headers();
        session_headers.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_str(&session.session_token).expect("session token"),
        );
        auth.authorize_operator_endpoint(&session_headers, loopback_ip())
            .await
            .expect("session accepted");

        let ctx = auth
            .authorize_bootstrap(&session_headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect("session bootstrap");
        let options = auth.webauthn_registration_options(&ctx).expect("options");
        let challenge = extract_challenge(&options);
        let signing_key = SigningKey::random(&mut OsRng);
        let credential_id = random_bytes(16);
        let cose_key = build_cose_key_es256(&signing_key);
        let auth_data = build_auth_data_registration(policy, &credential_id, &cose_key, 1);
        let client_data_json =
            build_client_data(&challenge, "https://example.com", "webauthn.create");
        let attestation_object = build_attestation_object(auth_data);
        let payload =
            build_registration_payload(&credential_id, &client_data_json, &attestation_object);
        let outcome = auth
            .webauthn_finish_registration(&ctx, &payload)
            .expect("rollover registration");
        assert_eq!(outcome.credentials_total, 2);
    }

    #[tokio::test]
    async fn operator_auth_accepts_operator_token_when_fallback_always() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            OperatorTokenFallback::Always,
            OperatorTokenSource::OperatorTokens,
            vec!["operator-token".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, HashSet::new(), tempdir.path());

        let headers = headers_with_operator_token("operator-token");
        auth.authorize_operator_endpoint(&headers, loopback_ip())
            .await
            .expect("operator token allowed");
    }

    #[tokio::test]
    async fn operator_auth_accepts_api_token_when_configured() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            OperatorTokenFallback::Always,
            OperatorTokenSource::ApiTokens,
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let mut api_tokens = HashSet::new();
        api_tokens.insert("api-token".to_owned());
        let auth = build_operator_auth(config, api_tokens, tempdir.path());

        let headers = headers_with_api_token("api-token");
        auth.authorize_operator_endpoint(&headers, loopback_ip())
            .await
            .expect("api token allowed");
    }

    #[tokio::test]
    async fn operator_auth_enforces_mtls_and_lockout() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lockout = OperatorAuthLockout {
            failures: std::num::NonZeroU32::new(2),
            ..OperatorAuthLockout::default()
        };
        let mut config = base_operator_auth_config(
            OperatorTokenFallback::Always,
            OperatorTokenSource::OperatorTokens,
            vec!["valid".to_owned()],
            lockout,
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.require_mtls = true;
        let auth = build_operator_auth(config, HashSet::new(), tempdir.path());

        let err = auth
            .authorize_login(&base_headers(), None, ACTION_LOGIN_OPTIONS)
            .await
            .expect_err("missing mTLS");
        assert_eq!(err.code, "operator_mtls_required");

        let mut headers = base_headers();
        headers.insert(
            HEADER_MTLS_FORWARD,
            HeaderValue::from_static("cert=present"),
        );
        let _ = auth
            .authorize_operator_endpoint(&headers, loopback_ip())
            .await;
        let _ = auth
            .authorize_operator_endpoint(&headers, loopback_ip())
            .await;
        let err = auth
            .authorize_operator_endpoint(&headers, loopback_ip())
            .await
            .expect_err("locked out");
        assert_eq!(err.code, "operator_auth_locked");
    }

    #[tokio::test]
    async fn operator_auth_rejects_forwarded_mtls_from_untrusted_proxy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            OperatorTokenFallback::Always,
            OperatorTokenSource::OperatorTokens,
            vec!["valid".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.require_mtls = true;
        let auth = build_operator_auth(config, HashSet::new(), tempdir.path());

        let mut headers = base_headers();
        headers.insert(
            HEADER_MTLS_FORWARD,
            HeaderValue::from_static("cert=present"),
        );
        let err = auth
            .authorize_login(
                &headers,
                Some("198.51.100.10".parse().expect("untrusted proxy")),
                ACTION_LOGIN_OPTIONS,
            )
            .await
            .expect_err("untrusted proxy must not satisfy mTLS");
        assert_eq!(err.code, "operator_mtls_required");
    }

    #[test]
    fn ed25519_cose_key_and_signature_verify() {
        let mut rng = OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        let public_key = signing_key.verifying_key().to_bytes();
        let map = vec![
            (CborValue::Integer(1.into()), CborValue::Integer(1.into())),
            (
                CborValue::Integer(3.into()),
                CborValue::Integer((-8).into()),
            ),
            (
                CborValue::Integer((-1).into()),
                CborValue::Integer(6.into()),
            ),
            (
                CborValue::Integer((-2).into()),
                CborValue::Bytes(public_key.to_vec()),
            ),
        ];
        let cose_key = CborValue::Map(map);
        let parsed = parse_cose_key(&cose_key, &[OperatorWebAuthnAlgorithm::Ed25519])
            .expect("parse cose key");
        assert_eq!(parsed.alg, OperatorWebAuthnAlgorithm::Ed25519);
        assert_eq!(parsed.public_key, public_key.to_vec());

        let message = b"operator-auth-test";
        let signature = signing_key.sign(message).to_bytes();
        verify_signature(
            OperatorWebAuthnAlgorithm::Ed25519,
            &public_key,
            message,
            &signature,
        )
        .expect("signature ok");
    }

    #[tokio::test]
    async fn operator_auth_handlers_reject_when_disabled() {
        let app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let err = handle_operator_register_options(
            State(app.clone()),
            loopback_connect_info(),
            headers.clone(),
        )
        .await
        .err()
        .expect("register options disabled");
        assert_eq!(err.code, "operator_auth_disabled");

        let err = handle_operator_login_options(
            State(app.clone()),
            loopback_connect_info(),
            headers.clone(),
        )
        .await
        .err()
        .expect("login options disabled");
        assert_eq!(err.code, "operator_auth_disabled");
    }

    #[test]
    fn operator_auth_error_response_sets_status() {
        let response = OperatorAuthError::missing_token().into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
