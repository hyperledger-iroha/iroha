//! Capacity metering helpers for the embedded SoraFS node.
//!
//! This module provides a lightweight accumulator that tracks replication
//! activity for the currently active provider declaration. The intent is to
//! surface deterministic counters (GiB reserved, GiB released, order counts,
//! uptime/PoR placeholders) that higher layers can translate into Norito
//! telemetry payloads and fee accrual reports.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::capacity::{DeclarationWindow, ReplicationPlan, ReplicationRelease};

/// Outstanding replication order tracked for utilisation accounting.
#[derive(Debug, Clone, Copy)]
struct OutstandingAllocation {
    slice_gib: u64,
    issued_at: u64,
}

/// Usage sample emitted when a replication order completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicationUsageSample {
    /// GiB slice reserved by the replication order.
    pub slice_gib: u64,
    /// Duration in seconds between scheduling and completion.
    pub duration_secs: u64,
}

impl ReplicationUsageSample {
    /// Construct a new usage sample.
    #[must_use]
    pub const fn new(slice_gib: u64, duration_secs: u64) -> Self {
        Self {
            slice_gib,
            duration_secs,
        }
    }
}

/// Parameters for exponential moving average smoothing.
#[derive(Debug, Clone, Copy)]
pub struct SmoothingParams {
    alpha: f64,
}

impl SmoothingParams {
    /// Construct a smoothing configuration, returning `None` when `alpha <= 0.0`.
    #[must_use]
    pub fn new(alpha: f64) -> Option<Self> {
        if alpha <= 0.0 {
            None
        } else {
            Some(Self {
                alpha: alpha.min(1.0),
            })
        }
    }

    /// Return the smoothing factor (0 – 1 range) associated with this configuration.
    #[must_use]
    pub const fn alpha(self) -> f64 {
        self.alpha
    }
}

/// Optional smoothing configuration for metering outputs.
#[derive(Debug, Clone, Copy)]
pub struct SmoothingConfig {
    gib_hours: Option<SmoothingParams>,
    por_success: Option<SmoothingParams>,
}

impl SmoothingConfig {
    /// Return a configuration with all smoothing disabled.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            gib_hours: None,
            por_success: None,
        }
    }

    /// Construct a configuration from optional alpha parameters.
    #[must_use]
    pub fn from_optional_alphas(
        gib_hours_alpha: Option<f64>,
        por_success_alpha: Option<f64>,
    ) -> Self {
        Self {
            gib_hours: gib_hours_alpha.and_then(SmoothingParams::new),
            por_success: por_success_alpha.and_then(SmoothingParams::new),
        }
    }

    /// Access the smoothing configuration for accumulated GiB·hours.
    #[must_use]
    pub const fn gib_hours(&self) -> Option<SmoothingParams> {
        self.gib_hours
    }

    /// Access the smoothing configuration for PoR success sampling.
    #[must_use]
    pub const fn por_success(&self) -> Option<SmoothingParams> {
        self.por_success
    }

    /// Return `true` when all smoothing knobs are disabled.
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.gib_hours.is_none() && self.por_success.is_none()
    }
}

impl Default for SmoothingConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Debug, Clone, Copy)]
struct SmoothingRuntime {
    config: SmoothingConfig,
    gib_hours: Option<f64>,
    por_success_bps: Option<f64>,
}

impl SmoothingRuntime {
    fn new(config: SmoothingConfig) -> Self {
        Self {
            config,
            gib_hours: None,
            por_success_bps: None,
        }
    }

    fn reset(&mut self) {
        self.gib_hours = None;
        self.por_success_bps = None;
    }

    fn update_gib_hours(&mut self, raw: f64) -> Option<f64> {
        match self.config.gib_hours {
            Some(params) => {
                let alpha = params.alpha();
                let next = match self.gib_hours {
                    Some(prev) => alpha.mul_add(raw, (1.0 - alpha) * prev),
                    None => raw,
                };
                self.gib_hours = Some(next);
                Some(next)
            }
            None => {
                self.gib_hours = None;
                None
            }
        }
    }

    fn update_por_success_bps(&mut self, raw: f64) -> Option<f64> {
        match self.config.por_success {
            Some(params) => {
                let alpha = params.alpha();
                let next = match self.por_success_bps {
                    Some(prev) => alpha.mul_add(raw, (1.0 - alpha) * prev),
                    None => raw,
                };
                self.por_success_bps = Some(next);
                Some(next)
            }
            None => {
                self.por_success_bps = None;
                None
            }
        }
    }
}

impl Default for SmoothingRuntime {
    fn default() -> Self {
        Self::new(SmoothingConfig::default())
    }
}

#[derive(Debug, Default, Clone)]
struct WindowCounters {
    window_start_epoch: Option<u64>,
    window_end_epoch: Option<u64>,
    declared_gib: u64,
    effective_gib: u64,
    deductions_gib: u64,
    utilised_gib: u64,
    orders_issued: u64,
    orders_completed: u64,
    orders_failed: u64,
    uptime_samples_success: u64,
    uptime_samples_total: u64,
    por_samples_success: u64,
    por_samples_total: u64,
    gib_seconds_accum: u128,
    gib_hours_smoothed: Option<f64>,
    uptime_bps: u32,
    por_success_bps: u32,
    por_success_smoothed_bps: Option<f64>,
    notes: Option<String>,
}

impl WindowCounters {
    fn apply_effective_override(&mut self, effective_gib: u64) {
        let capped = effective_gib.min(self.declared_gib);
        self.effective_gib = capped;
        self.deductions_gib = self.declared_gib.saturating_sub(capped);
    }

    fn apply_deductions(&mut self, deducted_gib: u64) {
        let capped = deducted_gib.min(self.declared_gib);
        self.deductions_gib = capped;
        self.effective_gib = self.declared_gib.saturating_sub(capped);
    }
}

#[derive(Debug)]
struct MeterState {
    window: WindowCounters,
    outstanding: HashMap<[u8; 32], OutstandingAllocation>,
    smoothing: SmoothingRuntime,
}

impl MeterState {
    fn new(config: SmoothingConfig) -> Self {
        Self {
            window: WindowCounters::default(),
            outstanding: HashMap::new(),
            smoothing: SmoothingRuntime::new(config),
        }
    }

    fn outstanding_total_gib(&self) -> u64 {
        self.outstanding
            .values()
            .fold(0_u64, |acc, entry| acc.saturating_add(entry.slice_gib))
    }
}

impl Default for MeterState {
    fn default() -> Self {
        Self::new(SmoothingConfig::default())
    }
}

/// Immutable snapshot of the current metering counters.
#[derive(Debug, Clone, Default)]
pub struct MeteringSnapshot {
    /// Window start epoch (inclusive), if configured.
    pub window_start_epoch: Option<u64>,
    /// Window end epoch (inclusive), if configured.
    pub window_end_epoch: Option<u64>,
    /// UNIX timestamp (seconds) when the snapshot was produced.
    pub observed_at_epoch: u64,
    /// Declared GiB seen during the window.
    pub declared_gib: u64,
    /// Effective GiB after governance deductions.
    pub effective_gib: u64,
    /// GiB deducted by governance during the window.
    pub deductions_gib: u64,
    /// Utilised GiB recorded from completed orders.
    pub utilised_gib: u64,
    /// Number of replication orders issued for this provider.
    pub orders_issued: u64,
    /// Number of replication orders completed within the window.
    pub orders_completed: u64,
    /// Number of replication orders failed/aborted within the window.
    pub orders_failed: u64,
    /// Cumulative slice GiB still outstanding.
    pub outstanding_total_gib: u64,
    /// Outstanding order count.
    pub outstanding_orders: u64,
    /// Accumulated GiB·seconds observed for the current window.
    pub accumulated_gib_seconds: u128,
    /// Accumulated GiB·hours observed for the current window.
    pub accumulated_gib_hours: f64,
    /// Smoothed GiB·hours (EMA) if smoothing is enabled.
    pub smoothed_gib_hours: Option<f64>,
    /// Successful uptime samples recorded for the window.
    pub uptime_samples_success: u64,
    /// Total uptime samples recorded for the window.
    pub uptime_samples_total: u64,
    /// Uptime success rate (basis points, 0 – 10_000).
    pub uptime_bps: u32,
    /// Successful PoR samples recorded for the window.
    pub por_samples_success: u64,
    /// Total PoR samples recorded for the window.
    pub por_samples_total: u64,
    /// Proof-of-retrieval success rate (basis points, 0 – 10_000).
    pub por_success_bps: u32,
    /// Smoothed PoR success rate (basis points, EMA) if smoothing is enabled.
    pub smoothed_por_success_bps: Option<u32>,
    /// Optional notes carried alongside telemetry.
    pub notes: Option<String>,
}

/// Deterministic fee projection derived from a [`MeteringSnapshot`].
#[derive(Debug, Clone)]
pub struct FeeProjection {
    /// Provider identifier tied to the projection.
    pub provider_id: [u8; 32],
    /// Window start epoch (inclusive).
    pub window_start_epoch: u64,
    /// Window end epoch (inclusive).
    pub window_end_epoch: u64,
    /// Duration of the window in seconds.
    pub window_duration_secs: u64,
    /// Declared GiB recorded for the window.
    pub declared_gib: u64,
    /// Effective GiB after deductions.
    pub effective_gib: u64,
    /// Utilised GiB recorded from completed orders.
    pub utilised_gib: u64,
    /// Accumulated GiB·seconds observed during the window.
    pub accumulated_gib_seconds: u128,
    /// Uptime success rate (basis points).
    pub uptime_bps: u32,
    /// Proof-of-retrievability success rate (basis points).
    pub por_success_bps: u32,
    /// Fee increment (nano units) derived from the snapshot.
    pub fee_nanos: u128,
}

impl FeeProjection {
    /// Derive a fee projection from `snapshot`, returning `None` when mandatory
    /// window bounds are missing or invalid.
    #[must_use]
    pub fn from_snapshot(provider_id: [u8; 32], snapshot: &MeteringSnapshot) -> Option<Self> {
        let start = snapshot.window_start_epoch?;
        let end = snapshot.window_end_epoch?;
        if end <= start {
            return None;
        }
        let window_duration_secs = end.saturating_sub(start);

        let uptime_bps = snapshot.uptime_bps.min(10_000);
        let por_success_bps = snapshot.por_success_bps.min(10_000);
        let uptime_factor = u128::from(uptime_bps);
        let por_factor = u128::from(por_success_bps);
        let utilisation_gib = u128::from(snapshot.utilised_gib);
        let combined = uptime_factor.saturating_mul(por_factor);
        let rounded = utilisation_gib
            .saturating_mul(combined)
            .saturating_add(99_99);
        let fee_scaled = rounded / 10_000 / 10_000;
        let fee_nanos = fee_scaled.saturating_mul(1_000);

        Some(Self {
            provider_id,
            window_start_epoch: start,
            window_end_epoch: end,
            window_duration_secs,
            declared_gib: snapshot.declared_gib,
            effective_gib: snapshot.effective_gib,
            utilised_gib: snapshot.utilised_gib,
            accumulated_gib_seconds: snapshot.accumulated_gib_seconds,
            uptime_bps,
            por_success_bps,
            fee_nanos,
        })
    }

    /// Convenience helper returning accumulated GiB·hours as a floating-point value.
    #[must_use]
    pub fn accumulated_gib_hours(&self) -> f64 {
        self.accumulated_gib_seconds as f64 / SECONDS_PER_HOUR as f64
    }
}

/// Accumulates per-window capacity metrics for a provider.
///
/// The meter is intentionally conservative: it only relies on deterministic
/// inputs captured locally (scheduled/complete events, declarations) and applies
/// optional exponential smoothing to the GiB·hour and PoR counters to minimise
/// jitter in operator dashboards.
#[derive(Debug, Clone)]
pub struct CapacityMeter {
    state: Arc<RwLock<MeterState>>,
}

impl CapacityMeter {
    /// Construct a new meter with no active window.
    #[must_use]
    pub fn new() -> Self {
        Self::with_smoothing(SmoothingConfig::default())
    }

    /// Construct a new meter with the provided smoothing configuration.
    #[must_use]
    pub fn with_smoothing(config: SmoothingConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(MeterState::new(config))),
        }
    }

    /// Update the smoothing configuration, resetting cached EMA values.
    pub fn configure_smoothing(&self, config: SmoothingConfig) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.smoothing = SmoothingRuntime::new(config);
        guard.window.gib_hours_smoothed = None;
        guard.window.por_success_smoothed_bps = None;
    }

    /// Reset counters for a freshly registered capacity declaration.
    ///
    /// This clears outstanding allocations (they should be empty after a
    /// declaration refresh) and seeds the window bounds using the declaration
    /// validity range.
    pub fn reset_for_declaration(&self, committed_gib: u64, window: DeclarationWindow) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.outstanding.clear();
        guard.window = WindowCounters {
            window_start_epoch: Some(window.valid_from_epoch),
            window_end_epoch: Some(window.valid_until_epoch),
            declared_gib: committed_gib,
            effective_gib: committed_gib,
            deductions_gib: 0,
            ..WindowCounters::default()
        };
        guard.smoothing.reset();
    }

    /// Update the declared capacity counters (GiB).
    pub fn record_declared_gib(&self, declared_gib: u64) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.declared_gib = guard.window.declared_gib.max(declared_gib);
        // Preserve existing deductions while respecting the new declaration ceiling.
        let deductions = guard.window.deductions_gib.min(guard.window.declared_gib);
        guard.window.apply_deductions(deductions);
    }

    /// Explicitly set the effective GiB recorded for the current window.
    /// Saturates the value against the declared capacity and updates the tracked deduction.
    pub fn record_effective_gib(&self, effective_gib: u64) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.apply_effective_override(effective_gib);
    }

    /// Apply governance deductions (in GiB) to the current window.
    /// The resulting effective GiB becomes `declared_gib - deducted_gib` (never negative).
    pub fn apply_capacity_deduction(&self, deducted_gib: u64) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.apply_deductions(deducted_gib);
    }

    /// Record that an order has been scheduled for the active provider.
    pub fn on_order_scheduled(&self, plan: &ReplicationPlan) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.orders_issued = guard.window.orders_issued.saturating_add(1);
        guard.outstanding.insert(
            plan.order_id,
            OutstandingAllocation {
                slice_gib: plan.assigned_slice_gib,
                issued_at: plan.issued_at,
            },
        );
    }

    /// Record that an order has been completed and moved out of the backlog.
    pub fn on_order_completed(&self, release: &ReplicationRelease) -> ReplicationUsageSample {
        let completed_at = current_unix_epoch_secs();
        self.on_order_completed_at(release, completed_at)
    }

    /// Record that an order has been completed at a specific timestamp.
    ///
    /// This helper is primarily used by tests to inject deterministic timestamps.
    pub fn on_order_completed_at(
        &self,
        release: &ReplicationRelease,
        completed_at: u64,
    ) -> ReplicationUsageSample {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.orders_completed = guard.window.orders_completed.saturating_add(1);
        let allocation =
            guard
                .outstanding
                .remove(&release.order_id)
                .unwrap_or(OutstandingAllocation {
                    slice_gib: release.released_gib,
                    issued_at: completed_at,
                });
        let slice_gib = allocation.slice_gib;
        let duration_secs = completed_at.saturating_sub(allocation.issued_at);
        let delta = u128::from(slice_gib) * u128::from(duration_secs);
        guard.window.gib_seconds_accum = guard.window.gib_seconds_accum.saturating_add(delta);
        guard.window.utilised_gib = guard.window.utilised_gib.saturating_add(slice_gib);
        ReplicationUsageSample::new(slice_gib, duration_secs)
    }

    /// Mark the current telemetry window as closed at `epoch`.
    pub fn close_window(&self, epoch: u64) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.window_end_epoch = Some(epoch);
    }

    /// Override the uptime basis-points metric for the current window.
    pub fn record_uptime_bps(&self, uptime_bps: u32) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.uptime_bps = uptime_bps.min(10_000);
    }

    /// Override the PoR success basis-points metric for the current window.
    pub fn record_por_success_bps(&self, por_success_bps: u32) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.por_success_bps = por_success_bps.min(10_000);
    }

    /// Record a single uptime probe result.
    pub fn record_uptime_sample(&self, success: bool) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.uptime_samples_total = guard.window.uptime_samples_total.saturating_add(1);
        if success {
            guard.window.uptime_samples_success =
                guard.window.uptime_samples_success.saturating_add(1);
        }
        guard.window.uptime_bps = ratio_to_bps(
            guard.window.uptime_samples_success,
            guard.window.uptime_samples_total,
        );
    }

    /// Record a single PoR probe result.
    pub fn record_por_sample(&self, success: bool) {
        self.record_por_samples(if success { 1 } else { 0 }, if success { 0 } else { 1 });
    }

    /// Record aggregated PoR probe results.
    pub fn record_por_samples(&self, success: u64, failed: u64) {
        if success == 0 && failed == 0 {
            return;
        }
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.por_samples_success = guard.window.por_samples_success.saturating_add(success);
        let total_delta = success.saturating_add(failed);
        guard.window.por_samples_total = guard.window.por_samples_total.saturating_add(total_delta);
        guard.window.por_success_bps = ratio_to_bps(
            guard.window.por_samples_success,
            guard.window.por_samples_total,
        );
    }

    /// Record that a replication order failed prior to completion.
    pub fn record_replication_failure(&self, order_id: [u8; 32]) -> Option<ReplicationUsageSample> {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.orders_failed = guard.window.orders_failed.saturating_add(1);
        let allocation = guard.outstanding.remove(&order_id)?;
        let completed_at = current_unix_epoch_secs();
        let elapsed = completed_at.saturating_sub(allocation.issued_at);
        let delta = u128::from(allocation.slice_gib) * u128::from(elapsed);
        guard.window.gib_seconds_accum = guard.window.gib_seconds_accum.saturating_add(delta);
        Some(ReplicationUsageSample::new(allocation.slice_gib, elapsed))
    }

    /// Attach free-form notes that will be exposed alongside telemetry.
    pub fn set_notes(&self, notes: Option<String>) {
        let mut guard = self.state.write().expect("metering state poisoned");
        guard.window.notes = notes.filter(|s| !s.trim().is_empty());
    }

    /// Produce a snapshot of the current counters.
    #[must_use]
    pub fn snapshot(&self) -> MeteringSnapshot {
        let now = current_unix_epoch_secs();
        self.snapshot_at(now)
    }

    /// Produce a snapshot of the current counters observed at `now_secs`.
    #[must_use]
    pub fn snapshot_at(&self, now_secs: u64) -> MeteringSnapshot {
        let mut guard = self.state.write().expect("metering state poisoned");
        let mut accumulated_gib_seconds = guard.window.gib_seconds_accum;
        for allocation in guard.outstanding.values() {
            let elapsed = now_secs.saturating_sub(allocation.issued_at);
            let delta = u128::from(allocation.slice_gib) * u128::from(elapsed);
            accumulated_gib_seconds = accumulated_gib_seconds.saturating_add(delta);
        }
        let accumulated_gib_hours = accumulated_gib_seconds as f64 / SECONDS_PER_HOUR as f64;
        let smoothed_gib_hours = guard
            .smoothing
            .update_gib_hours(accumulated_gib_hours)
            .map(|value| value.max(0.0));
        guard.window.gib_hours_smoothed = smoothed_gib_hours;

        let raw_por_bps = guard.window.por_success_bps as f64;
        let smoothed_por_bps = guard
            .smoothing
            .update_por_success_bps(raw_por_bps)
            .map(|value| value.clamp(0.0, 10_000.0));
        guard.window.por_success_smoothed_bps = smoothed_por_bps;
        let smoothed_por_bps_u32 =
            smoothed_por_bps.map(|value| value.round().clamp(0.0, 10_000.0) as u32);

        MeteringSnapshot {
            window_start_epoch: guard.window.window_start_epoch,
            window_end_epoch: guard.window.window_end_epoch,
            observed_at_epoch: now_secs,
            declared_gib: guard.window.declared_gib,
            effective_gib: guard.window.effective_gib,
            deductions_gib: guard.window.deductions_gib,
            utilised_gib: guard.window.utilised_gib,
            orders_issued: guard.window.orders_issued,
            orders_completed: guard.window.orders_completed,
            orders_failed: guard.window.orders_failed,
            outstanding_total_gib: guard.outstanding_total_gib(),
            outstanding_orders: guard.outstanding.len() as u64,
            accumulated_gib_seconds,
            accumulated_gib_hours,
            smoothed_gib_hours,
            uptime_samples_success: guard.window.uptime_samples_success,
            uptime_samples_total: guard.window.uptime_samples_total,
            uptime_bps: guard.window.uptime_bps,
            por_samples_success: guard.window.por_samples_success,
            por_samples_total: guard.window.por_samples_total,
            por_success_bps: guard.window.por_success_bps,
            smoothed_por_success_bps: smoothed_por_bps_u32,
            notes: guard.window.notes.clone(),
        }
    }

    /// Borrow the underlying shared state, primarily for tests.
    #[cfg(test)]
    fn outstanding_len(&self) -> usize {
        let guard = self.state.read().expect("metering state poisoned");
        guard.outstanding.len()
    }
}

impl Default for CapacityMeter {
    fn default() -> Self {
        Self::new()
    }
}

const SECONDS_PER_HOUR: u64 = 3_600;

fn current_unix_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs()
}

fn ratio_to_bps(success: u64, total: u64) -> u32 {
    if total == 0 {
        return 0;
    }
    let scaled = success
        .saturating_mul(10_000)
        .checked_div(total)
        .unwrap_or(0);
    scaled.min(10_000) as u32
}

#[cfg(test)]
mod tests {
    use sorafs_manifest::capacity::ReplicationOrderSlaV1;

    use super::*;
    use crate::capacity::ReplicationPlan;

    #[test]
    fn meter_tracks_orders() {
        let meter = CapacityMeter::new();
        let window = DeclarationWindow {
            registered_epoch: 1,
            valid_from_epoch: 10,
            valid_until_epoch: 20,
        };
        meter.reset_for_declaration(512, window);

        let plan = ReplicationPlan {
            order_id: [0xAB; 32],
            provider_id: [0x11; 32],
            manifest_cid: vec![0x01],
            manifest_digest: [0x02; 32],
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            assigned_slice_gib: 128,
            remaining_total_gib: 384,
            remaining_chunker_gib: 200,
            lane: Some("global".into()),
            remaining_lane_gib: Some(256),
            deadline_at: 42,
            issued_at: 21,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        };

        meter.on_order_scheduled(&plan);
        assert_eq!(meter.outstanding_len(), 1);

        let release = ReplicationRelease {
            order_id: plan.order_id,
            provider_id: plan.provider_id,
            released_gib: plan.assigned_slice_gib,
            remaining_total_gib: plan.remaining_total_gib,
            remaining_chunker_gib: plan.remaining_chunker_gib,
            lane: plan.lane.clone(),
            remaining_lane_gib: plan.remaining_lane_gib,
        };

        let usage_sample = meter.on_order_completed_at(&release, plan.issued_at + 600);
        assert_eq!(meter.outstanding_len(), 0);

        let snapshot = meter.snapshot_at(plan.issued_at + 600);
        assert_eq!(snapshot.declared_gib, 512);
        assert_eq!(snapshot.utilised_gib, 128);
        assert_eq!(snapshot.orders_issued, 1);
        assert_eq!(snapshot.orders_completed, 1);
        assert_eq!(snapshot.window_start_epoch, Some(10));
        assert_eq!(snapshot.window_end_epoch, Some(20));
        assert_eq!(snapshot.accumulated_gib_seconds, 128 * 600);
        assert!((snapshot.accumulated_gib_hours - (128.0 * 600.0 / 3600.0)).abs() < f64::EPSILON);
        assert!(snapshot.smoothed_gib_hours.is_none());
        assert!(snapshot.smoothed_por_success_bps.is_none());
        assert_eq!(usage_sample, ReplicationUsageSample::new(128, 600));
    }

    #[test]
    fn uptime_and_por_samples_update_rates() {
        let meter = CapacityMeter::new();
        meter.record_uptime_sample(true);
        meter.record_uptime_sample(false);
        meter.record_por_sample(true);
        meter.record_por_sample(true);
        meter.record_por_sample(false);

        let snapshot = meter.snapshot();
        assert_eq!(snapshot.uptime_samples_total, 2);
        assert_eq!(snapshot.uptime_samples_success, 1);
        assert_eq!(snapshot.uptime_bps, 5000);
        assert_eq!(snapshot.por_samples_total, 3);
        assert_eq!(snapshot.por_samples_success, 2);
        assert_eq!(snapshot.por_success_bps, 6666);
        assert!(snapshot.smoothed_gib_hours.is_none());
        assert!(snapshot.smoothed_por_success_bps.is_none());
    }

    #[test]
    fn smoothing_applies_ema_when_enabled() {
        let smoothing = SmoothingConfig::from_optional_alphas(Some(0.5), Some(0.25));
        let meter = CapacityMeter::with_smoothing(smoothing);
        let window = DeclarationWindow {
            registered_epoch: 1,
            valid_from_epoch: 10,
            valid_until_epoch: 20,
        };
        meter.reset_for_declaration(256, window);

        let base_plan = ReplicationPlan {
            order_id: [0x01; 32],
            provider_id: [0x02; 32],
            manifest_cid: vec![],
            manifest_digest: [0; 32],
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            assigned_slice_gib: 64,
            remaining_total_gib: 192,
            remaining_chunker_gib: 128,
            lane: None,
            remaining_lane_gib: None,
            deadline_at: 600,
            issued_at: 0,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        };

        meter.on_order_scheduled(&base_plan);
        let release = ReplicationRelease {
            order_id: base_plan.order_id,
            provider_id: base_plan.provider_id,
            released_gib: base_plan.assigned_slice_gib,
            remaining_total_gib: base_plan.remaining_total_gib,
            remaining_chunker_gib: base_plan.remaining_chunker_gib,
            lane: base_plan.lane.clone(),
            remaining_lane_gib: base_plan.remaining_lane_gib,
        };
        meter.on_order_completed_at(&release, 600);
        meter.record_por_samples(8, 2); // 80%

        let first = meter.snapshot_at(600);
        let raw_first_hours = first.accumulated_gib_hours;
        let smoothed_first_hours = first.smoothed_gib_hours.expect("smoothing enabled");
        assert!((raw_first_hours - smoothed_first_hours).abs() < 1e-6);
        assert_eq!(
            first.smoothed_por_success_bps.expect("por smoothing"),
            first.por_success_bps
        );

        let mut second_plan = base_plan;
        second_plan.order_id = [0x03; 32];
        second_plan.issued_at = 600;
        meter.on_order_scheduled(&second_plan);
        let release_second = ReplicationRelease {
            order_id: second_plan.order_id,
            provider_id: second_plan.provider_id,
            released_gib: second_plan.assigned_slice_gib,
            remaining_total_gib: second_plan.remaining_total_gib,
            remaining_chunker_gib: second_plan.remaining_chunker_gib,
            lane: second_plan.lane.clone(),
            remaining_lane_gib: second_plan.remaining_lane_gib,
        };
        meter.on_order_completed_at(&release_second, 900);
        meter.record_por_samples(2, 2); // total now 10/12 -> 83.33%

        let second = meter.snapshot_at(900);
        let expected_hours = 0.5 * second.accumulated_gib_hours + 0.5 * raw_first_hours;
        let smoothed_second_hours = second.smoothed_gib_hours.expect("hours smoothing retained");
        assert!((smoothed_second_hours - expected_hours).abs() < 1e-6);

        let raw_second_por = second.por_success_bps as f64;
        let expected_por =
            (0.25 * raw_second_por + 0.75 * f64::from(first.por_success_bps)).round() as u32;
        let smoothed_second_por = second
            .smoothed_por_success_bps
            .expect("por smoothing should persist");
        assert_eq!(smoothed_second_por, expected_por);
    }

    #[test]
    fn fee_projection_matches_ledger_formula() {
        let meter = CapacityMeter::new();
        let window = DeclarationWindow {
            registered_epoch: 1,
            valid_from_epoch: 10,
            valid_until_epoch: 30,
        };
        meter.reset_for_declaration(512, window);

        let plan = ReplicationPlan {
            order_id: [0xCD; 32],
            provider_id: [0x22; 32],
            manifest_cid: vec![0x01],
            manifest_digest: [0xEE; 32],
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            assigned_slice_gib: 128,
            remaining_total_gib: 384,
            remaining_chunker_gib: 200,
            lane: Some("global".into()),
            remaining_lane_gib: Some(256),
            deadline_at: 60,
            issued_at: 21,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        };

        meter.on_order_scheduled(&plan);

        let release = ReplicationRelease {
            order_id: plan.order_id,
            provider_id: plan.provider_id,
            released_gib: plan.assigned_slice_gib,
            remaining_total_gib: plan.remaining_total_gib,
            remaining_chunker_gib: plan.remaining_chunker_gib,
            lane: plan.lane.clone(),
            remaining_lane_gib: plan.remaining_lane_gib,
        };

        meter.on_order_completed_at(&release, plan.issued_at + 600);

        meter.record_uptime_sample(true);
        meter.record_uptime_sample(true);
        meter.record_por_sample(true);
        meter.record_por_sample(false);
        meter.close_window(window.valid_until_epoch);

        let snapshot = meter.snapshot_at(plan.issued_at + 600);
        let provider_id = [0x55; 32];
        let projection =
            FeeProjection::from_snapshot(provider_id, &snapshot).expect("fee projection");

        assert_eq!(projection.provider_id, provider_id);
        assert_eq!(projection.declared_gib, snapshot.declared_gib);
        assert_eq!(projection.utilised_gib, snapshot.utilised_gib);
        assert_eq!(
            projection.accumulated_gib_seconds,
            snapshot.accumulated_gib_seconds
        );

        let uptime_bps = snapshot.uptime_bps.min(10_000);
        let por_bps = snapshot.por_success_bps.min(10_000);
        let expected_fee = u128::from(snapshot.utilised_gib)
            .saturating_mul(u128::from(uptime_bps))
            .saturating_mul(u128::from(por_bps))
            .saturating_add(99_99)
            / 10_000
            / 10_000
            * 1_000;
        assert_eq!(projection.fee_nanos, expected_fee);
    }
}
