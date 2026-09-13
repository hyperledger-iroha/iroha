//! Shipping semantic forwarding controls with exact retained count ownership.
//!
//! These channel fixtures do not authenticate consensus statements or qualify
//! physical byte bounds. They drive the production nine-receiver future; the
//! attached real semaphore permits witness ownership through its handoffs.

use super::super::{
    RelayWorkItem, SumeragiRelayCapacityGeometry, SumeragiRelayIngress, SumeragiRelaySourceCredits,
    drive_semantic_network_relay_ingress,
};
use super::*;
use iroha_core::{NetworkMessage, retained_gossip::RetainedGossip};
use iroha_p2p::{TransportAdmissionClass as A, network::message::ClassifyTopic};
use tokio::sync::{Semaphore, mpsc};

fn retained_message(message: NetworkMessage, marker: usize) -> (RelayWorkItem, Arc<Semaphore>) {
    let count = Arc::new(Semaphore::new(1));
    let mut work = RelayWorkItem::new(sample_peer(), message, marker);
    work.retain_authenticated_source_credit(Arc::clone(&count).try_acquire_owned().unwrap());
    (work, count)
}

fn sidecar_chunk() -> NetworkMessage {
    use iroha_core::merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
        CertifiedMergeSidecarMessage, CertifiedMergeSidecarRequestV1,
        CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
        CertifiedMergeSidecarStreamEpochV1,
    };
    let mut request = CertifiedMergeSidecarRequestV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(NonZeroU64::new(1).unwrap()),
        semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(NonZeroU64::new(1).unwrap()),
        closed_through: 0,
        request_id: Hash::prehashed([0; Hash::LENGTH]),
        entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"semantic relay sidecar entry")),
        encoded_len: 1,
        epoch_id: 1,
        reference_digest: Hash::new(b"semantic relay sidecar reference"),
        requester: sample_peer().id().clone(),
        responder: sample_peer().id().clone(),
    };
    request.request_id = request.canonical_request_id();
    NetworkMessage::CertifiedMergeSidecar(Arc::new(CertifiedMergeSidecarMessage::Chunk(
        CertifiedMergeSidecarChunkV1 {
            version: request.version,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            semantic_sequence: request.semantic_sequence,
            request_id: request.request_id,
            entry_hash: request.entry_hash,
            encoded_len: request.encoded_len,
            epoch_id: request.epoch_id,
            reference_digest: request.reference_digest,
            requester: request.requester,
            responder: request.responder,
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![0xA5],
        },
    )))
}

#[tokio::test]
async fn shipping_semantic_ingress_bypasses_full_workers_and_preserves_exact_fifo_owners() {
    let channels: [_; A::COUNT] = std::array::from_fn(|_| mpsc::channel(2));
    let (senders, receivers): (Vec<_>, Vec<_>) = channels.into_iter().unzip();
    let receivers = receivers.try_into().ok().unwrap();
    let (high_tx, mut high_rx) = mpsc::channel(1);
    let (payload_tx, mut payload_rx) = mpsc::channel(1);
    let (chunk_tx, mut chunk_rx) = mpsc::channel(1);
    let (low_tx, mut low_rx) = mpsc::channel(1);
    // Saturate the four actual physical worker corridors without consuming a
    // Sumeragi source permit. These explicitly synthetic fillers are not
    // submitted through semantic classification.
    for sender in [&high_tx, &payload_tx, &chunk_tx, &low_tx] {
        sender
            .try_send(RelayWorkItem::new(sample_peer(), NetworkMessage::Health, 0))
            .unwrap();
    }
    let geometry = SumeragiRelayCapacityGeometry::checked(2, 8, 9).unwrap();
    let (v2, mut v2_rx) = mpsc::channel(geometry.class_capacity);
    let (lane, mut lane_rx) = mpsc::channel(geometry.class_capacity);
    let ingress = SumeragiRelayIngress {
        v2,
        lane,
        source_credits: SumeragiRelaySourceCredits::new(geometry),
    };
    let (_, close_ack, _) = certified_merge_sidecar_control_messages();
    let inputs = [
        (A::Safety, v2_vote_msg()),
        (
            A::Lane,
            sumeragi_msg(sumeragi_v2_commit_certificate_request()),
        ),
        (A::Payload, v2_certified_body_response_msg()),
        (A::Payload, v2_certified_body_response_msg()),
        (
            A::Availability,
            sumeragi_msg(v2_payload_chunk_block_message()),
        ),
        (A::RecoveryControl, close_ack),
        (A::RecoveryData, sidecar_chunk()),
        (A::Control, torii_proxy_request_msg()),
        (A::Low, NetworkMessage::Health),
    ];
    let mut counts = Vec::new();
    for (index, (class, message)) in inputs.into_iter().enumerate() {
        assert_eq!(message.admission_class(), class);
        let (work, count) = retained_message(message, index + 1);
        senders[class.index()].try_send(work).unwrap();
        counts.push(count);
    }
    // The production Core message enum currently has no BlockSync variant.
    // Its ninth receiver remains open and empty; never forge Health as BlockSync.
    let workers = [
        &high_tx,
        &high_tx,
        &payload_tx,
        &chunk_tx,
        &high_tx,
        &chunk_tx,
        &high_tx,
        &low_tx,
        &low_tx,
    ];
    let pump = drive_semantic_network_relay_ingress(receivers, workers, &ingress);
    tokio::pin!(pump);
    let observe = async {
        let mut v2_items = Vec::new();
        for _ in 0..5 {
            v2_items.push(v2_rx.recv().await.unwrap());
        }
        let mut lane_items = Vec::new();
        for _ in 0..2 {
            lane_items.push(lane_rx.recv().await.unwrap());
        }
        assert!(counts.iter().all(|owner| owner.available_permits() == 0));
        let payload_order: Vec<_> = v2_items
            .iter()
            .filter(|item| item.work.work.payload.admission_class() == A::Payload)
            .map(|item| item.work.work.payload_bytes)
            .collect();
        assert_eq!(
            payload_order,
            [3, 4],
            "same-class FIFO survives inter-class polling"
        );
        assert!(
            v2_items
                .iter()
                .any(|item| item.work.work.payload.admission_class() == A::Availability)
        );
        let mut recovery_classes: Vec<_> = lane_items
            .iter()
            .map(|item| item.work.work.payload.admission_class().index())
            .collect();
        recovery_classes.sort_unstable();
        assert_eq!(
            recovery_classes,
            [A::RecoveryControl.index(), A::RecoveryData.index()]
        );
        // Every accepted item remains owned until its exact downstream retirement.
        for item in v2_items.into_iter().chain(lane_items) {
            let index = item.work.work.payload_bytes - 1;
            assert_eq!(counts[index].available_permits(), 0);
            drop(item);
            assert_eq!(counts[index].available_permits(), 1);
        }
        assert_eq!(
            counts[7].available_permits(),
            0,
            "blocked Control remains retained"
        );
        assert_eq!(
            counts[8].available_permits(),
            0,
            "blocked Low remains retained"
        );
        for receiver in [&mut high_rx, &mut payload_rx, &mut chunk_rx, &mut low_rx] {
            assert_eq!(receiver.try_recv().unwrap().payload_bytes, 0);
        }
        let control = high_rx.recv().await.unwrap();
        let low = low_rx.recv().await.unwrap();
        assert_eq!((control.payload_bytes, low.payload_bytes), (8, 9));
        assert!(payload_rx.try_recv().is_err());
        assert!(chunk_rx.try_recv().is_err());
        assert_eq!(counts[7].available_permits(), 0);
        assert_eq!(counts[8].available_permits(), 0);
        drop((control, low));
        assert!(counts.iter().all(|owner| owner.available_permits() == 1));
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = &mut pump => panic!("shipping pump must retain its live subscribers"),
            () = observe => {},
        }
    })
    .await
    .expect("all eight inhabited Core classes must make bounded progress");
    // All submitted owners are retired before dropping this borrowed pump.
    // Senders stay open throughout; no production fail-stop or task abort is used.
    assert_eq!(senders.len(), A::COUNT);
}

fn retire_public_gossip_owner<T>(owner: RetainedGossip<T>, count: &Semaphore) {
    assert_eq!(
        count.available_permits(),
        0,
        "public envelope owns the original permit"
    );
    let inline_owner = |owned: RetainedGossip<T>| {
        assert_eq!(
            count.available_permits(),
            0,
            "moving the opaque owner retains the permit"
        );
        drop(owned);
        assert_eq!(
            count.available_permits(),
            1,
            "opaque owner retirement releases once"
        );
    };
    inline_owner(owner);
}

#[tokio::test]
async fn shipping_semantic_ingress_keeps_gossip_owners_through_public_handoff_and_retirement() {
    use iroha_core::{
        gossiper::TransactionGossip,
        peers_gossiper::{PeerTrustGossip, PeersGossip},
    };
    let channels: [_; A::COUNT] = std::array::from_fn(|_| mpsc::channel(3));
    let (senders, receivers): (Vec<_>, Vec<_>) = channels.into_iter().unzip();
    let receivers = receivers.try_into().ok().unwrap();
    let (worker_tx, mut worker_rx) = mpsc::channel(1);
    let geometry = SumeragiRelayCapacityGeometry::checked(2, 3, 3).unwrap();
    let (v2, _v2_rx) = mpsc::channel(geometry.class_capacity);
    let (lane, _lane_rx) = mpsc::channel(geometry.class_capacity);
    let ingress = SumeragiRelayIngress {
        v2,
        lane,
        source_credits: SumeragiRelaySourceCredits::new(geometry),
    };
    let inputs = [
        NetworkMessage::TransactionGossiper(Arc::new(TransactionGossip::new(Vec::new()))),
        NetworkMessage::PeersGossiper(Box::new(PeersGossip {
            peers: Default::default(),
            peer_capabilities: Default::default(),
        })),
        NetworkMessage::PeerTrustGossip(Box::new(PeerTrustGossip {
            network_id: NetworkId::from_genesis_hash(dummy_block_hash(91)),
            trust: Vec::new(),
        })),
    ];
    let mut counts = Vec::new();
    for (index, message) in inputs.into_iter().enumerate() {
        assert_eq!(message.admission_class(), A::Low);
        let (work, count) = retained_message(message, index + 1);
        senders[A::Low.index()].try_send(work).unwrap();
        counts.push(count);
    }
    let pump = drive_semantic_network_relay_ingress(receivers, [&worker_tx; A::COUNT], &ingress);
    tokio::pin!(pump);
    let observe = async {
        for (index, count) in counts.iter().enumerate() {
            let message = worker_rx.recv().await.unwrap();
            assert_eq!(message.payload_bytes, index + 1);
            let (peer, _, payload, _, retention) = message.into_parts();
            assert_eq!(
                count.available_permits(),
                0,
                "unwrapping PeerMessage must retain ownership"
            );
            // Only the public constructor and opaque ownership handoff are
            // exercised here. Core owns inspection and physical callback tests.
            match payload {
                NetworkMessage::TransactionGossiper(data) => {
                    assert!(data.txs.is_empty());
                    retire_public_gossip_owner(RetainedGossip::new(data, retention), count);
                }
                NetworkMessage::PeersGossiper(data) => {
                    assert!(data.peers.is_empty());
                    retire_public_gossip_owner(
                        RetainedGossip::new((*data, peer), retention),
                        count,
                    );
                }
                NetworkMessage::PeerTrustGossip(data) => {
                    assert!(data.trust.is_empty());
                    retire_public_gossip_owner(
                        RetainedGossip::new((*data, peer), retention),
                        count,
                    );
                }
                _ => panic!("exact gossip fixture kind must survive forwarding"),
            }
            assert_eq!(count.available_permits(), 1, "opaque handoff owner retired");
        }
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = &mut pump => panic!("shipping pump must retain its live subscribers"),
            () = observe => {},
        }
    })
    .await
    .expect("the bounded Low corridor must forward each exact gossip owner");
    assert!(counts.iter().all(|owner| owner.available_permits() == 1));
    assert_eq!(senders.len(), A::COUNT);
}
