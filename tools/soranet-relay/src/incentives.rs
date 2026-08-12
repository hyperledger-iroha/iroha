//! Relay incentive helpers used to aggregate blinded measurement proofs and uptime reports.
//!
//! The accumulator provides a lightweight in-process view over the measurements required by
//! SNNet-7 so the relay runtime can surface Norito-friendly `RelayEpochMetricsV1` payloads for the
//! treasury pipeline. It deduplicates measurement flows, tracks the minimum confidence across
//! samples, and packages the results into the canonical data-model structures.

use iroha_data_model::{
    metadata::Metadata,
    soranet::{
        incentives::{MeasurementId, RelayBandwidthProofV1},
        prelude::{RelayComplianceStatusV1, RelayEpochMetricsV1, RelayId},
    },
};
use thiserror::Error;

/// Default number of simultaneously retained incentive epochs.
pub const INCENTIVE_DEFAULT_ACTIVE_EPOCHS: usize = 16;
/// First-release hard ceiling for simultaneously retained incentive epochs.
pub const INCENTIVE_MAX_ACTIVE_EPOCHS_V1: usize = 256;
/// Default number of distinct measurement IDs retained per epoch.
pub const INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH: usize = 4_096;
/// First-release aggregate ceiling for retained measurement IDs.
pub const INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1: usize = 65_536;

/// Capacity failures surfaced by the bounded incentive accumulator.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IncentiveCapacityError {
    /// Too many distinct epochs are already resident.
    #[error("active incentive epoch capacity {limit} reached")]
    ActiveEpochs { limit: usize },
    /// One epoch reached its configured measurement-ID ceiling.
    #[error("incentive measurement capacity {limit} reached for epoch {epoch}")]
    MeasurementsPerEpoch { epoch: u32, limit: usize },
    /// The aggregate retained measurement-ID corridor is full.
    #[error("aggregate incentive measurement capacity {limit} reached")]
    AggregateMeasurements { limit: usize },
    /// A bounded collection could not reserve its admitted storage.
    #[error("incentive accumulator memory capacity is unavailable")]
    Allocation,
}

/// Result of admitting one blinded bandwidth proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthProofIngest {
    /// The proof was new and its counters were applied.
    Accepted,
    /// The same measurement ID was already present in the target epoch.
    Duplicate,
    /// The proof targets another relay.
    ForeignRelay,
}

/// Per-epoch accumulation of relay performance signals.
#[derive(Debug, Default)]
struct RelayEpochAccumulator {
    uptime_seconds: u64,
    scheduled_uptime_seconds: u64,
    verified_bandwidth_bytes: u128,
    /// Sorted IDs provide deterministic output without infallible tree-node
    /// allocation on admission.
    measurement_ids: Vec<MeasurementId>,
    confidence_floor_per_mille: u16,
}

impl RelayEpochAccumulator {
    fn update_uptime(&mut self, uptime_seconds: u64, scheduled_uptime_seconds: u64) {
        self.uptime_seconds = self.uptime_seconds.saturating_add(uptime_seconds);
        self.scheduled_uptime_seconds = self
            .scheduled_uptime_seconds
            .saturating_add(scheduled_uptime_seconds);
    }

    fn ingest_prepared(&mut self, proof: &RelayBandwidthProofV1, position: usize) {
        self.measurement_ids.insert(position, proof.measurement_id);
        self.verified_bandwidth_bytes = self
            .verified_bandwidth_bytes
            .saturating_add(proof.verified_bytes);
        self.confidence_floor_per_mille = if self.measurement_ids.len() == 1 {
            proof.confidence.confidence_per_mille
        } else {
            self.confidence_floor_per_mille
                .min(proof.confidence.confidence_per_mille)
        };
    }

    fn into_metrics(
        self,
        relay_id: RelayId,
        epoch: u32,
        compliance: RelayComplianceStatusV1,
        reward_score: u64,
        metadata: Metadata,
    ) -> RelayEpochMetricsV1 {
        let confidence_floor = if self.measurement_ids.is_empty() {
            0
        } else {
            self.confidence_floor_per_mille.min(1_000)
        };
        RelayEpochMetricsV1 {
            relay_id,
            epoch,
            uptime_seconds: self.uptime_seconds,
            scheduled_uptime_seconds: self.scheduled_uptime_seconds,
            verified_bandwidth_bytes: self.verified_bandwidth_bytes,
            compliance,
            reward_score,
            confidence_floor_per_mille: confidence_floor,
            measurement_ids: self.measurement_ids,
            metadata,
        }
    }
}

/// Aggregates blinded measurement proofs and uptime counters for a specific relay.
#[derive(Debug)]
pub struct RelayPerformanceAccumulator {
    relay_id: RelayId,
    /// Sorted by epoch for deterministic snapshots and bounded binary search.
    epochs: Vec<(u32, RelayEpochAccumulator)>,
    total_measurements: usize,
    max_active_epochs: usize,
    max_measurements_per_epoch: usize,
    max_total_measurements: usize,
}

/// Snapshot of the relay performance accumulator for telemetry export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochSummary {
    /// Epoch identifier.
    pub epoch: u32,
    /// Total uptime seconds recorded for the epoch.
    pub uptime_seconds: u64,
    /// Scheduled uptime seconds expected for the epoch.
    pub scheduled_uptime_seconds: u64,
    /// Total verified bandwidth bytes observed during the epoch.
    pub verified_bandwidth_bytes: u128,
    /// Minimum confidence across all accepted bandwidth proofs.
    pub confidence_floor_per_mille: u16,
    /// Measurement identifiers contributing to the epoch summary.
    pub measurement_ids: Vec<MeasurementId>,
}

impl RelayPerformanceAccumulator {
    /// Creates a new accumulator bound to the supplied relay identifier.
    #[must_use]
    pub fn new(relay_id: RelayId) -> Self {
        Self::with_limits(
            relay_id,
            INCENTIVE_DEFAULT_ACTIVE_EPOCHS,
            INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH,
        )
    }

    /// Creates an accumulator with explicit, hard-clamped memory corridors.
    #[must_use]
    pub fn with_limits(
        relay_id: RelayId,
        max_active_epochs: usize,
        max_measurements_per_epoch: usize,
    ) -> Self {
        let max_active_epochs = max_active_epochs.clamp(1, INCENTIVE_MAX_ACTIVE_EPOCHS_V1);
        let max_measurements_per_epoch = max_measurements_per_epoch.clamp(
            1,
            INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1 / max_active_epochs,
        );
        Self {
            relay_id,
            epochs: Vec::new(),
            total_measurements: 0,
            max_active_epochs,
            max_measurements_per_epoch,
            max_total_measurements: max_active_epochs * max_measurements_per_epoch,
        }
    }

    /// Returns the relay identifier tracked by this accumulator.
    #[must_use]
    pub const fn relay_id(&self) -> RelayId {
        self.relay_id
    }

    /// Records uptime counters for a given epoch.
    pub fn record_uptime(
        &mut self,
        epoch: u32,
        uptime_seconds: u64,
        scheduled_uptime_seconds: u64,
    ) {
        let _ = self.try_record_uptime(epoch, uptime_seconds, scheduled_uptime_seconds);
    }

    /// Records uptime while surfacing bounded-allocation/capacity failures.
    pub fn try_record_uptime(
        &mut self,
        epoch: u32,
        uptime_seconds: u64,
        scheduled_uptime_seconds: u64,
    ) -> Result<(), IncentiveCapacityError> {
        let index = match self
            .epochs
            .binary_search_by_key(&epoch, |(epoch, _)| *epoch)
        {
            Ok(index) => index,
            Err(index) => {
                if self.epochs.len() >= self.max_active_epochs {
                    return Err(IncentiveCapacityError::ActiveEpochs {
                        limit: self.max_active_epochs,
                    });
                }
                self.epochs
                    .try_reserve_exact(1)
                    .map_err(|_| IncentiveCapacityError::Allocation)?;
                self.epochs
                    .insert(index, (epoch, RelayEpochAccumulator::default()));
                index
            }
        };
        self.epochs[index]
            .1
            .update_uptime(uptime_seconds, scheduled_uptime_seconds);
        Ok(())
    }

    /// Adds a blinded bandwidth proof to the accumulator.
    ///
    /// Returns `true` when the proof was accepted (i.e., not a duplicate and targeting this relay).
    pub fn ingest_bandwidth_proof(&mut self, proof: &RelayBandwidthProofV1) -> bool {
        matches!(
            self.try_ingest_bandwidth_proof(proof),
            Ok(BandwidthProofIngest::Accepted)
        )
    }

    /// Adds a proof while distinguishing duplicate, foreign, and capacity
    /// outcomes without mutating counters on rejection.
    pub fn try_ingest_bandwidth_proof(
        &mut self,
        proof: &RelayBandwidthProofV1,
    ) -> Result<BandwidthProofIngest, IncentiveCapacityError> {
        if proof.relay_id != self.relay_id {
            return Ok(BandwidthProofIngest::ForeignRelay);
        }
        match self
            .epochs
            .binary_search_by_key(&proof.epoch, |(epoch, _)| *epoch)
        {
            Ok(index) => {
                let accumulator = &mut self.epochs[index].1;
                let position = match accumulator
                    .measurement_ids
                    .binary_search(&proof.measurement_id)
                {
                    Ok(_) => return Ok(BandwidthProofIngest::Duplicate),
                    Err(position) => position,
                };
                if accumulator.measurement_ids.len() >= self.max_measurements_per_epoch {
                    return Err(IncentiveCapacityError::MeasurementsPerEpoch {
                        epoch: proof.epoch,
                        limit: self.max_measurements_per_epoch,
                    });
                }
                if self.total_measurements >= self.max_total_measurements {
                    return Err(IncentiveCapacityError::AggregateMeasurements {
                        limit: self.max_total_measurements,
                    });
                }
                accumulator
                    .measurement_ids
                    .try_reserve_exact(1)
                    .map_err(|_| IncentiveCapacityError::Allocation)?;
                accumulator.ingest_prepared(proof, position);
            }
            Err(index) => {
                if self.epochs.len() >= self.max_active_epochs {
                    return Err(IncentiveCapacityError::ActiveEpochs {
                        limit: self.max_active_epochs,
                    });
                }
                if self.total_measurements >= self.max_total_measurements {
                    return Err(IncentiveCapacityError::AggregateMeasurements {
                        limit: self.max_total_measurements,
                    });
                }
                let mut accumulator = RelayEpochAccumulator::default();
                accumulator
                    .measurement_ids
                    .try_reserve_exact(1)
                    .map_err(|_| IncentiveCapacityError::Allocation)?;
                accumulator.ingest_prepared(proof, 0);
                self.epochs
                    .try_reserve_exact(1)
                    .map_err(|_| IncentiveCapacityError::Allocation)?;
                self.epochs.insert(index, (proof.epoch, accumulator));
            }
        }
        self.total_measurements += 1;
        Ok(BandwidthProofIngest::Accepted)
    }

    /// Finalises metrics for the supplied epoch, producing a `RelayEpochMetricsV1` payload.
    ///
    /// The epoch entry is removed from the accumulator to avoid double-accounting.
    pub fn finalize_epoch(
        &mut self,
        epoch: u32,
        compliance: RelayComplianceStatusV1,
        reward_score: u64,
        metadata: Metadata,
    ) -> RelayEpochMetricsV1 {
        let accumulator = match self
            .epochs
            .binary_search_by_key(&epoch, |(epoch, _)| *epoch)
        {
            Ok(index) => {
                let (_, accumulator) = self.epochs.remove(index);
                self.total_measurements = self
                    .total_measurements
                    .saturating_sub(accumulator.measurement_ids.len());
                accumulator
            }
            Err(_) => RelayEpochAccumulator::default(),
        };
        accumulator.into_metrics(self.relay_id, epoch, compliance, reward_score, metadata)
    }

    fn try_summary(
        epoch: u32,
        accumulator: &RelayEpochAccumulator,
    ) -> Result<EpochSummary, IncentiveCapacityError> {
        let mut measurement_ids = Vec::new();
        measurement_ids
            .try_reserve_exact(accumulator.measurement_ids.len())
            .map_err(|_| IncentiveCapacityError::Allocation)?;
        measurement_ids.extend_from_slice(&accumulator.measurement_ids);
        Ok(EpochSummary {
            epoch,
            uptime_seconds: accumulator.uptime_seconds,
            scheduled_uptime_seconds: accumulator.scheduled_uptime_seconds,
            verified_bandwidth_bytes: accumulator.verified_bandwidth_bytes,
            confidence_floor_per_mille: accumulator.confidence_floor_per_mille,
            measurement_ids,
        })
    }

    /// Returns one epoch snapshot without cloning unrelated epochs.
    pub fn summary(&self, epoch: u32) -> Result<Option<EpochSummary>, IncentiveCapacityError> {
        let Ok(index) = self
            .epochs
            .binary_search_by_key(&epoch, |(epoch, _)| *epoch)
        else {
            return Ok(None);
        };
        Self::try_summary(epoch, &self.epochs[index].1).map(Some)
    }

    /// Returns all bounded epoch snapshots with fallible reservations.
    pub fn try_summaries(&self) -> Result<Vec<EpochSummary>, IncentiveCapacityError> {
        let mut summaries = Vec::new();
        summaries
            .try_reserve_exact(self.epochs.len())
            .map_err(|_| IncentiveCapacityError::Allocation)?;
        for (epoch, accumulator) in &self.epochs {
            summaries.push(Self::try_summary(*epoch, accumulator)?);
        }
        Ok(summaries)
    }

    /// Returns a snapshot of the currently accumulated epoch data.
    #[must_use]
    pub fn summaries(&self) -> Vec<EpochSummary> {
        self.try_summaries().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{account::AccountId, metadata::Metadata};

    use super::*;

    const RELAY: RelayId = [7_u8; 32];

    fn sample_account(seed: u8) -> AccountId {
        let (public_key, _) = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive incentive fixture account key")
            .into_parts();
        AccountId::new(public_key)
    }

    #[test]
    fn sample_account_uses_checked_seed_derivation() {
        let (public_key, _) = KeyPair::try_from_seed(vec![9; 32], Algorithm::Ed25519)
            .expect("derive incentive fixture account key")
            .into_parts();

        assert_eq!(sample_account(9), AccountId::new(public_key));
    }

    fn proof(
        measurement: MeasurementId,
        epoch: u32,
        verified_bytes: u128,
        confidence_per_mille: u16,
    ) -> RelayBandwidthProofV1 {
        RelayBandwidthProofV1 {
            relay_id: RELAY,
            measurement_id: measurement,
            epoch,
            verified_bytes,
            verifier_id: sample_account(1),
            issued_at_unix: 12,
            confidence: iroha_data_model::soranet::prelude::BandwidthConfidenceV1 {
                sample_count: 16,
                jitter_p95_ms: 8,
                confidence_per_mille,
            },
            signature: Signature::try_from_bytes(&[1_u8; 64])
                .expect("relay incentive fixture signature is non-empty and nonzero"),
            metadata: Metadata::default(),
        }
    }

    #[test]
    fn accepts_unique_measurements() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        assert!(accumulator.ingest_bandwidth_proof(&proof([1; 32], 5, 512, 900)));
        assert!(accumulator.ingest_bandwidth_proof(&proof([2; 32], 5, 1_024, 850)));

        let metrics =
            accumulator.finalize_epoch(5, RelayComplianceStatusV1::Clean, 10, Metadata::default());

        assert_eq!(metrics.verified_bandwidth_bytes, 1_536);
        assert_eq!(metrics.measurement_ids.len(), 2);
        assert_eq!(metrics.confidence_floor_per_mille, 850);
    }

    #[test]
    fn rejects_duplicate_measurements() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        assert!(accumulator.ingest_bandwidth_proof(&proof([3; 32], 5, 512, 900)));
        assert!(!accumulator.ingest_bandwidth_proof(&proof([3; 32], 5, 256, 800)));

        let metrics =
            accumulator.finalize_epoch(5, RelayComplianceStatusV1::Clean, 10, Metadata::default());
        assert_eq!(metrics.verified_bandwidth_bytes, 512);
        assert_eq!(metrics.confidence_floor_per_mille, 900);
    }

    #[test]
    fn rejects_foreign_relay_proofs() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        let mut foreign = proof([4; 32], 5, 256, 900);
        foreign.relay_id = [1_u8; 32];
        assert!(!accumulator.ingest_bandwidth_proof(&foreign));
        let metrics =
            accumulator.finalize_epoch(5, RelayComplianceStatusV1::Clean, 0, Metadata::default());
        assert_eq!(metrics.verified_bandwidth_bytes, 0);
        assert_eq!(metrics.measurement_ids.len(), 0);
    }

    #[test]
    fn uptime_updates_accumulate() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        accumulator.record_uptime(7, 100, 120);
        accumulator.record_uptime(7, 80, 150);
        let metrics =
            accumulator.finalize_epoch(7, RelayComplianceStatusV1::Clean, 5, Metadata::default());
        assert_eq!(metrics.uptime_seconds, 180);
        assert_eq!(metrics.scheduled_uptime_seconds, 270);
    }

    #[test]
    fn uptime_overflow_saturates() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        accumulator.record_uptime(7, u64::MAX, u64::MAX - 1);
        accumulator.record_uptime(7, 1, 2);

        let metrics =
            accumulator.finalize_epoch(7, RelayComplianceStatusV1::Clean, 0, Metadata::default());
        assert_eq!(metrics.uptime_seconds, u64::MAX);
        assert_eq!(metrics.scheduled_uptime_seconds, u64::MAX);
    }

    #[test]
    fn bandwidth_overflow_saturates() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        assert!(accumulator.ingest_bandwidth_proof(&proof([1; 32], 9, u128::MAX, 900,)));
        assert!(accumulator.ingest_bandwidth_proof(&proof([2; 32], 9, 1, 850)));

        let metrics =
            accumulator.finalize_epoch(9, RelayComplianceStatusV1::Clean, 0, Metadata::default());
        assert_eq!(metrics.verified_bandwidth_bytes, u128::MAX);
        assert_eq!(metrics.confidence_floor_per_mille, 850);
    }

    #[test]
    fn exact_measurement_and_epoch_corridors_fail_before_mutation() {
        let mut accumulator = RelayPerformanceAccumulator::with_limits(RELAY, 2, 2);
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([2; 32], 5, 20, 900)),
            Ok(BandwidthProofIngest::Accepted)
        );
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([1; 32], 5, 10, 800)),
            Ok(BandwidthProofIngest::Accepted)
        );
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([3; 32], 5, 30, 700)),
            Err(IncentiveCapacityError::MeasurementsPerEpoch { epoch: 5, limit: 2 })
        );
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([4; 32], 6, 40, 700)),
            Ok(BandwidthProofIngest::Accepted)
        );
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([5; 32], 7, 50, 700)),
            Err(IncentiveCapacityError::ActiveEpochs { limit: 2 })
        );

        let summary = accumulator
            .summary(5)
            .expect("bounded summary allocation")
            .expect("epoch exists");
        assert_eq!(summary.measurement_ids, vec![[1; 32], [2; 32]]);
        assert_eq!(summary.verified_bandwidth_bytes, 30);
        assert_eq!(accumulator.epochs.len(), 2);
        assert_eq!(accumulator.total_measurements, 3);
    }

    #[test]
    fn finalization_releases_aggregate_and_epoch_capacity() {
        let mut accumulator = RelayPerformanceAccumulator::with_limits(RELAY, 1, 1);
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([1; 32], 5, 10, 900)),
            Ok(BandwidthProofIngest::Accepted)
        );
        let finalized =
            accumulator.finalize_epoch(5, RelayComplianceStatusV1::Clean, 0, Metadata::default());
        assert_eq!(finalized.measurement_ids, vec![[1; 32]]);
        assert_eq!(accumulator.total_measurements, 0);
        assert_eq!(
            accumulator.try_ingest_bandwidth_proof(&proof([2; 32], 6, 20, 800)),
            Ok(BandwidthProofIngest::Accepted)
        );
    }
}
