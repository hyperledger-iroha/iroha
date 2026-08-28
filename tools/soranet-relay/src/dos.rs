//! DoS and abuse mitigation utilities for the relay handshake path.
use crate::{
    canonical_remote_ip, capability,
    config::{
        ConfigError, EmergencyThrottleConfig, PowConfig, QuotaConfig,
        RELAY_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1, RelayMode, SlowlorisConfig, TokenPolicySource,
        read_bounded_direct_regular_file,
    },
    metrics::Metrics,
};
use blake3::Hasher;
use hex;
use iroha_crypto::soranet::{
    puzzle,
    token::{AdmissionToken, AdmissionTokenVerifier, VerifyError as TokenVerifyError},
};
use norito::{DecodeLimits, derive::JsonDeserialize, json};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};
use thiserror::Error;
use tracing::warn;
// First-release bounds for the operator-reloadable emergency descriptor set.
// The entry ceiling is shared with the inline relay-config list. A fully
// escaped 64-byte descriptor string can occupy 386 source bytes, so 4 MiB
// admits all 8,192 descriptors while keeping both lexical and owned decode
// allocation finite before the retained HashSet is built.
const EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1: usize = RELAY_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1;
const EMERGENCY_THROTTLE_DESCRIPTOR_HEX_BYTES: usize = 64;
const EMERGENCY_THROTTLE_MAX_ENCODED_STRING_BYTES_V1: usize =
    EMERGENCY_THROTTLE_DESCRIPTOR_HEX_BYTES * 6 + 2;
const EMERGENCY_THROTTLE_MAX_TOTAL_STRING_BYTES_V1: usize =
    EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1 * EMERGENCY_THROTTLE_DESCRIPTOR_HEX_BYTES + 128;
const EMERGENCY_THROTTLE_DOCUMENT_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
const EMERGENCY_THROTTLE_MAX_TOTAL_ELEMENTS_V1: usize = EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1 + 2;
const EMERGENCY_THROTTLE_MAX_DEPTH_V1: usize = 4;
const EMERGENCY_THROTTLE_MAX_ALLOCATED_BYTES_V1: usize = 2 * 1024 * 1024;
const EMERGENCY_THROTTLE_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1,
    EMERGENCY_THROTTLE_DESCRIPTOR_HEX_BYTES,
    EMERGENCY_THROTTLE_MAX_TOTAL_ELEMENTS_V1,
    EMERGENCY_THROTTLE_MAX_ALLOCATED_BYTES_V1,
    EMERGENCY_THROTTLE_MAX_DEPTH_V1,
);
const fn emergency_throttle_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        EMERGENCY_THROTTLE_DOCUMENT_MAX_BYTES_V1,
        EMERGENCY_THROTTLE_MAX_TOTAL_ELEMENTS_V1 + 1,
        EMERGENCY_THROTTLE_MAX_ENCODED_STRING_BYTES_V1,
        EMERGENCY_THROTTLE_DESCRIPTOR_HEX_BYTES,
        EMERGENCY_THROTTLE_MAX_TOTAL_STRING_BYTES_V1,
        EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1,
        EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1,
        2,
        EMERGENCY_THROTTLE_MAX_TOTAL_ELEMENTS_V1,
        EMERGENCY_THROTTLE_MAX_DEPTH_V1,
    )
}
/// Aggregated controls applied to inbound handshakes.
pub struct DoSControls {
    remote_limiter: Mutex<RateLimiter<IpAddr>>,
    slowloris: SlowlorisDetector,
    puzzle: PuzzlePolicy,
    signed_ticket_public_key: Option<Arc<Vec<u8>>>,
    token: Option<TokenPolicy>,
    metrics: Arc<Metrics>,
    remote_limits: QuotaLimits,
    emergency: Option<EmergencyThrottle>,
}
impl DoSControls {
    /// Create a new controller from the relay PoW configuration.
    pub fn new(
        config: &PowConfig,
        token: Option<TokenPolicySource>,
        metrics: Arc<Metrics>,
        mode: RelayMode,
    ) -> Result<Self, ConfigError> {
        let puzzle = config.puzzle_parameters()?;
        let quotas_cfg = config.quotas_for_mode(mode);
        let mut slowloris_cfg = config.slowloris.clone();
        slowloris_cfg.apply_defaults();
        let remote_params = RateLimitParams::from_remote(&quotas_cfg);
        let remote_limits = QuotaLimits::from(&remote_params);
        let remote_limiter = Mutex::new(RateLimiter::new(remote_params));
        metrics.set_pow_difficulty(puzzle.difficulty());
        metrics.set_active_remote_cooldowns(0);
        let puzzle_policy = PuzzlePolicy::new(puzzle);
        let signed_ticket_public_key = config.signed_ticket_public_key()?.map(Arc::new);
        let emergency = config
            .emergency_throttle()
            .map(|cfg| EmergencyThrottle::new(cfg.clone()))
            .transpose()?;
        Ok(Self {
            remote_limiter,
            slowloris: SlowlorisDetector::new(slowloris_cfg, remote_limits.max_entries()),
            puzzle: puzzle_policy,
            signed_ticket_public_key,
            token: token.map(TokenPolicy::from_source),
            metrics,
            remote_limits,
            emergency,
        })
    }
    /// Returns the static configured first-release puzzle parameters.
    pub fn current_puzzle_parameters(&self) -> puzzle::Parameters {
        self.puzzle.parameters()
    }
    /// Return the authenticated signed-puzzle verifier key, when configured.
    pub fn signed_ticket_public_key(&self) -> Option<Arc<Vec<u8>>> {
        self.signed_ticket_public_key.as_ref().map(Arc::clone)
    }
    /// Returns the configured admission token policy, if any.
    pub(crate) fn has_token_policy(&self) -> bool {
        self.token.is_some()
    }
    /// Verify an admission token against the configured policy.
    pub fn verify_token(
        &self,
        token: &AdmissionToken,
        relay_id: &[u8; 32],
        transcript_hash: &[u8; 32],
        now: SystemTime,
    ) -> Result<(), TokenPolicyError> {
        let policy = self.token.as_ref().ok_or(TokenPolicyError::Unavailable)?;
        let issuer_hex = policy.issuer_hex().to_owned();
        let relay_hex = hex::encode(relay_id);
        let result = policy.verify(token, relay_id, transcript_hash, now);
        match &result {
            Ok(_) => self
                .metrics
                .record_token_outcome(&issuer_hex, &relay_hex, "accepted"),
            Err(err) => {
                self.metrics
                    .record_token_outcome(&issuer_hex, &relay_hex, token_outcome_label(err))
            }
        }
        result
    }
    /// Returns the active remote quota limits.
    pub fn remote_quota_limits(&self) -> QuotaLimits {
        self.remote_limits
    }
    /// Registers a pending handshake attempt, enforcing quota limits.
    pub fn begin(
        &self,
        remote: SocketAddr,
        descriptor_commit: Option<&[u8]>,
    ) -> Result<AttemptContext, Throttle> {
        self.begin_at(remote, descriptor_commit, Instant::now())
    }
    /// Same as [`Self::begin`] but allows tests to supply a deterministic timestamp.
    pub fn begin_at(
        &self,
        remote: SocketAddr,
        descriptor_commit: Option<&[u8]>,
        now: Instant,
    ) -> Result<AttemptContext, Throttle> {
        let ip = canonical_remote_ip(remote);
        if let Some(cooldown) = self.slowloris.unavailable_cooldown() {
            return Err(Throttle {
                cooldown,
                reason: ThrottleReason::RemoteQuota,
            });
        }
        if let Some(emergency) = &self.emergency {
            if let Some(duration) = emergency.unavailable_cooldown() {
                self.metrics.record_emergency_throttle();
                return Err(Throttle {
                    cooldown: duration,
                    reason: ThrottleReason::Emergency,
                });
            }
            if let Some(commit_bytes) = descriptor_commit
                && let Some(duration) = emergency.should_throttle(commit_bytes)
            {
                self.metrics.record_emergency_throttle();
                return Err(Throttle {
                    cooldown: duration,
                    reason: ThrottleReason::Emergency,
                });
            }
        }
        if let Err(cooldown) = self.check_remote_limit(ip, now) {
            return Err(Throttle {
                cooldown,
                reason: ThrottleReason::RemoteQuota,
            });
        }
        Ok(AttemptContext {
            remote: ip,
            started_at: now,
        })
    }
    /// Records a successful handshake outcome.
    pub fn record_success(&self, attempt: &AttemptContext, elapsed: Duration) {
        self.record_success_at(attempt, elapsed, Instant::now());
    }
    /// Same as [`Self::record_success`] but accepts an explicit timestamp.
    pub fn record_success_at(&self, attempt: &AttemptContext, elapsed: Duration, now: Instant) {
        self.observe_slowloris_success_at(attempt.remote, elapsed, now);
    }
    /// Records a PoW verification failure.
    pub fn record_pow_failure(&self, attempt: &AttemptContext, elapsed: Duration) {
        self.record_pow_failure_at(attempt, elapsed, Instant::now());
    }
    /// Same as [`Self::record_pow_failure`] but accepts an explicit timestamp.
    pub fn record_pow_failure_at(&self, attempt: &AttemptContext, elapsed: Duration, now: Instant) {
        self.observe_slowloris_success_at(attempt.remote, elapsed, now);
    }
    /// Records a timeout while reading the handshake.
    pub fn record_timeout(&self, attempt: &AttemptContext, _elapsed: Duration) {
        self.record_timeout_at(attempt, _elapsed, Instant::now());
    }
    /// Same as [`Self::record_timeout`] but accepts an explicit timestamp.
    pub fn record_timeout_at(&self, attempt: &AttemptContext, _elapsed: Duration, now: Instant) {
        if let Some(penalty) = self
            .slowloris
            .observe(attempt.remote, SlowlorisEvent::Timeout, now)
        {
            self.impose_remote_cooldown(attempt.remote, now, penalty);
        }
    }
    /// Records a non-PoW failure outcome.
    pub fn record_failure(&self, attempt: &AttemptContext, elapsed: Duration) {
        self.record_failure_at(attempt, elapsed, Instant::now());
    }
    /// Same as [`Self::record_failure`] but accepts an explicit timestamp.
    pub fn record_failure_at(&self, attempt: &AttemptContext, elapsed: Duration, now: Instant) {
        self.observe_slowloris_success_at(attempt.remote, elapsed, now);
    }
    fn observe_slowloris_success_at(&self, ip: IpAddr, elapsed: Duration, now: Instant) {
        if let Some(penalty) = self
            .slowloris
            .observe(ip, SlowlorisEvent::Success(elapsed), now)
        {
            self.impose_remote_cooldown(ip, now, penalty);
        }
    }
    fn impose_remote_cooldown(&self, ip: IpAddr, now: Instant, cooldown: Duration) {
        if cooldown.is_zero() {
            return;
        }
        match self.remote_limiter.lock() {
            Ok(mut limiter) => {
                limiter.impose_cooldown(ip, now, cooldown);
                let count = limiter.cooldown_count();
                self.metrics.set_active_remote_cooldowns(count);
            }
            Err(error) => {
                // Future admission checks reject while this mutex is poisoned;
                // make the failed state update observable instead of silently
                // pretending that the cooldown was installed.
                warn!(%error, "remote quota mutex poisoned; cooldown was not recorded");
            }
        }
    }
    fn check_remote_limit(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        let mut limiter = self.remote_limiter.lock().map_err(|error| {
            warn!(%error, "remote quota mutex poisoned; rejecting admission");
            self.remote_limits.cooldown()
        })?;
        let result = limiter.check(ip, now);
        let count = limiter.cooldown_count();
        self.metrics.set_active_remote_cooldowns(count);
        result
    }
}
/// Context associated with an inbound handshake attempt.
#[derive(Debug, Clone)]
pub struct AttemptContext {
    remote: IpAddr,
    started_at: Instant,
}
impl AttemptContext {
    /// Returns the elapsed handshake duration.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}
/// Throttling decision describing the applied cooldown.
#[derive(Debug, Clone, Copy)]
pub struct Throttle {
    pub cooldown: Duration,
    pub reason: ThrottleReason,
}
/// Reasons why a handshake was throttled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleReason {
    RemoteQuota,
    Emergency,
}
impl fmt::Display for ThrottleReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThrottleReason::RemoteQuota => f.write_str("remote quota exceeded"),
            ThrottleReason::Emergency => f.write_str("consensus emergency throttle"),
        }
    }
}
fn descriptor_key(bytes: &[u8]) -> Option<[u8; 16]> {
    if bytes.is_empty() {
        return None;
    }
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    Some(out)
}
/// Static first-release puzzle policy.
struct PuzzlePolicy {
    base: puzzle::Parameters,
}
impl PuzzlePolicy {
    fn new(base: puzzle::Parameters) -> Self {
        Self { base }
    }
    fn parameters(&self) -> puzzle::Parameters {
        self.base
    }
}
/// Admission-token verifier plus local revocation set.
struct TokenPolicy {
    verifier: AdmissionTokenVerifier,
    revoked: HashSet<[u8; 32]>,
    issuer_hex: String,
}
impl TokenPolicy {
    fn from_source(source: TokenPolicySource) -> Self {
        let revoked = source.revocations.into_iter().collect::<HashSet<_>>();
        let issuer_hex = hex::encode(source.verifier.issuer_fingerprint());
        Self {
            verifier: source.verifier,
            revoked,
            issuer_hex,
        }
    }
    fn is_revoked(&self, token_id: &[u8; 32]) -> bool {
        self.revoked.contains(token_id)
    }
    fn issuer_hex(&self) -> &str {
        &self.issuer_hex
    }
    fn verify(
        &self,
        token: &AdmissionToken,
        relay_id: &[u8; 32],
        transcript_hash: &[u8; 32],
        now: SystemTime,
    ) -> Result<(), TokenPolicyError> {
        let token_id = token.token_id();
        if self.is_revoked(&token_id) {
            return Err(TokenPolicyError::Revoked(hex::encode(token_id)));
        }
        self.verifier
            .verify(token, relay_id, transcript_hash, now)
            .map_err(TokenPolicyError::Verify)
    }
}
/// Errors surfaced by the token policy checker.
#[derive(Debug, Error)]
pub enum TokenPolicyError {
    #[error("token verification failed: {0}")]
    Verify(#[from] TokenVerifyError),
    #[error("token revoked ({0})")]
    Revoked(String),
    #[error("token policy not available")]
    Unavailable,
}
fn token_outcome_label(error: &TokenPolicyError) -> &'static str {
    match error {
        TokenPolicyError::Verify(inner) => match inner {
            TokenVerifyError::IssuerMismatch(_) => "issuer_mismatch",
            TokenVerifyError::RelayMismatch => "relay_mismatch",
            TokenVerifyError::TranscriptMismatch => "transcript_mismatch",
            TokenVerifyError::NotYetValid { .. } => "not_yet_valid",
            TokenVerifyError::Expired { .. } => "expired",
            TokenVerifyError::TtlExceeded { .. } => "ttl_exceeded",
            TokenVerifyError::InvalidTemporalBounds => "invalid_temporal_bounds",
            TokenVerifyError::Clock(_) => "store_error",
            TokenVerifyError::Signature(_) => "signature_invalid",
            TokenVerifyError::InertSignature => "signature_invalid",
            TokenVerifyError::Store(_) => "store_error",
            TokenVerifyError::Replay(_) => "replay",
        },
        TokenPolicyError::Revoked(_) => "revoked",
        TokenPolicyError::Unavailable => "store_error",
    }
}
struct EmergencyThrottle {
    cooldown_millis: AtomicU64,
    refresh: Duration,
    file_path: Option<PathBuf>,
    static_descriptors: HashSet<[u8; 16]>,
    state: RwLock<EmergencyThrottleState>,
    unavailable: AtomicBool,
}
struct EmergencyThrottleState {
    last_loaded: Instant,
    dynamic_descriptors: HashSet<[u8; 16]>,
}
impl EmergencyThrottle {
    fn new(config: EmergencyThrottleConfig) -> Result<Self, ConfigError> {
        let static_descriptors = Self::decode_descriptor_list(config.descriptor_commit_hex.iter())
            .map_err(ConfigError::EmergencyThrottle)?;
        let refresh = Duration::from_secs(config.refresh_secs);
        let mut cooldown_secs = config.cooldown_secs.max(1);
        let (dynamic_descriptors, override_secs) = match &config.file_path {
            Some(path) => {
                let document = Self::load_document(path).map_err(ConfigError::EmergencyThrottle)?;
                (document.descriptors, document.cooldown_override_secs)
            }
            None => (HashSet::new(), None),
        };
        if let Some(secs) = override_secs
            && secs > 0
        {
            cooldown_secs = secs;
        }
        let cooldown_millis = cooldown_secs.saturating_mul(1_000).max(1);
        let state = EmergencyThrottleState {
            last_loaded: Instant::now(),
            dynamic_descriptors,
        };
        Ok(Self {
            cooldown_millis: AtomicU64::new(cooldown_millis),
            refresh,
            file_path: config.file_path.clone(),
            static_descriptors,
            state: RwLock::new(state),
            unavailable: AtomicBool::new(false),
        })
    }
    fn mark_unavailable(&self) {
        if !self.unavailable.swap(true, Ordering::AcqRel) {
            warn!("emergency throttle state lock poisoned; rejecting future admission");
        }
    }
    fn unavailable_cooldown(&self) -> Option<Duration> {
        if self.state.is_poisoned() {
            self.mark_unavailable();
        }
        self.unavailable
            .load(Ordering::Acquire)
            .then(|| self.cooldown_duration())
    }
    fn should_throttle(&self, descriptor_commit: &[u8]) -> Option<Duration> {
        let key = descriptor_key(descriptor_commit)?;
        if self.static_descriptors.contains(&key) {
            return Some(self.cooldown_duration());
        }
        if self.file_path.is_none() {
            let state = match self.state.read() {
                Ok(state) => state,
                Err(_) => {
                    self.mark_unavailable();
                    return Some(self.cooldown_duration());
                }
            };
            if state.dynamic_descriptors.contains(&key) {
                return Some(self.cooldown_duration());
            }
            return None;
        }
        let needs_reload = {
            let state = match self.state.read() {
                Ok(state) => state,
                Err(_) => {
                    self.mark_unavailable();
                    return Some(self.cooldown_duration());
                }
            };
            if state.dynamic_descriptors.contains(&key) {
                return Some(self.cooldown_duration());
            }
            state.last_loaded.elapsed() >= self.refresh
        };
        if !needs_reload {
            return None;
        }
        let mut guard = match self.state.write() {
            Ok(guard) => guard,
            Err(_) => {
                self.mark_unavailable();
                return Some(self.cooldown_duration());
            }
        };
        if guard.last_loaded.elapsed() >= self.refresh
            && let Err(err) = self.reload_locked(&mut guard)
        {
            warn!(%err, "failed to reload emergency throttle document");
        }
        if guard.dynamic_descriptors.contains(&key) || self.static_descriptors.contains(&key) {
            return Some(self.cooldown_duration());
        }
        None
    }
    fn cooldown_duration(&self) -> Duration {
        let millis = self.cooldown_millis.load(Ordering::Relaxed).max(1);
        Duration::from_millis(millis)
    }
    fn reload_locked(&self, state: &mut EmergencyThrottleState) -> Result<(), String> {
        state.last_loaded = Instant::now();
        let Some(path) = &self.file_path else {
            state.dynamic_descriptors.clear();
            return Ok(());
        };
        match Self::load_document(path) {
            Ok(doc) => {
                state.dynamic_descriptors = doc.descriptors;
                if let Some(secs) = doc.cooldown_override_secs
                    && secs > 0
                {
                    let millis = secs.saturating_mul(1_000).max(1);
                    self.cooldown_millis.store(millis, Ordering::Relaxed);
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
    fn decode_descriptor_list<I, S>(hex_values: I) -> Result<HashSet<[u8; 16]>, String>
    where
        I: ExactSizeIterator<Item = S>,
        S: AsRef<str>,
    {
        let descriptor_count = hex_values.len();
        if descriptor_count > EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1 {
            return Err(format!(
                "emergency throttle contains {descriptor_count} descriptors; first-release limit is {EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1}"
            ));
        }
        let mut set = HashSet::new();
        set.try_reserve(descriptor_count)
            .map_err(|_| "failed to reserve the bounded emergency descriptor set".to_owned())?;
        for value in hex_values {
            if let Some(key) = Self::decode_descriptor(value.as_ref()) {
                set.insert(key);
            }
        }
        Ok(set)
    }
    fn decode_descriptor(hex_value: &str) -> Option<[u8; 16]> {
        match capability::parse_descriptor_commit_hex(hex_value) {
            Ok(bytes) => match descriptor_key(&bytes) {
                Some(key) => Some(key),
                None => {
                    warn!(
                        "failed to derive descriptor key for `{hex_value}` in emergency throttle config"
                    );
                    None
                }
            },
            Err(_) => {
                warn!("invalid descriptor commit `{hex_value}` in emergency throttle config");
                None
            }
        }
    }
    fn load_document(path: &Path) -> Result<LoadedEmergencyThrottle, String> {
        let bytes = read_bounded_direct_regular_file(
            path,
            EMERGENCY_THROTTLE_DOCUMENT_MAX_BYTES_V1,
            "emergency throttle JSON",
        )
        .map_err(|err| format!("failed to read emergency throttle file {path:?}: {err}"))?;
        json::preflight_slice(&bytes, emergency_throttle_preflight_limits_v1()).map_err(
            |error| format!("emergency throttle file {path:?} failed JSON admission: {error}"),
        )?;
        let document: EmergencyThrottleDocument =
            norito::with_decode_limits_scope(EMERGENCY_THROTTLE_DECODE_LIMITS_V1, || {
                json::from_slice(&bytes)
            })
            .map_err(|_| format!("failed to parse bounded emergency throttle file {path:?}"))?;
        let descriptors = Self::decode_descriptor_list(document.descriptor_commit_hex.into_iter())?;
        Ok(LoadedEmergencyThrottle {
            descriptors,
            cooldown_override_secs: document.cooldown_secs,
        })
    }
}
#[derive(Debug)]
struct LoadedEmergencyThrottle {
    descriptors: HashSet<[u8; 16]>,
    cooldown_override_secs: Option<u64>,
}
#[derive(Debug, JsonDeserialize)]
struct EmergencyThrottleDocument {
    #[norito(default)]
    descriptor_commit_hex: Vec<String>,
    #[norito(default)]
    cooldown_secs: Option<u64>,
}
/// Rate limiter entry.
struct RateEntry {
    window_start: Instant,
    count: u32,
    cooldown: Option<RateCooldown>,
}
#[derive(Clone, Copy)]
struct RateCooldown {
    started_at: Instant,
    duration: Duration,
}
impl RateCooldown {
    fn new(started_at: Instant, duration: Duration) -> Self {
        Self {
            started_at,
            duration,
        }
    }
    fn remaining(self, now: Instant) -> Duration {
        let elapsed = now
            .checked_duration_since(self.started_at)
            .unwrap_or_default();
        self.duration.saturating_sub(elapsed)
    }
    fn is_active(self, now: Instant) -> bool {
        !self.remaining(now).is_zero()
    }
}
#[derive(Clone, Copy)]
struct RateLimitParams {
    window: Duration,
    burst: u32,
    cooldown: Duration,
    max_entries: usize,
}
impl RateLimitParams {
    fn new(window: Duration, burst: u32, cooldown: Duration, max_entries: usize) -> Self {
        Self {
            window,
            burst,
            cooldown,
            max_entries,
        }
    }
    fn from_remote(cfg: &QuotaConfig) -> Self {
        Self::new(
            Duration::from_secs(cfg.per_remote_window_secs.max(1)),
            cfg.per_remote_burst,
            Duration::from_secs(cfg.cooldown_secs.max(1)),
            cfg.max_entries.max(1),
        )
    }
}
/// Per-remote rate limiter with cooldown tracking.
struct RateLimiter<K> {
    params: RateLimitParams,
    entries: HashMap<K, RateEntry>,
    last_cleanup: Option<Instant>,
    active_cooldowns: u64,
}
/// Snapshot of quota settings for metrics and compliance logging.
#[derive(Debug, Clone, Copy)]
pub struct QuotaLimits {
    /// Maximum bursts permitted within a window.
    burst: u32,
    /// Window length used when enforcing quotas.
    window: Duration,
    /// Cooldown applied after exceeding the quota.
    cooldown: Duration,
    /// Maximum distinct entries tracked by the limiter.
    max_entries: usize,
}
impl From<&RateLimitParams> for QuotaLimits {
    fn from(params: &RateLimitParams) -> Self {
        Self {
            burst: params.burst,
            window: params.window,
            cooldown: params.cooldown,
            max_entries: params.max_entries,
        }
    }
}
impl QuotaLimits {
    pub fn burst(&self) -> u32 {
        self.burst
    }
    pub fn window(&self) -> Duration {
        self.window
    }
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }
}
impl<K> RateLimiter<K>
where
    K: Copy + Eq + Hash,
{
    fn new(params: RateLimitParams) -> Self {
        Self {
            params,
            entries: HashMap::new(),
            last_cleanup: None,
            active_cooldowns: 0,
        }
    }
    fn check(&mut self, key: K, now: Instant) -> Result<(), Duration> {
        self.maybe_cleanup(now);
        if let Some(cooldown) = self.entries.get(&key).and_then(|entry| entry.cooldown) {
            let remaining = cooldown.remaining(now);
            if !remaining.is_zero() {
                return Err(remaining);
            }
            self.entries.remove(&key);
            self.active_cooldowns = self.active_cooldowns.saturating_sub(1);
        }
        if self.params.burst == 0 {
            // A zero burst disables ordinary quota accounting, but externally
            // imposed cooldowns (for example slowloris penalties) still occupy
            // the bounded table. Fail closed for unseen clients while every
            // slot contains an active penalty.
            if self.entries.len() >= self.params.max_entries {
                self.cleanup(now);
            }
            return if self.entries.len() >= self.params.max_entries {
                Err(self.params.cooldown)
            } else {
                Ok(())
            };
        }
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= self.params.max_entries {
                self.cleanup(now);
            }
            if self.entries.len() >= self.params.max_entries || self.entries.try_reserve(1).is_err()
            {
                return Err(self.params.cooldown);
            }
        }
        let entry = self.entries.entry(key).or_insert(RateEntry {
            window_start: now,
            count: 0,
            cooldown: None,
        });
        let elapsed = now
            .checked_duration_since(entry.window_start)
            .unwrap_or_default();
        if elapsed >= self.params.window {
            entry.window_start = now;
            entry.count = 0;
        }
        entry.count = entry.count.saturating_add(1);
        if entry.count > self.params.burst {
            entry.cooldown = Some(RateCooldown::new(now, self.params.cooldown));
            entry.count = self.params.burst;
            self.active_cooldowns = self.active_cooldowns.saturating_add(1);
            return Err(self.params.cooldown);
        }
        Ok(())
    }
    fn impose_cooldown(&mut self, key: K, now: Instant, cooldown: Duration) {
        let cooldown = if cooldown.is_zero() {
            self.params.cooldown
        } else {
            cooldown
        };
        self.maybe_cleanup(now);
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= self.params.max_entries {
                self.cleanup(now);
            }
            if self.entries.len() >= self.params.max_entries || self.entries.try_reserve(1).is_err()
            {
                return;
            }
        }
        let entry = self.entries.entry(key).or_insert(RateEntry {
            window_start: now,
            count: 0,
            cooldown: None,
        });
        entry.window_start = now;
        entry.count = 0;
        let had_cached_cooldown = entry.cooldown.is_some();
        if entry
            .cooldown
            .is_none_or(|current| current.remaining(now) < cooldown)
        {
            entry.cooldown = Some(RateCooldown::new(now, cooldown));
            if !had_cached_cooldown {
                self.active_cooldowns = self.active_cooldowns.saturating_add(1);
            }
        }
    }
    fn cooldown_count(&self) -> u64 {
        self.active_cooldowns
    }
    fn maybe_cleanup(&mut self, now: Instant) {
        let interval = self
            .params
            .window
            .min(self.params.cooldown)
            .min(Duration::from_secs(1));
        let due = self.last_cleanup.is_none_or(|last| {
            now.checked_duration_since(last)
                .is_none_or(|elapsed| elapsed >= interval)
        });
        if due {
            self.cleanup(now);
        }
    }
    fn cleanup(&mut self, now: Instant) {
        self.last_cleanup = Some(now);
        let horizon = self.params.window.saturating_add(self.params.cooldown);
        let mut active_cooldowns = 0_u64;
        self.entries.retain(|_, entry| {
            if let Some(cooldown) = entry.cooldown {
                let active = cooldown.is_active(now);
                if active {
                    active_cooldowns = active_cooldowns.saturating_add(1);
                }
                return active;
            }
            now.checked_duration_since(entry.window_start)
                .unwrap_or_default()
                <= horizon
        });
        self.active_cooldowns = active_cooldowns;
    }
}
/// Events observed by the slowloris detector.
enum SlowlorisEvent {
    Success(Duration),
    Timeout,
}
struct SlowlorisEntry {
    window_start: Instant,
    score: u32,
}
struct SlowlorisDetector {
    cfg: SlowlorisConfig,
    max_entries: usize,
    entries: Mutex<HashMap<IpAddr, SlowlorisEntry>>,
    unavailable: AtomicBool,
}
impl SlowlorisDetector {
    fn new(cfg: SlowlorisConfig, max_entries: usize) -> Self {
        Self {
            cfg,
            max_entries: max_entries.max(1),
            entries: Mutex::new(HashMap::new()),
            unavailable: AtomicBool::new(false),
        }
    }
    fn penalty(&self) -> Duration {
        Duration::from_secs(self.cfg.penalty_secs)
    }
    fn mark_unavailable(&self) {
        if !self.unavailable.swap(true, Ordering::AcqRel) {
            warn!("slowloris mutex poisoned; rejecting future admission");
        }
    }
    fn unavailable_cooldown(&self) -> Option<Duration> {
        if !self.cfg.enabled {
            return None;
        }
        if self.entries.is_poisoned() {
            self.mark_unavailable();
        }
        self.unavailable
            .load(Ordering::Acquire)
            .then(|| self.penalty())
    }
    fn observe(&self, ip: IpAddr, event: SlowlorisEvent, now: Instant) -> Option<Duration> {
        if !self.cfg.enabled {
            return None;
        }
        let mut guard = match self.entries.lock() {
            Ok(guard) => guard,
            Err(_) => {
                // The score map can no longer make a trustworthy allow or
                // penalty decision. Latch the failure so every later begin is
                // rejected, and apply the deterministic configured penalty to
                // this just-completed attempt without inspecting poisoned data.
                self.mark_unavailable();
                return Some(self.penalty());
            }
        };
        let mut penalise = matches!(event, SlowlorisEvent::Timeout);
        if let SlowlorisEvent::Success(elapsed) = event {
            let threshold = Duration::from_millis(self.cfg.max_handshake_millis);
            if elapsed >= threshold {
                penalise = true;
            }
        }
        let window = Duration::from_secs(self.cfg.window_secs);
        if let Some(entry) = guard.get_mut(&ip) {
            if now
                .checked_duration_since(entry.window_start)
                .is_some_and(|elapsed| elapsed >= window)
            {
                entry.window_start = now;
                entry.score = 0;
            }
            if penalise {
                entry.score = entry.score.saturating_add(1);
            } else {
                entry.score = entry.score.saturating_sub(1);
            }
            let threshold_reached = entry.score >= self.cfg.timeout_threshold;
            let inactive = entry.score == 0;
            if threshold_reached || inactive {
                guard.remove(&ip);
            }
            return threshold_reached.then(|| self.penalty());
        }
        // Fast first-time outcomes carry no slowloris evidence and must not allocate
        // attacker-keyed history. Only suspicious observations enter the bounded map.
        if !penalise {
            return None;
        }
        if self.cfg.timeout_threshold <= 1 {
            return Some(self.penalty());
        }
        if guard.len() >= self.max_entries {
            // Reclaim scores whose complete observation window has elapsed before
            // rejecting an unseen source. Clock regression retains state fail closed.
            guard.retain(|_, entry| {
                !now.checked_duration_since(entry.window_start)
                    .is_some_and(|elapsed| elapsed >= window)
            });
        }
        if guard.len() >= self.max_entries || guard.try_reserve(1).is_err() {
            return Some(self.penalty());
        }
        guard.insert(
            ip,
            SlowlorisEntry {
                window_start: now,
                score: 1,
            },
        );
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{PuzzleConfig, TokenConfig},
        metrics::TokenOutcomeKey,
    };
    use iroha_crypto::soranet::token::compute_issuer_fingerprint;
    use rand::{SeedableRng, rngs::StdRng};
    use std::{fs, net::SocketAddr, time::UNIX_EPOCH};
    use tempfile::tempdir;
    fn canonical_system_now() -> SystemTime {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock after Unix epoch")
            .as_secs();
        UNIX_EPOCH + Duration::from_secs(seconds)
    }
    #[test]
    fn rate_limiter_throttles_after_burst() {
        let params = RateLimitParams::new(Duration::from_secs(1), 2, Duration::from_secs(2), 16);
        let mut limiter = RateLimiter::new(params);
        let now = Instant::now();
        assert!(limiter.check(IpAddr::from([127, 0, 0, 1]), now).is_ok());
        assert!(limiter.check(IpAddr::from([127, 0, 0, 1]), now).is_ok());
        let err = limiter
            .check(IpAddr::from([127, 0, 0, 1]), now)
            .expect_err("expected throttle after burst exceeded");
        assert_eq!(err, Duration::from_secs(2));
    }
    #[test]
    fn rate_limiter_rejects_new_keys_at_capacity_without_overshoot() {
        let cooldown = Duration::from_secs(2);
        let params = RateLimitParams::new(Duration::from_secs(60), 2, cooldown, 2);
        let mut limiter = RateLimiter::new(params);
        let now = Instant::now();
        let first = IpAddr::from([127, 0, 0, 1]);
        let second = IpAddr::from([127, 0, 0, 2]);
        let overflow = IpAddr::from([127, 0, 0, 3]);
        assert!(limiter.check(first, now).is_ok());
        assert!(limiter.check(second, now).is_ok());
        assert_eq!(limiter.check(overflow, now), Err(cooldown));
        assert_eq!(limiter.entries.len(), 2);
        limiter.impose_cooldown(overflow, now, cooldown);
        assert_eq!(limiter.entries.len(), 2);
        assert!(!limiter.entries.contains_key(&overflow));
    }
    #[test]
    fn rate_limiter_batches_cleanup_between_admissions() {
        let params = RateLimitParams::new(Duration::from_secs(60), 4, Duration::from_secs(20), 16);
        let mut limiter = RateLimiter::new(params);
        let now = Instant::now();
        limiter
            .check(IpAddr::from([127, 0, 0, 1]), now)
            .expect("first admission");
        let first_cleanup = limiter.last_cleanup;

        limiter
            .check(
                IpAddr::from([127, 0, 0, 2]),
                now + Duration::from_millis(100),
            )
            .expect("second admission");

        assert_eq!(limiter.last_cleanup, first_cleanup);
    }
    #[test]
    fn rate_limiter_forces_cleanup_before_capacity_rejection() {
        let params = RateLimitParams::new(Duration::from_secs(1), 1, Duration::from_secs(1), 1);
        let mut limiter = RateLimiter::new(params);
        let now = Instant::now();
        let stale = IpAddr::from([127, 0, 0, 1]);
        let replacement = IpAddr::from([127, 0, 0, 2]);
        limiter.entries.insert(
            stale,
            RateEntry {
                window_start: now
                    .checked_sub(Duration::from_secs(3))
                    .expect("test instant supports subtraction"),
                count: 1,
                cooldown: None,
            },
        );
        limiter.last_cleanup = Some(now);

        assert_eq!(limiter.check(replacement, now), Ok(()));
        assert!(!limiter.entries.contains_key(&stale));
        assert!(limiter.entries.contains_key(&replacement));
    }
    #[test]
    fn rate_limiter_handles_max_durations_without_absolute_instant_arithmetic() {
        let maximum = Duration::from_secs(u64::MAX);
        let params = RateLimitParams::new(maximum, 1, maximum, 16);
        let mut limiter = RateLimiter::new(params);
        let now = Instant::now();
        let throttled = IpAddr::from([127, 0, 0, 1]);
        let other = IpAddr::from([127, 0, 0, 2]);

        assert_eq!(limiter.check(throttled, now), Ok(()));
        assert_eq!(limiter.check(throttled, now), Err(maximum));
        assert_eq!(limiter.check(other, now), Ok(()));
        assert_eq!(limiter.check(throttled, now), Err(maximum));
    }
    #[test]
    fn remote_quota_throttle_sets_active_gauge() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 1,
                cooldown_secs: 1,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = DoSControls::new(&pow_cfg, None, Arc::clone(&metrics), RelayMode::Entry)
            .expect("dos controls");
        let remote: SocketAddr = "127.0.0.1:2000".parse().expect("remote addr");
        controls.begin(remote, None).expect("first attempt allowed");
        let throttle = controls
            .begin(remote, None)
            .expect_err("second attempt should throttle");
        assert!(matches!(throttle.reason, ThrottleReason::RemoteQuota));
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active_remote_cooldowns, 1);
    }
    #[test]
    fn slowloris_penalty_is_enforced_when_remote_burst_is_disabled() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 0,
                per_remote_window_secs: 1,
                cooldown_secs: 2,
                ..QuotaConfig::default()
            },
            slowloris: SlowlorisConfig {
                enabled: true,
                timeout_threshold: 1,
                window_secs: 1,
                penalty_secs: 5,
                ..SlowlorisConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = DoSControls::new(&pow_cfg, None, Arc::clone(&metrics), RelayMode::Entry)
            .expect("dos controls");
        let remote: SocketAddr = "127.0.0.20:2000".parse().expect("remote addr");
        let other: SocketAddr = "127.0.0.21:2000".parse().expect("remote addr");
        let now = Instant::now();
        let attempt = controls
            .begin_at(remote, None, now)
            .expect("disabled burst quota must admit the initial attempt");

        controls.record_timeout_at(&attempt, Duration::ZERO, now);
        let throttle = controls
            .begin_at(remote, None, now + Duration::from_secs(1))
            .expect_err("slowloris penalty must survive a disabled burst quota");
        assert_eq!(throttle.reason, ThrottleReason::RemoteQuota);
        assert_eq!(throttle.cooldown, Duration::from_secs(4));
        controls
            .begin_at(other, None, now + Duration::from_secs(1))
            .expect("one penalized remote must not block unrelated capacity");
        controls
            .begin_at(remote, None, now + Duration::from_secs(5))
            .expect("remote must be admitted at the configured penalty boundary");
        assert_eq!(metrics.snapshot().active_remote_cooldowns, 0);
    }
    #[test]
    fn slowloris_penalty_outlives_the_quota_cleanup_horizon() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 4,
                per_remote_window_secs: 1,
                cooldown_secs: 1,
                ..QuotaConfig::default()
            },
            slowloris: SlowlorisConfig {
                enabled: true,
                timeout_threshold: 1,
                window_secs: 1,
                penalty_secs: 10,
                ..SlowlorisConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls =
            DoSControls::new(&pow_cfg, None, metrics, RelayMode::Entry).expect("dos controls");
        let remote: SocketAddr = "127.0.0.22:2000".parse().expect("remote addr");
        let now = Instant::now();
        let attempt = controls
            .begin_at(remote, None, now)
            .expect("initial attempt");

        controls.record_timeout_at(&attempt, Duration::ZERO, now);
        let throttle = controls
            .begin_at(remote, None, now + Duration::from_secs(3))
            .expect_err("active penalty must not be evicted by quota cleanup");
        assert_eq!(throttle.cooldown, Duration::from_secs(7));
        controls
            .begin_at(remote, None, now + Duration::from_secs(10))
            .expect("remote must be admitted when the full penalty expires");
    }
    #[test]
    fn maximum_slowloris_penalty_does_not_poison_remote_admission() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 0,
                per_remote_window_secs: 1,
                cooldown_secs: 1,
                ..QuotaConfig::default()
            },
            slowloris: SlowlorisConfig {
                enabled: true,
                timeout_threshold: 1,
                window_secs: 1,
                penalty_secs: u64::MAX,
                ..SlowlorisConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls =
            DoSControls::new(&pow_cfg, None, metrics, RelayMode::Entry).expect("dos controls");
        let remote: SocketAddr = "127.0.0.23:2000".parse().expect("remote addr");
        let other: SocketAddr = "127.0.0.24:2000".parse().expect("remote addr");
        let now = Instant::now();
        let attempt = controls
            .begin_at(remote, None, now)
            .expect("initial attempt");

        controls.record_timeout_at(&attempt, Duration::ZERO, now);
        let throttle = controls
            .begin_at(remote, None, now)
            .expect_err("maximum representable penalty must remain enforceable");
        assert_eq!(throttle.cooldown, Duration::from_secs(u64::MAX));
        controls
            .begin_at(other, None, now)
            .expect("maximum penalty must not poison shared admission state");
    }
    #[test]
    fn remote_quota_canonicalizes_ipv4_mapped_ipv6() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 1,
                cooldown_secs: 1,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls =
            DoSControls::new(&pow_cfg, None, metrics, RelayMode::Entry).expect("dos controls");
        let address = std::net::Ipv4Addr::new(192, 0, 2, 9);
        let ipv4 = SocketAddr::new(IpAddr::V4(address), 2_000);
        let mapped = SocketAddr::new(IpAddr::V6(address.to_ipv6_mapped()), 2_001);

        controls.begin(ipv4, None).expect("first attempt allowed");
        let throttle = controls
            .begin(mapped, None)
            .expect_err("mapped address must share the IPv4 quota");
        assert_eq!(throttle.reason, ThrottleReason::RemoteQuota);
    }
    #[test]
    fn poisoned_remote_quota_fails_closed() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 4,
                cooldown_secs: 7,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = Arc::new(
            DoSControls::new(&pow_cfg, None, metrics, RelayMode::Entry).expect("dos controls"),
        );
        let poison_target = Arc::clone(&controls);
        let poisoned = std::thread::spawn(move || {
            let _guard = poison_target
                .remote_limiter
                .lock()
                .expect("remote quota lock");
            panic!("poison remote quota state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        let remote: SocketAddr = "127.0.0.8:2000".parse().expect("remote addr");
        let throttle = controls
            .begin(remote, None)
            .expect_err("poisoned remote quota must reject admission");
        assert_eq!(throttle.reason, ThrottleReason::RemoteQuota);
        assert_eq!(throttle.cooldown, Duration::from_secs(7));
    }
    #[test]
    fn poisoned_slowloris_state_latches_fail_closed_admission() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 4,
                ..QuotaConfig::default()
            },
            slowloris: SlowlorisConfig {
                enabled: true,
                penalty_secs: 13,
                ..SlowlorisConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = Arc::new(
            DoSControls::new(&pow_cfg, None, metrics, RelayMode::Entry).expect("dos controls"),
        );
        let first_remote: SocketAddr = "127.0.0.11:2000".parse().expect("remote addr");
        let attempt = controls
            .begin(first_remote, None)
            .expect("healthy slowloris state must admit");
        let poison_target = Arc::clone(&controls);
        let poisoned = std::thread::spawn(move || {
            let _guard = poison_target
                .slowloris
                .entries
                .lock()
                .expect("slowloris lock");
            panic!("poison slowloris state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        controls.record_timeout_at(&attempt, Duration::ZERO, Instant::now());
        let next_remote: SocketAddr = "127.0.0.12:2000".parse().expect("remote addr");
        let throttle = controls
            .begin(next_remote, None)
            .expect_err("poisoned slowloris state must reject future admission");
        assert_eq!(throttle.reason, ThrottleReason::RemoteQuota);
        assert_eq!(throttle.cooldown, Duration::from_secs(13));
    }
    #[test]
    fn slowloris_fast_first_outcomes_do_not_allocate_history() {
        let detector = SlowlorisDetector::new(SlowlorisConfig::default(), 2);
        let now = Instant::now();
        for octet in 1..=32 {
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, octet));
            assert_eq!(
                detector.observe(ip, SlowlorisEvent::Success(Duration::ZERO), now),
                None
            );
        }
        assert!(
            detector
                .entries
                .lock()
                .expect("slowloris entries")
                .is_empty(),
            "benign first-time outcomes must not consume attacker-keyed capacity"
        );
    }
    #[test]
    fn slowloris_rejects_new_suspicious_sources_at_capacity() {
        let detector = SlowlorisDetector::new(
            SlowlorisConfig {
                timeout_threshold: 3,
                penalty_secs: 17,
                ..SlowlorisConfig::default()
            },
            2,
        );
        let now = Instant::now();
        for ip in ["192.0.2.1", "192.0.2.2"] {
            assert_eq!(
                detector.observe(ip.parse().expect("test IP"), SlowlorisEvent::Timeout, now),
                None
            );
        }
        assert_eq!(
            detector.observe(
                "192.0.2.3".parse().expect("test IP"),
                SlowlorisEvent::Timeout,
                now,
            ),
            Some(Duration::from_secs(17)),
            "an unseen suspicious source must fail closed at capacity"
        );
        assert_eq!(detector.entries.lock().expect("slowloris entries").len(), 2);
    }
    #[test]
    fn slowloris_reclaims_stale_entries_at_window_boundary() {
        let cfg = SlowlorisConfig {
            timeout_threshold: 3,
            window_secs: 5,
            ..SlowlorisConfig::default()
        };
        let detector = SlowlorisDetector::new(cfg, 1);
        let now = Instant::now();
        let stale_ip = "192.0.2.1".parse().expect("test IP");
        let replacement_ip = "192.0.2.2".parse().expect("test IP");
        assert_eq!(
            detector.observe(stale_ip, SlowlorisEvent::Timeout, now),
            None
        );
        assert_eq!(
            detector.observe(
                replacement_ip,
                SlowlorisEvent::Timeout,
                now + Duration::from_secs(5),
            ),
            None,
            "the exact window boundary must make the stale slot reusable"
        );
        let entries = detector.entries.lock().expect("slowloris entries");
        assert_eq!(entries.len(), 1);
        assert!(entries.contains_key(&replacement_ip));
    }
    #[test]
    fn emergency_throttle_blocks_descriptor() {
        let metrics = Arc::new(Metrics::new());
        let descriptor = [0x42u8; 32];
        let mut pow_cfg = PowConfig {
            emergency: Some(EmergencyThrottleConfig {
                descriptor_commit_hex: vec![hex::encode(descriptor)],
                file_path: None,
                cooldown_secs: 5,
                refresh_secs: 60,
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = DoSControls::new(&pow_cfg, None, Arc::clone(&metrics), RelayMode::Entry)
            .expect("dos controls");
        let remote: SocketAddr = "127.0.0.3:3030".parse().expect("remote addr");
        let throttle = controls
            .begin(remote, Some(&descriptor))
            .expect_err("descriptor should be blocked by emergency throttle");
        assert!(matches!(throttle.reason, ThrottleReason::Emergency));
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.throttled_emergency, 1);
    }
    #[test]
    fn poisoned_emergency_state_latches_fail_closed_admission() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 4,
                ..QuotaConfig::default()
            },
            emergency: Some(EmergencyThrottleConfig {
                descriptor_commit_hex: Vec::new(),
                file_path: None,
                cooldown_secs: 17,
                refresh_secs: 60,
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = Arc::new(
            DoSControls::new(&pow_cfg, None, Arc::clone(&metrics), RelayMode::Entry)
                .expect("dos controls"),
        );
        let poison_target = Arc::clone(&controls);
        let poisoned = std::thread::spawn(move || {
            let emergency = poison_target
                .emergency
                .as_ref()
                .expect("configured emergency throttle");
            let _guard = emergency.state.write().expect("emergency state lock");
            panic!("poison emergency throttle state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        let remote: SocketAddr = "127.0.0.13:3030".parse().expect("remote addr");
        let throttle = controls
            .begin(remote, None)
            .expect_err("poisoned emergency state must reject even without a descriptor");
        assert_eq!(throttle.reason, ThrottleReason::Emergency);
        assert_eq!(throttle.cooldown, Duration::from_secs(17));
        assert_eq!(metrics.snapshot().throttled_emergency, 1);
    }
    #[test]
    fn configured_emergency_file_must_load_at_startup() {
        let directory = tempdir().expect("temporary directory");
        let missing_path = directory.path().join("missing-emergency-policy.json");
        let mut pow_cfg = PowConfig {
            emergency: Some(EmergencyThrottleConfig {
                descriptor_commit_hex: Vec::new(),
                file_path: Some(missing_path),
                cooldown_secs: 19,
                refresh_secs: 60,
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");

        let error =
            match DoSControls::new(&pow_cfg, None, Arc::new(Metrics::new()), RelayMode::Entry) {
                Ok(_) => panic!("missing configured emergency document must prevent startup"),
                Err(error) => error,
            };
        assert!(
            matches!(
                &error,
                ConfigError::EmergencyThrottle(message)
                    if message.contains("failed to read emergency throttle file")
            ),
            "unexpected startup error: {error}"
        );
    }
    #[test]
    fn emergency_throttle_descriptor_count_accepts_exact_limit() {
        let descriptor = hex::encode([0x31_u8; 32]);
        let exact = vec![descriptor.clone(); EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1];
        let decoded = EmergencyThrottle::decode_descriptor_list(exact.iter())
            .expect("exact descriptor count");
        assert_eq!(decoded.len(), 1, "duplicate descriptors collapse");
        let overflow = vec![descriptor; EMERGENCY_THROTTLE_MAX_DESCRIPTORS_V1 + 1];
        let error = EmergencyThrottle::decode_descriptor_list(overflow.iter())
            .expect_err("max+1 descriptors must fail before retention");
        assert!(error.contains("first-release limit"), "{error}");
    }
    #[test]
    fn emergency_throttle_document_accepts_exact_file_limit() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("emergency.json");
        let prefix = br#"{"descriptor_commit_hex":[]}"#;
        let mut exact = vec![b' '; EMERGENCY_THROTTLE_DOCUMENT_MAX_BYTES_V1];
        exact[..prefix.len()].copy_from_slice(prefix);
        fs::write(&path, &exact).expect("write exact document");
        let loaded = EmergencyThrottle::load_document(&path).expect("exact document");
        assert!(loaded.descriptors.is_empty());
        exact.push(b' ');
        fs::write(&path, exact).expect("write oversized document");
        let error = EmergencyThrottle::load_document(&path)
            .expect_err("max+1 document must fail before decode");
        assert!(error.contains("first-release limit"), "{error}");
    }
    #[cfg(unix)]
    #[test]
    fn emergency_throttle_document_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("temporary directory");
        let target = directory.path().join("target.json");
        let link = directory.path().join("emergency.json");
        fs::write(&target, br#"{"descriptor_commit_hex":[]}"#).expect("write target");
        symlink(&target, &link).expect("create symlink");
        let error = EmergencyThrottle::load_document(&link)
            .expect_err("symlinked document must fail before read");
        assert!(error.contains("direct regular file"), "{error}");
    }
    #[test]
    fn puzzle_parameters_share_static_configured_difficulty() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            difficulty: 6,
            max_future_skew_secs: 90,
            min_ticket_ttl_secs: 30,
            puzzle: PuzzleConfig {
                memory_kib: 4096,
                time_cost: 1,
                lanes: 1,
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("defaults");
        let quotas = pow_cfg.quotas_for_mode(RelayMode::Entry);
        assert_eq!(quotas.per_remote_burst, pow_cfg.quotas.per_remote_burst);
        let controls =
            DoSControls::new(&pow_cfg, None, metrics, RelayMode::Entry).expect("dos controls");
        let params = controls.current_puzzle_parameters();
        assert_eq!(params.difficulty(), 6);
        assert_eq!(params.memory_kib().get(), 4096);
    }
    #[test]
    fn token_policy_verifies_valid_token() {
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let issuer_hex = hex::encode(keypair.public_key());
        let replay_dir = tempdir().expect("replay tempdir");
        let mut pow_cfg = PowConfig {
            token: Some(TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(issuer_hex),
                max_ttl_secs: 600,
                clock_skew_secs: 5,
                replay_store_path: replay_dir.path().join("tokens.norito"),
                revocation_list_hex: Vec::new(),
                revocation_list_path: None,
                ..TokenConfig::default()
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("defaults");
        let token_policy = pow_cfg
            .token_policy()
            .expect("token policy result")
            .expect("token policy enabled");
        let metrics = Arc::new(Metrics::new());
        let controls = DoSControls::new(
            &pow_cfg,
            Some(token_policy),
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
        .expect("dos controls");
        let relay_id = [0xAB; 32];
        let transcript_hash = [0xCD; 32];
        let issued = canonical_system_now();
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            compute_issuer_fingerprint(keypair.public_key()),
            relay_id,
            transcript_hash,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint token");
        controls
            .verify_token(
                &token,
                &relay_id,
                &transcript_hash,
                issued + Duration::from_secs(10),
            )
            .expect("token should verify");
    }
    #[test]
    fn token_policy_rejects_revoked_token() {
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let issuer_hex = hex::encode(keypair.public_key());
        let relay_id = [0x44; 32];
        let transcript_hash = [0x11; 32];
        let issued = canonical_system_now();
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(42);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            compute_issuer_fingerprint(keypair.public_key()),
            relay_id,
            transcript_hash,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint token");
        let revoked_hex = hex::encode(token.token_id());
        let replay_dir = tempdir().expect("replay tempdir");
        let mut pow_cfg = PowConfig {
            token: Some(TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(issuer_hex),
                max_ttl_secs: 600,
                clock_skew_secs: 5,
                replay_store_path: replay_dir.path().join("tokens.norito"),
                revocation_list_hex: vec![revoked_hex],
                revocation_list_path: None,
                ..TokenConfig::default()
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("defaults");
        let token_policy = pow_cfg
            .token_policy()
            .expect("token policy result")
            .expect("token policy enabled");
        let metrics = Arc::new(Metrics::new());
        let controls = DoSControls::new(
            &pow_cfg,
            Some(token_policy),
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
        .expect("dos controls");
        let err = controls
            .verify_token(
                &token,
                &relay_id,
                &transcript_hash,
                issued + Duration::from_secs(15),
            )
            .expect_err("token should be revoked");
        assert!(matches!(err, TokenPolicyError::Revoked(_)));
    }
    #[test]
    fn token_outcome_labels_inert_signature_as_invalid_signature() {
        let error = TokenPolicyError::Verify(TokenVerifyError::InertSignature);
        assert_eq!(token_outcome_label(&error), "signature_invalid");
    }
    #[test]
    fn token_outcome_metrics_recorded() {
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let issuer_hex = hex::encode(keypair.public_key());
        let relay_id = [0x55; 32];
        let transcript_hash = [0x66; 32];
        let issued = canonical_system_now();
        let expires = issued + Duration::from_secs(300);
        let mut rng = StdRng::seed_from_u64(17);
        let token = AdmissionToken::mint(
            MlDsaSuite::MlDsa44,
            keypair.secret_key(),
            compute_issuer_fingerprint(keypair.public_key()),
            relay_id,
            transcript_hash,
            issued,
            expires,
            0,
            &mut rng,
        )
        .expect("mint token");
        let replay_dir = tempdir().expect("replay tempdir");
        let mut pow_cfg = PowConfig {
            token: Some(TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(issuer_hex),
                max_ttl_secs: 600,
                clock_skew_secs: 5,
                replay_store_path: replay_dir.path().join("tokens.norito"),
                ..TokenConfig::default()
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("defaults");
        let token_policy = pow_cfg
            .token_policy()
            .expect("token policy result")
            .expect("token policy enabled");
        let metrics = Arc::new(Metrics::new());
        let controls = DoSControls::new(
            &pow_cfg,
            Some(token_policy),
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
        .expect("dos controls");
        let now = issued + Duration::from_secs(5);
        controls
            .verify_token(&token, &relay_id, &transcript_hash, now)
            .expect("first use");
        let _ = controls.verify_token(&token, &relay_id, &transcript_hash, now);
        let snapshot = metrics.snapshot();
        let accepted_key = TokenOutcomeKey {
            issuer: hex::encode(token.issuer_fingerprint()),
            relay: hex::encode(relay_id),
            outcome: "accepted".to_string(),
        };
        let replay_key = TokenOutcomeKey {
            issuer: hex::encode(token.issuer_fingerprint()),
            relay: hex::encode(relay_id),
            outcome: "replay".to_string(),
        };
        assert_eq!(snapshot.token_outcomes.get(&accepted_key), Some(&1));
        assert_eq!(snapshot.token_outcomes.get(&replay_key), Some(&1));
    }
}
