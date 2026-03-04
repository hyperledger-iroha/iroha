//! DoS and abuse mitigation utilities for the relay handshake path.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt, fs,
    hash::Hash,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use blake3::Hasher;
use hex;
use iroha_crypto::soranet::{
    pow::Parameters,
    puzzle,
    token::{AdmissionToken, AdmissionTokenVerifier, VerifyError as TokenVerifyError},
};
use norito::{derive::JsonDeserialize, json};
use thiserror::Error;
use tracing::warn;

use crate::{
    capability,
    config::{
        AdaptiveDifficultyConfig, EmergencyThrottleConfig, PowConfig, QuotaConfig, RelayMode,
        SlowlorisConfig, TokenPolicySource,
    },
    metrics::Metrics,
};

/// Aggregated controls applied to inbound handshakes.
pub struct DoSControls {
    adaptive: AdaptiveDifficulty,
    remote_limiter: Mutex<RateLimiter<IpAddr>>,
    descriptor_limiter: Mutex<Option<RateLimiter<[u8; 16]>>>,
    slowloris: SlowlorisDetector,
    require_pow: bool,
    puzzle: Option<PuzzlePolicy>,
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
    ) -> Self {
        let mut adaptive_cfg = config.adaptive.clone();
        adaptive_cfg.apply_defaults();

        let quotas_cfg = config.quotas_for_mode(mode);

        let mut slowloris_cfg = config.slowloris.clone();
        slowloris_cfg.apply_defaults();

        let adaptive = AdaptiveDifficulty::new(base_params, adaptive_cfg, Arc::clone(&metrics));

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

        metrics.set_pow_difficulty(adaptive.current_difficulty());
        metrics.set_active_remote_cooldowns(0);
        if descriptor_limits.is_none() {
            metrics.set_active_descriptor_cooldowns(0);
        }

        let puzzle_policy = puzzle.map(PuzzlePolicy::new);
        let replay_filter = if config.replay_filter().is_enabled() {
            Some(Mutex::new(ReplayFilter::new(
                config.replay_filter().bits_usize(),
                config.replay_filter().hash_count(),
                config.replay_filter().ttl(),
            )))
        } else {
            None
        };
        let emergency = config
            .emergency_throttle()
            .map(|cfg| EmergencyThrottle::new(cfg.clone()));

        Self {
            adaptive,
            remote_limiter,
            descriptor_limiter,
            slowloris: SlowlorisDetector::new(slowloris_cfg),
            require_pow: config.required,
            puzzle: puzzle_policy,
            token: token.map(TokenPolicy::from_source),
            replay_filter,
            metrics,
            remote_limits,
            descriptor_limits,
            emergency,
        }
    }

    /// Returns the current PoW parameters (possibly adapted).
    pub fn current_pow_parameters(&self) -> Parameters {
        self.adaptive.current_parameters()
    }

    /// Returns the current puzzle parameters if a puzzle policy is enabled.
    pub fn current_puzzle_parameters(&self) -> Option<puzzle::Parameters> {
        let policy = self.puzzle.as_ref()?;
        Some(policy.current_parameters(self.adaptive.current_difficulty()))
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

        if let (Some(emergency), Some(commit_bytes)) = (&self.emergency, descriptor_commit)
            && let Some(duration) = emergency.should_throttle(commit_bytes)
        {
            self.metrics.record_emergency_throttle();
            return Err(Throttle {
                cooldown: duration,
                reason: ThrottleReason::Emergency,
            });
        }

        if let Err(cooldown) = self.check_remote_limit(ip, now) {
            return Err(Throttle {
                cooldown,
                reason: ThrottleReason::RemoteQuota,
            });
        }

        if let (Some(filter), Some(commit_bytes)) = (self.replay_filter.as_ref(), descriptor_commit)
        {
            let mut guard = filter.lock().expect("replay filter mutex poisoned");
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
        self.adaptive.observe_success(now);
        self.observe_slowloris_success_at(attempt.remote, elapsed, now);
    }

    /// Records a PoW verification failure.
    pub fn record_pow_failure(&self, attempt: &AttemptContext, elapsed: Duration) {
        self.record_pow_failure_at(attempt, elapsed, Instant::now());
    }

    /// Same as [`Self::record_pow_failure`] but accepts an explicit timestamp.
    pub fn record_pow_failure_at(&self, attempt: &AttemptContext, elapsed: Duration, now: Instant) {
        self.adaptive.observe_pow_failure(now);
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
        if let Ok(mut limiter) = self.remote_limiter.lock() {
            limiter.impose_cooldown(ip, now, cooldown);
            let count = limiter.cooldown_count(now);
            self.metrics.set_active_remote_cooldowns(count);
        }
    }

    fn check_remote_limit(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        if let Ok(mut limiter) = self.remote_limiter.lock() {
            let result = limiter.check(ip, now);
            let count = limiter.cooldown_count(now);
            self.metrics.set_active_remote_cooldowns(count);
            result
        } else {
            Ok(())
        }
    }

    fn check_descriptor_limit(&self, key: [u8; 16], now: Instant) -> Result<(), Duration> {
        if let Ok(mut guard) = self.descriptor_limiter.lock() {
            if let Some(limiter) = guard.as_mut() {
                let result = limiter.check(key, now);
                let count = limiter.cooldown_count(now);
                self.metrics.set_active_descriptor_cooldowns(count);
                result
            } else {
                self.metrics.set_active_descriptor_cooldowns(0);
                Ok(())
            }
        } else {
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

/// Adaptive PoW difficulty controller.
struct AdaptiveDifficulty {
    enabled: bool,
    base: Parameters,
    current: AtomicU8,
    cfg: AdaptiveDifficultyConfig,
    window: Mutex<WindowStats>,
    metrics: Arc<Metrics>,
}

/// Puzzle parameters tied to the current difficulty.
struct PuzzlePolicy {
    base: puzzle::Parameters,
}

impl PuzzlePolicy {
    fn new(base: puzzle::Parameters) -> Self {
        Self { base }
    }

    fn current_parameters(&self, difficulty: u8) -> puzzle::Parameters {
        self.base.with_difficulty(difficulty)
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
            TokenVerifyError::Clock(_) => "store_error",
            TokenVerifyError::Signature(_) => "signature_invalid",
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
}

struct EmergencyThrottleState {
    last_loaded: Instant,
    dynamic_descriptors: HashSet<[u8; 16]>,
}

impl EmergencyThrottle {
    fn new(config: EmergencyThrottleConfig) -> Self {
        let static_descriptors = Self::decode_descriptor_list(&config.descriptor_commit_hex);
        let refresh = Duration::from_secs(config.refresh_secs);
        let mut cooldown_secs = config.cooldown_secs.max(1);

        let (dynamic_descriptors, override_secs) = match &config.file_path {
            Some(path) => match Self::load_document(path) {
                Ok(doc) => (doc.descriptors, doc.cooldown_override_secs),
                Err(err) => {
                    warn!(path = %path.display(), %err, "failed to load emergency throttle document");
                    (HashSet::new(), None)
                }
            },
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

        Self {
            cooldown_millis: AtomicU64::new(cooldown_millis),
            refresh,
            file_path: config.file_path.clone(),
            static_descriptors,
            state: RwLock::new(state),
        }
    }

    fn should_throttle(&self, descriptor_commit: &[u8]) -> Option<Duration> {
        let key = descriptor_key(descriptor_commit)?;

        if self.static_descriptors.contains(&key) {
            return Some(self.cooldown_duration());
        }

        if self.file_path.is_none() {
            let state = self.state.read().expect("emergency state poisoned");
            if state.dynamic_descriptors.contains(&key) {
                return Some(self.cooldown_duration());
            }
            return None;
        }

        let needs_reload = {
            let state = self.state.read().expect("emergency state poisoned");
            if state.dynamic_descriptors.contains(&key) {
                return Some(self.cooldown_duration());
            }
            state.last_loaded.elapsed() >= self.refresh
        };

        if !needs_reload {
            return None;
        }

        let mut guard = self.state.write().expect("emergency state poisoned");
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

    fn decode_descriptor_list(hex_values: &[String]) -> HashSet<[u8; 16]> {
        let mut set = HashSet::with_capacity(hex_values.len());
        for value in hex_values {
            if let Some(key) = Self::decode_descriptor(value) {
                set.insert(key);
            }
        }
        set
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

    fn load_document(path: &PathBuf) -> Result<LoadedEmergencyThrottle, String> {
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read emergency throttle file {path:?}: {err}"))?;
        let document: EmergencyThrottleDocument = json::from_slice(&bytes)
            .map_err(|err| format!("failed to parse emergency throttle file {path:?}: {err}"))?;
        let descriptors = Self::decode_descriptor_list(&document.descriptor_commit_hex);
        Ok(LoadedEmergencyThrottle {
            descriptors,
            cooldown_override_secs: document.cooldown_secs,
        })
    }
}

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

struct WindowStats {
    start: Instant,
    successes: u32,
    pow_failures: u32,
}

impl AdaptiveDifficulty {
    fn new(base: Parameters, mut cfg: AdaptiveDifficultyConfig, metrics: Arc<Metrics>) -> Self {
        let base_diff = base.difficulty();

        if base_diff == 0 {
            cfg.min_difficulty = 0;
        }
        if cfg.max_difficulty < cfg.min_difficulty {
            cfg.max_difficulty = cfg.min_difficulty;
        }
        if cfg.max_difficulty == 0 {
            cfg.enabled = false;
        }

        let current = if cfg.enabled {
            base_diff.clamp(cfg.min_difficulty, cfg.max_difficulty)
        } else {
            base_diff
        };

        metrics.set_pow_difficulty(current);

        Self {
            enabled: cfg.enabled,
            base,
            current: AtomicU8::new(current),
            cfg,
            window: Mutex::new(WindowStats {
                start: Instant::now(),
                successes: 0,
                pow_failures: 0,
            }),
            metrics,
        }
    }

    fn current_parameters(&self) -> Parameters {
        let difficulty = if self.enabled {
            self.current.load(Ordering::Relaxed)
        } else {
            self.base.difficulty()
        };
        self.base.with_difficulty(difficulty)
    }

    fn current_difficulty(&self) -> u8 {
        if self.enabled {
            self.current.load(Ordering::Relaxed)
        } else {
            self.base.difficulty()
        }
    }

    fn observe_success(&self, now: Instant) {
        if !self.enabled {
            return;
        }
        let mut guard = self.window.lock().expect("adaptive window mutex poisoned");
        guard.successes = guard.successes.strict_add(1);
        Self::maybe_adjust(&self.cfg, &self.metrics, &self.current, &mut guard, now);
    }

    fn observe_pow_failure(&self, now: Instant) {
        if !self.enabled {
            return;
        }
        let mut guard = self.window.lock().expect("adaptive window mutex poisoned");
        guard.pow_failures = guard.pow_failures.strict_add(1);
        Self::maybe_adjust(&self.cfg, &self.metrics, &self.current, &mut guard, now);
    }

    fn maybe_adjust(
        cfg: &AdaptiveDifficultyConfig,
        metrics: &Metrics,
        current: &AtomicU8,
        stats: &mut WindowStats,
        now: Instant,
    ) {
        let elapsed = now.checked_duration_since(stats.start).unwrap_or_default();
        if elapsed < Duration::from_secs(cfg.window_secs) {
            return;
        }

        let mut updated = current.load(Ordering::Relaxed);
        if stats.pow_failures >= cfg.pow_failure_threshold && updated < cfg.max_difficulty {
            updated = updated.strict_add(cfg.increase_step);
            if updated > cfg.max_difficulty {
                updated = cfg.max_difficulty;
            }
        } else if stats.pow_failures == 0
            && stats.successes >= cfg.success_threshold
            && updated > cfg.min_difficulty
        {
            updated = updated.strict_sub(cfg.decrease_step);
            if updated < cfg.min_difficulty {
                updated = cfg.min_difficulty;
            }
        }

        stats.start = now;
        stats.successes = 0;
        stats.pow_failures = 0;

        if updated != current.load(Ordering::Relaxed) {
            current.store(updated, Ordering::Relaxed);
            metrics.set_pow_difficulty(updated);
        }
    }
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
    K: Eq + Hash + Clone,
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
        let entry = self.entries.entry(key.clone()).or_insert(RateEntry {
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
        if self.entries.len() > self.params.max_entries {
            let overflow = self.entries.len() - self.params.max_entries;
            let mut removed = 0usize;
            let keys: Vec<_> = self.entries.keys().cloned().collect();
            for key in keys {
                self.entries.remove(&key);
                removed += 1;
                if removed >= overflow {
                    break;
                }
            }
        }
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
    fn new(bits: usize, hash_count: u8, ttl: Duration) -> Self {
        let size = bits.max(64).next_power_of_two();
        let mask = size - 1;
        debug_assert!(hash_count > 0);
        Self {
            mask,
            hash_count,
            ttl,
            counters: vec![0u16; size],
            entries: VecDeque::new(),
        }
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
}

impl SlowlorisDetector {
    fn new(cfg: SlowlorisConfig) -> Self {
        Self {
            cfg,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn observe(&self, ip: IpAddr, event: SlowlorisEvent, now: Instant) -> Option<Duration> {
        if !self.cfg.enabled {
            return None;
        }

        let mut guard = self.entries.lock().expect("slowloris mutex poisoned");
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
            return Some(Duration::from_secs(self.cfg.penalty_secs));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddr,
        thread,
        time::{Duration as StdDuration, UNIX_EPOCH},
    };

    use iroha_crypto::soranet::token::compute_issuer_fingerprint;
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    use crate::{
        config::{PuzzleConfig, ReplayFilterConfig, TokenConfig},
        metrics::TokenOutcomeKey,
    };

    fn base_params() -> Parameters {
        Parameters::new(8, Duration::from_secs(600), Duration::from_secs(30))
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
    fn adaptive_difficulty_respects_thresholds() {
        let metrics = Arc::new(Metrics::new());
        let cfg = AdaptiveDifficultyConfig {
            min_difficulty: 4,
            max_difficulty: 10,
            pow_failure_threshold: 2,
            success_threshold: 2,
            window_secs: 1,
            ..AdaptiveDifficultyConfig::default()
        };
        let adaptive = AdaptiveDifficulty::new(base_params(), cfg, metrics.clone());
        let now = Instant::now();
        adaptive.observe_pow_failure(now);
        adaptive.observe_pow_failure(now);
        thread::sleep(StdDuration::from_millis(1100));
        adaptive.observe_pow_failure(Instant::now());
        let diff = adaptive.current_difficulty();
        assert!(diff >= 8);
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
        );
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
        );
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
        );
        let remote: SocketAddr = "127.0.0.3:3030".parse().expect("remote addr");

        let throttle = controls
            .begin(remote, Some(&descriptor))
            .expect_err("descriptor should be blocked by emergency throttle");
        assert!(matches!(throttle.reason, ThrottleReason::Emergency));

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.throttled_emergency, 1);
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
        );
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
    fn replay_filter_allows_reentry_after_ttl() {
        let ttl = Duration::from_millis(200);
        let mut filter = ReplayFilter::new(128, 3, ttl);
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
    fn puzzle_parameters_follow_current_difficulty() {
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
        );
        let params = controls
            .current_puzzle_parameters()
            .expect("puzzle params present");
        assert_eq!(params.difficulty(), 6);
        assert_eq!(params.memory_kib().get(), 4096);
    }

    #[test]
    fn token_policy_verifies_valid_token() {
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let issuer_hex = hex::encode(keypair.public_key());
        let mut pow_cfg = PowConfig {
            required: true,
            token: Some(TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(issuer_hex),
                max_ttl_secs: 600,
                clock_skew_secs: 5,
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
        );

        let relay_id = [0xAB; 32];
        let transcript_hash = [0xCD; 32];
        let issued = UNIX_EPOCH + Duration::from_secs(1_000_000);
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
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let issuer_hex = hex::encode(keypair.public_key());
        let relay_id = [0x44; 32];
        let transcript_hash = [0x11; 32];
        let issued = UNIX_EPOCH + Duration::from_secs(2_000_000);
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

        let mut pow_cfg = PowConfig {
            required: true,
            token: Some(TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(issuer_hex),
                max_ttl_secs: 600,
                clock_skew_secs: 5,
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
        );

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
    fn token_outcome_metrics_recorded() {
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let issuer_hex = hex::encode(keypair.public_key());
        let relay_id = [0x55; 32];
        let transcript_hash = [0x66; 32];
        let issued = UNIX_EPOCH + Duration::from_secs(2_000_000);
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

        let mut pow_cfg = PowConfig {
            required: true,
            token: Some(TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(issuer_hex),
                max_ttl_secs: 600,
                clock_skew_secs: 5,
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
        );

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
