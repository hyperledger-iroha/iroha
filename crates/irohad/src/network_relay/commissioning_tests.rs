use std::sync::Arc;

use iroha_core::{
    NetworkMessage,
    gossiper::{GossipPlane, TransactionGossip},
    peers_gossiper::{PeerTrustGossip, PeersGossip},
    time::{TimePing, TimePong},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::NetworkId;
use iroha_p2p::{
    TransportAdmissionClass,
    network::{
        SubscriberFilter,
        message::{ClassifyTopic, SubscriberRoute, Topic},
    },
};
use iroha_torii_shared::connect::{ConnectP2pMessageV1, ConnectSessionTerminatedV1};
use norito::streaming::{ControlErrorFrame, ControlFrame, ErrorCode};

use super::{
    EMERGENCY_FAST_SUBSCRIBER_CAP, RelayStartup, Shared, accepts_message, early_subscription,
    subscriptions_after_torii,
};

#[test]
fn both_transaction_planes_are_eligible_before_torii_on_the_rate_limited_low_route() {
    for (plane, topic) in [
        (GossipPlane::Public, Topic::TxGossip),
        (GossipPlane::Restricted, Topic::TxGossipRestricted),
    ] {
        let mut batch = TransactionGossip::new(Vec::new());
        batch.plane = plane;
        let message = NetworkMessage::TransactionGossiper(Arc::new(batch));

        assert_eq!(message.topic(), topic);
        assert_eq!(message.subscriber_route(), SubscriberRoute::General);
        assert_eq!(message.admission_class(), TransportAdmissionClass::Low);
        assert!(Shared::low_priority(&message));
        assert!(accepts_message(false, &message));
        assert!(accepts_message(true, &message));
    }
}

#[test]
fn commissioning_keeps_peer_trust_time_and_streaming_work_behind_torii() {
    let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"relay commissioning fixture",
    )));
    let messages = [
        NetworkMessage::PeersGossiper(Box::new(PeersGossip {
            peers: Default::default(),
            peer_capabilities: Default::default(),
        })),
        NetworkMessage::PeerTrustGossip(Box::new(PeerTrustGossip {
            network_id,
            trust: Vec::new(),
        })),
        NetworkMessage::TimePing(Box::new(TimePing { id: 1, t1_ms: 2 })),
        NetworkMessage::TimePong(Box::new(TimePong {
            id: 1,
            t2_ms: 2,
            t3_ms: 3,
        })),
        NetworkMessage::Health,
        NetworkMessage::Connect(Box::new(ConnectP2pMessageV1::SessionTerminated(
            ConnectSessionTerminatedV1 {
                sid: [0; 32],
                reason: String::from("closed"),
            },
        ))),
        NetworkMessage::StreamingControl(Box::new(ControlFrame::Error(ControlErrorFrame {
            code: ErrorCode::ProtocolViolation,
            message: String::from("fixture"),
        }))),
    ];
    for message in messages {
        assert!(!accepts_message(false, &message), "{:?}", message.topic());
        assert!(accepts_message(true, &message), "{:?}", message.topic());
    }
}

#[test]
fn managed_startup_registers_one_bounded_low_fifo_across_both_phases() {
    let capacity = 7;
    let early =
        early_subscription(RelayStartup::Managed, capacity).expect("pre-Torii normal subscription");
    assert_eq!(
        early,
        (
            SubscriberFilter::semantic_class(TransportAdmissionClass::Low),
            capacity
        )
    );

    let late = subscriptions_after_torii(RelayStartup::Managed, capacity);
    let subscriptions: Vec<_> = std::iter::once(early).chain(late).collect();
    assert_eq!(subscriptions.len(), TransportAdmissionClass::ALL.len());
    for class in TransportAdmissionClass::ALL {
        let expected = (SubscriberFilter::semantic_class(class), capacity);
        assert_eq!(
            subscriptions
                .iter()
                .filter(|entry| **entry == expected)
                .count(),
            1,
            "{class:?}"
        );
    }
}

#[test]
fn ordinary_daemon_registers_all_original_classes_after_torii() {
    let capacity = 7;
    assert!(early_subscription(RelayStartup::Ordinary, capacity).is_none());
    let expected: Vec<_> = TransportAdmissionClass::ALL
        .into_iter()
        .map(|class| (SubscriberFilter::semantic_class(class), capacity))
        .collect();
    assert_eq!(
        subscriptions_after_torii(RelayStartup::Ordinary, capacity),
        expected
    );
}

#[test]
fn emergency_fast_waits_for_torii_and_registers_only_the_bounded_peer_trust_filter() {
    let capacity = 7;
    assert!(early_subscription(RelayStartup::EmergencyFast, capacity).is_none());
    assert_eq!(
        subscriptions_after_torii(RelayStartup::EmergencyFast, capacity),
        vec![(
            SubscriberFilter::topics([Topic::PeerGossip, Topic::TrustGossip]),
            EMERGENCY_FAST_SUBSCRIBER_CAP
        )]
    );
}

#[test]
fn daemon_commissions_the_relay_before_gateway_capture_then_opens_it_at_torii() {
    let startup = include_str!("../main.rs")
        .split_once("async fn activate_with_runtime_deps(")
        .expect("node activation entry")
        .1
        .split_once("pub fn soracloud_runtime(")
        .expect("end of node activation")
        .0;
    let mut remaining = startup;
    for milestone in [
        "TransactionGossiper::from_config(",
        ".commission(managed_launch, supervisor.shutdown_signal())",
        "supervisor.monitor(task::spawn(relay_task))",
        "sorafs_stream_token_gateway_runtime::prepare_capture(",
        "torii.start(",
        "torii_started.send(())",
    ] {
        remaining = remaining
            .split_once(milestone)
            .unwrap_or_else(|| {
                panic!("missing or out-of-order node activation milestone {milestone}")
            })
            .1;
    }
}
