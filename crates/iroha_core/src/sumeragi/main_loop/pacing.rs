//! Pacemaker/backpressure and adaptive observability helpers.

use std::time::{Duration, Instant};

use iroha_config::parameters::actual::AdaptiveObservability;
use tokio::sync::watch;

use super::{
    PROPOSE_ATTEMPT_LOG_COOLDOWN, TICK_COST_LOG_THRESHOLD, TICK_LAG_LOG_THRESHOLD,
    TICK_TIMING_LOG_COOLDOWN,
};
use crate::{queue::BackpressureState, sumeragi::status};

/// Track queue backpressure so proposal assembly can be deferred under saturation.
pub(super) struct BackpressureGate {
    rx: watch::Receiver<BackpressureState>,
    current: BackpressureState,
}

impl BackpressureGate {
    pub(super) fn new(rx: watch::Receiver<BackpressureState>) -> Self {
        let current = *rx.borrow();
        Self { rx, current }
    }

    pub(super) fn refresh(&mut self) -> bool {
        let snapshot = *self.rx.borrow();
        if snapshot == self.current {
            false
        } else {
            self.current = snapshot;
            true
        }
    }

    pub(super) fn state(&self) -> BackpressureState {
        self.current
    }

    pub(super) fn should_defer(&mut self) -> bool {
        self.refresh();
        self.current.is_saturated()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PacemakerBackpressureAction {
    /// Entered a saturated state for the first time; record deferral and skip proposals.
    First,
    /// Still saturated; continue deferring without recording a new deferral.
    Subsequent,
    /// Not saturated; pacemaker may proceed.
    None,
}

pub(super) struct PacemakerBackpressure {
    saturated: bool,
}

impl PacemakerBackpressure {
    pub(super) fn new() -> Self {
        Self { saturated: false }
    }

    pub(super) fn update(&mut self, saturated: bool) -> PacemakerBackpressureAction {
        match (self.saturated, saturated) {
            (false, true) => {
                self.saturated = true;
                PacemakerBackpressureAction::First
            }
            (true, true) => PacemakerBackpressureAction::Subsequent,
            (true, false) => {
                self.saturated = false;
                PacemakerBackpressureAction::None
            }
            (false, false) => PacemakerBackpressureAction::None,
        }
    }
}

pub(super) struct Pacemaker {
    pub(super) propose_interval: Duration,
    pub(super) next_deadline: Instant,
}

impl Pacemaker {
    pub(super) fn new(interval: Duration, now: Instant) -> Self {
        Self {
            propose_interval: interval,
            next_deadline: now + interval,
        }
    }

    #[cfg(test)]
    pub(super) fn with_interval(interval: Duration, now: Instant) -> Self {
        Self {
            propose_interval: interval,
            next_deadline: now + interval,
        }
    }

    pub(super) fn set_interval(&mut self, interval: Duration, now: Instant) {
        self.propose_interval = interval;
        self.next_deadline = now + interval;
    }

    pub(super) fn should_fire(&mut self, now: Instant) -> bool {
        if now < self.next_deadline {
            false
        } else {
            self.next_deadline = now + self.propose_interval;
            true
        }
    }

    #[cfg(test)]
    pub(super) fn deadline(&self) -> Instant {
        self.next_deadline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TickTimingReport {
    pub(super) since_last_tick: Duration,
    pub(super) tick_cost: Duration,
    pub(super) log_gap: bool,
    pub(super) log_cost: bool,
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)] // Fields intentionally share prefix for clarity.
pub(super) struct TickTimingMonitor {
    last_tick_start: Instant,
    last_gap_log: Option<Instant>,
    last_cost_log: Option<Instant>,
}

impl TickTimingMonitor {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            last_tick_start: now,
            last_gap_log: None,
            last_cost_log: None,
        }
    }

    pub(super) fn observe(&mut self, tick_start: Instant, tick_cost: Duration) -> TickTimingReport {
        let since_last_tick = tick_start.saturating_duration_since(self.last_tick_start);
        self.last_tick_start = tick_start;

        let log_gap = since_last_tick >= TICK_LAG_LOG_THRESHOLD
            && Self::should_log(tick_start, &mut self.last_gap_log);
        let log_cost = tick_cost >= TICK_COST_LOG_THRESHOLD
            && Self::should_log(tick_start, &mut self.last_cost_log);

        TickTimingReport {
            since_last_tick,
            tick_cost,
            log_gap,
            log_cost,
        }
    }

    fn should_log(now: Instant, last_log: &mut Option<Instant>) -> bool {
        if let Some(prev) = *last_log {
            if now.saturating_duration_since(prev) < TICK_TIMING_LOG_COOLDOWN {
                return false;
            }
        }
        *last_log = Some(now);
        true
    }
}

#[derive(Debug)]
pub(super) struct ProposeAttemptMonitor {
    last_log: Option<Instant>,
}

impl ProposeAttemptMonitor {
    pub(super) fn new() -> Self {
        Self { last_log: None }
    }

    pub(super) fn should_log(&mut self, now: Instant) -> bool {
        if let Some(prev) = self.last_log {
            if now.saturating_duration_since(prev) < PROPOSE_ATTEMPT_LOG_COOLDOWN {
                return false;
            }
        }
        self.last_log = Some(now);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AdaptiveAction {
    Applied,
    Reset,
    None,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct AdaptiveObservabilityMetrics {
    pub(super) missing_local_data_total: u64,
    pub(super) max_qc_latency_ms: u64,
}

impl AdaptiveObservabilityMetrics {
    pub(super) fn gather() -> Self {
        let max_qc_latency_ms = status::qc_latency_snapshot()
            .into_iter()
            .map(|(_, latency_ms)| latency_ms)
            .max()
            .unwrap_or(0);
        Self {
            missing_local_data_total: status::da_gate_missing_local_data_total(),
            max_qc_latency_ms,
        }
    }
}

#[derive(Debug)]
pub(super) struct AdaptiveObservabilityState {
    base_propose_interval: Duration,
    base_collector_limit: u8,
    last_missing_local_data_total: u64,
    last_trigger: Option<Instant>,
    applied: bool,
}

impl AdaptiveObservabilityState {
    pub(super) fn new(
        cfg: AdaptiveObservability,
        base_propose_interval: Duration,
        base_collector_limit: u8,
        initial_missing_local_data_total: u64,
    ) -> Self {
        let mut state = Self {
            base_propose_interval,
            base_collector_limit: base_collector_limit.max(1),
            last_missing_local_data_total: initial_missing_local_data_total,
            last_trigger: None,
            applied: false,
        };
        // Respect the configured baseline collector fan-out even when the adaptive feature is off.
        if !cfg.enabled {
            state.applied = false;
        }
        state
    }

    pub(super) fn update_base_collector_limit(&mut self, limit: u8) {
        self.base_collector_limit = limit.max(1);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate(
        &mut self,
        cfg: AdaptiveObservability,
        metrics: AdaptiveObservabilityMetrics,
        pacemaker: &mut Pacemaker,
        collector_redundant_limit: &mut u8,
        now: Instant,
    ) -> AdaptiveAction {
        let missing_delta = metrics
            .missing_local_data_total
            .saturating_sub(self.last_missing_local_data_total);
        self.last_missing_local_data_total = metrics.missing_local_data_total;

        if !cfg.enabled {
            return self.reset(pacemaker, collector_redundant_limit, now);
        }

        let qc_alert = metrics.max_qc_latency_ms >= cfg.qc_latency_alert_ms;
        let missing_data_alert = missing_delta >= cfg.da_reschedule_burst;
        let cooldown = Duration::from_millis(cfg.cooldown_ms);
        let past_cooldown = self
            .last_trigger
            .is_none_or(|last| now.saturating_duration_since(last) >= cooldown);

        if (qc_alert || missing_data_alert) && past_cooldown {
            *collector_redundant_limit = (*collector_redundant_limit)
                .max(cfg.collector_redundant_r.max(self.base_collector_limit));
            let boosted = self
                .base_propose_interval
                .saturating_add(Duration::from_millis(cfg.pacemaker_extra_ms));
            pacemaker.set_interval(boosted, now);
            self.applied = true;
            self.last_trigger = Some(now);
            return AdaptiveAction::Applied;
        }

        if self.applied && past_cooldown && !qc_alert && missing_delta == 0 {
            pacemaker.set_interval(self.base_propose_interval, now);
            *collector_redundant_limit = self.base_collector_limit;
            self.applied = false;
            self.last_trigger = None;
            return AdaptiveAction::Reset;
        }

        AdaptiveAction::None
    }

    fn reset(
        &mut self,
        pacemaker: &mut Pacemaker,
        collector_redundant_limit: &mut u8,
        now: Instant,
    ) -> AdaptiveAction {
        if !self.applied {
            return AdaptiveAction::None;
        }
        pacemaker.set_interval(self.base_propose_interval, now);
        *collector_redundant_limit = self.base_collector_limit;
        self.applied = false;
        self.last_trigger = None;
        AdaptiveAction::Reset
    }
}
