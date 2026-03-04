//! Relay incentive helpers used to aggregate blinded measurement proofs and uptime reports.
//!
//! The accumulator provides a lightweight in-process view over the measurements required by
//! SNNet-7 so the relay runtime can surface Norito-friendly `RelayEpochMetricsV1` payloads for the
//! treasury pipeline. It deduplicates measurement flows, tracks the minimum confidence across
//! samples, and packages the results into the canonical data-model structures.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    metadata::Metadata,
    soranet::{
        incentives::{MeasurementId, RelayBandwidthProofV1},
        prelude::{RelayComplianceStatusV1, RelayEpochMetricsV1, RelayId},
    },
};

/// Per-epoch accumulation of relay performance signals.
#[derive(Debug, Default)]
struct RelayEpochAccumulator {
    uptime_seconds: u64,
    scheduled_uptime_seconds: u64,
    verified_bandwidth_bytes: u128,
    measurement_ids: BTreeSet<MeasurementId>,
    confidence_floor_per_mille: u16,
}

impl RelayEpochAccumulator {
    fn update_uptime(&mut self, uptime_seconds: u64, scheduled_uptime_seconds: u64) {
        self.uptime_seconds = self.uptime_seconds.strict_add(uptime_seconds);
        self.scheduled_uptime_seconds = self
            .scheduled_uptime_seconds
            .strict_add(scheduled_uptime_seconds);
    }

    fn ingest_proof(&mut self, proof: &RelayBandwidthProofV1) -> bool {
        if !self.measurement_ids.insert(proof.measurement_id) {
            return false;
        }
        self.verified_bandwidth_bytes = self
            .verified_bandwidth_bytes
            .strict_add(proof.verified_bytes);
        self.confidence_floor_per_mille = if self.measurement_ids.len() == 1 {
            proof.confidence.confidence_per_mille
        } else {
            self.confidence_floor_per_mille
                .min(proof.confidence.confidence_per_mille)
        };
        true
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
            measurement_ids: self.measurement_ids.into_iter().collect(),
            metadata,
        }
    }
}

/// Aggregates blinded measurement proofs and uptime counters for a specific relay.
#[derive(Debug)]
pub struct RelayPerformanceAccumulator {
    relay_id: RelayId,
    epochs: BTreeMap<u32, RelayEpochAccumulator>,
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
        Self {
            relay_id,
            epochs: BTreeMap::new(),
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
        let accumulator = self.epochs.entry(epoch).or_default();
        accumulator.update_uptime(uptime_seconds, scheduled_uptime_seconds);
    }

    /// Adds a blinded bandwidth proof to the accumulator.
    ///
    /// Returns `true` when the proof was accepted (i.e., not a duplicate and targeting this relay).
    pub fn ingest_bandwidth_proof(&mut self, proof: &RelayBandwidthProofV1) -> bool {
        if proof.relay_id != self.relay_id {
            return false;
        }
        let accumulator = self.epochs.entry(proof.epoch).or_default();
        accumulator.ingest_proof(proof)
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
        let accumulator = self.epochs.remove(&epoch).unwrap_or_default();
        accumulator.into_metrics(self.relay_id, epoch, compliance, reward_score, metadata)
    }

    /// Returns a snapshot of the currently accumulated epoch data.
    #[must_use]
    pub fn summaries(&self) -> Vec<EpochSummary> {
        self.epochs
            .iter()
            .map(|(&epoch, accumulator)| EpochSummary {
                epoch,
                uptime_seconds: accumulator.uptime_seconds,
                scheduled_uptime_seconds: accumulator.scheduled_uptime_seconds,
                verified_bandwidth_bytes: accumulator.verified_bandwidth_bytes,
                confidence_floor_per_mille: accumulator.confidence_floor_per_mille,
                measurement_ids: accumulator.measurement_ids.iter().copied().collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{account::AccountId, domain::DomainId, metadata::Metadata};

    use super::*;

    const RELAY: RelayId = [7_u8; 32];

    fn sample_account(seed: u8) -> AccountId {
        let domain = DomainId::from_str("sora").expect("domain id");
        let (public_key, _) = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519).into_parts();
        AccountId::new(domain, public_key)
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
            signature: Signature::from_bytes(&[1_u8; 64]),
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
    #[should_panic]
    fn uptime_overflow_panics() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        accumulator.record_uptime(7, u64::MAX, 0);
        accumulator.record_uptime(7, 1, 0);
    }

    #[test]
    #[should_panic]
    fn bandwidth_overflow_panics() {
        let mut accumulator = RelayPerformanceAccumulator::new(RELAY);
        assert!(accumulator.ingest_bandwidth_proof(&proof([1; 32], 9, u128::MAX, 900,)));
        accumulator.ingest_bandwidth_proof(&proof([2; 32], 9, 1, 850));
    }
}
