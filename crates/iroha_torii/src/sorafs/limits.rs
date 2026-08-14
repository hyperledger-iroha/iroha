//! Quota enforcement for SoraFS control-plane endpoints.
use dashmap::DashMap;
use iroha_logger::warn;
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};
const MAX_QUOTA_SUBJECTS: usize = 4_096;
/// Categories of SoraFS operations subject to quotas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SorafsAction {
    /// Capacity declaration submission.
    CapacityDeclaration,
    /// Capacity telemetry reporting.
    CapacityTelemetry,
    /// Capacity dispute submission.
    CapacityDispute,
    /// Proof-of-retrievability submissions (challenge/proof/verdict).
    PorSubmission,
}
impl fmt::Display for SorafsAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityDeclaration => write!(f, "capacity_declaration"),
            Self::CapacityTelemetry => write!(f, "capacity_telemetry"),
            Self::CapacityDispute => write!(f, "capacity_dispute"),
            Self::PorSubmission => write!(f, "por_submission"),
        }
    }
}
/// Error returned when a quota is exceeded.
#[derive(Debug, Clone, Copy)]
pub struct QuotaExceeded {
    action: SorafsAction,
    max_events: u32,
    window: Duration,
}
impl QuotaExceeded {
    fn new(action: SorafsAction, max_events: u32, window: Duration) -> Self {
        Self {
            action,
            max_events,
            window,
        }
    }
    /// Action whose quota was exceeded.
    #[must_use]
    pub fn action(&self) -> SorafsAction {
        self.action
    }
    /// Maximum events permitted within the quota window.
    #[must_use]
    pub fn max_events(&self) -> u32 {
        self.max_events
    }
    /// Duration of the enforced quota window.
    #[must_use]
    pub fn window(&self) -> Duration {
        self.window
    }
}
impl fmt::Display for QuotaExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SoraFS {} quota exceeded: maximum {} events per {:?}",
            self.action, self.max_events, self.window
        )
    }
}
/// Rolling-window quota for a single action.
struct ActionLimiter {
    window: Duration,
    max_events: u32,
    buckets: DashMap<[u8; 32], SubjectWindow>,
    bucket_admission: Mutex<()>,
}
struct SubjectWindow {
    events: VecDeque<Instant>,
    last_seen: Instant,
}
impl ActionLimiter {
    fn new(window: Duration, max_events: u32) -> Option<Self> {
        if max_events == 0 || window.is_zero() {
            return None;
        }
        Some(Self {
            window,
            max_events,
            buckets: DashMap::new(),
            bucket_admission: Mutex::new(()),
        })
    }
    fn allow(&self, subject: &[u8; 32]) -> bool {
        let now = Instant::now();
        if let Some(mut window) = self.buckets.get_mut(subject) {
            return self.consume(&mut window, now);
        }
        // Serialize only first-seen subjects so the memory ceiling remains exact under
        // concurrent requests without putting established authorities behind one lock.
        let _admission = self.bucket_admission.lock();
        if let Some(mut window) = self.buckets.get_mut(subject) {
            return self.consume(&mut window, now);
        }
        self.prune_inactive(now);
        if self.buckets.len() >= MAX_QUOTA_SUBJECTS {
            // Subjects are verified registered authorities, so admission cannot be inflated by
            // rotating a payload field. Replacing the least-recently-seen authority preserves
            // availability without allowing the accounting map to grow.
            let oldest = self
                .buckets
                .iter()
                .min_by_key(|entry| entry.value().last_seen)
                .map(|entry| *entry.key());
            if let Some(oldest) = oldest {
                self.buckets.remove(&oldest);
            }
        }
        let mut events = VecDeque::with_capacity(self.max_events.min(16) as usize);
        events.push_back(now);
        self.buckets.insert(
            *subject,
            SubjectWindow {
                events,
                last_seen: now,
            },
        );
        true
    }
    fn consume(&self, window: &mut SubjectWindow, now: Instant) -> bool {
        window.last_seen = now;
        while let Some(&front) = window.events.front() {
            if now.saturating_duration_since(front) > self.window {
                window.events.pop_front();
            } else {
                break;
            }
        }
        if window.events.len() as u32 >= self.max_events {
            return false;
        }
        window.events.push_back(now);
        true
    }
    fn prune_inactive(&self, now: Instant) {
        let inactive = self
            .buckets
            .iter()
            .filter_map(|entry| {
                entry
                    .value()
                    .events
                    .back()
                    .is_none_or(|event| now.saturating_duration_since(*event) > self.window)
                    .then_some(*entry.key())
            })
            .collect::<Vec<_>>();
        for subject in inactive {
            self.buckets.remove(&subject);
        }
    }
}
/// Quota enforcement covering SoraFS control-plane write endpoints.
#[derive(Clone, Copy, Debug)]
pub struct SorafsQuotaWindow {
    /// Maximum events permitted within the configured window. `None` disables the quota.
    pub max_events: Option<u32>,
    /// Rolling window length for quota accounting.
    pub window: Duration,
}
/// Consolidated quota configuration for all SoraFS control-plane actions.
#[derive(Clone, Copy, Debug)]
pub struct SorafsQuotaConfig {
    /// Quota applied to capacity declaration submissions.
    pub capacity_declaration: SorafsQuotaWindow,
    /// Quota applied to capacity telemetry reports.
    pub capacity_telemetry: SorafsQuotaWindow,
    /// Quota applied to capacity dispute submissions.
    pub capacity_dispute: SorafsQuotaWindow,
    /// Quota applied to proof-of-retrievability submissions.
    pub por_submission: SorafsQuotaWindow,
}
impl SorafsQuotaConfig {
    /// Configuration with quota enforcement disabled.
    #[must_use]
    pub fn unlimited() -> Self {
        let window = Duration::from_secs(1);
        Self {
            capacity_declaration: SorafsQuotaWindow {
                max_events: None,
                window,
            },
            capacity_telemetry: SorafsQuotaWindow {
                max_events: None,
                window,
            },
            capacity_dispute: SorafsQuotaWindow {
                max_events: None,
                window,
            },
            por_submission: SorafsQuotaWindow {
                max_events: None,
                window,
            },
        }
    }
}
impl Default for SorafsQuotaConfig {
    fn default() -> Self {
        const HOUR: Duration = Duration::from_hours(1);
        const DAY: Duration = Duration::from_hours(24);
        Self {
            capacity_declaration: SorafsQuotaWindow {
                max_events: Some(4),
                window: HOUR,
            },
            capacity_telemetry: SorafsQuotaWindow {
                max_events: Some(12),
                window: HOUR,
            },
            capacity_dispute: SorafsQuotaWindow {
                max_events: Some(2),
                window: DAY,
            },
            por_submission: SorafsQuotaWindow {
                max_events: Some(60),
                window: HOUR,
            },
        }
    }
}
/// Quota enforcement covering SoraFS control-plane write endpoints.
#[derive(Clone)]
pub struct SorafsQuotaEnforcer {
    declaration: Option<Arc<ActionLimiter>>,
    telemetry: Option<Arc<ActionLimiter>>,
    dispute: Option<Arc<ActionLimiter>>,
    por: Option<Arc<ActionLimiter>>,
}
impl SorafsQuotaEnforcer {
    /// Construct an enforcer with conservative defaults.
    ///
    /// Defaults are intentionally biased toward preventing abuse while remaining permissive
    /// for production workloads. Real deployments override these values via
    /// `torii.sorafs.quota` in `iroha_config`.
    #[must_use]
    pub fn new_default() -> Self {
        Self::from_config(&SorafsQuotaConfig::default())
    }
    /// Construct an enforcer from configuration supplied by Torii.
    #[must_use]
    pub fn from_config(config: &SorafsQuotaConfig) -> Self {
        Self {
            declaration: limiter_from_window(config.capacity_declaration),
            telemetry: limiter_from_window(config.capacity_telemetry),
            dispute: limiter_from_window(config.capacity_dispute),
            por: limiter_from_window(config.por_submission),
        }
    }
    /// Construct an enforcer with all quotas disabled (tests).
    #[must_use]
    pub fn unlimited() -> Self {
        Self::from_config(&SorafsQuotaConfig::unlimited())
    }
    /// Attempt to consume a quota unit for the specified action and authenticated subject.
    ///
    /// Returns `Ok(())` when the request is permitted, or [`QuotaExceeded`] when throttled.
    ///
    /// # Errors
    ///
    /// Returns [`QuotaExceeded`] when the relevant limiter rejects the action.
    pub fn enforce(&self, action: SorafsAction, subject: &[u8; 32]) -> Result<(), QuotaExceeded> {
        let Some(limiter) = self.limiter(action) else {
            return Ok(());
        };
        if limiter.allow(subject) {
            Ok(())
        } else {
            warn!(
                action = %action,
                quota_subject = %hex::encode(subject),
                "SoraFS quota exceeded"
            );
            Err(QuotaExceeded::new(
                action,
                limiter.max_events,
                limiter.window,
            ))
        }
    }
    fn limiter(&self, action: SorafsAction) -> Option<&Arc<ActionLimiter>> {
        match action {
            SorafsAction::CapacityDeclaration => self.declaration.as_ref(),
            SorafsAction::CapacityTelemetry => self.telemetry.as_ref(),
            SorafsAction::CapacityDispute => self.dispute.as_ref(),
            SorafsAction::PorSubmission => self.por.as_ref(),
        }
    }
}
fn limiter_from_window(window: SorafsQuotaWindow) -> Option<Arc<ActionLimiter>> {
    window
        .max_events
        .and_then(|max| ActionLimiter::new(window.window, max))
        .map(Arc::new)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    #[test]
    fn quota_allows_within_window() {
        let enforcer = SorafsQuotaEnforcer {
            declaration: ActionLimiter::new(Duration::from_mins(1), 2).map(Arc::new),
            telemetry: None,
            dispute: None,
            por: None,
        };
        let provider = [1u8; 32];
        assert!(
            enforcer
                .enforce(SorafsAction::CapacityDeclaration, &provider)
                .is_ok()
        );
        assert!(
            enforcer
                .enforce(SorafsAction::CapacityDeclaration, &provider)
                .is_ok()
        );
        assert!(
            enforcer
                .enforce(SorafsAction::CapacityDeclaration, &provider)
                .is_err()
        );
    }
    #[test]
    fn quota_resets_after_window() {
        let limiter = ActionLimiter::new(Duration::from_millis(50), 1).unwrap();
        let provider = [2u8; 32];
        assert!(limiter.allow(&provider));
        assert!(!limiter.allow(&provider));
        thread::sleep(Duration::from_millis(60));
        assert!(limiter.allow(&provider));
    }
    #[test]
    fn unlimited_enforcer_never_blocks() {
        let enforcer = SorafsQuotaEnforcer::unlimited();
        let provider = [3u8; 32];
        for _ in 0..100 {
            assert!(
                enforcer
                    .enforce(SorafsAction::PorSubmission, &provider)
                    .is_ok()
            );
        }
    }
    #[test]
    fn quota_exceeded_display_includes_limit_context() {
        let error = QuotaExceeded::new(SorafsAction::PorSubmission, 17, Duration::from_secs(90));
        assert_eq!(
            error.to_string(),
            "SoraFS por_submission quota exceeded: maximum 17 events per 90s"
        );
    }
    #[test]
    fn quota_subject_state_is_hard_bounded() {
        let limiter = ActionLimiter::new(Duration::from_mins(1), 1).unwrap();
        for index in 0..MAX_QUOTA_SUBJECTS + 128 {
            let mut subject = [0_u8; 32];
            subject[..8].copy_from_slice(&(index as u64).to_be_bytes());
            assert!(limiter.allow(&subject));
        }
        assert_eq!(limiter.buckets.len(), MAX_QUOTA_SUBJECTS);
    }
}
