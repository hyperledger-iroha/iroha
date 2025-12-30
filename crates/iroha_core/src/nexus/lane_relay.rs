//! Lane relay broadcaster for NX-4 cross-lane commitments.
//!
//! The broadcaster de-duplicates envelopes by `(lane_id, dataspace_id, block_height,
//! settlement_hash)`, validates each payload, persists it to the Sumeragi status snapshot,
//! and emits a high-priority control-plane frame so peers can ingest the relay evidence.

use std::collections::BTreeSet;

use iroha_data_model::{
    block::consensus::LaneBlockCommitment,
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayError},
};
use iroha_p2p::{Broadcast, Priority};
use iroha_telemetry::metrics;

use crate::{IrohaNetwork, NetworkMessage, sumeragi::status};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct LaneRelayKey {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    block_height: u64,
    settlement_hash: iroha_crypto::HashOf<LaneBlockCommitment>,
}

impl LaneRelayKey {
    fn from_envelope(envelope: &LaneRelayEnvelope) -> Self {
        Self {
            lane_id: envelope.lane_id,
            dataspace_id: envelope.dataspace_id,
            block_height: envelope.block_height,
            settlement_hash: envelope.settlement_hash,
        }
    }
}

fn record_relay_error(err: &LaneRelayError) {
    if let Some(metrics) = metrics::global() {
        metrics
            .lane_relay_invalid_total
            .with_label_values(&[err.as_label()])
            .inc();
    }
}

/// Minimal interface required to broadcast relay envelopes.
pub trait LaneRelayTx: Clone + Send + Sync + 'static {
    /// Broadcast a validated relay envelope to peers.
    fn broadcast_relay(&self, envelope: LaneRelayEnvelope);
}

impl LaneRelayTx for IrohaNetwork {
    fn broadcast_relay(&self, envelope: LaneRelayEnvelope) {
        self.broadcast(Broadcast {
            data: NetworkMessage::LaneRelay(Box::new(envelope)),
            priority: Priority::High,
        });
    }
}

/// Broadcasts validated lane relay envelopes and records them in the local status snapshot.
#[derive(Clone)]
pub struct LaneRelayBroadcaster<N: LaneRelayTx> {
    network: N,
    seen: BTreeSet<LaneRelayKey>,
}

impl<N: LaneRelayTx> LaneRelayBroadcaster<N> {
    /// Create a new broadcaster.
    #[must_use]
    pub fn new(network: N) -> Self {
        Self {
            network,
            seen: BTreeSet::new(),
        }
    }

    /// Validate, de-duplicate, record, and broadcast the provided envelopes.
    pub fn broadcast(&mut self, envelopes: impl IntoIterator<Item = LaneRelayEnvelope>) {
        for envelope in envelopes {
            if let Err(err) = envelope.verify() {
                record_relay_error(&err);
                iroha_logger::warn!(
                    lane_id = %envelope.lane_id,
                    dataspace_id = %envelope.dataspace_id,
                    block_height = envelope.block_height,
                    error_kind = err.as_label(),
                    error = %err,
                    "dropping invalid lane relay envelope before broadcast"
                );
                continue;
            }
            let key = LaneRelayKey::from_envelope(&envelope);
            if !self.seen.insert(key) {
                continue;
            }
            status::push_lane_relay_envelope(envelope.clone());
            self.network.broadcast_relay(envelope);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::NonZeroU64,
        sync::{Arc, Mutex},
    };

    use iroha_crypto::{Hash as UntypedHash, HashOf};
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        },
        nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    };

    use super::{LaneRelayBroadcaster, LaneRelayTx};

    #[derive(Clone, Default)]
    struct MockNetwork {
        sent: Arc<Mutex<Vec<LaneRelayEnvelope>>>,
    }

    impl LaneRelayTx for MockNetwork {
        fn broadcast_relay(&self, envelope: LaneRelayEnvelope) {
            self.sent
                .lock()
                .expect("mock network mutex poisoned")
                .push(envelope);
        }
    }

    impl MockNetwork {
        fn sent(&self) -> Vec<LaneRelayEnvelope> {
            self.sent
                .lock()
                .expect("mock network mutex poisoned")
                .clone()
        }
    }

    fn sample_envelope(height: u64, lane: u32) -> LaneRelayEnvelope {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        let settlement = LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::new(lane),
            dataspace_id: DataSpaceId::new(u64::from(lane) + 1),
            tx_count: 1,
            total_local_micro: 1,
            total_xor_due_micro: 2,
            total_xor_after_haircut_micro: 2,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: vec![LaneSettlementReceipt {
                source_id: [0x01; 32],
                local_amount_micro: 1,
                xor_due_micro: 2,
                xor_after_haircut_micro: 2,
                xor_variance_micro: 0,
                timestamp_ms: 1_700_000_000_000,
            }],
        };
        LaneRelayEnvelope::new(header, None, None, settlement, 0).expect("valid envelope")
    }

    #[test]
    fn broadcaster_deduplicates_verified_envelopes() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        // Ensure a clean slate for the shared status snapshot.
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = MockNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());

        let envelope = sample_envelope(1, 3);
        broadcaster.broadcast(vec![envelope.clone(), envelope.clone()]);

        let sent = network.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].block_height, 1);

        let snapshot = crate::sumeragi::status::lane_relay_envelopes_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].lane_id, LaneId::new(3));
    }

    #[test]
    fn broadcaster_skips_invalid_envelopes() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = MockNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());

        let mut envelope = sample_envelope(2, 4);
        envelope.da_commitment_hash = Some(HashOf::from_untyped_unchecked(UntypedHash::prehashed(
            [0xAB; UntypedHash::LENGTH],
        )));

        broadcaster.broadcast(vec![envelope]);

        assert!(network.sent().is_empty());
        assert!(crate::sumeragi::status::lane_relay_envelopes_snapshot().is_empty());
    }
}
