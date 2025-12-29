//! Telemetry accumulation helpers for SoraFS storage providers.

use std::cmp::min;

use sorafs_manifest::capacity::CapacityTelemetryV1;
use thiserror::Error;

/// Tracks per-epoch telemetry metrics before emitting a [`CapacityTelemetryV1`] payload.
#[derive(Debug, Clone)]
pub struct TelemetryAccumulator {
    provider_id: [u8; 32],
    declared_capacity_gib: u64,
    window_start_epoch: u64,
    window_end_epoch: u64,
    utilised_gib_seconds: u128,
    uptime_seconds: u64,
    uptime_observed_seconds: u64,
    por_successes: u32,
    por_total: u32,
    successful_replications: u32,
    failed_replications: u32,
    notes: Option<String>,
}

impl TelemetryAccumulator {
    /// Construct a new telemetry accumulator for the given provider and epoch window.
    #[must_use]
    pub fn new(provider_id: [u8; 32], declared_capacity_gib: u64, window_start_epoch: u64) -> Self {
        Self {
            provider_id,
            declared_capacity_gib,
            window_start_epoch,
            window_end_epoch: window_start_epoch,
            utilised_gib_seconds: 0,
            uptime_seconds: 0,
            uptime_observed_seconds: 0,
            por_successes: 0,
            por_total: 0,
            successful_replications: 0,
            failed_replications: 0,
            notes: None,
        }
    }

    /// Update the declared capacity to reflect governance changes.
    pub fn set_declared_capacity(&mut self, declared_capacity_gib: u64) {
        self.declared_capacity_gib = declared_capacity_gib;
    }

    /// Advance the window end epoch. Must be greater than the start epoch.
    pub fn set_window_end_epoch(&mut self, epoch: u64) -> Result<(), TelemetryError> {
        if epoch <= self.window_start_epoch {
            return Err(TelemetryError::InvalidWindow {
                start: self.window_start_epoch,
                end: epoch,
            });
        }
        self.window_end_epoch = epoch;
        Ok(())
    }

    /// Record an utilisation sample covering `duration_secs` at `utilised_gib`.
    pub fn record_utilisation(
        &mut self,
        utilised_gib: u64,
        duration_secs: u64,
    ) -> Result<(), TelemetryError> {
        if duration_secs == 0 {
            return Err(TelemetryError::ZeroDurationSample);
        }
        let increment = (utilised_gib as u128)
            .checked_mul(duration_secs as u128)
            .ok_or(TelemetryError::UtilisationOverflow)?;
        self.utilised_gib_seconds = self
            .utilised_gib_seconds
            .checked_add(increment)
            .ok_or(TelemetryError::UtilisationOverflow)?;
        Ok(())
    }

    /// Record uptime for the given observation window.
    pub fn record_uptime_sample(
        &mut self,
        uptime_secs: u64,
        observed_secs: u64,
    ) -> Result<(), TelemetryError> {
        if observed_secs == 0 || uptime_secs > observed_secs {
            return Err(TelemetryError::InvalidUptimeSample {
                uptime_secs,
                observed_secs,
            });
        }
        self.uptime_seconds = self
            .uptime_seconds
            .checked_add(uptime_secs)
            .ok_or(TelemetryError::UptimeOverflow)?;
        self.uptime_observed_seconds = self
            .uptime_observed_seconds
            .checked_add(observed_secs)
            .ok_or(TelemetryError::UptimeOverflow)?;
        Ok(())
    }

    /// Record a proof-of-retrievability sample outcome.
    pub fn record_por_sample(&mut self, success: bool) {
        self.por_total = self.por_total.saturating_add(1);
        if success {
            self.por_successes = self.por_successes.saturating_add(1);
        }
    }

    /// Record a successful replication order completion.
    pub fn record_replication_success(&mut self) {
        self.successful_replications = self.successful_replications.saturating_add(1);
    }

    /// Record a failed replication order outcome.
    pub fn record_replication_failure(&mut self) {
        self.failed_replications = self.failed_replications.saturating_add(1);
    }

    /// Attach optional telemetry notes that will be emitted with the payload.
    pub fn set_notes<S: Into<String>>(&mut self, notes: Option<S>) {
        self.notes = notes.map(Into::into);
    }

    /// Build the canonical `CapacityTelemetryV1` payload for the current window.
    pub fn build_payload(&self) -> Result<CapacityTelemetryV1, TelemetryError> {
        let window_duration = self
            .window_end_epoch
            .checked_sub(self.window_start_epoch)
            .ok_or(TelemetryError::InvalidWindow {
                start: self.window_start_epoch,
                end: self.window_end_epoch,
            })?;
        if window_duration == 0 {
            return Err(TelemetryError::ZeroWindowDuration);
        }

        let avg_utilised = self
            .utilised_gib_seconds
            .checked_div(window_duration as u128)
            .ok_or(TelemetryError::UtilisationOverflow)?;
        let utilised_capacity_gib = min(self.declared_capacity_gib, avg_utilised as u64);

        let uptime_percent_milli = if self.uptime_observed_seconds == 0 {
            100_000_u32
        } else {
            let ratio =
                self.uptime_seconds.saturating_mul(100_000) / self.uptime_observed_seconds.max(1);
            min(100_000_u64, ratio) as u32
        };

        let por_success_percent_milli = if self.por_total == 0 {
            100_000_u32
        } else {
            let ratio = self.por_successes.saturating_mul(100_000) / self.por_total.max(1);
            min(100_000_u32, ratio)
        };

        let mut telemetry = CapacityTelemetryV1 {
            version: sorafs_manifest::capacity::CAPACITY_TELEMETRY_VERSION_V1,
            provider_id: self.provider_id,
            epoch_start: self.window_start_epoch,
            epoch_end: self.window_end_epoch,
            declared_capacity_gib: self.declared_capacity_gib,
            utilised_capacity_gib,
            successful_replications: self.successful_replications,
            failed_replications: self.failed_replications,
            uptime_percent_milli,
            por_success_percent_milli,
            notes: None,
        };
        if let Some(notes) = &self.notes {
            telemetry.notes = Some(notes.clone());
        }
        telemetry
            .validate()
            .map_err(TelemetryError::TelemetryValidationFailed)?;
        Ok(telemetry)
    }
}

/// Errors surfaced while accumulating telemetry.
#[derive(Debug, Error)]
pub enum TelemetryError {
    /// Window boundaries are invalid.
    #[error("invalid telemetry window: start={start}, end={end}")]
    InvalidWindow {
        /// Start epoch supplied for the telemetry window.
        start: u64,
        /// End epoch supplied for the telemetry window.
        end: u64,
    },
    /// No duration was provided for the utilisation sample.
    #[error("utilisation sample requires a non-zero duration")]
    ZeroDurationSample,
    /// Utilisation accumulation overflowed internal counters.
    #[error("utilisation accumulation overflowed internal counters")]
    UtilisationOverflow,
    /// Uptime sample exceeded its observation window.
    #[error("invalid uptime sample: uptime={uptime_secs}s observed={observed_secs}s")]
    InvalidUptimeSample {
        /// Number of seconds reported as uptime.
        uptime_secs: u64,
        /// Observation window duration in seconds.
        observed_secs: u64,
    },
    /// Uptime accumulation overflowed internal counters.
    #[error("uptime accumulation overflowed internal counters")]
    UptimeOverflow,
    /// The telemetry window duration is zero.
    #[error("telemetry window duration must be greater than zero")]
    ZeroWindowDuration,
    /// Final telemetry payload failed validation.
    #[error("telemetry payload validation failed: {0}")]
    TelemetryValidationFailed(sorafs_manifest::capacity::CapacityTelemetryValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_accumulator() -> TelemetryAccumulator {
        let mut acc = TelemetryAccumulator::new([0xAB; 32], 500, 1_700_000_000);
        acc.set_window_end_epoch(1_700_000_600)
            .expect("valid window");
        acc
    }

    #[test]
    fn build_payload_clamps_utilisation_to_declared_capacity() {
        let mut acc = sample_accumulator();
        acc.record_utilisation(800, 600)
            .expect("record utilisation");
        let payload = acc.build_payload().expect("payload");
        assert_eq!(payload.declared_capacity_gib, 500);
        assert_eq!(
            payload.utilised_capacity_gib, 500,
            "utilised GiB should not exceed declared capacity"
        );
    }

    #[test]
    fn record_uptime_sample_rejects_invalid_inputs() {
        let mut acc = sample_accumulator();
        let zero_err = acc.record_uptime_sample(0, 0).unwrap_err();
        assert!(matches!(
            zero_err,
            TelemetryError::InvalidUptimeSample { .. }
        ));

        let overflow_err = acc.record_uptime_sample(10, 5).unwrap_err();
        assert!(matches!(
            overflow_err,
            TelemetryError::InvalidUptimeSample { .. }
        ));
    }

    #[test]
    fn utilisation_accumulates_gib_seconds() {
        let mut acc = sample_accumulator();
        acc.record_utilisation(200, 300).expect("first sample");
        acc.record_utilisation(100, 150).expect("second sample");

        let payload = acc.build_payload().expect("payload");
        // (200 * 300 + 100 * 150) / 600 seconds = 125 GiB average
        assert_eq!(payload.utilised_capacity_gib, 125);
    }

    #[test]
    fn window_end_epoch_must_exceed_start() {
        let mut acc = TelemetryAccumulator::new([0u8; 32], 128, 42);
        let err = acc.set_window_end_epoch(42).unwrap_err();
        assert!(matches!(err, TelemetryError::InvalidWindow { .. }));
    }

    #[test]
    fn por_and_replication_counters_increment() {
        let mut acc = sample_accumulator();
        acc.record_por_sample(true);
        acc.record_por_sample(false);
        acc.record_replication_success();
        acc.record_replication_failure();

        acc.record_uptime_sample(540, 600).expect("uptime sample");
        let payload = acc.build_payload().expect("payload");
        assert_eq!(payload.successful_replications, 1);
        assert_eq!(payload.failed_replications, 1);
        assert_eq!(payload.por_success_percent_milli, 50_000);
        assert_eq!(payload.uptime_percent_milli, 90_000);
    }

    #[test]
    fn aggregates_basic_metrics() {
        let provider = [0xAA; 32];
        let mut acc = TelemetryAccumulator::new(provider, 512, 1_000);
        acc.set_window_end_epoch(1_360).expect("set end");
        acc.record_utilisation(256, 360).expect("utilisation");
        acc.record_uptime_sample(340, 360).expect("uptime");
        acc.record_por_sample(true);
        acc.record_por_sample(false);
        acc.record_replication_success();
        acc.record_replication_failure();
        acc.set_notes(Some("window-ok"));

        let payload = acc.build_payload().expect("payload");
        assert_eq!(payload.provider_id, provider);
        assert_eq!(payload.declared_capacity_gib, 512);
        assert_eq!(payload.utilised_capacity_gib, 256);
        assert_eq!(payload.successful_replications, 1);
        assert_eq!(payload.failed_replications, 1);
        assert_eq!(payload.uptime_percent_milli, 94_444);
        assert_eq!(payload.por_success_percent_milli, 50_000);
        assert_eq!(payload.notes.as_deref(), Some("window-ok"));
    }

    #[test]
    fn uptime_defaults_to_full_when_missing() {
        let mut acc = TelemetryAccumulator::new([0xAB; 32], 128, 10);
        acc.set_window_end_epoch(20).expect("set end");
        let payload = acc.build_payload().expect("payload");
        assert_eq!(payload.uptime_percent_milli, 100_000);
        assert_eq!(payload.por_success_percent_milli, 100_000);
    }

    #[test]
    fn rejects_invalid_window() {
        let mut acc = TelemetryAccumulator::new([0; 32], 128, 42);
        assert!(matches!(
            acc.set_window_end_epoch(42),
            Err(TelemetryError::InvalidWindow { start: 42, end: 42 })
        ));
    }
}
