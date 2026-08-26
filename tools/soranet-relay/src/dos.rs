//! DoS and abuse mitigation utilities for the relay handshake path.
use crate::{
    capability,
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
    pow::Parameters,
    puzzle,
    token::{AdmissionToken, AdmissionTokenVerifier, VerifyError as TokenVerifyError},
};
use norito::{DecodeLimits, derive::JsonDeserialize, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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
    pow_params: Parameters,
    remote_limiter: Mutex<RateLimiter<IpAddr>>,
    descriptor_limiter: Mutex<Option<RateLimiter<[u8; 16]>>>,
    slowloris: SlowlorisDetector,
    require_pow: bool,
    puzzle: Option<PuzzlePolicy>,
    signed_ticket_public_key: Option<Arc<Vec<u8>>>,
    token: Option<TokenPolicy>,
    replay_filter: Option<Mutex<ReplayFilter>>,
    metrics: Arc<Metrics>,
    remote_limits: QuotaLimits,
    descriptor_limits: Option<QuotaLimits>,
    emergency: Option<EmergencyThrottle>,
}
impl DoSControls {
    /// Create a new controller from the relay PoW configuration.
    pub fn new(
        base_params: Parameters,
        config: &PowConfig,
        puzzle: Option<puzzle::Parameters>,
        token: Option<TokenPolicySource>,
        metrics: Arc<Metrics>,
        mode: RelayMode,
    ) -> Result<Self, ConfigError> {
        let quotas_cfg = config.quotas_for_mode(mode);
        let mut slowloris_cfg = config.slowloris.clone();
        slowloris_cfg.apply_defaults();
        let remote_params = RateLimitParams::from_remote(&quotas_cfg);
        let descriptor_params = RateLimitParams::from_descriptor(&quotas_cfg);
        let remote_limits = QuotaLimits::from(&remote_params);
        let remote_limiter = Mutex::new(RateLimiter::new(remote_params));
        let (descriptor_limits, descriptor_limiter) = match descriptor_params {
            Some(params) => {
                let limits = QuotaLimits::from(&params);
                let limiter = Mutex::new(Some(RateLimiter::new(params)));
                (Some(limits), limiter)
            }
            None => (None, Mutex::new(None)),
        };
        metrics.set_pow_difficulty(base_params.difficulty());
        metrics.set_active_remote_cooldowns(0);
        if descriptor_limits.is_none() {
            metrics.set_active_descriptor_cooldowns(0);
        }
        let puzzle_policy = puzzle.map(PuzzlePolicy::new);
        let signed_ticket_public_key = config.signed_ticket_public_key()?.map(Arc::new);
        if signed_ticket_public_key.is_some() && puzzle_policy.is_none() {
            return Err(ConfigError::Puzzle(
                "pow.signed_ticket_public_key_hex requires the mandatory Argon2 puzzle policy"
                    .to_owned(),
            ));
        }
        let replay_filter = if config.replay_filter().is_enabled() {
            Some(Mutex::new(ReplayFilter::new(
                config.replay_filter().bits_usize(),
                config.replay_filter().hash_count(),
                config.replay_filter().ttl(),
            )?))
        } else {
            None
        };
        let emergency = config
            .emergency_throttle()
            .map(|cfg| EmergencyThrottle::new(cfg.clone()))
            .transpose()?;
        Ok(Self {
            pow_params: base_params,
            remote_limiter,
            descriptor_limiter,
            slowloris: SlowlorisDetector::new(slowloris_cfg),
            require_pow: config.required,
            puzzle: puzzle_policy,
            signed_ticket_public_key,
            token: token.map(TokenPolicy::from_source),
            replay_filter,
            metrics,
            remote_limits,
            descriptor_limits,
            emergency,
        })
    }
    /// Returns the static configured first-release PoW parameters.
    pub fn current_pow_parameters(&self) -> Parameters {
        self.pow_params
    }
    /// Returns the static configured first-release puzzle parameters.
    pub fn current_puzzle_parameters(&self) -> Option<puzzle::Parameters> {
        let policy = self.puzzle.as_ref()?;
        Some(policy.parameters())
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
    /// Indicates whether PoW tickets are mandated by relay policy.
    pub fn is_pow_required(&self) -> bool {
        self.require_pow
    }
    /// Returns the active remote quota limits.
    pub fn remote_quota_limits(&self) -> QuotaLimits {
        self.remote_limits
    }
    /// Returns the active descriptor quota limits, if configured.
    pub fn descriptor_quota_limits(&self) -> Option<QuotaLimits> {
        self.descriptor_limits
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
        let ip = remote.ip();
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
        if let (Some(filter), Some(commit_bytes)) = (self.replay_filter.as_ref(), descriptor_commit)
        {
            let mut guard = match filter.lock() {
                Ok(guard) => guard,
                Err(error) => {
                    // Poisoned replay state is not trustworthy enough to make
                    // an allow decision. Preserve the configured TTL for the
                    // controlled throttle without observing the filter's
                    // partially-mutated counters or entries.
                    let cooldown = error.get_ref().ttl();
                    warn!(
                        %error,
                        "descriptor replay-filter mutex poisoned; rejecting admission"
                    );
                    return Err(Throttle {
                        cooldown,
                        reason: ThrottleReason::DescriptorReplay,
                    });
                }
            };
            let is_new = guard.observe(commit_bytes, now);
            if !is_new {
                return Err(Throttle {
                    cooldown: guard.ttl(),
                    reason: ThrottleReason::DescriptorReplay,
                });
            }
        }
        if let Some(key) = descriptor_commit.and_then(descriptor_key) {
            if let Err(cooldown) = self.check_descriptor_limit(key, now) {
                return Err(Throttle {
                    cooldown,
                    reason: ThrottleReason::DescriptorQuota,
                });
            }
            Ok(AttemptContext {
                remote: ip,
                _descriptor_key: Some(key),
                started_at: now,
            })
        } else {
            Ok(AttemptContext {
                remote: ip,
                _descriptor_key: None,
                started_at: now,
            })
        }
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
                let count = limiter.cooldown_count(now);
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
        let count = limiter.cooldown_count(now);
        self.metrics.set_active_remote_cooldowns(count);
        result
    }
    fn check_descriptor_limit(&self, key: [u8; 16], now: Instant) -> Result<(), Duration> {
        let mut guard = self.descriptor_limiter.lock().map_err(|error| {
            warn!(%error, "descriptor quota mutex poisoned; rejecting admission");
            self.descriptor_limits
                .as_ref()
                .map_or_else(|| self.remote_limits.cooldown(), QuotaLimits::cooldown)
        })?;
        if let Some(limiter) = guard.as_mut() {
            let result = limiter.check(key, now);
            let count = limiter.cooldown_count(now);
            self.metrics.set_active_descriptor_cooldowns(count);
            result
        } else {
            self.metrics.set_active_descriptor_cooldowns(0);
            Ok(())
        }
    }
}
/// Context associated with an inbound handshake attempt.
#[derive(Debug, Clone)]
pub struct AttemptContext {
    remote: IpAddr,
    _descriptor_key: Option<[u8; 16]>,
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
    DescriptorQuota,
    DescriptorReplay,
    Emergency,
}
impl fmt::Display for ThrottleReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThrottleReason::RemoteQuota => f.write_str("remote quota exceeded"),
            ThrottleReason::DescriptorQuota => f.write_str("descriptor quota exceeded"),
            ThrottleReason::DescriptorReplay => f.write_str("descriptor replay filter triggered"),
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
    cooldown_until: Option<Instant>,
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
    fn from_descriptor(cfg: &QuotaConfig) -> Option<Self> {
        if cfg.per_descriptor_burst == 0 {
            None
        } else {
            Some(Self::new(
                Duration::from_secs(cfg.per_descriptor_window_secs.max(1)),
                cfg.per_descriptor_burst,
                Duration::from_secs(cfg.cooldown_secs.max(1)),
                cfg.max_entries.max(1),
            ))
        }
    }
}
/// Per-remote or per-descriptor rate limiter with cooldown tracking.
struct RateLimiter<K> {
    params: RateLimitParams,
    entries: HashMap<K, RateEntry>,
}
/// Snapshot of quota settings for metrics and compliance logging.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    K: Eq + Hash,
{
    fn new(params: RateLimitParams) -> Self {
        Self {
            params,
            entries: HashMap::new(),
        }
    }
    fn check(&mut self, key: K, now: Instant) -> Result<(), Duration> {
        if self.params.burst == 0 {
            return Ok(());
        }
        self.cleanup(now);
        if !self.entries.contains_key(&key)
            && (self.entries.len() >= self.params.max_entries
                || self.entries.try_reserve(1).is_err())
        {
            return Err(self.params.cooldown);
        }
        let entry = self.entries.entry(key).or_insert(RateEntry {
            window_start: now,
            count: 0,
            cooldown_until: None,
        });
        if let Some(until) = entry.cooldown_until {
            if until > now {
                return Err(until.saturating_duration_since(now));
            }
            entry.cooldown_until = None;
            entry.count = 0;
            entry.window_start = now;
        }
        let elapsed = now
            .checked_duration_since(entry.window_start)
            .unwrap_or_default();
        if elapsed >= self.params.window {
            entry.window_start = now;
            entry.count = 0;
        }
        entry.count = entry.count.saturating_add(1);
        if entry.count > self.params.burst {
            let cooldown_until = now + self.params.cooldown;
            entry.cooldown_until = Some(cooldown_until);
            entry.count = self.params.burst;
            return Err(self.params.cooldown);
        }
        Ok(())
    }
    fn impose_cooldown(&mut self, key: K, now: Instant, cooldown: Duration) {
        if self.params.burst == 0 {
            return;
        }
        let cooldown = if cooldown.is_zero() {
            self.params.cooldown
        } else {
            cooldown
        };
        self.cleanup(now);
        if !self.entries.contains_key(&key)
            && (self.entries.len() >= self.params.max_entries
                || self.entries.try_reserve(1).is_err())
        {
            return;
        }
        let entry = self.entries.entry(key).or_insert(RateEntry {
            window_start: now,
            count: 0,
            cooldown_until: None,
        });
        entry.window_start = now;
        entry.count = 0;
        entry.cooldown_until = Some(now + cooldown);
    }
    fn cooldown_count(&self, now: Instant) -> u64 {
        self.entries
            .values()
            .filter(|entry| entry.cooldown_until.is_some_and(|until| until > now))
            .count() as u64
    }
    fn cleanup(&mut self, now: Instant) {
        if self.entries.is_empty() {
            return;
        }
        let horizon = self.params.window + self.params.cooldown;
        self.entries.retain(|_, entry| {
            now.checked_duration_since(entry.window_start)
                .unwrap_or_default()
                <= horizon
        });
    }
}
/// Stored positions for an observed replay entry.
struct ReplayEntry {
    expiry: Instant,
    positions: Box<[usize]>,
}
/// Counting bloom filter used to detect replayed PoW tickets.
struct ReplayFilter {
    mask: usize,
    hash_count: u8,
    ttl: Duration,
    counters: Vec<u16>,
    entries: VecDeque<ReplayEntry>,
}
impl ReplayFilter {
    fn new(bits: usize, hash_count: u8, ttl: Duration) -> Result<Self, ConfigError> {
        const MAX_BITS: usize = 1 << 24; // 16,777,216 counters
        let clamped = bits.max(64);
        if clamped > MAX_BITS {
            return Err(ConfigError::ReplayFilter(
                "replay_filter.bits must not exceed 16,777,216".to_string(),
            ));
        }
        let size = clamped.next_power_of_two();
        let mask = size - 1;
        debug_assert!(hash_count > 0);
        Ok(Self {
            mask,
            hash_count,
            ttl,
            counters: vec![0u16; size],
            entries: VecDeque::new(),
        })
    }
    fn ttl(&self) -> Duration {
        self.ttl
    }
    fn observe(&mut self, key: &[u8], now: Instant) -> bool {
        self.purge(now);
        let positions = self.hash_positions(key);
        let seen = positions.iter().all(|&pos| self.counters[pos] > 0);
        for &pos in &positions {
            let slot = &mut self.counters[pos];
            *slot = slot.saturating_add(1);
        }
        self.entries.push_back(ReplayEntry {
            expiry: now + self.ttl,
            positions: positions.into_boxed_slice(),
        });
        !seen
    }
    fn purge(&mut self, now: Instant) {
        while let Some(entry) = self.entries.front() {
            if entry.expiry > now {
                break;
            }
            for &pos in entry.positions.iter() {
                let slot = &mut self.counters[pos];
                if *slot > 0 {
                    *slot -= 1;
                }
            }
            self.entries.pop_front();
        }
    }
    fn hash_positions(&self, key: &[u8]) -> Vec<usize> {
        let mut hasher = Hasher::new();
        hasher.update(key);
        let mut reader = hasher.finalize_xof();
        let mut buffer = [0u8; 8];
        let mut positions = Vec::with_capacity(self.hash_count as usize);
        for _ in 0..self.hash_count {
            reader.fill(&mut buffer);
            let value = u64::from_le_bytes(buffer);
            positions.push((value as usize) & self.mask);
        }
        positions
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
    entries: Mutex<HashMap<IpAddr, SlowlorisEntry>>,
    unavailable: AtomicBool,
}
impl SlowlorisDetector {
    fn new(cfg: SlowlorisConfig) -> Self {
        Self {
            cfg,
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
        let entry = guard.entry(ip).or_insert(SlowlorisEntry {
            window_start: now,
            score: 0,
        });
        let window_elapsed = now
            .checked_duration_since(entry.window_start)
            .unwrap_or_default();
        if window_elapsed >= Duration::from_secs(self.cfg.window_secs) {
            entry.window_start = now;
            entry.score = 0;
        }
        let mut penalise = matches!(event, SlowlorisEvent::Timeout);
        if let SlowlorisEvent::Success(elapsed) = event {
            let threshold = Duration::from_millis(self.cfg.max_handshake_millis);
            if elapsed >= threshold {
                penalise = true;
            }
        }
        if penalise {
            entry.score = entry.score.saturating_add(1);
        } else {
            entry.score = entry.score.saturating_sub(1);
        }
        if entry.score >= self.cfg.timeout_threshold {
            entry.score = 0;
            entry.window_start = now;
            return Some(self.penalty());
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{PuzzleConfig, ReplayFilterConfig, TokenConfig},
        metrics::TokenOutcomeKey,
    };
    use iroha_crypto::soranet::token::compute_issuer_fingerprint;
    use rand::{SeedableRng, rngs::StdRng};
    use std::{fs, net::SocketAddr, time::UNIX_EPOCH};
    use tempfile::tempdir;
    fn base_params() -> Parameters {
        Parameters::new(8, Duration::from_secs(600), Duration::from_secs(30))
    }
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
    fn remote_quota_throttle_sets_active_gauge() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            required: true,
            quotas: QuotaConfig {
                per_remote_burst: 1,
                cooldown_secs: 1,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let base = Parameters::new(6, Duration::from_secs(60), Duration::from_secs(30));
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
            None,
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
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
    fn poisoned_remote_quota_fails_closed() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            required: true,
            quotas: QuotaConfig {
                per_remote_burst: 4,
                cooldown_secs: 7,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = Arc::new(
            DoSControls::new(
                base_params(),
                &pow_cfg,
                None,
                None,
                metrics,
                RelayMode::Entry,
            )
            .expect("dos controls"),
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
            required: true,
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
            DoSControls::new(
                base_params(),
                &pow_cfg,
                None,
                None,
                metrics,
                RelayMode::Entry,
            )
            .expect("dos controls"),
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
    fn descriptor_quota_throttle_sets_active_gauge() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            required: true,
            quotas: QuotaConfig {
                per_remote_burst: 4,
                per_descriptor_burst: 1,
                per_descriptor_window_secs: 60,
                cooldown_secs: 2,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let base = Parameters::new(6, Duration::from_secs(60), Duration::from_secs(30));
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
            None,
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
        .expect("dos controls");
        let remote: SocketAddr = "127.0.0.2:2000".parse().expect("remote addr");
        let descriptor = [0xAB; 32];
        controls
            .begin(remote, Some(&descriptor))
            .expect("first attempt allowed");
        let throttle = controls
            .begin(remote, Some(&descriptor))
            .expect_err("second attempt should throttle descriptor");
        assert!(matches!(throttle.reason, ThrottleReason::DescriptorQuota));
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active_descriptor_cooldowns, 1);
    }
    #[test]
    fn poisoned_descriptor_quota_fails_closed() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            required: true,
            quotas: QuotaConfig {
                per_remote_burst: 4,
                per_descriptor_burst: 2,
                cooldown_secs: 9,
                ..QuotaConfig::default()
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = Arc::new(
            DoSControls::new(
                base_params(),
                &pow_cfg,
                None,
                None,
                metrics,
                RelayMode::Entry,
            )
            .expect("dos controls"),
        );
        let poison_target = Arc::clone(&controls);
        let poisoned = std::thread::spawn(move || {
            let _guard = poison_target
                .descriptor_limiter
                .lock()
                .expect("descriptor quota lock");
            panic!("poison descriptor quota state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        let remote: SocketAddr = "127.0.0.9:2000".parse().expect("remote addr");
        let descriptor = [0xC3; 32];
        let throttle = controls
            .begin(remote, Some(&descriptor))
            .expect_err("poisoned descriptor quota must reject admission");
        assert_eq!(throttle.reason, ThrottleReason::DescriptorQuota);
        assert_eq!(throttle.cooldown, Duration::from_secs(9));
    }
    #[test]
    fn emergency_throttle_blocks_descriptor() {
        let metrics = Arc::new(Metrics::new());
        let descriptor = [0x42u8; 32];
        let mut pow_cfg = PowConfig {
            required: true,
            emergency: Some(EmergencyThrottleConfig {
                descriptor_commit_hex: vec![hex::encode(descriptor)],
                file_path: None,
                cooldown_secs: 5,
                refresh_secs: 60,
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let base = Parameters::new(6, Duration::from_secs(60), Duration::from_secs(30));
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
            None,
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
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
            required: true,
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
            DoSControls::new(
                base_params(),
                &pow_cfg,
                None,
                None,
                Arc::clone(&metrics),
                RelayMode::Entry,
            )
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
            required: true,
            emergency: Some(EmergencyThrottleConfig {
                descriptor_commit_hex: Vec::new(),
                file_path: Some(missing_path),
                cooldown_secs: 19,
                refresh_secs: 60,
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");

        let error = match DoSControls::new(
            base_params(),
            &pow_cfg,
            None,
            None,
            Arc::new(Metrics::new()),
            RelayMode::Entry,
        ) {
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
    fn replay_filter_triggers_descriptor_throttle() {
        let metrics = Arc::new(Metrics::new());
        let descriptor = [0x24u8; 32];
        let mut pow_cfg = PowConfig {
            required: true,
            replay_filter: ReplayFilterConfig {
                enabled: true,
                bits: 512,
                hash_functions: 3,
                ttl_secs: 2,
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let filter_ttl = pow_cfg.replay_filter().ttl();
        let base = Parameters::new(6, Duration::from_secs(60), Duration::from_secs(30));
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
            None,
            Arc::clone(&metrics),
            RelayMode::Entry,
        )
        .expect("dos controls");
        let remote: SocketAddr = "127.0.0.4:4040".parse().expect("remote addr");
        controls
            .begin(remote, Some(&descriptor))
            .expect("first attempt allowed");
        let throttle = controls
            .begin(remote, Some(&descriptor))
            .expect_err("descriptor replay should be throttled");
        assert!(matches!(throttle.reason, ThrottleReason::DescriptorReplay));
        assert_eq!(throttle.cooldown, filter_ttl);
    }
    #[test]
    fn poisoned_replay_filter_returns_controlled_throttle() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            required: true,
            quotas: QuotaConfig {
                per_remote_burst: 4,
                ..QuotaConfig::default()
            },
            replay_filter: ReplayFilterConfig {
                enabled: true,
                bits: 512,
                hash_functions: 3,
                ttl_secs: 11,
            },
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("pow defaults");
        let controls = Arc::new(
            DoSControls::new(
                base_params(),
                &pow_cfg,
                None,
                None,
                metrics,
                RelayMode::Entry,
            )
            .expect("dos controls"),
        );
        let poison_target = Arc::clone(&controls);
        let poisoned = std::thread::spawn(move || {
            let filter = poison_target
                .replay_filter
                .as_ref()
                .expect("configured replay filter");
            let _guard = filter.lock().expect("replay-filter lock");
            panic!("poison replay-filter state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        let remote: SocketAddr = "127.0.0.10:4040".parse().expect("remote addr");
        let descriptor = [0xD4; 32];
        let throttle = controls
            .begin(remote, Some(&descriptor))
            .expect_err("poisoned replay filter must reject without panicking");
        assert_eq!(throttle.reason, ThrottleReason::DescriptorReplay);
        assert_eq!(throttle.cooldown, Duration::from_secs(11));
    }
    #[test]
    fn replay_filter_allows_reentry_after_ttl() {
        let ttl = Duration::from_millis(200);
        let mut filter = ReplayFilter::new(128, 3, ttl).expect("replay filter");
        let key = b"descriptor-key";
        let now = Instant::now();
        assert!(filter.observe(key, now), "first insert should pass");
        assert!(
            !filter.observe(key, now + Duration::from_millis(10)),
            "replay within TTL must be rejected"
        );
        assert!(
            filter.observe(
                key,
                now + Duration::from_millis(10) + ttl + Duration::from_millis(1)
            ),
            "entry should expire after TTL"
        );
    }
    #[test]
    fn replay_filter_constructor_rejects_overflowing_bits_without_panic() {
        match ReplayFilter::new(usize::MAX, 3, Duration::from_secs(1)) {
            Err(ConfigError::ReplayFilter(message)) => assert!(
                message.contains("bits"),
                "unexpected replay filter error: {message}"
            ),
            Err(other) => panic!("expected replay filter config error, got {other:?}"),
            Ok(_) => panic!("expected replay filter config error, got Ok(_)"),
        }
    }
    #[test]
    fn puzzle_parameters_share_static_configured_difficulty() {
        let metrics = Arc::new(Metrics::new());
        let mut pow_cfg = PowConfig {
            required: true,
            difficulty: 6,
            max_future_skew_secs: 90,
            min_ticket_ttl_secs: 30,
            puzzle: Some(PuzzleConfig {
                enabled: true,
                memory_kib: 4096,
                time_cost: 1,
                lanes: 1,
            }),
            ..PowConfig::default()
        };
        pow_cfg.apply_defaults().expect("defaults");
        let quotas = pow_cfg.quotas_for_mode(RelayMode::Entry);
        assert_eq!(quotas.per_remote_burst, pow_cfg.quotas.per_remote_burst);
        let base = Parameters::new(6, Duration::from_secs(90), Duration::from_secs(30));
        let puzzle = pow_cfg
            .puzzle_parameters(&base)
            .expect("parameters")
            .expect("enabled puzzle");
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            Some(puzzle),
            None,
            metrics,
            RelayMode::Entry,
        )
        .expect("dos controls");
        let params = controls
            .current_puzzle_parameters()
            .expect("puzzle params present");
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
            required: true,
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
        let base = Parameters::new(4, Duration::from_secs(120), Duration::from_secs(30));
        let token_policy = pow_cfg
            .token_policy()
            .expect("token policy result")
            .expect("token policy enabled");
        let metrics = Arc::new(Metrics::new());
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
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
            required: true,
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
        let base = Parameters::new(4, Duration::from_secs(120), Duration::from_secs(30));
        let token_policy = pow_cfg
            .token_policy()
            .expect("token policy result")
            .expect("token policy enabled");
        let metrics = Arc::new(Metrics::new());
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
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
            required: true,
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
        let base = Parameters::new(4, Duration::from_secs(120), Duration::from_secs(30));
        let token_policy = pow_cfg
            .token_policy()
            .expect("token policy result")
            .expect("token policy enabled");
        let metrics = Arc::new(Metrics::new());
        let controls = DoSControls::new(
            base,
            &pow_cfg,
            None,
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
