//! Lane relay broadcaster for NX-4 cross-lane commitments.
//!
//! The broadcaster de-duplicates envelopes by `(lane_id, dataspace_id, block_height,
//! settlement_hash)`, validates each payload, persists it to the Sumeragi status snapshot,
//! and emits a high-priority control-plane frame so peers can ingest the relay evidence.

use std::collections::{BTreeMap, VecDeque};

use iroha_data_model::{
    block::consensus::LaneBlockCommitment,
    nexus::{
        DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayError, LaneRelayFastpqMaterialStatus,
    },
};
use iroha_p2p::{
    Broadcast, Priority,
    network::{
        NetworkActorAdmissionRejection, NetworkBroadcastAdmissionError,
        NetworkBroadcastAdmissionTicket, RELIABLE_PROGRESS_LANE_RELAY_OWNER_CAPACITY,
    },
};
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
    /// Opaque FIFO position retained across temporary actor backpressure.
    type RetryToken: Send + 'static;

    /// Try to transfer one validated relay envelope into the reliable actor corridor.
    fn try_broadcast_relay(
        &self,
        envelope: LaneRelayEnvelope,
        retry: Option<Self::RetryToken>,
    ) -> LaneRelaySendDisposition<Self::RetryToken>;
}

/// Ownership-preserving result of one lane-relay actor handoff.
pub enum LaneRelaySendDisposition<R> {
    /// The network actor accepted exact ownership.
    Accepted,
    /// Admission is temporarily full; the exact envelope and FIFO token are returned.
    Retry {
        /// Exact envelope which remains owned by the broadcaster.
        envelope: LaneRelayEnvelope,
        /// Stable actor-admission position, when the actor had ticket capacity.
        token: Option<R>,
    },
    /// The actor has terminated; this handle cannot make further progress.
    Closed {
        /// Exact envelope which never crossed admission.
        envelope: LaneRelayEnvelope,
    },
    /// The envelope permanently violates the actor admission contract.
    Rejected {
        /// Exact rejected envelope.
        envelope: LaneRelayEnvelope,
        /// Stable admission rejection reason.
        reason: NetworkActorAdmissionRejection,
    },
}

impl LaneRelayTx for IrohaNetwork {
    type RetryToken = NetworkBroadcastAdmissionTicket;

    fn try_broadcast_relay(
        &self,
        envelope: LaneRelayEnvelope,
        retry: Option<Self::RetryToken>,
    ) -> LaneRelaySendDisposition<Self::RetryToken> {
        let result = self.broadcast_recoverable(
            Broadcast {
                data: NetworkMessage::LaneRelay(Box::new(envelope)),
                priority: Priority::High,
            },
            retry,
        );
        match result {
            Ok(()) => LaneRelaySendDisposition::Accepted,
            Err(NetworkBroadcastAdmissionError::Backpressured {
                message, ticket, ..
            }) => LaneRelaySendDisposition::Retry {
                envelope: lane_relay_from_broadcast(message),
                token: Some(ticket),
            },
            Err(NetworkBroadcastAdmissionError::Closed { message, ticket: _ }) => {
                LaneRelaySendDisposition::Closed {
                    envelope: lane_relay_from_broadcast(message),
                }
            }
            Err(NetworkBroadcastAdmissionError::Rejected {
                message,
                ticket: _,
                reason,
            }) => LaneRelaySendDisposition::Rejected {
                envelope: lane_relay_from_broadcast(message),
                reason,
            },
        }
    }
}

fn lane_relay_from_broadcast(message: Broadcast<NetworkMessage>) -> LaneRelayEnvelope {
    match message.data {
        NetworkMessage::LaneRelay(envelope) => *envelope,
        _ => unreachable!("lane-relay admission must return the submitted lane relay"),
    }
}

struct PendingRelay<R> {
    envelope: LaneRelayEnvelope,
    token: Option<R>,
}

/// Terminal ownership returned by [`LaneRelayBroadcaster`].
#[derive(Debug)]
pub enum LaneRelayBroadcastError {
    /// All bounded semantic owners are occupied by undelivered relays.
    /// Resubmit the returned envelope through [`LaneRelayBroadcaster::broadcast`]
    /// after servicing the broadcaster's retained work.
    Capacity {
        /// Exact relay which was not admitted into the broadcaster.
        envelope: LaneRelayEnvelope,
    },
    /// The network actor has terminated.
    Closed {
        /// Exact relay which was not admitted into the actor.
        envelope: LaneRelayEnvelope,
    },
    /// The reliable actor rejected a protocol-valid relay permanently.
    Rejected {
        /// Exact rejected relay.
        envelope: LaneRelayEnvelope,
        /// Stable actor-admission reason.
        reason: NetworkActorAdmissionRejection,
    },
}

/// Broadcasts validated lane relay envelopes and records them in the local status snapshot.
pub struct LaneRelayBroadcaster<N: LaneRelayTx> {
    network: N,
    seen: BTreeMap<LaneRelayKey, LaneRelayFastpqMaterialStatus>,
    seen_order: VecDeque<LaneRelayKey>,
    pending: BTreeMap<LaneRelayKey, PendingRelay<N::RetryToken>>,
    pending_order: VecDeque<LaneRelayKey>,
}

impl<N: LaneRelayTx> LaneRelayBroadcaster<N> {
    /// Create a new broadcaster.
    #[must_use]
    pub fn new(network: N) -> Self {
        Self {
            network,
            seen: BTreeMap::new(),
            seen_order: VecDeque::new(),
            pending: BTreeMap::new(),
            pending_order: VecDeque::new(),
        }
    }

    /// Validate, de-duplicate, record, and retain the provided envelopes until actor handoff.
    ///
    /// Every capacity or terminal transport failure returns the exact unadmitted envelope. The
    /// caller must retain those errors across network restart or apply its own fail-stop policy.
    /// Temporary pressure remains owned internally with its actor FIFO ticket.
    #[must_use = "terminal lane-relay ownership must be handled"]
    pub fn broadcast(
        &mut self,
        envelopes: impl IntoIterator<Item = LaneRelayEnvelope>,
    ) -> Result<usize, Vec<LaneRelayBroadcastError>> {
        let mut errors = Vec::new();
        for envelope in envelopes {
            if let Err(err) = envelope.verify().and_then(|()| {
                if envelope.fastpq_proof.is_some() {
                    envelope.validate_fastpq_proof_metadata()
                } else {
                    Ok(())
                }
            }) {
                record_relay_error(&err);
                iroha_logger::warn!(
                    lane_id = %envelope.lane_id,
                    dataspace_id = %envelope.dataspace_id,
                    block_height = envelope.block_height,
                    error_kind = err.as_label(),
                    error = %err,
                    "dropping structurally invalid lane relay envelope before broadcast"
                );
                continue;
            }
            let key = LaneRelayKey::from_envelope(&envelope);
            let metadata_status = envelope.fastpq_metadata_status();
            if self.seen.get(&key).is_some_and(|existing| {
                *existing == LaneRelayFastpqMaterialStatus::Present
                    || metadata_status == LaneRelayFastpqMaterialStatus::Missing
            }) {
                continue;
            }

            if !self.seen.contains_key(&key)
                && self.seen.len() >= RELIABLE_PROGRESS_LANE_RELAY_OWNER_CAPACITY
            {
                let evictable = self
                    .seen_order
                    .iter()
                    .position(|candidate| !self.pending.contains_key(candidate));
                let Some(position) = evictable else {
                    // The bounded local owner set is the actor's mechanical
                    // waiter reserve. Returning before actor admission keeps a
                    // sixty-fifth residual from acquiring an unreserved
                    // target ticket. The caller retains the exact envelope and
                    // may resubmit it after `retry_pending` releases a slot.
                    status::push_lane_relay_envelope(envelope.clone());
                    errors.push(LaneRelayBroadcastError::Capacity { envelope });
                    continue;
                };
                let evicted = self
                    .seen_order
                    .remove(position)
                    .expect("located delivered relay must remain in seen order");
                self.seen.remove(&evicted);
            }
            if !self.seen.contains_key(&key) {
                self.seen_order.push_back(key);
            }
            self.seen.insert(key, metadata_status);
            status::push_lane_relay_envelope(envelope.clone());
            if let Some(pending) = self.pending.get_mut(&key) {
                // An upgraded proof has a different exact actor shape; dropping the old token
                // cancels its rank before the replacement acquires a fresh one.
                pending.envelope = envelope;
                pending.token = None;
            } else {
                self.pending.insert(
                    key,
                    PendingRelay {
                        envelope,
                        token: None,
                    },
                );
                self.pending_order.push_back(key);
            }
        }

        let transferred = self.retry_pending_inner(&mut errors);
        if errors.is_empty() {
            Ok(transferred)
        } else {
            Err(errors)
        }
    }

    /// Retry every currently owned relay at most once in round-robin order.
    #[must_use = "terminal lane-relay ownership must be handled"]
    pub fn retry_pending(&mut self) -> Result<usize, Vec<LaneRelayBroadcastError>> {
        let mut errors = Vec::new();
        let transferred = self.retry_pending_inner(&mut errors);
        if errors.is_empty() {
            Ok(transferred)
        } else {
            Err(errors)
        }
    }

    fn retry_pending_inner(&mut self, errors: &mut Vec<LaneRelayBroadcastError>) -> usize {
        let attempts = self.pending_order.len();
        let mut transferred = 0usize;
        for _ in 0..attempts {
            let Some(key) = self.pending_order.pop_front() else {
                break;
            };
            let Some(PendingRelay { envelope, token }) = self.pending.remove(&key) else {
                continue;
            };
            match self.network.try_broadcast_relay(envelope, token) {
                LaneRelaySendDisposition::Accepted => {
                    transferred = transferred.saturating_add(1);
                }
                LaneRelaySendDisposition::Retry { envelope, token } => {
                    self.pending.insert(key, PendingRelay { envelope, token });
                    self.pending_order.push_back(key);
                }
                LaneRelaySendDisposition::Closed { envelope } => {
                    self.seen.remove(&key);
                    self.seen_order.retain(|candidate| *candidate != key);
                    errors.push(LaneRelayBroadcastError::Closed { envelope });
                }
                LaneRelaySendDisposition::Rejected { envelope, reason } => {
                    self.seen.remove(&key);
                    self.seen_order.retain(|candidate| *candidate != key);
                    errors.push(LaneRelayBroadcastError::Rejected { envelope, reason });
                }
            }
        }
        transferred
    }

    /// Number of exact relays still waiting for actor ownership.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::NonZeroU64,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use iroha_crypto::{Hash as UntypedHash, HashOf};
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus::{CertPhase, LaneBlockCommitment, LaneSettlementReceipt, Qc, QcAggregate},
        },
        nexus::{DataSpaceId, LaneFastpqProofMaterial, LaneId, LaneRelayEnvelope},
    };

    use super::{
        LaneRelayBroadcastError, LaneRelayBroadcaster, LaneRelaySendDisposition, LaneRelayTx,
    };
    use iroha_p2p::network::RELIABLE_PROGRESS_LANE_RELAY_OWNER_CAPACITY;

    #[derive(Clone, Default)]
    struct MockNetwork {
        sent: Arc<Mutex<Vec<LaneRelayEnvelope>>>,
    }

    impl LaneRelayTx for MockNetwork {
        type RetryToken = ();

        fn try_broadcast_relay(
            &self,
            envelope: LaneRelayEnvelope,
            _retry: Option<Self::RetryToken>,
        ) -> super::LaneRelaySendDisposition<Self::RetryToken> {
            self.sent
                .lock()
                .expect("mock network mutex poisoned")
                .push(envelope);
            super::LaneRelaySendDisposition::Accepted
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

    #[derive(Clone, Default)]
    struct BackpressureOnceNetwork {
        attempts: Arc<AtomicUsize>,
        sent: Arc<Mutex<Vec<LaneRelayEnvelope>>>,
    }

    impl LaneRelayTx for BackpressureOnceNetwork {
        type RetryToken = u64;

        fn try_broadcast_relay(
            &self,
            envelope: LaneRelayEnvelope,
            retry: Option<Self::RetryToken>,
        ) -> LaneRelaySendDisposition<Self::RetryToken> {
            if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                assert!(retry.is_none());
                return LaneRelaySendDisposition::Retry {
                    envelope,
                    token: Some(17),
                };
            }
            assert_eq!(retry, Some(17), "retry must retain the actor FIFO token");
            self.sent
                .lock()
                .expect("backpressure mock mutex poisoned")
                .push(envelope);
            LaneRelaySendDisposition::Accepted
        }
    }

    #[derive(Clone, Default)]
    struct AlwaysBackpressuredNetwork {
        attempted_heights: Arc<Mutex<Vec<u64>>>,
    }

    impl LaneRelayTx for AlwaysBackpressuredNetwork {
        type RetryToken = u64;

        fn try_broadcast_relay(
            &self,
            envelope: LaneRelayEnvelope,
            retry: Option<Self::RetryToken>,
        ) -> LaneRelaySendDisposition<Self::RetryToken> {
            self.attempted_heights
                .lock()
                .expect("attempt history mutex poisoned")
                .push(envelope.block_height);
            LaneRelaySendDisposition::Retry {
                envelope,
                token: retry.or(Some(1)),
            }
        }
    }

    #[derive(Clone, Default)]
    struct FirstLaneBackpressuredNetwork {
        sent: Arc<Mutex<Vec<LaneRelayEnvelope>>>,
    }

    impl LaneRelayTx for FirstLaneBackpressuredNetwork {
        type RetryToken = u64;

        fn try_broadcast_relay(
            &self,
            envelope: LaneRelayEnvelope,
            retry: Option<Self::RetryToken>,
        ) -> LaneRelaySendDisposition<Self::RetryToken> {
            if envelope.lane_id == LaneId::new(0) {
                return LaneRelaySendDisposition::Retry {
                    envelope,
                    token: retry.or(Some(1)),
                };
            }
            self.sent
                .lock()
                .expect("selective backpressure mock mutex poisoned")
                .push(envelope);
            LaneRelaySendDisposition::Accepted
        }
    }

    #[derive(Clone, Copy)]
    enum TerminalNetwork {
        Closed,
        Rejected,
    }

    impl LaneRelayTx for TerminalNetwork {
        type RetryToken = ();

        fn try_broadcast_relay(
            &self,
            envelope: LaneRelayEnvelope,
            _retry: Option<Self::RetryToken>,
        ) -> LaneRelaySendDisposition<Self::RetryToken> {
            match self {
                Self::Closed => LaneRelaySendDisposition::Closed { envelope },
                Self::Rejected => LaneRelaySendDisposition::Rejected {
                    envelope,
                    reason: iroha_p2p::network::NetworkActorAdmissionRejection::OutboundDisallowed,
                },
            }
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
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::new(u64::from(lane) + 1),
            tx_count: 1,
            total_local_amount: "0.000001".parse().expect("valid settlement quantity"),
            total_xor_due: "0.000002".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0.000002".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: vec![LaneSettlementReceipt {
                source_id: [0x01; 32],
                local_amount: "0.000001".parse().expect("valid settlement quantity"),
                xor_due: "0.000002".parse().expect("valid settlement quantity"),
                xor_after_haircut: "0.000002".parse().expect("valid settlement quantity"),
                xor_variance: "0".parse().expect("valid settlement quantity"),
                timestamp_ms: 1_700_000_000_000,
            }],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let envelope = LaneRelayEnvelope::new(header, None, None, settlement, 0)
            .expect("valid envelope")
            .with_lane_block_descriptor_hash(Some(UntypedHash::new(
                b"lane-relay-broadcaster-test-descriptor",
            )))
            .with_manifest_root(Some([0x44; 32]));
        let verified_at_height = height;
        let proof_digest = UntypedHash::new(
            format!("lane-relay-test-proof:{height}:{lane}:{verified_at_height}").as_bytes(),
        );
        envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest,
            verified_at_height,
        }))
    }

    fn attach_qc(envelope: &mut LaneRelayEnvelope) {
        envelope.qc = Some(Qc {
            phase: CertPhase::Commit,
            subject_block_hash: envelope.block_header.hash(),
            parent_state_root: UntypedHash::prehashed([0x11; UntypedHash::LENGTH]),
            post_state_root: UntypedHash::prehashed([0x12; UntypedHash::LENGTH]),
            height: envelope.block_header.height().get(),
            view: 0,
            epoch: 0,
            chain_order_hash: UntypedHash::prehashed([0x13; UntypedHash::LENGTH]),
            rechain_seq: 0,
            mode_tag: envelope
                .lane_finality_qc_mode_tag("test-mode")
                .expect("complete test finality statement"),
            highest_qc: None,
            validator_set_hash: HashOf::from_untyped_unchecked(UntypedHash::prehashed(
                [0x14; UntypedHash::LENGTH],
            )),
            validator_set_hash_version: 1,
            validator_set: Vec::new(),
            aggregate: QcAggregate {
                signers_bitmap: vec![0b0000_0111],
                bls_aggregate_signature: vec![0xAA; 96],
            },
        });
    }

    #[test]
    fn broadcaster_deduplicates_verified_envelopes() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        // Ensure a clean slate for the shared status snapshot.
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = MockNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());

        let mut envelope = sample_envelope(1, 3);
        attach_qc(&mut envelope);
        broadcaster
            .broadcast(vec![envelope.clone(), envelope.clone()])
            .expect("mock actor accepts relay");

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

        broadcaster
            .broadcast(vec![envelope])
            .expect("invalid relays are terminally filtered before actor admission");

        assert!(network.sent().is_empty());
        assert!(crate::sumeragi::status::lane_relay_envelopes_snapshot().is_empty());
    }

    #[test]
    fn broadcaster_records_and_broadcasts_qcless_pending_relay() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = MockNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());

        let envelope = sample_envelope(3, 5);
        let mut missing_proof = envelope.clone();
        missing_proof.fastpq_proof = None;

        broadcaster
            .broadcast(vec![missing_proof])
            .expect("mock actor accepts relay");

        let sent = network.sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].fastpq_proof.is_none());
        assert!(sent[0].qc.is_none());
        let snapshot = crate::sumeragi::status::lane_relay_envelopes_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].fastpq_proof.is_none());
        assert!(snapshot[0].qc.is_none());
    }

    #[test]
    fn broadcaster_broadcasts_qc_backed_pending_then_upgrades_verified_relay() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = MockNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());

        let mut envelope = sample_envelope(3, 5);
        attach_qc(&mut envelope);
        let mut missing_proof = envelope.clone();
        missing_proof.fastpq_proof = None;

        broadcaster
            .broadcast(vec![missing_proof])
            .expect("mock actor accepts pending relay");
        let sent = network.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].block_height, 3);
        assert!(sent[0].fastpq_proof.is_none());
        assert!(sent[0].qc.is_some());
        let snapshot = crate::sumeragi::status::lane_relay_envelopes_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].fastpq_proof.is_none());
        assert!(snapshot[0].qc.is_some());

        broadcaster
            .broadcast(vec![envelope])
            .expect("mock actor accepts upgraded relay");
        let sent = network.sent();
        assert_eq!(sent.len(), 2);
        assert!(sent[1].fastpq_proof.is_some());
        let snapshot = crate::sumeragi::status::lane_relay_envelopes_snapshot();
        assert_eq!(
            snapshot.len(),
            1,
            "status keeps one relay per key and upgrades pending to verified"
        );
        assert!(snapshot[0].fastpq_proof.is_some());
    }

    #[test]
    fn broadcaster_does_not_downgrade_verified_status_with_pending_duplicate() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = MockNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());

        let verified = sample_envelope(4, 6);
        let mut pending_duplicate = verified.clone();
        pending_duplicate.fastpq_proof = None;

        broadcaster
            .broadcast(vec![verified])
            .expect("mock actor accepts verified relay");
        let snapshot = crate::sumeragi::status::lane_relay_envelopes_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].fastpq_proof.is_some());

        broadcaster
            .broadcast(vec![pending_duplicate])
            .expect("pending downgrade is ignored before actor admission");

        let sent = network.sent();
        assert_eq!(sent.len(), 1);
        let snapshot = crate::sumeragi::status::lane_relay_envelopes_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!(
            snapshot[0].fastpq_proof.is_some(),
            "pending duplicate must not replace retained verified proof material"
        );
    }

    #[test]
    fn actor_backpressure_retains_exact_relay_and_fifo_ticket() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = BackpressureOnceNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());
        let envelope = sample_envelope(5, 7);

        assert_eq!(
            broadcaster
                .broadcast([envelope.clone()])
                .expect("temporary pressure remains internally owned"),
            0
        );
        assert_eq!(broadcaster.pending_len(), 1);
        assert_eq!(
            broadcaster
                .retry_pending()
                .expect("second actor attempt succeeds"),
            1
        );
        assert_eq!(broadcaster.pending_len(), 0);
        assert_eq!(
            network
                .sent
                .lock()
                .expect("backpressure mock mutex poisoned")
                .as_slice(),
            &[envelope]
        );
    }

    #[test]
    fn blocked_relay_does_not_starve_a_responsive_relay() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = FirstLaneBackpressuredNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());
        let blocked = sample_envelope(6, 0);
        let responsive = sample_envelope(6, 1);

        assert_eq!(
            broadcaster
                .broadcast([blocked])
                .expect("temporary pressure remains internally owned"),
            0
        );
        assert_eq!(
            broadcaster
                .broadcast([responsive.clone()])
                .expect("independent relay is accepted"),
            1
        );
        assert_eq!(broadcaster.pending_len(), 1);
        assert_eq!(
            network
                .sent
                .lock()
                .expect("selective backpressure mock mutex poisoned")
                .as_slice(),
            &[responsive]
        );
    }

    #[test]
    fn terminal_actor_failures_return_exact_relay_ownership() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());

        for terminal in [TerminalNetwork::Closed, TerminalNetwork::Rejected] {
            let mut broadcaster = LaneRelayBroadcaster::new(terminal);
            let envelope = sample_envelope(7, 8);
            let errors = broadcaster
                .broadcast([envelope.clone()])
                .expect_err("terminal actor failure must return exact ownership");
            assert_eq!(errors.len(), 1);
            match &errors[0] {
                LaneRelayBroadcastError::Closed { envelope: returned }
                | LaneRelayBroadcastError::Rejected {
                    envelope: returned, ..
                } => assert_eq!(returned, &envelope),
                LaneRelayBroadcastError::Capacity { .. } => {
                    panic!("terminal actor failure cannot become local capacity pressure")
                }
            }
            assert_eq!(broadcaster.pending_len(), 0);
        }
    }

    #[test]
    fn saturated_relay_owner_returns_sixty_fifth_without_actor_ticket() {
        let _guard = crate::sumeragi::status::lane_relay_test_guard();
        crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
        let network = AlwaysBackpressuredNetwork::default();
        let mut broadcaster = LaneRelayBroadcaster::new(network.clone());
        for index in 0..RELIABLE_PROGRESS_LANE_RELAY_OWNER_CAPACITY {
            broadcaster
                .broadcast([sample_envelope(
                    u64::try_from(index).expect("small index") + 1,
                    u32::try_from(index).expect("small index"),
                )])
                .expect("the bounded owner has an exact slot");
        }
        assert_eq!(
            broadcaster.pending_len(),
            RELIABLE_PROGRESS_LANE_RELAY_OWNER_CAPACITY
        );

        let overflow = sample_envelope(10_000, 10_000);
        let errors = broadcaster
            .broadcast([overflow.clone()])
            .expect_err("all bounded owners are occupied");
        assert_eq!(errors.len(), 1);
        let returned = match errors.into_iter().next().expect("one exact error") {
            LaneRelayBroadcastError::Capacity { envelope } => envelope,
            other => panic!("expected exact capacity return, got {other:?}"),
        };
        assert_eq!(returned, overflow);
        assert!(
            !network
                .attempted_heights
                .lock()
                .expect("attempt history mutex poisoned")
                .contains(&10_000),
            "a sixty-fifth producer residual must not acquire an unreserved actor ticket"
        );
        assert_eq!(
            broadcaster.pending_len(),
            RELIABLE_PROGRESS_LANE_RELAY_OWNER_CAPACITY
        );
    }
}
