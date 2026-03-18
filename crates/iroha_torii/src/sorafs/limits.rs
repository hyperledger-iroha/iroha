//! Quota enforcement for SoraFS control-plane endpoints.

use std::{
    collections::VecDeque,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::{DashMap, mapref::entry::Entry};
use iroha_logger::warn;

/// Categories of SoraFS operations subject to quotas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SorafsAction {
    /// Capacity declaration submission.
    CapacityDeclaration,
    /// Capacity telemetry reporting.
    CapacityTelemetry,
    /// Deal usage/settlement telemetry reporting.
    DealTelemetry,
    /// Capacity dispute submission.
    CapacityDispute,
    /// Manifest pin submission.
    StoragePin,
    /// Proof-of-retrievability submissions (challenge/proof/verdict).
    PorSubmission,
}

impl fmt::Display for SorafsAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityDeclaration => write!(f, "capacity_declaration"),
            Self::CapacityTelemetry => write!(f, "capacity_telemetry"),
            Self::DealTelemetry => write!(f, "deal_telemetry"),
            Self::CapacityDispute => write!(f, "capacity_dispute"),
            Self::StoragePin => write!(f, "storage_pin"),
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

/// Rolling-window quota for a single action.
struct ActionLimiter {
    window: Duration,
    max_events: u32,
    buckets: DashMap<[u8; 32], VecDeque<Instant>>,
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
        })
    }

    fn allow(&self, provider: &[u8; 32]) -> bool {
        let now = Instant::now();
        match self.buckets.entry(*provider) {
            Entry::Occupied(mut entry) => {
                let deque = entry.get_mut();
                while let Some(&front) = deque.front() {
                    if now.duration_since(front) > self.window {
                        deque.pop_front();
                    } else {
                        break;
                    }
                }
                if deque.len() as u32 >= self.max_events {
                    return false;
                }
                deque.push_back(now);
            }
            Entry::Vacant(entry) => {
                let mut deque = VecDeque::with_capacity(self.max_events.min(16) as usize);
                deque.push_back(now);
                entry.insert(deque);
            }
        }
        true
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
    /// Quota applied to deal telemetry (usage + settlement) submissions.
    pub deal_telemetry: SorafsQuotaWindow,
    /// Quota applied to capacity dispute submissions.
    pub capacity_dispute: SorafsQuotaWindow,
    /// Quota applied to manifest pin submissions.
    pub storage_pin: SorafsQuotaWindow,
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
            deal_telemetry: SorafsQuotaWindow {
                max_events: None,
                window,
            },
            capacity_dispute: SorafsQuotaWindow {
                max_events: None,
                window,
            },
            storage_pin: SorafsQuotaWindow {
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
            deal_telemetry: SorafsQuotaWindow {
                max_events: Some(60),
                window: HOUR,
            },
            capacity_dispute: SorafsQuotaWindow {
                max_events: Some(2),
                window: DAY,
            },
            storage_pin: SorafsQuotaWindow {
                max_events: Some(4),
                window: HOUR,
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
    deal: Option<Arc<ActionLimiter>>,
    dispute: Option<Arc<ActionLimiter>>,
    storage_pin: Option<Arc<ActionLimiter>>,
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
            deal: limiter_from_window(config.deal_telemetry),
            dispute: limiter_from_window(config.capacity_dispute),
            storage_pin: limiter_from_window(config.storage_pin),
            por: limiter_from_window(config.por_submission),
        }
    }

    /// Construct an enforcer with all quotas disabled (tests).
    #[must_use]
    pub fn unlimited() -> Self {
        Self::from_config(&SorafsQuotaConfig::unlimited())
    }

    /// Attempt to consume a quota unit for the specified action/provider.
    ///
    /// Returns `Ok(())` when the request is permitted, or [`QuotaExceeded`] when throttled.
    ///
    /// # Errors
    ///
    /// Returns [`QuotaExceeded`] when the relevant limiter rejects the action.
    pub fn enforce(&self, action: SorafsAction, provider: &[u8; 32]) -> Result<(), QuotaExceeded> {
        let Some(limiter) = self.limiter(action) else {
            return Ok(());
        };
        if limiter.allow(provider) {
            Ok(())
        } else {
            warn!(
                action = %action,
                provider_id = %hex::encode(provider),
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
            SorafsAction::DealTelemetry => self.deal.as_ref(),
            SorafsAction::CapacityDispute => self.dispute.as_ref(),
            SorafsAction::StoragePin => self.storage_pin.as_ref(),
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
    use std::thread;

    use super::*;

    #[test]
    fn quota_allows_within_window() {
        let enforcer = SorafsQuotaEnforcer {
            declaration: ActionLimiter::new(Duration::from_mins(1), 2).map(Arc::new),
            telemetry: None,
            deal: None,
            dispute: None,
            storage_pin: None,
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
    fn storage_pin_quota_blocks_after_limit() {
        let enforcer = SorafsQuotaEnforcer {
            declaration: None,
            telemetry: None,
            deal: None,
            dispute: None,
            storage_pin: ActionLimiter::new(Duration::from_millis(10), 1).map(Arc::new),
            por: None,
        };
        let provider = [9u8; 32];
        assert!(
            enforcer
                .enforce(SorafsAction::StoragePin, &provider)
                .is_ok()
        );
        assert!(matches!(
            enforcer
                .enforce(SorafsAction::StoragePin, &provider)
                .unwrap_err()
                .action(),
            SorafsAction::StoragePin
        ));
    }
}
