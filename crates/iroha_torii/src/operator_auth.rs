//! WebAuthn and mTLS authentication for Torii operator endpoints.
//!
//! Ordinary operator routes still require their exact-network request signature at middleware;
//! a WebAuthn session is an additional gate, not a substitute. The four credential-exchange
//! routes are the deliberate exception: mTLS plus a first-credential bootstrap token or an
//! authenticated WebAuthn session owns enrollment, and a verified assertion owns session issue.
use crate::{
    JsonBody, JsonOnly, SharedAppState, json_entry, json_object, json_value, limits,
    routing::MaybeTelemetry,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ciborium::{de::from_reader, value::Value as CborValue};
use iroha_config::parameters::actual::{
    OperatorAuthLockout, OperatorWebAuthnAlgorithm, OperatorWebAuthnConfig, ToriiOperatorAuth,
};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256Key, signature::Verifier as _};
use parking_lot::Mutex;
use rand::rand_core::{TryCryptoRng, TryRngCore as _};
use sha2::{Digest as _, Sha256};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
    fs,
    io::{Cursor, Read as _, Write as _},
    net::IpAddr,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;
const HEADER_OPERATOR_SESSION: &str = "x-iroha-operator-session";
const HEADER_OPERATOR_TOKEN: &str = "x-iroha-operator-token";
const HEADER_MTLS_FORWARD: &str = "x-forwarded-client-cert";
const CREDENTIALS_FILENAME: &str = "operator_webauthn.json";
/// Maximum accepted JSON body size for one operator WebAuthn exchange request.
pub(crate) const CREDENTIAL_EXCHANGE_BODY_LIMIT: usize = 64 * 1024;
const CHALLENGE_BYTES: usize = 32;
const SESSION_TOKEN_BYTES: usize = 32;
const SESSION_TOKEN_B64URL_BYTES: usize = (SESSION_TOKEN_BYTES * 4 + 2) / 3;
const SESSION_TOKEN_DECODE_BUFFER_BYTES: usize = SESSION_TOKEN_BYTES + 1;
const MAX_CREDENTIAL_ID_BYTES: usize = 1_024;
const MAX_CREDENTIAL_ID_B64URL_BYTES: usize = (MAX_CREDENTIAL_ID_BYTES * 4 + 2) / 3;
const P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN: usize = 65;
const MAX_CREDENTIAL_RECORD_JSON_BYTES: usize = 2_048;
const CREDENTIAL_FILE_JSON_OVERHEAD_BYTES: usize = 128;
const ACTION_GATE: &str = "gate";
const ACTION_REGISTER_OPTIONS: &str = "register_options";
const ACTION_REGISTER_VERIFY: &str = "register_verify";
const ACTION_LOGIN_OPTIONS: &str = "login_options";
const ACTION_LOGIN_VERIFY: &str = "login_verify";
const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_USER_VERIFIED: u8 = 0x04;
const FLAG_BACKUP_ELIGIBLE: u8 = 0x08;
const FLAG_BACKUP_STATE: u8 = 0x10;
const FLAG_ATTESTED_CREDENTIAL_DATA: u8 = 0x40;
const FLAG_EXTENSION_DATA: u8 = 0x80;
const RESERVED_AUTHENTICATOR_FLAGS: u8 = 0x22;
#[derive(Clone, Debug)]
pub struct AuthContext {
    key: String,
    enrollment_authority: EnrollmentAuthority,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnrollmentAuthority {
    None,
    BootstrapToken,
    Session(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionHeader<'a> {
    Missing,
    Invalid,
    Valid(&'a str),
}
#[derive(Debug, Clone)]
pub struct OperatorAuthError {
    status: StatusCode,
    code: &'static str,
    message: String,
    metric_label: &'static str,
    counts_toward_lockout: bool,
}
impl OperatorAuthError {
    fn denial(
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
            counts_toward_lockout: true,
        }
    }
    fn operational(
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
            counts_toward_lockout: false,
        }
    }
    fn metric_label(&self) -> &'static str {
        self.metric_label
    }
    fn disabled() -> Self {
        Self::operational(
            StatusCode::FORBIDDEN,
            "operator_auth_disabled",
            "operator authentication is disabled",
            "disabled",
        )
    }
    fn missing_mtls() -> Self {
        Self::denial(
            StatusCode::FORBIDDEN,
            "operator_mtls_required",
            "operator endpoints require mTLS at ingress",
            "missing_mtls",
        )
    }
    fn rate_limited() -> Self {
        Self::operational(
            StatusCode::TOO_MANY_REQUESTS,
            "operator_auth_rate_limited",
            "operator auth rate limit exceeded",
            "rate_limited",
        )
    }
    fn locked_out() -> Self {
        Self::operational(
            StatusCode::TOO_MANY_REQUESTS,
            "operator_auth_locked",
            "operator auth temporarily locked out",
            "locked_out",
        )
    }
    fn missing_session() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_session_missing",
            "missing operator session token",
            "missing_session",
        )
    }
    fn invalid_session() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_session_invalid",
            "operator session token is invalid or expired",
            "invalid_session",
        )
    }
    fn missing_token() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_token_missing",
            "missing operator bootstrap token",
            "missing_token",
        )
    }
    fn invalid_token() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_token_invalid",
            "operator bootstrap token is invalid",
            "invalid_token",
        )
    }
    fn webauthn_disabled() -> Self {
        Self::operational(
            StatusCode::FORBIDDEN,
            "operator_webauthn_disabled",
            "WebAuthn operator auth is disabled",
            "webauthn_disabled",
        )
    }
    fn no_credentials() -> Self {
        Self::operational(
            StatusCode::CONFLICT,
            "operator_webauthn_no_credentials",
            "no operator credentials are enrolled",
            "no_credentials",
        )
    }
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self::denial(
            StatusCode::BAD_REQUEST,
            "operator_webauthn_payload_invalid",
            message,
            "invalid_payload",
        )
    }
    fn challenge_invalid() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_challenge_invalid",
            "webauthn challenge is invalid or expired",
            "challenge_invalid",
        )
    }
    fn origin_denied() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_origin_denied",
            "webauthn origin is not allowed",
            "origin_denied",
        )
    }
    fn credential_unknown() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_credential_unknown",
            "webauthn credential is not registered",
            "credential_unknown",
        )
    }
    fn signature_invalid() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_signature_invalid",
            "webauthn assertion signature failed verification",
            "signature_invalid",
        )
    }
    fn credential_not_allowed() -> Self {
        Self::denial(
            StatusCode::BAD_REQUEST,
            "operator_webauthn_credential_not_allowed",
            "webauthn credential algorithm is not allowed",
            "credential_not_allowed",
        )
    }
    fn rp_id_mismatch() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_rp_id_mismatch",
            "webauthn rpId hash mismatch",
            "rp_id_mismatch",
        )
    }
    fn user_verification_required() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_user_verification_required",
            "webauthn user verification is required",
            "user_verification_required",
        )
    }
    fn user_presence_required() -> Self {
        Self::denial(
            StatusCode::UNAUTHORIZED,
            "operator_webauthn_user_presence_required",
            "webauthn user presence is required",
            "user_presence_required",
        )
    }
    fn persistence_failure(message: impl Into<String>) -> Self {
        Self::operational(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operator_webauthn_persist_failed",
            message,
            "persist_failed",
        )
    }
    fn credential_state_unavailable() -> Self {
        Self::operational(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operator_webauthn_state_unavailable",
            "operator credential state is unavailable",
            "credential_state_unavailable",
        )
    }
    fn random_bytes_failure(message: impl Into<String>) -> Self {
        Self::operational(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operator_auth_random_bytes_failed",
            message,
            "random_bytes",
        )
    }
    fn state_capacity_exhausted() -> Self {
        Self::operational(
            StatusCode::SERVICE_UNAVAILABLE,
            "operator_auth_state_capacity_exhausted",
            "operator authentication ephemeral state is at capacity",
            "state_capacity_exhausted",
        )
    }
    fn credential_capacity_exhausted() -> Self {
        Self::operational(
            StatusCode::CONFLICT,
            "operator_webauthn_credential_capacity_exhausted",
            "operator WebAuthn credential capacity is exhausted",
            "credential_capacity_exhausted",
        )
    }
    fn credential_duplicate() -> Self {
        Self::operational(
            StatusCode::CONFLICT,
            "operator_webauthn_credential_duplicate",
            "operator WebAuthn credential is already enrolled",
            "credential_duplicate",
        )
    }
    fn credential_not_found() -> Self {
        Self::operational(
            StatusCode::NOT_FOUND,
            "operator_webauthn_credential_not_found",
            "operator WebAuthn credential was not found",
            "credential_not_found",
        )
    }
    fn last_credential() -> Self {
        Self::operational(
            StatusCode::CONFLICT,
            "operator_webauthn_last_credential",
            "the last operator WebAuthn credential cannot be deleted without a configured bootstrap token",
            "last_credential",
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
    InvalidPolicy(String),
    CredentialLoad(String),
}
impl std::fmt::Display for OperatorAuthInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingWebAuthn => write!(f, "torii.operator_auth.webauthn is required"),
            Self::InvalidWebAuthn(msg) => write!(f, "{msg}"),
            Self::InvalidPolicy(msg) => write!(f, "{msg}"),
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
#[derive(Clone, Debug, PartialEq, Eq)]
enum ChallengeKind {
    Registration,
    Authentication,
}
#[derive(Clone, Debug)]
struct ChallengeEntry {
    kind: ChallengeKind,
}
#[derive(Clone, Debug)]
struct SessionEntry {
    credential_revocation_generation: u64,
}
#[derive(Debug)]
struct ExpiringEntry<V> {
    value: V,
    expires_at: Instant,
    generation: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ExpiryRecord {
    expires_at: Instant,
    key: String,
    generation: u64,
}
#[derive(Debug)]
struct BoundedExpiringStore<V> {
    capacity: usize,
    entries: HashMap<String, ExpiringEntry<V>>,
    expiries: BinaryHeap<Reverse<ExpiryRecord>>,
    next_generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExpiringStoreAtCapacity;
impl<V> BoundedExpiringStore<V> {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity: capacity.get(),
            entries: HashMap::new(),
            expiries: BinaryHeap::new(),
            next_generation: 0,
        }
    }
    fn purge_expired(&mut self, now: Instant) {
        while self
            .expiries
            .peek()
            .is_some_and(|Reverse(expiry)| expiry.expires_at <= now)
        {
            let Reverse(expiry) = self.expiries.pop().expect("peeked expiry must exist");
            let matches_live_entry = self.entries.get(&expiry.key).is_some_and(|entry| {
                entry.generation == expiry.generation && entry.expires_at <= now
            });
            if matches_live_entry {
                self.entries.remove(&expiry.key);
            }
        }
    }
    fn insert(
        &mut self,
        key: String,
        value: V,
        expires_at: Instant,
        now: Instant,
    ) -> Result<Option<V>, ExpiringStoreAtCapacity> {
        self.purge_expired(now);
        if !self.entries.contains_key(&key) && self.entries.len() >= self.capacity {
            return Err(ExpiringStoreAtCapacity);
        }
        let generation = self.allocate_generation();
        let replaced = self
            .entries
            .insert(
                key.clone(),
                ExpiringEntry {
                    value,
                    expires_at,
                    generation,
                },
            )
            .map(|entry| entry.value);
        self.expiries.push(Reverse(ExpiryRecord {
            expires_at,
            key,
            generation,
        }));
        if self.expiries.len() > self.capacity.saturating_mul(2) {
            self.rebuild_expiries();
        }
        Ok(replaced)
    }
    fn remove(&mut self, key: &str, now: Instant) -> Option<V> {
        self.purge_expired(now);
        self.entries.remove(key).map(|entry| entry.value)
    }
    fn get(&mut self, key: &str, now: Instant) -> Option<&V> {
        self.purge_expired(now);
        self.entries.get(key).map(|entry| &entry.value)
    }
    fn is_at_capacity(&mut self, now: Instant) -> bool {
        self.purge_expired(now);
        self.entries.len() >= self.capacity
    }
    fn allocate_generation(&mut self) -> u64 {
        if self.next_generation == u64::MAX {
            self.rebuild_expiries();
        }
        let generation = self.next_generation;
        self.next_generation += 1;
        generation
    }
    fn rebuild_expiries(&mut self) {
        let mut keys: Vec<_> = self.entries.keys().cloned().collect();
        keys.sort_unstable();
        self.expiries.clear();
        for (generation, key) in keys.into_iter().enumerate() {
            let generation = u64::try_from(generation)
                .expect("entry count cannot exceed the addressable process memory");
            let entry = self
                .entries
                .get_mut(&key)
                .expect("key collected from the same store must exist");
            entry.generation = generation;
            self.expiries.push(Reverse(ExpiryRecord {
                expires_at: entry.expires_at,
                key,
                generation,
            }));
        }
        self.next_generation = u64::try_from(self.entries.len())
            .expect("entry count cannot exceed the addressable process memory");
    }
    fn clear(&mut self) {
        self.entries.clear();
        self.expiries.clear();
        self.next_generation = 0;
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}
struct LockoutTracker {
    config: OperatorAuthLockout,
    entries: Mutex<BoundedExpiringStore<FailureEntry>>,
}
#[derive(Clone, Debug)]
struct FailureEntry {
    failures: u32,
    window_start: Instant,
    locked_until: Option<Instant>,
}
impl LockoutTracker {
    fn new(config: OperatorAuthLockout, capacity: NonZeroUsize) -> Self {
        Self {
            config,
            entries: Mutex::new(BoundedExpiringStore::new(capacity)),
        }
    }
    fn is_locked(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut entries = self.entries.lock();
        entries
            .get(key, now)
            .is_some_and(|entry| entry.locked_until.is_some_and(|until| now < until))
    }
    fn record_failure(&self, key: &str) -> Result<bool, ExpiringStoreAtCapacity> {
        let Some(limit) = self.config.failures else {
            return Ok(false);
        };
        let now = Instant::now();
        let mut entries = self.entries.lock();
        let mut entry = match entries.remove(key, now) {
            Some(entry) => entry,
            // Preserve every live tracked identity and the hard memory bound. An attacker that
            // rotates identities must not turn a full failure table into a global admission
            // failure for otherwise valid, previously unseen callers.
            None if entries.is_at_capacity(now) => return Ok(false),
            None => FailureEntry {
                failures: 0,
                window_start: now,
                locked_until: None,
            },
        };
        if let Some(locked_until) = entry.locked_until {
            if now < locked_until {
                entries.insert(key.to_owned(), entry, locked_until, now)?;
                return Ok(true);
            }
            entry.locked_until = None;
            entry.failures = 0;
            entry.window_start = now;
        }
        if now.duration_since(entry.window_start) > self.config.window {
            entry.window_start = now;
            entry.failures = 0;
        }
        entry.failures = entry.failures.saturating_add(1);
        if entry.failures >= limit.get() {
            let locked_until = now
                .checked_add(self.config.duration)
                .expect("operator auth durations are validated during initialization");
            entry.locked_until = Some(locked_until);
            entries.insert(key.to_owned(), entry, locked_until, now)?;
            return Ok(true);
        }
        let expires_at = entry
            .window_start
            .checked_add(self.config.window)
            .expect("operator auth durations are validated during initialization");
        entries.insert(key.to_owned(), entry, expires_at, now)?;
        Ok(false)
    }
    fn clear(&self, key: &str) {
        self.entries.lock().remove(key, Instant::now());
    }
}
pub struct OperatorAuth {
    enabled: bool,
    require_mtls: bool,
    mtls_trusted_proxy_nets: Vec<limits::IpNet>,
    bootstrap_token_hashes: HashSet<[u8; 32]>,
    webauthn: Option<WebAuthnPolicy>,
    credentials: Arc<RwLock<Vec<StoredCredential>>>,
    credential_capacity: usize,
    sessions: Mutex<BoundedExpiringStore<SessionEntry>>,
    challenges: Mutex<BoundedExpiringStore<ChallengeEntry>>,
    credential_revocation_generation: AtomicU64,
    limiter: limits::RateLimiter,
    lockout: LockoutTracker,
    telemetry: MaybeTelemetry,
    credentials_path: PathBuf,
}
impl OperatorAuth {
    pub(crate) fn new(
        config: ToriiOperatorAuth,
        data_dir: PathBuf,
        telemetry: MaybeTelemetry,
    ) -> Result<Self, OperatorAuthInitError> {
        validate_operator_auth_capacities(&config)?;
        let webauthn = if config.enabled {
            let Some(cfg) = config.webauthn.clone() else {
                return Err(OperatorAuthInitError::MissingWebAuthn);
            };
            Some(WebAuthnPolicy::from_config(cfg)?)
        } else {
            None
        };
        if let Some(policy) = &webauthn {
            validate_ephemeral_duration(
                "torii.operator_auth.webauthn.challenge_ttl_secs",
                policy.challenge_ttl,
            )?;
            validate_ephemeral_duration(
                "torii.operator_auth.webauthn.session_ttl_secs",
                policy.session_ttl,
            )?;
        }
        if config.lockout.failures.is_some() {
            validate_ephemeral_duration(
                "torii.operator_auth.lockout_window_secs",
                config.lockout.window,
            )?;
            validate_ephemeral_duration(
                "torii.operator_auth.lockout_duration_secs",
                config.lockout.duration,
            )?;
        }
        let bootstrap_token_hashes = validate_bootstrap_tokens(config.enabled, &config.tokens)?;
        let credentials_path = operator_credentials_path(&data_dir);
        let credentials = if config.enabled {
            let policy = webauthn
                .as_ref()
                .expect("enabled operator auth has a validated WebAuthn policy");
            load_credentials(
                &credentials_path,
                &policy.allowed_algorithms,
                config.credential_capacity,
            )
            .map_err(OperatorAuthInitError::CredentialLoad)?
        } else {
            Vec::new()
        };
        if config.enabled && credentials.is_empty() && bootstrap_token_hashes.is_empty() {
            return Err(OperatorAuthInitError::InvalidPolicy(
                "torii.operator_auth.tokens must contain a bootstrap token until the first WebAuthn credential is persisted"
                    .to_owned(),
            ));
        }
        let rate_per_minute = config.rate_per_minute.map(std::num::NonZeroU32::get);
        let burst = config.burst.map(std::num::NonZeroU32::get);
        let limiter = limits::RateLimiter::new_per_minute(rate_per_minute, burst);
        let ephemeral_state_capacity = config.ephemeral_state_capacity;
        Ok(Self {
            enabled: config.enabled,
            require_mtls: config.require_mtls,
            mtls_trusted_proxy_nets: limits::parse_cidrs(&config.mtls_trusted_proxy_cidrs),
            bootstrap_token_hashes,
            webauthn,
            credentials: Arc::new(RwLock::new(credentials)),
            credential_capacity: config.credential_capacity.get(),
            sessions: Mutex::new(BoundedExpiringStore::new(ephemeral_state_capacity)),
            challenges: Mutex::new(BoundedExpiringStore::new(ephemeral_state_capacity)),
            credential_revocation_generation: AtomicU64::new(0),
            limiter,
            lockout: LockoutTracker::new(config.lockout, ephemeral_state_capacity),
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
    fn credentials_read(
        &self,
    ) -> Result<RwLockReadGuard<'_, Vec<StoredCredential>>, OperatorAuthError> {
        self.credentials.read().map_err(|_| {
            iroha_logger::error!("operator credentials lock poisoned; failing closed");
            OperatorAuthError::credential_state_unavailable()
        })
    }
    fn credentials_write(
        &self,
    ) -> Result<RwLockWriteGuard<'_, Vec<StoredCredential>>, OperatorAuthError> {
        self.credentials.write().map_err(|_| {
            iroha_logger::error!("operator credentials lock poisoned; failing closed");
            OperatorAuthError::credential_state_unavailable()
        })
    }
    fn has_credentials(&self) -> Result<bool, OperatorAuthError> {
        Ok(!self.credentials_read()?.is_empty())
    }
    async fn check_common(
        &self,
        headers: &HeaderMap,
        remote_ip: Option<IpAddr>,
        action: &'static str,
    ) -> Result<AuthContext, OperatorAuthError> {
        let key = auth_key(headers, remote_ip);
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
        let ctx = AuthContext {
            key,
            enrollment_authority: EnrollmentAuthority::None,
        };
        if self.require_mtls && !mtls_present(headers, remote_ip, &self.mtls_trusted_proxy_nets) {
            let err = OperatorAuthError::missing_mtls();
            // A client that never crossed the trusted mTLS boundary has no authenticated
            // operator identity to lock out. Tracking these failures lets arbitrary network
            // sources consume every bounded lockout slot before credential authentication.
            self.record_event(action, "denied", err.metric_label());
            return Err(err);
        }
        Ok(ctx)
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
        match session_from_headers(headers) {
            SessionHeader::Valid(session) if self.session_generation(session).is_some() => {
                self.record_success(&ctx, ACTION_GATE, "session");
                Ok(())
            }
            SessionHeader::Missing => {
                let err = OperatorAuthError::missing_session();
                Err(self.record_error(&ctx, ACTION_GATE, err))
            }
            SessionHeader::Invalid | SessionHeader::Valid(_) => {
                let err = OperatorAuthError::invalid_session();
                Err(self.record_error(&ctx, ACTION_GATE, err))
            }
        }
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
        let mut ctx = self.check_common(headers, remote_ip, action).await?;
        match session_from_headers(headers) {
            SessionHeader::Valid(session) => {
                if let Some(generation) = self.session_generation(session) {
                    ctx.enrollment_authority = EnrollmentAuthority::Session(generation);
                    return Ok(ctx);
                }
                let err = OperatorAuthError::invalid_session();
                return Err(self.record_error(&ctx, action, err));
            }
            SessionHeader::Invalid => {
                let err = OperatorAuthError::invalid_session();
                return Err(self.record_error(&ctx, action, err));
            }
            SessionHeader::Missing => {}
        }
        if !self
            .has_credentials()
            .map_err(|err| self.record_error(&ctx, action, err))?
        {
            match self.check_bootstrap_token(headers) {
                TokenCheck::Valid => {
                    ctx.enrollment_authority = EnrollmentAuthority::BootstrapToken;
                    return Ok(ctx);
                }
                TokenCheck::Missing => {
                    let err = OperatorAuthError::missing_token();
                    return Err(self.record_error(&ctx, action, err));
                }
                TokenCheck::Invalid => {
                    let err = OperatorAuthError::invalid_token();
                    return Err(self.record_error(&ctx, action, err));
                }
            }
        }
        let err = OperatorAuthError::missing_session();
        Err(self.record_error(&ctx, action, err))
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
        let mut rng = rand::rngs::OsRng;
        self.webauthn_registration_options_with_rng(ctx, &mut rng)
    }
    fn webauthn_registration_options_with_rng<R: TryCryptoRng + ?Sized>(
        &self,
        ctx: &AuthContext,
        rng: &mut R,
    ) -> Result<norito::json::Value, OperatorAuthError> {
        let policy = self
            .webauthn_policy()
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_OPTIONS, err))?;
        let exclude_credentials = {
            let credentials = self
                .credentials_read()
                .map_err(|err| self.record_error(ctx, ACTION_REGISTER_OPTIONS, err))?;
            credentials
                .iter()
                .map(|credential| {
                    json_object(vec![
                        json_entry("type", "public-key"),
                        json_entry("id", encode_b64url(&credential.id)),
                    ])
                })
                .collect::<Vec<_>>()
        };
        let challenge_bytes = random_bytes_with_rng(CHALLENGE_BYTES, rng)
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_OPTIONS, err))?;
        let challenge_b64 = encode_b64url(&challenge_bytes);
        let now = Instant::now();
        let expires_at = now
            .checked_add(policy.challenge_ttl)
            .expect("operator auth durations are validated during initialization");
        self.challenges
            .lock()
            .insert(
                challenge_b64.clone(),
                ChallengeEntry {
                    kind: ChallengeKind::Registration,
                },
                expires_at,
                now,
            )
            .map_err(|_| {
                self.record_error(
                    ctx,
                    ACTION_REGISTER_OPTIONS,
                    OperatorAuthError::state_capacity_exhausted(),
                )
            })?;
        let user_id_b64 = encode_b64url(&policy.user_id);
        let mut params = Vec::new();
        for alg in &policy.allowed_algorithms {
            params.push(json_object(vec![
                json_entry("type", "public-key"),
                json_entry("alg", alg.cose_alg()),
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
        let policy = self
            .webauthn_policy()
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
        let input = parse_registration_payload(payload)
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
        let client = parse_client_data(&input.client_data_json, "webauthn.create")
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
        let _challenge_entry = self
            .take_challenge(&client.challenge, ChallengeKind::Registration)
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
        if !origin_allowed(&client.origin, &policy.origins) {
            let err = OperatorAuthError::origin_denied();
            return Err(self.record_error(ctx, ACTION_REGISTER_VERIFY, err));
        }
        let attestation = parse_attestation_object(&input.attestation_object)
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
        let auth_data = parse_auth_data_registration(&attestation.auth_data, policy)
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
        if auth_data.credential_id != input.raw_id {
            let err = OperatorAuthError::invalid_payload("credential id mismatch");
            return Err(self.record_error(ctx, ACTION_REGISTER_VERIFY, err));
        }
        let created_at_ms = now_ms();
        let credential = StoredCredential {
            id: auth_data.credential_id.clone(),
            public_key: auth_data.cose_key.public_key.clone(),
            alg: auth_data.cose_key.alg,
            sign_count: auth_data.sign_count,
            created_at_ms,
        };
        let total = self
            .insert_credential(credential, ctx.enrollment_authority)
            .map_err(|err| self.record_error(ctx, ACTION_REGISTER_VERIFY, err))?;
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
        let mut rng = rand::rngs::OsRng;
        self.webauthn_authentication_options_with_rng(ctx, &mut rng)
    }
    fn webauthn_authentication_options_with_rng<R: TryCryptoRng + ?Sized>(
        &self,
        ctx: &AuthContext,
        rng: &mut R,
    ) -> Result<norito::json::Value, OperatorAuthError> {
        let policy = self
            .webauthn_policy()
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_OPTIONS, err))?;
        let allow = {
            let credentials = self
                .credentials_read()
                .map_err(|err| self.record_error(ctx, ACTION_LOGIN_OPTIONS, err))?;
            if credentials.is_empty() {
                let err = OperatorAuthError::no_credentials();
                return Err(self.record_error(ctx, ACTION_LOGIN_OPTIONS, err));
            }
            credentials
                .iter()
                .map(|credential| {
                    json_object(vec![
                        json_entry("type", "public-key"),
                        json_entry("id", encode_b64url(&credential.id)),
                    ])
                })
                .collect::<Vec<_>>()
        };
        let challenge_bytes = random_bytes_with_rng(CHALLENGE_BYTES, rng)
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_OPTIONS, err))?;
        let challenge_b64 = encode_b64url(&challenge_bytes);
        let now = Instant::now();
        let expires_at = now
            .checked_add(policy.challenge_ttl)
            .expect("operator auth durations are validated during initialization");
        self.challenges
            .lock()
            .insert(
                challenge_b64.clone(),
                ChallengeEntry {
                    kind: ChallengeKind::Authentication,
                },
                expires_at,
                now,
            )
            .map_err(|_| {
                self.record_error(
                    ctx,
                    ACTION_LOGIN_OPTIONS,
                    OperatorAuthError::state_capacity_exhausted(),
                )
            })?;
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
        // Producing an authentication challenge does not prove the caller's identity.
        // In particular, do not clear the failure window here: otherwise a caller can
        // alternate options requests with invalid assertions and evade lockout forever.
        self.record_event(ACTION_LOGIN_OPTIONS, "allowed", "ok");
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
        let mut rng = rand::rngs::OsRng;
        self.webauthn_finish_authentication_with_rng(ctx, payload, &mut rng)
    }
    fn webauthn_finish_authentication_with_rng<R: TryCryptoRng + ?Sized>(
        &self,
        ctx: &AuthContext,
        payload: &norito::json::Value,
        rng: &mut R,
    ) -> Result<SessionOutcome, OperatorAuthError> {
        let policy = self
            .webauthn_policy()
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        let input = parse_assertion_payload(payload)
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        let client = parse_client_data(&input.client_data_json, "webauthn.get")
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        let _challenge_entry = self
            .take_challenge(&client.challenge, ChallengeKind::Authentication)
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        if !origin_allowed(&client.origin, &policy.origins) {
            let err = OperatorAuthError::origin_denied();
            return Err(self.record_error(ctx, ACTION_LOGIN_VERIFY, err));
        }
        let auth_data = parse_auth_data_assertion(&input.authenticator_data, policy)
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        let mut credentials = self
            .credentials_write()
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        let Some(pos) = credentials
            .iter()
            .position(|entry| entry.id == input.raw_id)
        else {
            let err = OperatorAuthError::credential_unknown();
            return Err(self.record_error(ctx, ACTION_LOGIN_VERIFY, err));
        };
        let credential = credentials.get(pos).expect("position valid");
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
        .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        if credential.sign_count != 0 && auth_data.sign_count <= credential.sign_count {
            let err = OperatorAuthError::invalid_payload("webauthn signCount did not advance");
            return Err(self.record_error(ctx, ACTION_LOGIN_VERIFY, err));
        }
        let mut updated = credentials.clone();
        updated[pos].sign_count = auth_data.sign_count;
        persist_credentials(&self.credentials_path, &updated)
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        *credentials = updated;
        let outcome = self
            .issue_session_with_rng(&input.raw_id, policy.session_ttl, rng)
            .map_err(|err| self.record_error(ctx, ACTION_LOGIN_VERIFY, err))?;
        self.record_success(ctx, ACTION_LOGIN_VERIFY, "ok");
        Ok(outcome)
    }
    fn issue_session_with_rng<R: TryCryptoRng + ?Sized>(
        &self,
        credential_id: &[u8],
        ttl: Duration,
        rng: &mut R,
    ) -> Result<SessionOutcome, OperatorAuthError> {
        let token_bytes = random_bytes_with_rng(SESSION_TOKEN_BYTES, rng)?;
        let token = encode_b64url(&token_bytes);
        let now = Instant::now();
        let expires_at = now
            .checked_add(ttl)
            .expect("operator auth durations are validated during initialization");
        self.sessions
            .lock()
            .insert(
                token.clone(),
                SessionEntry {
                    credential_revocation_generation: self
                        .credential_revocation_generation
                        .load(Ordering::Acquire),
                },
                expires_at,
                now,
            )
            .map_err(|_| OperatorAuthError::state_capacity_exhausted())?;
        Ok(SessionOutcome {
            session_token: token,
            expires_in_secs: ttl.as_secs().max(1),
            credential_id: encode_b64url(credential_id),
        })
    }
    fn insert_credential(
        &self,
        credential: StoredCredential,
        authority: EnrollmentAuthority,
    ) -> Result<usize, OperatorAuthError> {
        let policy = self.webauthn_policy()?;
        validate_stored_credential(&credential, &policy.allowed_algorithms)
            .map_err(OperatorAuthError::invalid_payload)?;
        let mut credentials = self.credentials_write()?;
        if let EnrollmentAuthority::Session(generation) = authority
            && generation
                != self
                    .credential_revocation_generation
                    .load(Ordering::Acquire)
        {
            return Err(OperatorAuthError::invalid_session());
        }
        if authority == EnrollmentAuthority::BootstrapToken && !credentials.is_empty() {
            return Err(OperatorAuthError::missing_session());
        }
        if credentials.iter().any(|entry| entry.id == credential.id) {
            return Err(OperatorAuthError::credential_duplicate());
        }
        if credentials.len() >= self.credential_capacity {
            return Err(OperatorAuthError::credential_capacity_exhausted());
        }
        let mut updated = credentials.clone();
        updated.push(credential);
        persist_credentials(&self.credentials_path, &updated)?;
        *credentials = updated;
        Ok(credentials.len())
    }
    fn credential_inventory(&self) -> Result<norito::json::Value, OperatorAuthError> {
        self.webauthn_policy()?;
        let credentials = self.credentials_read()?;
        let mut entries = credentials
            .iter()
            .map(|credential| {
                let credential_id = encode_b64url(&credential.id);
                let metadata = json_object(vec![
                    json_entry("credential_id", credential_id.clone()),
                    json_entry("algorithm", credential.alg.label()),
                    json_entry("sign_count", credential.sign_count),
                    json_entry("created_at_ms", credential.created_at_ms),
                ]);
                (credential_id, metadata)
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let credentials_total = entries.len();
        Ok(json_object(vec![
            json_entry(
                "credentials",
                entries
                    .into_iter()
                    .map(|(_, metadata)| metadata)
                    .collect::<Vec<_>>(),
            ),
            json_entry("credentials_total", credentials_total),
        ]))
    }
    fn delete_credential(
        &self,
        encoded_id: &str,
        authorized_generation: u64,
    ) -> Result<CredentialDeletionOutcome, OperatorAuthError> {
        self.webauthn_policy()?;
        let credential_id = decode_managed_credential_id(encoded_id)?;
        let mut credentials = self.credentials_write()?;
        if authorized_generation
            != self
                .credential_revocation_generation
                .load(Ordering::Acquire)
        {
            return Err(OperatorAuthError::invalid_session());
        }
        let next_generation = authorized_generation
            .checked_add(1)
            .ok_or_else(OperatorAuthError::credential_state_unavailable)?;
        let Some(position) = credentials
            .iter()
            .position(|credential| credential.id == credential_id)
        else {
            return Err(OperatorAuthError::credential_not_found());
        };
        if credentials.len() == 1 && self.bootstrap_token_hashes.is_empty() {
            return Err(OperatorAuthError::last_credential());
        }
        let mut updated = credentials.clone();
        let deleted = updated.remove(position);
        persist_credentials(&self.credentials_path, &updated)?;
        let credentials_total = updated.len();
        *credentials = updated;
        self.credential_revocation_generation
            .store(next_generation, Ordering::Release);
        drop(credentials);

        // Credential removal is a revocation boundary. Sessions are intentionally not tied to
        // one credential in the in-memory store, so invalidate every outstanding authorization
        // and ceremony rather than leaving an attacker a session issued before the removal.
        self.sessions.lock().clear();
        self.challenges.lock().clear();
        Ok(CredentialDeletionOutcome {
            credential_id: encode_b64url(&deleted.id),
            credentials_total,
        })
    }
    fn take_challenge(
        &self,
        challenge: &str,
        kind: ChallengeKind,
    ) -> Result<ChallengeEntry, OperatorAuthError> {
        match self.challenges.lock().remove(challenge, Instant::now()) {
            Some(entry) => {
                if entry.kind != kind {
                    return Err(OperatorAuthError::challenge_invalid());
                }
                Ok(entry)
            }
            None => Err(OperatorAuthError::challenge_invalid()),
        }
    }
    fn check_bootstrap_token(&self, headers: &HeaderMap) -> TokenCheck {
        operator_token(headers)
            .map(|token| {
                if self
                    .bootstrap_token_hashes
                    .contains(&bootstrap_token_digest(token))
                {
                    TokenCheck::Valid
                } else {
                    TokenCheck::Invalid
                }
            })
            .unwrap_or(TokenCheck::Missing)
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
    fn record_failure(
        &self,
        ctx: &AuthContext,
        action: &'static str,
        reason: &'static str,
    ) -> Result<(), ExpiringStoreAtCapacity> {
        self.record_event(action, "denied", reason);
        if self.lockout.record_failure(&ctx.key)? {
            self.record_lockout(action, reason);
        }
        Ok(())
    }
    fn record_error(
        &self,
        ctx: &AuthContext,
        action: &'static str,
        error: OperatorAuthError,
    ) -> OperatorAuthError {
        if error.counts_toward_lockout {
            if self
                .record_failure(ctx, action, error.metric_label())
                .is_err()
            {
                let capacity_error = OperatorAuthError::state_capacity_exhausted();
                self.record_event(action, "error", capacity_error.metric_label());
                return capacity_error;
            }
        } else {
            self.record_event(action, "error", error.metric_label());
        }
        error
    }
    fn record_success(&self, ctx: &AuthContext, action: &'static str, reason: &'static str) {
        self.lockout.clear(&ctx.key);
        self.record_event(action, "allowed", reason);
    }
    fn session_generation(&self, token: &str) -> Option<u64> {
        let generation = self
            .sessions
            .lock()
            .get(token, Instant::now())?
            .credential_revocation_generation;
        (generation
            == self
                .credential_revocation_generation
                .load(Ordering::Acquire))
        .then_some(generation)
    }
    fn credential_management_generation(
        &self,
        headers: &HeaderMap,
    ) -> Result<u64, OperatorAuthError> {
        self.webauthn_policy()?;
        let session = match session_from_headers(headers) {
            SessionHeader::Missing => return Err(OperatorAuthError::missing_session()),
            SessionHeader::Invalid => return Err(OperatorAuthError::invalid_session()),
            SessionHeader::Valid(session) => session,
        };
        self.session_generation(session)
            .ok_or_else(OperatorAuthError::invalid_session)
    }
    fn session_valid(&self, token: &str) -> bool {
        self.session_generation(token).is_some()
    }
}
fn validate_ephemeral_duration(
    label: &'static str,
    duration: Duration,
) -> Result<(), OperatorAuthInitError> {
    if duration.is_zero() {
        return Err(OperatorAuthInitError::InvalidPolicy(format!(
            "{label} must be greater than zero"
        )));
    }
    if Instant::now().checked_add(duration).is_none() {
        return Err(OperatorAuthInitError::InvalidPolicy(format!(
            "{label} exceeds the platform timer range"
        )));
    }
    Ok(())
}
fn validate_bootstrap_tokens(
    enabled: bool,
    tokens: &[String],
) -> Result<HashSet<[u8; 32]>, OperatorAuthInitError> {
    if !enabled {
        return Ok(HashSet::new());
    }
    let token_capacity =
        iroha_config::parameters::defaults::torii::operator_auth::MAX_BOOTSTRAP_TOKENS;
    if tokens.len() > token_capacity {
        return Err(OperatorAuthInitError::InvalidPolicy(format!(
            "torii.operator_auth.tokens must not contain more than {token_capacity} entries"
        )));
    }
    let mut validated = HashSet::with_capacity(tokens.len());
    for token in tokens {
        let min =
            iroha_config::parameters::defaults::torii::operator_auth::BOOTSTRAP_TOKEN_MIN_BYTES;
        let max =
            iroha_config::parameters::defaults::torii::operator_auth::BOOTSTRAP_TOKEN_MAX_BYTES;
        if !(min..=max).contains(&token.len()) {
            return Err(OperatorAuthInitError::InvalidPolicy(format!(
                "torii.operator_auth.tokens entries must contain {min}..={max} bytes"
            )));
        }
        if !token.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
            return Err(OperatorAuthInitError::InvalidPolicy(
                "torii.operator_auth.tokens entries must use visible ASCII without whitespace"
                    .to_owned(),
            ));
        }
        if !validated.insert(bootstrap_token_digest(token)) {
            return Err(OperatorAuthInitError::InvalidPolicy(
                "torii.operator_auth.tokens must not contain duplicates".to_owned(),
            ));
        }
    }
    Ok(validated)
}
fn validate_operator_auth_capacities(
    config: &ToriiOperatorAuth,
) -> Result<(), OperatorAuthInitError> {
    let max_ephemeral =
        iroha_config::parameters::defaults::torii::operator_auth::MAX_EPHEMERAL_STATE_CAPACITY;
    if config.ephemeral_state_capacity.get() > max_ephemeral {
        return Err(OperatorAuthInitError::InvalidPolicy(format!(
            "torii.operator_auth.ephemeral_state_capacity must not exceed {max_ephemeral}"
        )));
    }
    let max_credentials =
        iroha_config::parameters::defaults::torii::operator_auth::MAX_CREDENTIAL_CAPACITY;
    if config.credential_capacity.get() > max_credentials {
        return Err(OperatorAuthInitError::InvalidPolicy(format!(
            "torii.operator_auth.credential_capacity must not exceed {max_credentials}"
        )));
    }
    Ok(())
}
fn bootstrap_token_digest(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"iroha:torii:operator-bootstrap:v1\0");
    hasher.update(token.as_bytes());
    hasher.finalize().into()
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
#[derive(Debug)]
struct CredentialDeletionOutcome {
    credential_id: String,
    credentials_total: usize,
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
    Valid,
    Missing,
    Invalid,
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
#[cfg(test)]
fn random_bytes(len: usize) -> Result<Vec<u8>, OperatorAuthError> {
    let mut buf = vec![0u8; len];
    let mut rng = rand::rngs::OsRng;
    random_bytes_with_rng_into(&mut buf, &mut rng)?;
    Ok(buf)
}
fn random_bytes_with_rng<R: TryCryptoRng + ?Sized>(
    len: usize,
    rng: &mut R,
) -> Result<Vec<u8>, OperatorAuthError> {
    let mut buf = vec![0u8; len];
    random_bytes_with_rng_into(&mut buf, rng)?;
    Ok(buf)
}
fn random_bytes_with_rng_into<R: TryCryptoRng + ?Sized>(
    buf: &mut [u8],
    rng: &mut R,
) -> Result<(), OperatorAuthError> {
    rng.try_fill_bytes(buf).map_err(|err| {
        OperatorAuthError::random_bytes_failure(format!(
            "failed to generate operator auth random bytes: {err}"
        ))
    })
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
    let decoded = URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .map_err(|_| OperatorAuthError::invalid_payload(format!("{label} must be base64url")))?;
    if URL_SAFE_NO_PAD.encode(&decoded) != value {
        return Err(OperatorAuthError::invalid_payload(format!(
            "{label} must use canonical unpadded base64url"
        )));
    }
    Ok(decoded)
}
fn decode_managed_credential_id(value: &str) -> Result<Vec<u8>, OperatorAuthError> {
    if value.len() > MAX_CREDENTIAL_ID_B64URL_BYTES {
        return Err(OperatorAuthError::invalid_payload(format!(
            "credential id must not exceed {MAX_CREDENTIAL_ID_BYTES} bytes"
        )));
    }
    let decoded = decode_b64url("credential id", value)?;
    if decoded.len() > MAX_CREDENTIAL_ID_BYTES {
        return Err(OperatorAuthError::invalid_payload(format!(
            "credential id must not exceed {MAX_CREDENTIAL_ID_BYTES} bytes"
        )));
    }
    Ok(decoded)
}
fn auth_key(headers: &HeaderMap, remote: Option<IpAddr>) -> String {
    limits::effective_remote_ip(headers, remote)
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
fn single_header_text<'a>(headers: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
fn session_from_headers(headers: &HeaderMap) -> SessionHeader<'_> {
    let mut values = headers.get_all(HEADER_OPERATOR_SESSION).iter();
    let Some(header) = values.next() else {
        return SessionHeader::Missing;
    };
    if values.next().is_some() || header.as_bytes().len() != SESSION_TOKEN_B64URL_BYTES {
        return SessionHeader::Invalid;
    }
    let Ok(value) = header.to_str() else {
        return SessionHeader::Invalid;
    };
    // `base64` asks `decode_slice` for its one-byte-conservative estimate for a 43-symbol
    // unpadded value. The fixed buffer remains independent of attacker-controlled input.
    let mut decoded = [0_u8; SESSION_TOKEN_DECODE_BUFFER_BYTES];
    let Ok(decoded_len) = URL_SAFE_NO_PAD.decode_slice(value.as_bytes(), &mut decoded) else {
        return SessionHeader::Invalid;
    };
    if decoded_len != SESSION_TOKEN_BYTES || URL_SAFE_NO_PAD.encode(&decoded[..decoded_len]) != value
    {
        return SessionHeader::Invalid;
    }
    SessionHeader::Valid(value)
}
fn operator_token(headers: &HeaderMap) -> Option<&str> {
    single_header_text(headers, HEADER_OPERATOR_TOKEN).filter(|value| !value.trim().is_empty())
}
fn origin_allowed(origin: &str, allowed: &[Url]) -> bool {
    let Ok(parsed) = Url::parse(origin) else {
        return false;
    };
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return false;
    }
    let parsed_origin = parsed.origin();
    if matches!(parsed_origin, url::Origin::Opaque(_)) {
        return false;
    }
    allowed
        .iter()
        .any(|candidate| candidate.origin() == parsed_origin)
}
async fn require_empty_options_body(body: Body) -> Result<(), OperatorAuthError> {
    match axum::body::to_bytes(body, 1).await {
        Ok(bytes) if bytes.is_empty() => Ok(()),
        Ok(_) | Err(_) => Err(OperatorAuthError::invalid_payload(
            "operator WebAuthn options requests must have an empty body",
        )),
    }
}
fn load_credentials(
    path: &Path,
    allowed_algorithms: &[OperatorWebAuthnAlgorithm],
    capacity: NonZeroUsize,
) -> Result<Vec<StoredCredential>, String> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.to_string()),
    };
    let max_file_bytes = max_credentials_file_bytes(capacity)?;
    let metadata_len = file.metadata().map_err(|error| error.to_string())?.len();
    if metadata_len > max_file_bytes {
        return Err(format!(
            "credentials payload is {metadata_len} bytes but the configured capacity permits at most {max_file_bytes}"
        ));
    }
    let read_limit = max_file_bytes
        .checked_add(1)
        .ok_or_else(|| "credentials payload read bound overflow".to_owned())?;
    let mut raw = String::new();
    file.by_ref()
        .take(read_limit)
        .read_to_string(&mut raw)
        .map_err(|error| error.to_string())?;
    if u64::try_from(raw.len()).map_err(|_| "credentials payload length overflow")?
        > max_file_bytes
    {
        return Err(format!(
            "credentials payload exceeds the configured {max_file_bytes}-byte bound"
        ));
    }
    let value: norito::json::Value = norito::json::from_str(&raw).map_err(|err| err.to_string())?;
    let obj = value
        .as_object()
        .ok_or_else(|| "credentials payload must be a JSON object".to_string())?;
    require_exact_json_fields(obj, &["credentials", "version"], "credentials payload")?;
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
    if items.len() > capacity.get() {
        return Err(format!(
            "credentials payload contains {} entries but credential_capacity is {}",
            items.len(),
            capacity
        ));
    }
    let mut result = Vec::with_capacity(items.len());
    let mut ids = HashSet::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_obj = item
            .as_object()
            .ok_or_else(|| "credential entry must be an object".to_string())?;
        require_exact_json_fields(
            item_obj,
            &[
                "alg",
                "created_at_ms",
                "id_b64",
                "public_key_b64",
                "sign_count",
            ],
            &format!("credential entry {index}"),
        )?;
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
        let id = decode_canonical_stored_base64url("credential id_b64", id_b64)?;
        let public_key =
            decode_canonical_stored_base64url("credential public_key_b64", public_key_b64)?;
        let alg = match alg_label {
            "es256" => OperatorWebAuthnAlgorithm::Es256,
            "ed25519" => OperatorWebAuthnAlgorithm::Ed25519,
            other => return Err(format!("unsupported credential alg {other}")),
        };
        let sign_count = u32::try_from(sign_count)
            .map_err(|_| format!("credential entry {index} sign_count exceeds u32"))?;
        let credential = StoredCredential {
            id,
            public_key,
            alg,
            sign_count,
            created_at_ms,
        };
        validate_stored_credential(&credential, allowed_algorithms)
            .map_err(|message| format!("credential entry {index}: {message}"))?;
        if !ids.insert(credential.id.clone()) {
            return Err(format!("credential entry {index} duplicates an earlier id"));
        }
        result.push(credential);
    }
    Ok(result)
}
fn max_credentials_file_bytes(capacity: NonZeroUsize) -> Result<u64, String> {
    let bytes = capacity
        .get()
        .checked_mul(MAX_CREDENTIAL_RECORD_JSON_BYTES)
        .and_then(|bytes| bytes.checked_add(CREDENTIAL_FILE_JSON_OVERHEAD_BYTES))
        .ok_or_else(|| "credential file size bound overflow".to_owned())?;
    u64::try_from(bytes).map_err(|_| "credential file size bound exceeds u64".to_owned())
}
fn require_exact_json_fields(
    object: &norito::json::Map,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(format!("{context} contains unknown field `{field}`"));
    }
    Ok(())
}
fn decode_canonical_stored_base64url(label: &str, encoded: &str) -> Result<Vec<u8>, String> {
    if encoded.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|_| format!("invalid {label}"))?;
    if URL_SAFE_NO_PAD.encode(&decoded) != encoded {
        return Err(format!("{label} must use canonical unpadded base64url"));
    }
    Ok(decoded)
}
fn validate_stored_credential(
    credential: &StoredCredential,
    allowed_algorithms: &[OperatorWebAuthnAlgorithm],
) -> Result<(), String> {
    if credential.id.is_empty() {
        return Err("credential id must not be empty".to_owned());
    }
    if credential.id.len() > MAX_CREDENTIAL_ID_BYTES {
        return Err(format!(
            "credential id exceeds {MAX_CREDENTIAL_ID_BYTES} bytes"
        ));
    }
    if !allowed_algorithms.contains(&credential.alg) {
        return Err(format!(
            "credential algorithm {} is not allowed by the active WebAuthn policy",
            credential.alg.label()
        ));
    }
    validate_credential_public_key(credential.alg, &credential.public_key)
        .map_err(|error| error.message)
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
    let parent_directory = open_parent_directory_for_sync(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to create temp file: {err}"))
    })?;
    tmp.write_all(body.as_bytes()).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to write credentials: {err}"))
    })?;
    tmp.flush().map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to flush credentials: {err}"))
    })?;
    tmp.as_file().sync_all().map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to sync credentials: {err}"))
    })?;
    let _persisted = tmp.persist(path).map_err(|err| {
        OperatorAuthError::persistence_failure(format!("failed to persist credentials: {err}"))
    })?;
    // The rename above is the logical commit point. The file's data and metadata were synced
    // before it, so a later directory-fsync error must not be returned as an apparent rollback:
    // callers would otherwise retain an older in-memory credential set than the visible file.
    if let Some(directory) = parent_directory
        && let Err(error) = directory.sync_all()
    {
        iroha_logger::error!(
            ?error,
            path = %parent.display(),
            "operator credential rename committed but directory sync failed"
        );
    }
    Ok(())
}
fn open_parent_directory_for_sync(parent: &Path) -> Result<Option<fs::File>, OperatorAuthError> {
    #[cfg(unix)]
    {
        fs::File::open(parent).map(Some).map_err(|err| {
            OperatorAuthError::persistence_failure(format!(
                "failed to open operator auth directory for durable sync: {err}"
            ))
        })
    }
    #[cfg(not(unix))]
    {
        let _ = parent;
        Ok(None)
    }
}
fn parse_registration_payload(
    payload: &norito::json::Value,
) -> Result<RegistrationInput, OperatorAuthError> {
    let obj = payload.as_object().ok_or_else(|| {
        OperatorAuthError::invalid_payload("credential payload must be an object")
    })?;
    require_exact_json_fields(
        obj,
        &["id", "rawId", "response", "type"],
        "credential payload",
    )
    .map_err(OperatorAuthError::invalid_payload)?;
    require_public_key_credential_type(obj)?;
    let raw_id = parse_credential_id(obj)?;
    let response = obj
        .get("response")
        .and_then(|value| value.as_object())
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential response missing"))?;
    require_exact_json_fields(
        response,
        &["attestationObject", "clientDataJSON"],
        "credential response",
    )
    .map_err(OperatorAuthError::invalid_payload)?;
    let client_data = response
        .get("clientDataJSON")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("clientDataJSON missing"))?;
    let attestation = response
        .get("attestationObject")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("attestationObject missing"))?;
    Ok(RegistrationInput {
        raw_id,
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
    require_exact_json_fields(
        obj,
        &["id", "rawId", "response", "type"],
        "credential payload",
    )
    .map_err(OperatorAuthError::invalid_payload)?;
    require_public_key_credential_type(obj)?;
    let raw_id = parse_credential_id(obj)?;
    let response = obj
        .get("response")
        .and_then(|value| value.as_object())
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential response missing"))?;
    require_exact_json_fields(
        response,
        &["authenticatorData", "clientDataJSON", "signature"],
        "credential response",
    )
    .map_err(OperatorAuthError::invalid_payload)?;
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
        raw_id,
        client_data_json: decode_b64url("clientDataJSON", client_data)?,
        authenticator_data: decode_b64url("authenticatorData", authenticator_data)?,
        signature: decode_b64url("signature", signature)?,
    })
}
fn require_public_key_credential_type(object: &norito::json::Map) -> Result<(), OperatorAuthError> {
    match object.get("type").and_then(norito::json::Value::as_str) {
        Some("public-key") => Ok(()),
        _ => Err(OperatorAuthError::invalid_payload(
            "credential type must be `public-key`",
        )),
    }
}
fn parse_credential_id(object: &norito::json::Map) -> Result<Vec<u8>, OperatorAuthError> {
    let raw_id = object
        .get("rawId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential rawId missing"))?;
    let id = object
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| OperatorAuthError::invalid_payload("credential id missing"))?;
    let raw_id = decode_b64url("rawId", raw_id)?;
    let id = decode_b64url("id", id)?;
    if raw_id.len() > MAX_CREDENTIAL_ID_BYTES {
        return Err(OperatorAuthError::invalid_payload(format!(
            "credential id must not exceed {MAX_CREDENTIAL_ID_BYTES} bytes"
        )));
    }
    if raw_id != id {
        return Err(OperatorAuthError::invalid_payload(
            "credential id and rawId must identify the same credential",
        ));
    }
    Ok(raw_id)
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
    match obj.get("crossOrigin") {
        None | Some(norito::json::Value::Bool(false)) => {}
        Some(norito::json::Value::Bool(true)) => {
            return Err(OperatorAuthError::invalid_payload(
                "cross-origin WebAuthn ceremonies are not allowed",
            ));
        }
        Some(_) => {
            return Err(OperatorAuthError::invalid_payload(
                "clientDataJSON crossOrigin must be a boolean",
            ));
        }
    }
    if obj.contains_key("topOrigin") {
        return Err(OperatorAuthError::invalid_payload(
            "clientDataJSON topOrigin is not allowed",
        ));
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
    let value = decode_single_cbor_value(bytes, "attestationObject")?;
    let map = match value {
        CborValue::Map(map) => map,
        _ => {
            return Err(OperatorAuthError::invalid_payload(
                "attestationObject must be a CBOR map",
            ));
        }
    };
    require_exact_cbor_text_keys(&map, &["attStmt", "authData", "fmt"], "attestationObject")?;
    match expect_cbor_value_text_key(&map, "fmt")? {
        CborValue::Text(format) if format == "none" => {}
        _ => {
            return Err(OperatorAuthError::invalid_payload(
                "attestationObject fmt must be `none`",
            ));
        }
    }
    match expect_cbor_value_text_key(&map, "attStmt")? {
        CborValue::Map(statement) if statement.is_empty() => {}
        _ => {
            return Err(OperatorAuthError::invalid_payload(
                "attestationObject attStmt must be an empty CBOR map for fmt `none`",
            ));
        }
    }
    let auth_data = expect_cbor_bytes(&map, "authData")?;
    Ok(AttestationObject { auth_data })
}
fn decode_single_cbor_value(
    bytes: &[u8],
    label: &'static str,
) -> Result<CborValue, OperatorAuthError> {
    let mut reader = Cursor::new(bytes);
    let value: CborValue = from_reader(&mut reader).map_err(|_| {
        OperatorAuthError::invalid_payload(format!("{label} must contain one CBOR value"))
    })?;
    if reader.position() != u64::try_from(bytes.len()).expect("slice length fits u64") {
        return Err(OperatorAuthError::invalid_payload(format!(
            "{label} contains trailing CBOR data"
        )));
    }
    Ok(value)
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
    validate_authenticator_flags(flags, policy.require_user_verification, true)?;
    let sign_count =
        u32::from_be_bytes(auth_data[33..37].try_into().expect("slice length verified"));
    let mut offset = 37 + 16;
    let credential_len = u16::from_be_bytes(
        auth_data[offset..offset + 2]
            .try_into()
            .expect("slice length verified"),
    ) as usize;
    offset += 2;
    if credential_len == 0 || credential_len > MAX_CREDENTIAL_ID_BYTES {
        return Err(OperatorAuthError::invalid_payload(format!(
            "credential id length must be between 1 and {MAX_CREDENTIAL_ID_BYTES} bytes"
        )));
    }
    if auth_data.len() < offset + credential_len {
        return Err(OperatorAuthError::invalid_payload(
            "credential id extends past authenticatorData",
        ));
    }
    let credential_id = auth_data[offset..offset + credential_len].to_vec();
    offset += credential_len;
    let cose_value = decode_single_cbor_value(&auth_data[offset..], "credential public key")?;
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
    validate_authenticator_flags(flags, policy.require_user_verification, false)?;
    if auth_data.len() != 37 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData contains trailing bytes without extensions",
        ));
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
    require_unique_cbor_integer_keys(map, "credential public key")?;
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
            parse_es256_public_key(&public_key)?;
            Ok(CoseKey {
                alg: OperatorWebAuthnAlgorithm::Es256,
                public_key,
            })
        }
        (1, -8, 6) => {
            if !allowed.contains(&OperatorWebAuthnAlgorithm::Ed25519) {
                return Err(OperatorAuthError::credential_not_allowed());
            }
            validate_credential_public_key(OperatorWebAuthnAlgorithm::Ed25519, &x)?;
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
fn validate_credential_public_key(
    alg: OperatorWebAuthnAlgorithm,
    public_key: &[u8],
) -> Result<(), OperatorAuthError> {
    match alg {
        OperatorWebAuthnAlgorithm::Es256 => {
            if public_key.len() != P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN
                || public_key.first() != Some(&0x04)
            {
                return Err(OperatorAuthError::invalid_payload(
                    "ES256 public key must be canonical uncompressed SEC1",
                ));
            }
            parse_es256_public_key(public_key).map(|_| ())
        }
        OperatorWebAuthnAlgorithm::Ed25519 => {
            let bytes: &[u8; 32] = public_key.try_into().map_err(|_| {
                OperatorAuthError::invalid_payload("Ed25519 public key must contain 32 bytes")
            })?;
            let key = ed25519_dalek::VerifyingKey::from_bytes(bytes)
                .map_err(|_| OperatorAuthError::invalid_payload("invalid Ed25519 public key"))?;
            if key.is_weak() {
                return Err(OperatorAuthError::invalid_payload(
                    "weak Ed25519 public key is not allowed",
                ));
            }
            Ok(())
        }
    }
}
fn verify_signature(
    alg: OperatorWebAuthnAlgorithm,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), OperatorAuthError> {
    if !signature.is_empty() && signature.iter().all(|byte| *byte == 0) {
        return Err(OperatorAuthError::signature_invalid());
    }
    match alg {
        OperatorWebAuthnAlgorithm::Es256 => {
            let verifying_key = parse_es256_public_key(public_key)?;
            let sig = P256Signature::from_der(signature)
                .map_err(|_| OperatorAuthError::signature_invalid())?;
            if sig.normalize_s().is_some() {
                return Err(OperatorAuthError::signature_invalid());
            }
            verifying_key
                .verify(message, &sig)
                .map_err(|_| OperatorAuthError::signature_invalid())
        }
        OperatorWebAuthnAlgorithm::Ed25519 => {
            validate_credential_public_key(alg, public_key)
                .map_err(|_| OperatorAuthError::signature_invalid())?;
            if signature.len() != 64 {
                return Err(OperatorAuthError::signature_invalid());
            }
            let verifying_key = PublicKey::from_bytes(Algorithm::Ed25519, public_key)
                .map_err(|_| OperatorAuthError::signature_invalid())?;
            let signature = iroha_crypto::ed25519_parse_signature(signature)
                .map_err(|_| OperatorAuthError::signature_invalid())?;
            signature
                .verify(&verifying_key, message)
                .map_err(|_| OperatorAuthError::signature_invalid())
        }
    }
}
fn parse_es256_public_key(public_key: &[u8]) -> Result<P256Key, OperatorAuthError> {
    if p256_public_key_has_zero_coordinate_material(public_key) {
        return Err(OperatorAuthError::invalid_payload(
            "invalid ES256 public key",
        ));
    }
    let encoded = p256::EncodedPoint::from_bytes(public_key)
        .map_err(|_| OperatorAuthError::invalid_payload("invalid ES256 public key encoding"))?;
    P256Key::from_encoded_point(&encoded)
        .map_err(|_| OperatorAuthError::invalid_payload("invalid ES256 public key"))
}
fn p256_public_key_has_zero_coordinate_material(public_key: &[u8]) -> bool {
    public_key.len() == P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN
        && public_key.first().copied() == Some(0x04)
        && public_key[1..].iter().all(|byte| *byte == 0)
}
fn cbor_int(value: &CborValue) -> Result<i128, OperatorAuthError> {
    match value {
        CborValue::Integer(value) => Ok(i128::from(value.clone())),
        _ => Err(OperatorAuthError::invalid_payload(
            "COSE value must be an integer",
        )),
    }
}
fn validate_authenticator_flags(
    flags: u8,
    require_user_verification: bool,
    require_attested_credential_data: bool,
) -> Result<(), OperatorAuthError> {
    if flags & RESERVED_AUTHENTICATOR_FLAGS != 0 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData uses reserved flag bits",
        ));
    }
    if flags & FLAG_BACKUP_STATE != 0 && flags & FLAG_BACKUP_ELIGIBLE == 0 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData backup-state flag requires backup eligibility",
        ));
    }
    if flags & FLAG_EXTENSION_DATA != 0 {
        return Err(OperatorAuthError::invalid_payload(
            "authenticatorData extensions are not supported by the V1 operator profile",
        ));
    }
    if flags & FLAG_USER_PRESENT == 0 {
        return Err(OperatorAuthError::user_presence_required());
    }
    if require_user_verification && flags & FLAG_USER_VERIFIED == 0 {
        return Err(OperatorAuthError::user_verification_required());
    }
    let has_attested_credential_data = flags & FLAG_ATTESTED_CREDENTIAL_DATA != 0;
    if has_attested_credential_data != require_attested_credential_data {
        return Err(OperatorAuthError::invalid_payload(
            if require_attested_credential_data {
                "authenticatorData missing attested credential data"
            } else {
                "assertion authenticatorData must not contain attested credential data"
            },
        ));
    }
    Ok(())
}
fn require_exact_cbor_text_keys(
    map: &[(CborValue, CborValue)],
    expected: &[&str],
    context: &str,
) -> Result<(), OperatorAuthError> {
    let mut keys = HashSet::with_capacity(map.len());
    for (key, _) in map {
        let CborValue::Text(key) = key else {
            return Err(OperatorAuthError::invalid_payload(format!(
                "{context} keys must be text"
            )));
        };
        if !expected.contains(&key.as_str()) {
            return Err(OperatorAuthError::invalid_payload(format!(
                "{context} contains unknown field `{key}`"
            )));
        }
        if !keys.insert(key.as_str()) {
            return Err(OperatorAuthError::invalid_payload(format!(
                "{context} contains duplicate field `{key}`"
            )));
        }
    }
    if keys.len() != expected.len() {
        return Err(OperatorAuthError::invalid_payload(format!(
            "{context} is missing a required field"
        )));
    }
    Ok(())
}
fn require_unique_cbor_integer_keys(
    map: &[(CborValue, CborValue)],
    context: &str,
) -> Result<(), OperatorAuthError> {
    let mut keys = HashSet::with_capacity(map.len());
    for (key, _) in map {
        let key = cbor_int(key)?;
        if !keys.insert(key) {
            return Err(OperatorAuthError::invalid_payload(format!(
                "{context} contains duplicate COSE label {key}"
            )));
        }
    }
    Ok(())
}
fn expect_cbor_value_text_key<'a>(
    map: &'a [(CborValue, CborValue)],
    key: &str,
) -> Result<&'a CborValue, OperatorAuthError> {
    map.iter()
        .find(|(candidate, _)| matches!(candidate, CborValue::Text(text) if text == key))
        .map(|(_, value)| value)
        .ok_or_else(|| OperatorAuthError::invalid_payload("missing CBOR map entry"))
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
pub async fn handle_operator_register_options(
    State(app): State<SharedAppState>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let ctx = app
        .operator_auth
        .authorize_bootstrap(&headers, Some(remote.ip()), ACTION_REGISTER_OPTIONS)
        .await?;
    require_empty_options_body(body)
        .await
        .map_err(|error| app.operator_auth.record_error(&ctx, ACTION_REGISTER_OPTIONS, error))?;
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
/// List enrolled operator WebAuthn credentials without exposing verification keys.
pub async fn handle_operator_credentials(
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, OperatorAuthError> {
    Ok(JsonBody(app.operator_auth.credential_inventory()?))
}
/// Delete one operator WebAuthn credential and revoke all outstanding operator auth state.
pub async fn handle_operator_credential_delete(
    State(app): State<SharedAppState>,
    AxumPath(credential_id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let generation = app
        .operator_auth
        .credential_management_generation(&headers)?;
    let outcome = app
        .operator_auth
        .delete_credential(&credential_id, generation)?;
    Ok(JsonBody(json_object(vec![
        json_entry("status", "ok"),
        json_entry("credential_id", outcome.credential_id),
        json_entry("credentials_total", outcome.credentials_total),
    ])))
}
pub async fn handle_operator_login_options(
    State(app): State<SharedAppState>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, OperatorAuthError> {
    let ctx = app
        .operator_auth
        .authorize_login(&headers, Some(remote.ip()), ACTION_LOGIN_OPTIONS)
        .await?;
    require_empty_options_body(body)
        .await
        .map_err(|error| app.operator_auth.record_error(&ctx, ACTION_LOGIN_OPTIONS, error))?;
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
#[cfg(all(test, feature = "app_api"))]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use ciborium::ser::into_writer;
    use ed25519_dalek::Signer as _;
    use p256::{
        ecdsa::{SigningKey, signature::Signer as _},
        elliptic_curve::rand_core::OsRng,
    };
    use rand::rand_core::{TryCryptoRng, TryRngCore};
    use rand::rngs::OsRng as FallibleOsRng;
    const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const ED25519_NONCANONICAL_IDENTITY: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    #[derive(Debug)]
    struct FailingOperatorAuthRng;
    #[derive(Debug)]
    struct FailingOperatorAuthRngError;
    impl std::fmt::Display for FailingOperatorAuthRngError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failing operator auth RNG")
        }
    }
    impl std::error::Error for FailingOperatorAuthRngError {}
    impl TryRngCore for FailingOperatorAuthRng {
        type Error = FailingOperatorAuthRngError;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(FailingOperatorAuthRngError)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(FailingOperatorAuthRngError)
        }
        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            Err(FailingOperatorAuthRngError)
        }
    }
    impl TryCryptoRng for FailingOperatorAuthRng {}
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
        tokens: Vec<String>,
        lockout: OperatorAuthLockout,
        algorithms: Vec<OperatorWebAuthnAlgorithm>,
    ) -> ToriiOperatorAuth {
        let tokens = if tokens.is_empty() {
            vec![test_bootstrap_token("default")]
        } else {
            tokens
                .into_iter()
                .map(|token| test_bootstrap_token(&token))
                .collect()
        };
        ToriiOperatorAuth {
            enabled: true,
            require_mtls: false,
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::operator_auth::mtls_trusted_proxy_cidrs(),
            tokens,
            rate_per_minute: None,
            burst: None,
            ephemeral_state_capacity: NonZeroUsize::new(4_096).expect("non-zero capacity"),
            credential_capacity: NonZeroUsize::new(64).expect("non-zero capacity"),
            lockout,
            webauthn: Some(base_webauthn_config(algorithms)),
        }
    }
    fn test_bootstrap_token(label: &str) -> String {
        format!("iroha-test-bootstrap-token-{label}-0123456789")
    }
    fn test_session_token(seed: u8) -> String {
        encode_b64url(&[seed; SESSION_TOKEN_BYTES])
    }
    fn build_operator_auth(config: ToriiOperatorAuth, data_dir: &Path) -> OperatorAuth {
        OperatorAuth::new(config, data_dir.to_path_buf(), MaybeTelemetry::disabled())
            .expect("operator auth")
    }
    fn session_authority(auth: &OperatorAuth) -> EnrollmentAuthority {
        EnrollmentAuthority::Session(
            auth.credential_revocation_generation.load(Ordering::Acquire),
        )
    }
    fn credential_management_authority(auth: &OperatorAuth) -> u64 {
        auth.credential_revocation_generation.load(Ordering::Acquire)
    }
    fn write_credentials_fixture(data_dir: &Path, body: &str) {
        let path = operator_credentials_path(data_dir);
        fs::create_dir_all(path.parent().expect("credentials parent"))
            .expect("create credentials directory");
        fs::write(path, body).expect("write credentials fixture");
    }
    fn es256_credential(id: &[u8], sign_count: u32, created_at_ms: u64) -> StoredCredential {
        let signing_key = SigningKey::random(&mut OsRng);
        StoredCredential {
            id: id.to_vec(),
            public_key: signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes()
                .to_vec(),
            alg: OperatorWebAuthnAlgorithm::Es256,
            sign_count,
            created_at_ms,
        }
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
    fn credential_inventory_is_stable_and_never_exposes_public_keys() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"z-credential", 7, 200),
            session_authority(&auth),
        )
        .expect("insert z credential");
        auth.insert_credential(
            es256_credential(b"a-credential", 3, 100),
            session_authority(&auth),
        )
        .expect("insert a credential");

        let inventory = auth.credential_inventory().expect("credential inventory");
        assert_eq!(inventory["credentials_total"].as_u64(), Some(2));
        let credentials = inventory["credentials"]
            .as_array()
            .expect("credentials array");
        let a_credential_id = encode_b64url(b"a-credential");
        let z_credential_id = encode_b64url(b"z-credential");
        assert_eq!(
            credentials[0]["credential_id"].as_str(),
            Some(a_credential_id.as_str())
        );
        assert_eq!(credentials[0]["algorithm"].as_str(), Some("es256"));
        assert_eq!(credentials[0]["sign_count"].as_u64(), Some(3));
        assert_eq!(credentials[0]["created_at_ms"].as_u64(), Some(100));
        assert_eq!(
            credentials[1]["credential_id"].as_str(),
            Some(z_credential_id.as_str())
        );
        for credential in credentials {
            let object = credential.as_object().expect("credential metadata");
            assert_eq!(object.len(), 4);
            assert!(!object.contains_key("public_key"));
            assert!(!object.contains_key("public_key_b64"));
        }
    }
    #[test]
    fn credential_deletion_requires_a_canonical_bounded_known_id() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"delete-me", 0, 1),
            session_authority(&auth),
        )
        .expect("insert credential to delete");
        auth.insert_credential(
            es256_credential(b"keep-me", 0, 2),
            session_authority(&auth),
        )
        .expect("insert credential to keep");

        for malformed in [
            String::new(),
            "ZGVsZXRlLW1l=".to_owned(),
            encode_b64url(&vec![0; MAX_CREDENTIAL_ID_BYTES + 1]),
        ] {
            let error = auth
                .delete_credential(&malformed, credential_management_authority(&auth))
                .expect_err("noncanonical or oversized id must fail");
            assert_eq!(error.status, StatusCode::BAD_REQUEST);
            assert_eq!(error.code, "operator_webauthn_payload_invalid");
        }
        let unknown = auth
            .delete_credential(
                &encode_b64url(b"not-enrolled"),
                credential_management_authority(&auth),
            )
            .expect_err("unknown credential must fail");
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);
        assert_eq!(unknown.code, "operator_webauthn_credential_not_found");

        let deleted = auth
            .delete_credential(
                &encode_b64url(b"delete-me"),
                credential_management_authority(&auth),
            )
            .expect("canonical enrolled id deletes");
        assert_eq!(deleted.credential_id, encode_b64url(b"delete-me"));
        assert_eq!(deleted.credentials_total, 1);
        assert_eq!(auth.credentials_read().expect("credential state").len(), 1);
    }
    #[test]
    fn last_credential_requires_a_bootstrap_recovery_path() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"only-credential", 0, 1),
            session_authority(&auth),
        )
        .expect("persist sole credential");
        drop(auth);

        let mut no_bootstrap = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        no_bootstrap.tokens.clear();
        let restarted = build_operator_auth(no_bootstrap, tempdir.path());
        let error = restarted
            .delete_credential(
                &encode_b64url(b"only-credential"),
                credential_management_authority(&restarted),
            )
            .expect_err("last credential needs a bootstrap recovery path");
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "operator_webauthn_last_credential");
        assert_eq!(
            restarted
                .credentials_read()
                .expect("credential state")
                .len(),
            1
        );
        drop(restarted);

        let with_bootstrap = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let restarted = build_operator_auth(with_bootstrap, tempdir.path());
        let deleted = restarted
            .delete_credential(
                &encode_b64url(b"only-credential"),
                credential_management_authority(&restarted),
            )
            .expect("bootstrap token permits deleting the last credential");
        assert_eq!(deleted.credentials_total, 0);
    }
    #[test]
    fn credential_deletion_is_persisted_across_restart() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"removed", 0, 1),
            session_authority(&auth),
        )
        .expect("insert removed credential");
        auth.insert_credential(
            es256_credential(b"retained", 0, 2),
            session_authority(&auth),
        )
        .expect("insert retained credential");
        auth.delete_credential(
            &encode_b64url(b"removed"),
            credential_management_authority(&auth),
        )
        .expect("delete persisted credential");
        drop(auth);

        let mut restart = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        restart.tokens.clear();
        let restarted = build_operator_auth(restart, tempdir.path());
        let credentials = restarted.credentials_read().expect("credential state");
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].id, b"retained");
    }
    #[test]
    fn credential_deletion_changes_nothing_when_persistence_fails() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let mut auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"delete-me", 0, 1),
            session_authority(&auth),
        )
        .expect("insert deleted credential");
        auth.insert_credential(
            es256_credential(b"keep-me", 0, 2),
            session_authority(&auth),
        )
        .expect("insert retained credential");
        let persisted_path = auth.credentials_path.clone();
        let persisted_before = fs::read(&persisted_path).expect("persisted credentials");
        let ctx = AuthContext {
            key: "credential-delete-rollback".to_owned(),
            enrollment_authority: session_authority(&auth),
        };
        auth.webauthn_authentication_options(&ctx)
            .expect("authentication challenge");
        let mut rng = FallibleOsRng;
        let session = auth
            .issue_session_with_rng(b"delete-me", Duration::from_secs(60), &mut rng)
            .expect("operator session");
        let generation_before = auth
            .credential_revocation_generation
            .load(Ordering::Acquire);
        let blocked_parent = tempdir.path().join("delete-not-a-directory");
        fs::write(&blocked_parent, b"block directory creation").expect("blocker file");
        auth.credentials_path = blocked_parent.join(CREDENTIALS_FILENAME);

        let error = auth
            .delete_credential(&encode_b64url(b"delete-me"), generation_before)
            .expect_err("failed persistence must abort deletion");
        assert_eq!(error.code, "operator_webauthn_persist_failed");
        assert_eq!(auth.credentials_read().expect("credential state").len(), 2);
        assert_eq!(
            fs::read(persisted_path).expect("persisted credentials"),
            persisted_before
        );
        assert_eq!(auth.challenges.lock().len(), 1);
        assert!(auth.session_valid(&session.session_token));
        assert_eq!(
            auth.credential_revocation_generation.load(Ordering::Acquire),
            generation_before
        );
    }
    #[test]
    fn credential_deletion_invalidates_all_sessions_and_challenges() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"revoked", 0, 1),
            session_authority(&auth),
        )
        .expect("insert revoked credential");
        auth.insert_credential(
            es256_credential(b"remaining", 0, 2),
            session_authority(&auth),
        )
        .expect("insert remaining credential");
        let ctx = AuthContext {
            key: "credential-revocation".to_owned(),
            enrollment_authority: session_authority(&auth),
        };
        auth.webauthn_registration_options(&ctx)
            .expect("registration challenge");
        auth.webauthn_authentication_options(&ctx)
            .expect("authentication challenge");
        let mut rng = FallibleOsRng;
        let session = auth
            .issue_session_with_rng(b"revoked", Duration::from_secs(60), &mut rng)
            .expect("operator session");
        assert_eq!(auth.challenges.lock().len(), 2);
        assert_eq!(auth.sessions.lock().len(), 1);

        auth.delete_credential(
            &encode_b64url(b"revoked"),
            credential_management_authority(&auth),
        )
        .expect("delete credential");
        assert_eq!(auth.challenges.lock().len(), 0);
        assert_eq!(auth.sessions.lock().len(), 0);
        assert!(!auth.session_valid(&session.session_token));
    }
    #[test]
    fn credential_deletion_rechecks_each_captured_session_generation() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"first-delete", 0, 1),
            session_authority(&auth),
        )
        .expect("insert first credential to delete");
        auth.insert_credential(
            es256_credential(b"second-delete", 0, 2),
            session_authority(&auth),
        )
        .expect("insert second credential to delete");
        auth.insert_credential(
            es256_credential(b"retained", 0, 3),
            session_authority(&auth),
        )
        .expect("insert retained credential");

        let generation = credential_management_authority(&auth);
        let now = Instant::now();
        let first_session = test_session_token(1);
        let second_session = test_session_token(2);
        {
            let mut sessions = auth.sessions.lock();
            for token in [&first_session, &second_session] {
                sessions
                    .insert(
                        token.clone(),
                        SessionEntry {
                            credential_revocation_generation: generation,
                        },
                        now + Duration::from_secs(60),
                        now,
                    )
                    .expect("test session fits bounded state");
            }
        }
        let mut first_headers = HeaderMap::new();
        first_headers.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_str(&first_session).expect("first session header"),
        );
        let mut second_headers = HeaderMap::new();
        second_headers.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_str(&second_session).expect("second session header"),
        );
        let first_authority = auth
            .credential_management_generation(&first_headers)
            .expect("first deletion authority");
        let second_authority = auth
            .credential_management_generation(&second_headers)
            .expect("second deletion authority captured before revocation");
        assert_eq!(first_authority, second_authority);

        auth.delete_credential(&encode_b64url(b"first-delete"), first_authority)
            .expect("first deletion succeeds");
        let generation_after_first = credential_management_authority(&auth);
        let persisted_after_first =
            fs::read(&auth.credentials_path).expect("credentials persisted after first deletion");
        let credential_ids_after_first = auth
            .credentials_read()
            .expect("credential state after first deletion")
            .iter()
            .map(|credential| credential.id.clone())
            .collect::<Vec<_>>();

        let error = auth
            .delete_credential(&encode_b64url(b"second-delete"), second_authority)
            .expect_err("stale second deletion authority must be rejected");
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert_eq!(error.code, "operator_session_invalid");
        assert_eq!(
            credential_management_authority(&auth),
            generation_after_first
        );
        assert_eq!(
            fs::read(&auth.credentials_path).expect("credentials remain persisted"),
            persisted_after_first
        );
        assert_eq!(
            auth.credentials_read()
                .expect("credential state remains readable")
                .iter()
                .map(|credential| credential.id.clone())
                .collect::<Vec<_>>(),
            credential_ids_after_first
        );
    }
    #[test]
    fn credential_deletion_rejects_an_in_flight_stale_session_enrollment() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.insert_credential(
            es256_credential(b"revoked", 0, 1),
            session_authority(&auth),
        )
        .expect("insert revoked credential");
        auth.insert_credential(
            es256_credential(b"remaining", 0, 2),
            session_authority(&auth),
        )
        .expect("insert remaining credential");
        let stale_session_authority = session_authority(&auth);

        auth.delete_credential(
            &encode_b64url(b"revoked"),
            credential_management_authority(&auth),
        )
        .expect("delete credential");
        let error = auth
            .insert_credential(
                es256_credential(b"stale-enrollment", 0, 3),
                stale_session_authority,
            )
            .expect_err("a session captured before deletion must not enroll a credential");
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert_eq!(error.code, "operator_session_invalid");
        let credentials = auth.credentials_read().expect("credential state");
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].id, b"remaining");
    }
    #[test]
    fn origin_allows_default_port_and_trailing_slash() {
        let allowed = vec![Url::parse("https://example.com").expect("origin")];
        assert!(origin_allowed("https://example.com/", &allowed));
        assert!(origin_allowed("https://example.com:443", &allowed));
        assert!(!origin_allowed("https://example.com:444", &allowed));
        for malformed in [
            "https://user@example.com/",
            "https://example.com/path",
            "https://example.com/?query",
            "https://example.com/#fragment",
        ] {
            assert!(!origin_allowed(malformed, &allowed));
        }
    }
    #[test]
    fn credential_id_requires_matching_id_and_raw_id() {
        let encoded = encode_b64url(b"credential");
        let payload = json_object(vec![
            json_entry("id", encoded.clone()),
            json_entry("rawId", encoded),
        ]);
        let object = payload.as_object().expect("credential object");
        assert_eq!(
            parse_credential_id(object).expect("matching identifiers"),
            b"credential"
        );

        let mismatch = json_object(vec![
            json_entry("id", encode_b64url(b"credential-a")),
            json_entry("rawId", encode_b64url(b"credential-b")),
        ]);
        let error = parse_credential_id(mismatch.as_object().expect("credential object"))
            .expect_err("mismatched identifiers must fail closed");
        assert_eq!(error.code, "operator_webauthn_payload_invalid");

        let missing_raw_id = json_object(vec![json_entry("id", encode_b64url(b"credential"))]);
        assert!(
            parse_credential_id(missing_raw_id.as_object().expect("credential object")).is_err()
        );
    }
    #[test]
    fn webauthn_verify_payloads_require_exact_v1_fields() {
        let mut assertion = build_assertion_payload(b"credential", b"client", b"auth", b"sig");
        parse_assertion_payload(&assertion).expect("canonical assertion envelope");
        assertion
            .as_object_mut()
            .expect("assertion object")
            .insert("legacy".to_owned(), true.into());
        assert!(parse_assertion_payload(&assertion).is_err());

        let mut registration = build_registration_payload(b"credential", b"client", b"attestation");
        parse_registration_payload(&registration).expect("canonical registration envelope");
        registration
            .as_object_mut()
            .expect("registration object")
            .insert("type".to_owned(), "not-public-key".into());
        assert!(parse_registration_payload(&registration).is_err());

        let mut response_extra = build_assertion_payload(b"credential", b"client", b"auth", b"sig");
        response_extra
            .get_mut("response")
            .and_then(norito::json::Value::as_object_mut)
            .expect("assertion response")
            .insert("userHandle".to_owned(), norito::json::Value::Null);
        assert!(parse_assertion_payload(&response_extra).is_err());
    }
    #[tokio::test]
    async fn webauthn_options_require_an_exact_empty_body() {
        require_empty_options_body(Body::empty())
            .await
            .expect("empty options body");
        let error = require_empty_options_body(Body::from("x"))
            .await
            .expect_err("nonempty options body must fail");
        assert_eq!(error.code, "operator_webauthn_payload_invalid");
    }
    #[test]
    fn client_data_rejects_cross_origin_contexts() {
        for extra in [
            json_entry("crossOrigin", true),
            json_entry("topOrigin", "https://embedder.example"),
        ] {
            let payload = json_object(vec![
                json_entry("type", "webauthn.get"),
                json_entry("challenge", "challenge"),
                json_entry("origin", "https://example.com"),
                extra,
            ]);
            let bytes = norito::json::to_vec(&payload).expect("clientDataJSON");
            let error = parse_client_data(&bytes, "webauthn.get")
                .expect_err("cross-origin context must fail closed");
            assert_eq!(error.code, "operator_webauthn_payload_invalid");
        }
    }
    #[test]
    fn credentials_lock_fails_closed_after_poison() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout {
                failures: None,
                window: Duration::from_secs(0),
                duration: Duration::from_secs(0),
            },
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        {
            let mut creds = auth.credentials_write().expect("credential lock");
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
        let err = auth
            .has_credentials()
            .expect_err("poisoned credential state must fail closed");
        assert_eq!(err.code, "operator_webauthn_state_unavailable");
    }
    #[test]
    fn operator_auth_rejects_zero_ephemeral_duration() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config
            .webauthn
            .as_mut()
            .expect("WebAuthn config")
            .challenge_ttl = Duration::ZERO;

        let error = match OperatorAuth::new(
            config,
            tempdir.path().to_path_buf(),
            MaybeTelemetry::disabled(),
        ) {
            Ok(_) => panic!("zero challenge TTL must fail initialization"),
            Err(error) => error,
        };
        assert!(matches!(error, OperatorAuthInitError::InvalidPolicy(_)));
    }
    #[test]
    fn operator_auth_requires_an_exact_first_enrollment_path() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.tokens.clear();
        assert!(matches!(
            OperatorAuth::new(
                config,
                tempdir.path().to_path_buf(),
                MaybeTelemetry::disabled(),
            ),
            Err(OperatorAuthInitError::InvalidPolicy(_))
        ));

        for tokens in [
            vec!["short".to_owned()],
            vec![" token-with-whitespace-012345678901".to_owned()],
            vec![
                test_bootstrap_token("duplicate"),
                test_bootstrap_token("duplicate"),
            ],
        ] {
            assert!(matches!(
                validate_bootstrap_tokens(true, &tokens),
                Err(OperatorAuthInitError::InvalidPolicy(_))
            ));
        }

        let mut too_many_tokens = Vec::new();
        for index in 0..=iroha_config::parameters::defaults::torii::operator_auth::MAX_BOOTSTRAP_TOKENS
        {
            too_many_tokens.push(test_bootstrap_token(&format!("token-{index}")));
        }
        assert!(matches!(
            validate_bootstrap_tokens(true, &too_many_tokens),
            Err(OperatorAuthInitError::InvalidPolicy(_))
        ));

        let mut oversized_capacity = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        oversized_capacity.credential_capacity = NonZeroUsize::new(
            iroha_config::parameters::defaults::torii::operator_auth::MAX_CREDENTIAL_CAPACITY + 1,
        )
        .expect("non-zero capacity");
        assert!(matches!(
            OperatorAuth::new(
                oversized_capacity,
                tempdir.path().to_path_buf(),
                MaybeTelemetry::disabled(),
            ),
            Err(OperatorAuthInitError::InvalidPolicy(_))
        ));
    }
    #[test]
    fn enrolled_operator_auth_restarts_without_a_bootstrap_token() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let signing_key = SigningKey::random(&mut OsRng);
        auth.insert_credential(
            StoredCredential {
                id: b"restart-credential".to_vec(),
                public_key: signing_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes()
                    .to_vec(),
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 0,
                created_at_ms: 1,
            },
            EnrollmentAuthority::BootstrapToken,
        )
        .expect("persist first credential");
        drop(auth);

        let mut restart = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        restart.tokens.clear();
        let restarted = OperatorAuth::new(
            restart,
            tempdir.path().to_path_buf(),
            MaybeTelemetry::disabled(),
        )
        .expect("persisted credential owns restart admission");
        assert!(restarted.has_credentials().expect("credential state"));
    }
    #[tokio::test]
    async fn operator_auth_preserves_fractional_per_minute_rate() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.rate_per_minute = std::num::NonZeroU32::new(1);
        config.burst = std::num::NonZeroU32::new(1);
        let auth = build_operator_auth(config, tempdir.path());

        assert!(auth.limiter.allow("operator").await);
        assert!(!auth.limiter.allow("operator").await);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(
            !auth.limiter.allow("operator").await,
            "a one-request-per-minute limit must not refill after one second"
        );
    }
    #[test]
    fn registration_options_reports_challenge_rng_failure() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let ctx = AuthContext {
            key: "registration-rng".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };
        let err = auth
            .webauthn_registration_options_with_rng(&ctx, &mut FailingOperatorAuthRng)
            .expect_err("registration challenge RNG failure");
        assert_eq!(err.code, "operator_auth_random_bytes_failed");
        assert!(err.message.contains("failing operator auth RNG"));
        assert_eq!(auth.challenges.lock().len(), 0);
    }
    #[test]
    fn authentication_options_reports_challenge_rng_failure() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        auth.credentials_write()
            .expect("credential lock")
            .push(StoredCredential {
                id: vec![1, 2, 3],
                public_key: Vec::new(),
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 0,
                created_at_ms: 0,
            });
        let ctx = AuthContext {
            key: "authentication-rng".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };
        let err = auth
            .webauthn_authentication_options_with_rng(&ctx, &mut FailingOperatorAuthRng)
            .expect_err("authentication challenge RNG failure");
        assert_eq!(err.code, "operator_auth_random_bytes_failed");
        assert!(err.message.contains("failing operator auth RNG"));
        assert_eq!(auth.challenges.lock().len(), 0);
    }
    #[test]
    fn challenge_and_session_admission_fail_closed_at_capacity() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.ephemeral_state_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        let auth = build_operator_auth(config, tempdir.path());
        auth.credentials_write()
            .expect("credential lock")
            .push(StoredCredential {
                id: vec![1],
                public_key: Vec::new(),
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 0,
                created_at_ms: 0,
            });
        let ctx = AuthContext {
            key: "capacity-test".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };

        auth.webauthn_authentication_options(&ctx)
            .expect("first challenge");
        let challenge_error = auth
            .webauthn_authentication_options(&ctx)
            .expect_err("second live challenge must exceed capacity");
        assert_eq!(
            challenge_error.code,
            "operator_auth_state_capacity_exhausted"
        );

        let mut rng = FallibleOsRng;
        auth.issue_session_with_rng(b"credential", Duration::from_secs(60), &mut rng)
            .expect("first session");
        let session_error = auth
            .issue_session_with_rng(b"credential", Duration::from_secs(60), &mut rng)
            .expect_err("second live session must exceed capacity");
        assert_eq!(session_error.code, "operator_auth_state_capacity_exhausted");
    }
    #[test]
    fn issue_session_reports_token_rng_failure() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let err = match auth.issue_session_with_rng(
            b"credential-id",
            Duration::from_secs(60),
            &mut FailingOperatorAuthRng,
        ) {
            Ok(_) => panic!("session token RNG failure must be reported"),
            Err(err) => err,
        };
        assert_eq!(err.code, "operator_auth_random_bytes_failed");
        assert!(err.message.contains("failing operator auth RNG"));
        assert_eq!(auth.sessions.lock().len(), 0);
    }
    fn headers_with_operator_token(token: &str) -> HeaderMap {
        let mut headers = base_headers();
        let token = test_bootstrap_token(token);
        headers.insert(
            HEADER_OPERATOR_TOKEN,
            HeaderValue::from_str(&token).expect("token"),
        );
        headers
    }
    #[test]
    fn operator_bootstrap_token_rejects_duplicate_header_lines() {
        let mut headers = HeaderMap::new();
        headers.append(HEADER_OPERATOR_TOKEN, HeaderValue::from_static("first"));
        headers.append(HEADER_OPERATOR_TOKEN, HeaderValue::from_static("second"));
        assert!(
            single_header_text(&headers, HEADER_OPERATOR_TOKEN).is_none(),
            "duplicate operator bootstrap token header lines must fail closed"
        );
    }
    #[test]
    fn operator_session_header_accepts_only_one_canonical_32_byte_token() {
        let canonical = test_session_token(0);
        assert_eq!(canonical.len(), SESSION_TOKEN_B64URL_BYTES);

        let mut headers = HeaderMap::new();
        assert_eq!(session_from_headers(&headers), SessionHeader::Missing);
        headers.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_str(&canonical).expect("canonical session header"),
        );
        assert_eq!(
            session_from_headers(&headers),
            SessionHeader::Valid(canonical.as_str())
        );

        let mut noncanonical = canonical.clone();
        noncanonical.replace_range(SESSION_TOKEN_B64URL_BYTES - 1.., "B");
        for malformed in [
            String::new(),
            encode_b64url(&[0_u8; SESSION_TOKEN_BYTES - 1]),
            encode_b64url(&[0_u8; SESSION_TOKEN_BYTES + 1]),
            format!("{canonical}="),
            "*".repeat(SESSION_TOKEN_B64URL_BYTES),
            noncanonical,
            "A".repeat(64 * 1_024),
        ] {
            headers.insert(
                HEADER_OPERATOR_SESSION,
                HeaderValue::from_str(&malformed).expect("syntactically valid HTTP header"),
            );
            assert_eq!(
                session_from_headers(&headers),
                SessionHeader::Invalid,
                "malformed session header must fail closed: length {}",
                malformed.len()
            );
        }

        headers.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_bytes(&[0x80; SESSION_TOKEN_B64URL_BYTES])
                .expect("opaque non-ASCII HTTP header"),
        );
        assert_eq!(session_from_headers(&headers), SessionHeader::Invalid);

        headers.clear();
        let canonical_header = HeaderValue::from_str(&canonical).expect("canonical session header");
        headers.append(HEADER_OPERATOR_SESSION, canonical_header.clone());
        headers.append(HEADER_OPERATOR_SESSION, canonical_header);
        assert_eq!(session_from_headers(&headers), SessionHeader::Invalid);
    }
    #[tokio::test]
    async fn session_header_missing_and_invalid_errors_are_exact_across_auth_paths() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let auth = build_operator_auth(
            base_operator_auth_config(
                vec!["bootstrap".to_owned()],
                OperatorAuthLockout {
                    failures: None,
                    ..OperatorAuthLockout::default()
                },
                vec![OperatorWebAuthnAlgorithm::Es256],
            ),
            tempdir.path(),
        );

        let missing = base_headers();
        let error = auth
            .authorize_operator_endpoint(&missing, loopback_ip())
            .await
            .expect_err("ordinary operator routes require a session");
        assert_eq!(error.code, "operator_session_missing");
        let error = auth
            .credential_management_generation(&missing)
            .expect_err("credential management requires a session");
        assert_eq!(error.code, "operator_session_missing");

        let mut invalid = base_headers();
        invalid.insert(HEADER_OPERATOR_SESSION, HeaderValue::from_static(""));
        let error = auth
            .authorize_operator_endpoint(&invalid, loopback_ip())
            .await
            .expect_err("a supplied malformed session is invalid, not missing");
        assert_eq!(error.code, "operator_session_invalid");
        let error = auth
            .credential_management_generation(&invalid)
            .expect_err("credential management rejects malformed sessions as invalid");
        assert_eq!(error.code, "operator_session_invalid");

        let canonical = test_session_token(7);
        let canonical_header = HeaderValue::from_str(&canonical).expect("canonical session header");
        let mut duplicate = base_headers();
        duplicate.append(HEADER_OPERATOR_SESSION, canonical_header.clone());
        duplicate.append(HEADER_OPERATOR_SESSION, canonical_header);
        let error = auth
            .authorize_operator_endpoint(&duplicate, loopback_ip())
            .await
            .expect_err("duplicate session headers are invalid");
        assert_eq!(error.code, "operator_session_invalid");

        let bootstrap = headers_with_operator_token("bootstrap");
        auth.authorize_bootstrap(&bootstrap, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect("a missing session preserves first-credential token bootstrap");
        let mut unknown_session_bootstrap = bootstrap.clone();
        unknown_session_bootstrap.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_str(&canonical).expect("canonical unknown session header"),
        );
        let error = auth
            .authorize_bootstrap(
                &unknown_session_bootstrap,
                loopback_ip(),
                ACTION_REGISTER_OPTIONS,
            )
            .await
            .expect_err("bootstrap must not override a supplied unknown session");
        assert_eq!(error.code, "operator_session_invalid");
        let mut malformed_bootstrap = bootstrap;
        malformed_bootstrap.insert(HEADER_OPERATOR_SESSION, HeaderValue::from_static(""));
        let error = auth
            .authorize_bootstrap(
                &malformed_bootstrap,
                loopback_ip(),
                ACTION_REGISTER_OPTIONS,
            )
            .await
            .expect_err("bootstrap must not hide a supplied malformed session");
        assert_eq!(error.code, "operator_session_invalid");
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
        let map = vec![
            (
                CborValue::Text("fmt".to_owned()),
                CborValue::Text("none".to_owned()),
            ),
            (
                CborValue::Text("attStmt".to_owned()),
                CborValue::Map(Vec::new()),
            ),
            (
                CborValue::Text("authData".to_owned()),
                CborValue::Bytes(auth_data),
            ),
        ];
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
        let credential_id = encode_b64url(credential_id);
        json_object(vec![
            json_entry("id", credential_id.clone()),
            json_entry("rawId", credential_id),
            json_entry("response", response),
            json_entry("type", "public-key"),
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
        let credential_id = encode_b64url(credential_id);
        json_object(vec![
            json_entry("id", credential_id.clone()),
            json_entry("rawId", credential_id),
            json_entry("response", response),
            json_entry("type", "public-key"),
        ])
    }
    #[test]
    fn attestation_object_requires_the_exact_none_profile() {
        let canonical = build_attestation_object(vec![1, 2, 3]);
        assert_eq!(
            parse_attestation_object(&canonical)
                .expect("canonical none attestation")
                .auth_data,
            [1, 2, 3]
        );

        let malformed = [
            CborValue::Map(vec![
                (
                    CborValue::Text("fmt".to_owned()),
                    CborValue::Text("packed".to_owned()),
                ),
                (
                    CborValue::Text("attStmt".to_owned()),
                    CborValue::Map(Vec::new()),
                ),
                (
                    CborValue::Text("authData".to_owned()),
                    CborValue::Bytes(vec![1]),
                ),
            ]),
            CborValue::Map(vec![
                (
                    CborValue::Text("fmt".to_owned()),
                    CborValue::Text("none".to_owned()),
                ),
                (
                    CborValue::Text("attStmt".to_owned()),
                    CborValue::Map(Vec::new()),
                ),
                (
                    CborValue::Text("authData".to_owned()),
                    CborValue::Bytes(vec![1]),
                ),
                (CborValue::Text("legacy".to_owned()), CborValue::Null),
            ]),
        ];
        for value in malformed {
            let mut encoded = Vec::new();
            into_writer(&value, &mut encoded).expect("malformed attestation fixture");
            assert!(parse_attestation_object(&encoded).is_err());
        }

        let mut trailing = canonical;
        trailing.push(0);
        assert!(parse_attestation_object(&trailing).is_err());
    }
    #[test]
    fn authenticator_data_rejects_unconsumed_or_invalid_flags() {
        let policy = WebAuthnPolicy::from_config(base_webauthn_config(vec![
            OperatorWebAuthnAlgorithm::Es256,
        ]))
        .expect("policy");
        let mut assertion = build_auth_data_assertion(&policy, 1);
        assertion.push(0);
        assert!(parse_auth_data_assertion(&assertion, &policy).is_err());

        let mut assertion = build_auth_data_assertion(&policy, 1);
        assertion[32] |= RESERVED_AUTHENTICATOR_FLAGS & 0x02;
        assert!(parse_auth_data_assertion(&assertion, &policy).is_err());

        let mut assertion = build_auth_data_assertion(&policy, 1);
        assertion[32] |= FLAG_BACKUP_STATE;
        assert!(parse_auth_data_assertion(&assertion, &policy).is_err());

        let signing_key = SigningKey::random(&mut OsRng);
        let cose_key = build_cose_key_es256(&signing_key);
        let mut registration = build_auth_data_registration(&policy, b"credential", &cose_key, 1);
        registration.push(0);
        assert!(parse_auth_data_registration(&registration, &policy).is_err());

        let mut registration = build_auth_data_registration(&policy, b"credential", &cose_key, 1);
        registration[32] |= FLAG_EXTENSION_DATA;
        assert!(parse_auth_data_registration(&registration, &policy).is_err());
    }
    #[tokio::test]
    async fn operator_auth_registration_login_and_rollover_es256() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let headers = headers_with_operator_token("bootstrap");
        let ctx = auth
            .authorize_bootstrap(&headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect("bootstrap allowed");
        let options = auth.webauthn_registration_options(&ctx).expect("options");
        let challenge = extract_challenge(&options);
        let signing_key = SigningKey::random(&mut OsRng);
        let credential_id = random_bytes(16).expect("credential id");
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
        let credential_id = random_bytes(16).expect("credential id");
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
    #[test]
    fn authentication_counter_changes_only_after_persistence_succeeds() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let mut auth = build_operator_auth(config, tempdir.path());
        let signing_key = SigningKey::random(&mut OsRng);
        let credential_id = random_bytes(16).expect("credential id");
        auth.credentials_write()
            .expect("credential lock")
            .push(StoredCredential {
                id: credential_id.clone(),
                public_key: signing_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes()
                    .to_vec(),
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 1,
                created_at_ms: 0,
            });
        let ctx = AuthContext {
            key: "persistence-failure".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };
        let options = auth
            .webauthn_authentication_options(&ctx)
            .expect("login options");
        let challenge = extract_challenge(&options);
        let policy = auth.webauthn_policy().expect("policy");
        let authenticator_data = build_auth_data_assertion(policy, 2);
        let client_data_json = build_client_data(&challenge, "https://example.com", "webauthn.get");
        let client_hash = Sha256::digest(&client_data_json);
        let mut signed_bytes =
            Vec::with_capacity(authenticator_data.len() + client_hash.as_slice().len());
        signed_bytes.extend_from_slice(&authenticator_data);
        signed_bytes.extend_from_slice(&client_hash);
        let signature: p256::ecdsa::Signature = signing_key.sign(&signed_bytes);
        let payload = build_assertion_payload(
            &credential_id,
            &client_data_json,
            &authenticator_data,
            signature.to_der().as_bytes(),
        );
        let blocked_parent = tempdir.path().join("not-a-directory");
        fs::write(&blocked_parent, b"block directory creation").expect("blocker file");
        auth.credentials_path = blocked_parent.join(CREDENTIALS_FILENAME);

        let err = match auth.webauthn_finish_authentication(&ctx, &payload) {
            Ok(_) => panic!("credential persistence must fail"),
            Err(err) => err,
        };
        assert_eq!(err.code, "operator_webauthn_persist_failed");
        assert_eq!(
            auth.credentials_read().expect("credential lock")[0].sign_count,
            1,
            "failed persistence must not advance the in-memory signature counter"
        );
    }
    #[test]
    fn authentication_counter_cannot_fall_back_to_zero() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let signing_key = SigningKey::random(&mut OsRng);
        let credential_id = random_bytes(16).expect("credential id");
        let credential = StoredCredential {
            id: credential_id.clone(),
            public_key: signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes()
                .to_vec(),
            alg: OperatorWebAuthnAlgorithm::Es256,
            sign_count: 1,
            created_at_ms: 0,
        };
        persist_credentials(&auth.credentials_path, std::slice::from_ref(&credential))
            .expect("persist initial counter");
        *auth.credentials_write().expect("credential lock") = vec![credential];
        let persisted_before = fs::read(&auth.credentials_path).expect("persisted credential");

        let ctx = AuthContext {
            key: "counter-rollback".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };
        let options = auth
            .webauthn_authentication_options(&ctx)
            .expect("login options");
        let challenge = extract_challenge(&options);
        let policy = auth.webauthn_policy().expect("policy");
        let authenticator_data = build_auth_data_assertion(policy, 0);
        let client_data_json = build_client_data(&challenge, "https://example.com", "webauthn.get");
        let client_hash = Sha256::digest(&client_data_json);
        let mut signed_bytes =
            Vec::with_capacity(authenticator_data.len() + client_hash.as_slice().len());
        signed_bytes.extend_from_slice(&authenticator_data);
        signed_bytes.extend_from_slice(&client_hash);
        let signature: p256::ecdsa::Signature = signing_key.sign(&signed_bytes);
        let payload = build_assertion_payload(
            &credential_id,
            &client_data_json,
            &authenticator_data,
            signature.to_der().as_bytes(),
        );

        let error = match auth.webauthn_finish_authentication(&ctx, &payload) {
            Ok(_) => panic!("a used counter cannot revert to zero"),
            Err(error) => error,
        };
        assert_eq!(error.code, "operator_webauthn_payload_invalid");
        assert_eq!(
            auth.credentials_read().expect("credential lock")[0].sign_count,
            1
        );
        assert_eq!(
            fs::read(&auth.credentials_path).expect("persisted credential"),
            persisted_before
        );
    }
    #[test]
    fn persisted_credentials_are_validated_strictly_at_startup() {
        let signing_key = SigningKey::random(&mut OsRng);
        let public_key = encode_b64url(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let valid_entry = |id: &str, sign_count: u64| {
            format!(
                r#"{{"id_b64":"{id}","public_key_b64":"{public_key}","alg":"es256","sign_count":{sign_count},"created_at_ms":1}}"#
            )
        };
        let duplicate = valid_entry(&encode_b64url(b"same-id"), 0);
        let fixtures = [
            (
                "duplicate id",
                format!(r#"{{"version":1,"credentials":[{duplicate},{duplicate}]}}"#),
            ),
            (
                "empty id",
                format!(r#"{{"version":1,"credentials":[{}]}}"#, valid_entry("", 0)),
            ),
            (
                "oversized counter",
                format!(
                    r#"{{"version":1,"credentials":[{}]}}"#,
                    valid_entry(&encode_b64url(b"counter"), u64::MAX)
                ),
            ),
            (
                "invalid key",
                format!(
                    r#"{{"version":1,"credentials":[{{"id_b64":"{}","public_key_b64":"{}","alg":"es256","sign_count":0,"created_at_ms":1}}]}}"#,
                    encode_b64url(b"invalid-key"),
                    encode_b64url(&[1; P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN])
                ),
            ),
            (
                "unknown field",
                r#"{"version":1,"credentials":[],"legacy":true}"#.to_owned(),
            ),
        ];

        for (label, body) in fixtures {
            let tempdir = tempfile::tempdir().expect("tempdir");
            write_credentials_fixture(tempdir.path(), &body);
            let config = base_operator_auth_config(
                Vec::new(),
                OperatorAuthLockout::default(),
                vec![OperatorWebAuthnAlgorithm::Es256],
            );
            let error = match OperatorAuth::new(
                config,
                tempdir.path().to_path_buf(),
                MaybeTelemetry::disabled(),
            ) {
                Ok(_) => panic!("{label} must fail startup"),
                Err(error) => error,
            };
            assert!(
                matches!(error, OperatorAuthInitError::CredentialLoad(_)),
                "{label}: {error}"
            );
        }
    }
    #[test]
    fn persisted_credential_file_is_bounded_before_json_allocation() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        let bytes = max_credentials_file_bytes(capacity).expect("file bound");
        let oversized = "x".repeat(usize::try_from(bytes + 1).expect("test bound fits usize"));
        write_credentials_fixture(tempdir.path(), &oversized);
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.credential_capacity = capacity;
        assert!(matches!(
            OperatorAuth::new(
                config,
                tempdir.path().to_path_buf(),
                MaybeTelemetry::disabled(),
            ),
            Err(OperatorAuthInitError::CredentialLoad(_))
        ));
    }
    #[test]
    fn credential_capacity_is_enforced_for_load_and_rollover() {
        let signing_key = SigningKey::random(&mut OsRng);
        let public_key = encode_b64url(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let entry = |id: &[u8]| {
            format!(
                r#"{{"id_b64":"{}","public_key_b64":"{public_key}","alg":"es256","sign_count":0,"created_at_ms":1}}"#,
                encode_b64url(id)
            )
        };
        let tempdir = tempfile::tempdir().expect("tempdir");
        write_credentials_fixture(
            tempdir.path(),
            &format!(
                r#"{{"version":1,"credentials":[{},{}]}}"#,
                entry(b"one"),
                entry(b"two")
            ),
        );
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.credential_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        let error = match OperatorAuth::new(
            config,
            tempdir.path().to_path_buf(),
            MaybeTelemetry::disabled(),
        ) {
            Ok(_) => panic!("oversized credential store must fail startup"),
            Err(error) => error,
        };
        assert!(matches!(error, OperatorAuthInitError::CredentialLoad(_)));

        let empty = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.credential_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        let auth = build_operator_auth(config, empty.path());
        let credential = |id| StoredCredential {
            id: vec![id],
            public_key: signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes()
                .to_vec(),
            alg: OperatorWebAuthnAlgorithm::Es256,
            sign_count: 0,
            created_at_ms: 1,
        };
        auth.insert_credential(credential(1), session_authority(&auth))
            .expect("first credential");
        let duplicate = auth
            .insert_credential(credential(1), session_authority(&auth))
            .expect_err("duplicate credential identifiers must not replace keys or counters");
        assert_eq!(
            duplicate.code,
            "operator_webauthn_credential_duplicate"
        );
        let error = auth
            .insert_credential(credential(2), session_authority(&auth))
            .expect_err("rollover beyond capacity must fail");
        assert_eq!(
            error.code,
            "operator_webauthn_credential_capacity_exhausted"
        );
    }
    #[test]
    fn persisted_credential_algorithm_must_match_active_policy() {
        let signing_key = SigningKey::random(&mut OsRng);
        let body = format!(
            r#"{{"version":1,"credentials":[{{"id_b64":"{}","public_key_b64":"{}","alg":"es256","sign_count":0,"created_at_ms":1}}]}}"#,
            encode_b64url(b"es256-id"),
            encode_b64url(
                signing_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes()
            )
        );
        let tempdir = tempfile::tempdir().expect("tempdir");
        write_credentials_fixture(tempdir.path(), &body);
        let config = base_operator_auth_config(
            Vec::new(),
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Ed25519],
        );
        let error = match OperatorAuth::new(
            config,
            tempdir.path().to_path_buf(),
            MaybeTelemetry::disabled(),
        ) {
            Ok(_) => panic!("credential outside active algorithm policy must fail startup"),
            Err(error) => error,
        };
        assert!(matches!(error, OperatorAuthInitError::CredentialLoad(_)));
    }
    #[tokio::test]
    async fn operator_token_only_bootstraps_credential_enrollment() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["operator-token".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let headers = headers_with_operator_token("operator-token");
        auth.authorize_bootstrap(&headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect("operator token bootstraps first credential");
        let error = auth
            .authorize_operator_endpoint(&headers, loopback_ip())
            .await
            .expect_err("operator token must never authorize an operator route");
        assert_eq!(error.code, "operator_session_missing");
    }
    #[test]
    fn bootstrap_enrollment_cannot_race_past_first_credential() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let public_key = SigningKey::random(&mut OsRng)
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        let credential = |id| StoredCredential {
            id: vec![id],
            public_key: public_key.clone(),
            alg: OperatorWebAuthnAlgorithm::Es256,
            sign_count: 0,
            created_at_ms: 0,
        };

        assert_eq!(
            auth.insert_credential(credential(1), EnrollmentAuthority::BootstrapToken)
                .expect("bootstrap may persist the first credential"),
            1
        );
        let error = auth
            .insert_credential(credential(2), EnrollmentAuthority::BootstrapToken)
            .expect_err("a concurrent bootstrap must not persist a rollover credential");
        assert_eq!(error.code, "operator_session_missing");
        assert_eq!(auth.credentials_read().expect("credential state").len(), 1);
        assert_eq!(
            auth.insert_credential(credential(2), session_authority(&auth))
                .expect("an authenticated session may persist a rollover credential"),
            2
        );
    }
    #[tokio::test]
    async fn persisted_first_credential_disables_bootstrap_after_restart() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config.clone(), tempdir.path());
        let public_key = SigningKey::random(&mut OsRng)
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        auth.insert_credential(
            StoredCredential {
                id: vec![1],
                public_key,
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 0,
                created_at_ms: 0,
            },
            EnrollmentAuthority::BootstrapToken,
        )
        .expect("first credential should persist");
        drop(auth);

        let restarted = build_operator_auth(config, tempdir.path());
        let headers = headers_with_operator_token("bootstrap");
        let error = restarted
            .authorize_bootstrap(&headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect_err("persisted enrollment must disable bootstrap after restart");
        assert_eq!(error.code, "operator_session_missing");
    }
    #[tokio::test]
    async fn api_token_never_bootstraps_operator_auth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["bootstrap".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let mut headers = base_headers();
        headers.insert("x-api-token", HeaderValue::from_static("bootstrap"));
        let error = auth
            .authorize_bootstrap(&headers, loopback_ip(), ACTION_REGISTER_OPTIONS)
            .await
            .expect_err("Torii API tokens must not bootstrap operator auth");
        assert_eq!(error.code, "operator_token_missing");
    }
    #[tokio::test]
    async fn operator_auth_enforces_mtls_and_lockout() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lockout = OperatorAuthLockout {
            failures: std::num::NonZeroU32::new(2),
            ..OperatorAuthLockout::default()
        };
        let mut config = base_operator_auth_config(
            vec!["valid".to_owned()],
            lockout,
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.require_mtls = true;
        let auth = build_operator_auth(config, tempdir.path());
        let err = auth
            .authorize_login(&base_headers(), None, ACTION_LOGIN_OPTIONS)
            .await
            .expect_err("missing mTLS");
        assert_eq!(err.code, "operator_mtls_required");
        assert_eq!(
            auth.lockout.entries.lock().len(),
            0,
            "callers outside the trusted mTLS boundary must not consume lockout state"
        );
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
    #[test]
    fn login_options_do_not_clear_assertion_failures() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lockout = OperatorAuthLockout {
            failures: std::num::NonZeroU32::new(2),
            ..OperatorAuthLockout::default()
        };
        let config =
            base_operator_auth_config(Vec::new(), lockout, vec![OperatorWebAuthnAlgorithm::Es256]);
        let auth = build_operator_auth(config, tempdir.path());
        auth.credentials_write()
            .expect("credential lock")
            .push(StoredCredential {
                id: vec![1],
                public_key: vec![2],
                alg: OperatorWebAuthnAlgorithm::Es256,
                sign_count: 0,
                created_at_ms: 0,
            });
        let ctx = AuthContext {
            key: "caller".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };

        auth.record_failure(&ctx, ACTION_LOGIN_VERIFY, "invalid_assertion")
            .expect("lockout state has capacity");
        auth.webauthn_authentication_options(&ctx)
            .expect("issue another login challenge");
        auth.record_failure(&ctx, ACTION_LOGIN_VERIFY, "invalid_assertion")
            .expect("lockout state has capacity");

        assert!(auth.lockout.is_locked(&ctx.key));
    }
    #[test]
    fn operational_errors_do_not_advance_lockout() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lockout = OperatorAuthLockout {
            failures: std::num::NonZeroU32::new(1),
            ..OperatorAuthLockout::default()
        };
        let config =
            base_operator_auth_config(Vec::new(), lockout, vec![OperatorWebAuthnAlgorithm::Es256]);
        let auth = build_operator_auth(config, tempdir.path());
        let ctx = AuthContext {
            key: "caller".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };

        let operational = OperatorAuthError::random_bytes_failure("entropy unavailable");
        let returned = auth.record_error(&ctx, ACTION_LOGIN_VERIFY, operational);
        assert_eq!(returned.code, "operator_auth_random_bytes_failed");
        assert!(!auth.lockout.is_locked(&ctx.key));

        let denial = OperatorAuthError::signature_invalid();
        let returned = auth.record_error(&ctx, ACTION_LOGIN_VERIFY, denial);
        assert_eq!(returned.code, "operator_webauthn_signature_invalid");
        assert!(auth.lockout.is_locked(&ctx.key));
    }
    #[tokio::test]
    async fn full_lockout_state_does_not_starve_valid_unseen_identity() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lockout = OperatorAuthLockout {
            failures: std::num::NonZeroU32::new(10),
            ..OperatorAuthLockout::default()
        };
        let mut config =
            base_operator_auth_config(Vec::new(), lockout, vec![OperatorWebAuthnAlgorithm::Es256]);
        config.ephemeral_state_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        let auth = build_operator_auth(config, tempdir.path());
        auth.lockout
            .record_failure("198.51.100.1")
            .expect("first identity fits");
        let ctx = AuthContext {
            key: "198.51.100.2".to_owned(),
            enrollment_authority: EnrollmentAuthority::None,
        };

        let returned = auth.record_error(
            &ctx,
            ACTION_LOGIN_VERIFY,
            OperatorAuthError::signature_invalid(),
        );
        assert_eq!(returned.code, "operator_webauthn_signature_invalid");
        assert_eq!(auth.lockout.entries.lock().len(), 1);
        assert!(!auth.lockout.is_locked(&ctx.key));

        let now = Instant::now();
        let session = test_session_token(3);
        auth.sessions
            .lock()
            .insert(
                session.clone(),
                SessionEntry {
                    credential_revocation_generation: auth
                        .credential_revocation_generation
                        .load(Ordering::Acquire),
                },
                now + Duration::from_secs(60),
                now,
            )
            .expect("session store has its independent capacity");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_OPERATOR_SESSION,
            HeaderValue::from_str(&session).expect("session header"),
        );
        auth.authorize_operator_endpoint(
            &headers,
            Some("198.51.100.2".parse().expect("unseen caller IP")),
        )
        .await
        .expect("full attacker-selected lockout state must not reject a valid unseen caller");
    }
    #[test]
    fn lockout_identity_state_is_bounded_and_preserves_tracked_locks() {
        let tracker = LockoutTracker::new(
            OperatorAuthLockout {
                failures: std::num::NonZeroU32::new(2),
                window: Duration::from_secs(60),
                duration: Duration::from_secs(60),
            },
            NonZeroUsize::new(2).expect("non-zero capacity"),
        );

        assert_eq!(tracker.record_failure("caller-a"), Ok(false));
        assert_eq!(tracker.record_failure("caller-b"), Ok(false));
        assert_eq!(tracker.entries.lock().len(), 2);
        assert!(!tracker.is_locked("caller-c"));
        assert_eq!(tracker.record_failure("caller-c"), Ok(false));
        assert_eq!(tracker.entries.lock().len(), 2);
        assert_eq!(tracker.record_failure("caller-a"), Ok(true));
        assert!(tracker.is_locked("caller-a"));

        tracker.clear("caller-a");
        assert!(!tracker.is_locked("caller-c"));
        assert_eq!(tracker.record_failure("caller-c"), Ok(false));
        assert_eq!(tracker.entries.lock().len(), 2);
    }
    #[test]
    fn ephemeral_store_reclaims_expired_entries_without_evicting_live_state() {
        let capacity = NonZeroUsize::new(2).expect("non-zero capacity");
        let mut store = BoundedExpiringStore::new(capacity);
        let now = Instant::now();
        let soon = now + Duration::from_secs(1);
        let later = now + Duration::from_secs(10);

        store
            .insert("soon".to_owned(), 1, soon, now)
            .expect("first entry");
        store
            .insert("later".to_owned(), 2, later, now)
            .expect("second entry");
        assert_eq!(
            store.insert("live-eviction".to_owned(), 3, later, now),
            Err(ExpiringStoreAtCapacity)
        );

        let after_soon = soon + Duration::from_nanos(1);
        store
            .insert("replacement".to_owned(), 3, later, after_soon)
            .expect("expired entry releases capacity");
        assert_eq!(store.get("soon", after_soon), None);
        assert_eq!(store.get("later", after_soon), Some(&2));
        assert_eq!(store.get("replacement", after_soon), Some(&3));
    }
    #[test]
    fn concurrent_ephemeral_admission_never_exceeds_capacity() {
        let capacity = NonZeroUsize::new(8).expect("non-zero capacity");
        let store = Mutex::new(BoundedExpiringStore::new(capacity));
        let now = Instant::now();
        let expires_at = now + Duration::from_secs(60);

        std::thread::scope(|scope| {
            for index in 0..64 {
                let store = &store;
                scope.spawn(move || {
                    let _ = store
                        .lock()
                        .insert(format!("caller-{index}"), index, expires_at, now);
                });
            }
        });

        assert_eq!(store.lock().len(), capacity.get());
    }
    #[tokio::test]
    async fn operator_auth_rejects_forwarded_mtls_from_untrusted_proxy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut config = base_operator_auth_config(
            vec!["valid".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        config.require_mtls = true;
        let auth = build_operator_auth(config, tempdir.path());
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
    #[tokio::test]
    async fn operator_auth_key_uses_remote_ip_when_internal_header_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["valid".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let remote_ip: IpAddr = "198.51.100.33".parse().expect("remote ip");
        let ctx = auth
            .authorize_login(&HeaderMap::new(), Some(remote_ip), ACTION_LOGIN_OPTIONS)
            .await
            .expect("login key derivation should succeed");
        assert_eq!(ctx.key, remote_ip.to_string());
    }
    #[tokio::test]
    async fn operator_auth_key_prefers_injected_header_over_transport_remote_ip() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let config = base_operator_auth_config(
            vec!["valid".to_owned()],
            OperatorAuthLockout::default(),
            vec![OperatorWebAuthnAlgorithm::Es256],
        );
        let auth = build_operator_auth(config, tempdir.path());
        let mut headers = HeaderMap::new();
        headers.insert(
            limits::REMOTE_ADDR_HEADER,
            HeaderValue::from_static("203.0.113.77"),
        );
        let ctx = auth
            .authorize_login(
                &headers,
                Some("198.51.100.33".parse().expect("transport remote ip")),
                ACTION_LOGIN_OPTIONS,
            )
            .await
            .expect("login key derivation should succeed");
        assert_eq!(ctx.key, "203.0.113.77");
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
    #[test]
    fn es256_cose_key_rejects_all_zero_public_key_material() {
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
            (
                CborValue::Integer((-2).into()),
                CborValue::Bytes(vec![0u8; 32]),
            ),
            (
                CborValue::Integer((-3).into()),
                CborValue::Bytes(vec![0u8; 32]),
            ),
        ];
        let cose_key = CborValue::Map(map);
        let err = match parse_cose_key(&cose_key, &[OperatorWebAuthnAlgorithm::Es256]) {
            Ok(_) => panic!("all-zero ES256 public key material must be rejected"),
            Err(err) => err,
        };
        assert_eq!(err.code, "operator_webauthn_payload_invalid");
        assert_eq!(err.metric_label, "invalid_payload");
    }
    #[test]
    fn es256_signature_verify_rejects_all_zero_signature_material() {
        let signing_key = SigningKey::random(&mut OsRng);
        let public_key = signing_key.verifying_key().to_encoded_point(false);
        let signature = [0u8; 64];
        let err = verify_signature(
            OperatorWebAuthnAlgorithm::Es256,
            public_key.as_bytes(),
            b"operator-auth-test",
            &signature,
        )
        .expect_err("all-zero ES256 signature material must be rejected");
        assert_eq!(err.code, "operator_webauthn_signature_invalid");
        assert_eq!(err.metric_label, "signature_invalid");
    }
    #[test]
    fn es256_signature_verify_rejects_all_zero_public_key_material() {
        let signing_key = SigningKey::random(&mut OsRng);
        let message = b"operator-auth-test";
        let signature: p256::ecdsa::Signature = signing_key.sign(message);
        let mut public_key = Vec::with_capacity(P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN);
        public_key.push(0x04);
        public_key.extend_from_slice(&[0u8; 64]);
        let err = verify_signature(
            OperatorWebAuthnAlgorithm::Es256,
            &public_key,
            message,
            signature.to_der().as_bytes(),
        )
        .expect_err("all-zero ES256 public key material must be rejected");
        assert_eq!(err.code, "operator_webauthn_payload_invalid");
        assert_eq!(err.metric_label, "invalid_payload");
    }
    #[test]
    fn es256_signature_verify_rejects_high_s_signature_material() {
        const P256_ORDER: [u8; 32] = [
            0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2,
            0xfc, 0x63, 0x25, 0x51,
        ];
        let signing_key = SigningKey::random(&mut OsRng);
        let public_key = signing_key.verifying_key().to_encoded_point(false);
        let message = b"operator-auth-test-high-s";
        let low_s = {
            let signature: P256Signature = signing_key.sign(message);
            signature.normalize_s().unwrap_or(signature)
        };
        let low_s_bytes = low_s.to_bytes();
        let mut high_s_bytes = [0_u8; 64];
        high_s_bytes[..32].copy_from_slice(&low_s_bytes[..32]);
        let mut borrow = 0_u16;
        for i in (0..32).rev() {
            let minuend = i16::from(P256_ORDER[i]) - i16::from(borrow as u8);
            let subtrahend = i16::from(low_s_bytes[32 + i]);
            if minuend >= subtrahend {
                high_s_bytes[32 + i] = (minuend - subtrahend) as u8;
                borrow = 0;
            } else {
                high_s_bytes[32 + i] = (minuend + 256 - subtrahend) as u8;
                borrow = 1;
            }
        }
        assert_eq!(borrow, 0);
        let high_s = P256Signature::from_slice(&high_s_bytes).expect("high-S signature");
        assert!(high_s.normalize_s().is_some());
        let err = verify_signature(
            OperatorWebAuthnAlgorithm::Es256,
            public_key.as_bytes(),
            message,
            high_s.to_der().as_bytes(),
        )
        .expect_err("high-S ES256 signature material must be rejected");
        assert_eq!(err.code, "operator_webauthn_signature_invalid");
        assert_eq!(err.metric_label, "signature_invalid");
    }
    #[test]
    fn ed25519_signature_verify_rejects_all_zero_signature_material() {
        let mut rng = OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        let public_key = signing_key.verifying_key().to_bytes();
        let signature = [0u8; 64];
        let err = verify_signature(
            OperatorWebAuthnAlgorithm::Ed25519,
            &public_key,
            b"operator-auth-test",
            &signature,
        )
        .expect_err("all-zero signature material must be rejected");
        assert_eq!(err.code, "operator_webauthn_signature_invalid");
        assert_eq!(err.metric_label, "signature_invalid");
    }
    #[test]
    fn ed25519_signature_verify_rejects_all_zero_public_key_material() {
        let mut rng = OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        let message = b"operator-auth-test";
        let signature = signing_key.sign(message).to_bytes();
        let err = verify_signature(
            OperatorWebAuthnAlgorithm::Ed25519,
            &[0u8; 32],
            message,
            &signature,
        )
        .expect_err("all-zero Ed25519 public key material must be rejected");
        assert_eq!(err.code, "operator_webauthn_signature_invalid");
        assert_eq!(err.metric_label, "signature_invalid");
    }
    #[test]
    fn ed25519_signature_verify_rejects_weak_or_noncanonical_public_key_material() {
        let mut rng = OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        let message = b"operator-auth-test";
        let signature = signing_key.sign(message).to_bytes();
        for (label, public_key_bytes) in [
            ("small-order", ED25519_SMALL_ORDER_POINT),
            ("noncanonical", ED25519_NONCANONICAL_IDENTITY),
        ] {
            let err = verify_signature(
                OperatorWebAuthnAlgorithm::Ed25519,
                &public_key_bytes,
                message,
                &signature,
            )
            .expect_err("malformed Ed25519 public key material must be rejected");
            assert_eq!(
                err.code, "operator_webauthn_signature_invalid",
                "{label} public key should fail"
            );
            assert_eq!(
                err.metric_label, "signature_invalid",
                "{label} public key should map to signature_invalid"
            );
        }
    }
    #[test]
    fn ed25519_signature_verify_rejects_malformed_signature_r() {
        let mut rng = OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        let public_key = signing_key.verifying_key().to_bytes();
        let message = b"operator-auth-test";
        for (label, replacement_r) in [
            ("small-order", ED25519_SMALL_ORDER_POINT),
            ("noncanonical", ED25519_NONCANONICAL_IDENTITY),
        ] {
            let mut signature = signing_key.sign(message).to_bytes();
            signature[..32].copy_from_slice(&replacement_r);
            let err = verify_signature(
                OperatorWebAuthnAlgorithm::Ed25519,
                &public_key,
                message,
                &signature,
            )
            .expect_err("malformed Ed25519 signature R must be rejected");
            assert_eq!(
                err.code, "operator_webauthn_signature_invalid",
                "{label} signature R should fail"
            );
            assert_eq!(
                err.metric_label, "signature_invalid",
                "{label} signature R should map to signature_invalid"
            );
        }
    }
    #[tokio::test]
    async fn operator_auth_handlers_reject_when_disabled() {
        let app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let err = handle_operator_register_options(
            State(app.clone()),
            loopback_connect_info(),
            headers.clone(),
            Body::empty(),
        )
        .await
        .err()
        .expect("register options disabled");
        assert_eq!(err.code, "operator_auth_disabled");
        let err = handle_operator_login_options(
            State(app.clone()),
            loopback_connect_info(),
            headers.clone(),
            Body::empty(),
        )
        .await
        .err()
        .expect("login options disabled");
        assert_eq!(err.code, "operator_auth_disabled");
        let err = handle_operator_credentials(State(app.clone()))
            .await
            .err()
            .expect("credential inventory disabled");
        assert_eq!(err.code, "operator_webauthn_disabled");
        let err = handle_operator_credential_delete(
            State(app),
            AxumPath(encode_b64url(b"credential")),
            headers,
        )
        .await
        .err()
        .expect("credential deletion disabled");
        assert_eq!(err.code, "operator_webauthn_disabled");
    }
    #[test]
    fn operator_auth_error_response_sets_status() {
        let response = OperatorAuthError::missing_token().into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
