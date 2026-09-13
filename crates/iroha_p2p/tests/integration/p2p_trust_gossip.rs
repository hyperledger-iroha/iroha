//! Trust-gossip capability gating integration tests.
#![allow(unexpected_cfgs)]
use super::next_port;
use iroha_config::parameters::actual::Network as Config;
use iroha_config::parameters::defaults::network::TRUST_GOSSIP;
use iroha_data_model::prelude::Peer;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::test_logger;
use iroha_p2p::{
    NetworkHandle,
    network::message::{ClassifyTopic, Post, Priority, Topic, UpdatePeers, UpdateTopology},
    peer::message::PeerMessage,
};
use iroha_primitives::addr::{SocketAddr, socket_addr};
use norito::codec::{Decode, Encode};
use std::{collections::HashSet, num::NonZeroUsize};
use tokio::{sync::mpsc, time::Duration};
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_p2p::tests::integration::p2p_trust_gossip::TrustTestMessage")]
#[derive(Clone, Debug, Decode, Encode)]
enum TrustTestMessage {
    Trust(u32),
    Peer(u32),
}
impl ClassifyTopic for TrustTestMessage {
    // This explicit synthetic payload has no Availability or sidecar variants.
    // A positive bound for each empty variant set funds mandatory geometry;
    // no production payload owner uses these fixture-only declarations.
    fn availability_frame_maximum(
        _: &iroha_model_base::peer::PeerId,
    ) -> Result<usize, norito::core::Error> {
        Ok(1)
    }
    fn recovery_frame_maxima(
        _: &iroha_model_base::peer::PeerId,
    ) -> Result<[usize; 2], norito::core::Error> {
        Ok([1, 1])
    }

    fn inbound_topic(payload: &[u8], flags: u8) -> Result<Option<Topic>, norito::core::Error> {
        // Two fixed-width variants: inspect only tag and scalar field length.
        use norito::core;
        core::validate_header_flags(flags)?;
        let tag = u32::from_le_bytes(
            payload
                .get(..4)
                .ok_or(core::Error::LengthMismatch)?
                .try_into()
                .map_err(|_| core::Error::LengthMismatch)?,
        );
        let field = &payload[4..];
        if flags & core::header_flags::PACKED_STRUCT != 0 {
            if field.len() != 4 {
                return Err(core::Error::LengthMismatch);
            }
        } else {
            let (length, prefix) = core::read_len_from_slice_with_flags(field, flags)?;
            if length != 4 || prefix.checked_add(length) != Some(field.len()) {
                return Err(core::Error::LengthMismatch);
            }
        }
        match tag {
            0 => Ok(Some(Topic::TrustGossip)),
            1 => Ok(Some(Topic::PeerGossip)),
            _ => Err(core::Error::Message("unknown trust fixture tag".to_owned())),
        }
    }

    fn topic(&self) -> Topic {
        match self {
            TrustTestMessage::Trust(_) => Topic::TrustGossip,
            TrustTestMessage::Peer(_) => Topic::PeerGossip,
        }
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for TrustTestMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}
fn make_config(addr: &SocketAddr, trust_gossip: bool) -> Config {
    // Keep admission inexpensive so this suite continues to measure
    // trust-gossip behavior. `test_network_config` isolates replay state.
    let mut soranet_handshake = super::low_cost_test_soranet_handshake();
    soranet_handshake.trust_gossip = trust_gossip;
    Config {
        happy_eyeballs_stagger: Duration::from_millis(50),
        p2p_queue_cap_high: NonZeroUsize::new(4096).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(4096).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(1024).expect("non-zero"),
        ..super::test_network_config(
            addr.clone(),
            addr.clone(),
            Duration::from_secs(10),
            soranet_handshake,
            trust_gossip,
        )
    }
}
async fn wait_for_peer(net: &NetworkHandle<TrustTestMessage>) {
    let mut handle = net.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut n = handle
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = handle
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("peer should connect");
}
async fn observe_peer_and_trust(
    rx: &mut mpsc::Receiver<PeerMessage<TrustTestMessage>>,
    expected_peer: u32,
    expected_trust: u32,
) -> (bool, bool) {
    let mut saw_peer = false;
    let mut saw_trust = false;
    let peer_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < peer_deadline && !saw_peer {
        let remaining = peer_deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(PeerMessage { payload, .. })) => match payload {
                TrustTestMessage::Peer(v) if v == expected_peer => saw_peer = true,
                TrustTestMessage::Trust(v) if v == expected_trust => saw_trust = true,
                _ => {}
            },
            Ok(None) | Err(_) => break,
        }
    }
    // Give the network a brief window to deliver any (unexpected) trust-gossip frames after the
    // peer-gossip message arrives.
    let trust_deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    while tokio::time::Instant::now() < trust_deadline {
        let remaining = trust_deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(PeerMessage { payload, .. })) => match payload {
                TrustTestMessage::Trust(v) if v == expected_trust => {
                    saw_trust = true;
                }
                TrustTestMessage::Peer(v) if v == expected_peer => saw_peer = true,
                _ => {}
            },
            Ok(None) | Err(_) => break,
        }
    }
    (saw_peer, saw_trust)
}
fn connect_topology(
    net_a: &NetworkHandle<TrustTestMessage>,
    net_b: &NetworkHandle<TrustTestMessage>,
    peer_a: &Peer,
    peer_b: &Peer,
) {
    // Only dial from A to B to avoid simultaneous connection churn.
    //
    // In permissioned mode peers refuse inbound observers not present in the topology, so B must
    // include A even if it does not dial out to it.
    net_a.update_topology(UpdateTopology([peer_b.id().clone()].into_iter().collect()));
    net_a.update_peers_addresses(UpdatePeers(vec![(
        peer_b.id().clone(),
        peer_b.address().clone(),
    )]));
    net_b.update_topology(UpdateTopology([peer_a.id().clone()].into_iter().collect()));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn trust_gossip_disabled_drops_frames_and_keeps_peer_gossip() {
    test_logger();
    let chain_id = super::test_network_id("test-chain");
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = super::random_node_key_pair();
    let kp_b = super::random_node_key_pair();
    let (net_a, _) = match NetworkHandle::start(
        super::p2p_identity_keys(kp_a.clone()),
        make_config(&addr_a, TRUST_GOSSIP),
        chain_id,
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(error) => panic!("trust-gossip fixture must start: {error}"),
    };
    let (net_b, _) = match NetworkHandle::start(
        super::p2p_identity_keys(kp_b.clone()),
        make_config(&addr_b, false),
        chain_id,
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(error) => panic!("trust-gossip fixture must start: {error}"),
    };
    let (mut rx_a, mut rx_b) = {
        let (tx_a, rx_a) = mpsc::channel(4);
        let (tx_b, rx_b) = mpsc::channel(4);
        net_a
            .subscribe_to_peers_messages(tx_a)
            .expect("subscribe net_a");
        net_b
            .subscribe_to_peers_messages(tx_b)
            .expect("subscribe net_b");
        (rx_a, rx_b)
    };
    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());
    connect_topology(&net_a, &net_b, &peer_a, &peer_b);
    wait_for_peer(&net_a).await;
    wait_for_peer(&net_b).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    net_a.post(Post {
        data: TrustTestMessage::Trust(1),
        peer_id: peer_b.id().clone(),
        priority: Priority::Low,
    });
    net_a.post(Post {
        data: TrustTestMessage::Peer(2),
        peer_id: peer_b.id().clone(),
        priority: Priority::Low,
    });
    net_b.post(Post {
        data: TrustTestMessage::Trust(3),
        peer_id: peer_a.id().clone(),
        priority: Priority::Low,
    });
    net_b.post(Post {
        data: TrustTestMessage::Peer(4),
        peer_id: peer_a.id().clone(),
        priority: Priority::Low,
    });
    let (b_saw_peer, b_saw_trust) = observe_peer_and_trust(&mut rx_b, 2, 1).await;
    assert!(b_saw_peer, "peer gossip should still be delivered");
    assert!(
        !b_saw_trust,
        "trust gossip should be dropped when the capability is disabled"
    );
    let (a_saw_peer, a_saw_trust) = observe_peer_and_trust(&mut rx_a, 4, 3).await;
    assert!(
        a_saw_peer,
        "peer gossip should still flow from a trust-gossip-disabled peer"
    );
    assert!(
        !a_saw_trust,
        "peer with trust_gossip disabled must not emit trust frames"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trust_gossip_enabled_reaches_both_peers() {
    test_logger();
    let chain_id = super::test_network_id("test-chain");
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = super::random_node_key_pair();
    let kp_b = super::random_node_key_pair();
    let (net_a, _) = match NetworkHandle::start(
        super::p2p_identity_keys(kp_a.clone()),
        make_config(&addr_a, TRUST_GOSSIP),
        chain_id,
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(error) => panic!("trust-gossip fixture must start: {error}"),
    };
    let (net_b, _) = match NetworkHandle::start(
        super::p2p_identity_keys(kp_b.clone()),
        make_config(&addr_b, TRUST_GOSSIP),
        chain_id,
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(error) => panic!("trust-gossip fixture must start: {error}"),
    };
    let (mut rx_a, mut rx_b) = {
        let (tx_a, rx_a) = mpsc::channel(4);
        let (tx_b, rx_b) = mpsc::channel(4);
        net_a
            .subscribe_to_peers_messages(tx_a)
            .expect("subscribe net_a");
        net_b
            .subscribe_to_peers_messages(tx_b)
            .expect("subscribe net_b");
        (rx_a, rx_b)
    };
    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());
    connect_topology(&net_a, &net_b, &peer_a, &peer_b);
    wait_for_peer(&net_a).await;
    wait_for_peer(&net_b).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    net_a.post(Post {
        data: TrustTestMessage::Trust(10),
        peer_id: peer_b.id().clone(),
        priority: Priority::Low,
    });
    net_b.post(Post {
        data: TrustTestMessage::Trust(11),
        peer_id: peer_a.id().clone(),
        priority: Priority::Low,
    });
    let recv_a = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(PeerMessage { payload, .. }) = rx_a.recv().await {
            if matches!(payload, TrustTestMessage::Trust(11)) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    let recv_b = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(PeerMessage { payload, .. }) = rx_b.recv().await {
            if matches!(payload, TrustTestMessage::Trust(10)) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(recv_a, "trust gossip should reach trust-enabled peer A");
    assert!(recv_b, "trust gossip should reach trust-enabled peer B");
}

#[test]
fn trust_fixture_raw_discriminator_matches_both_fixed_native_variants() {
    use norito::core;
    for value in [TrustTestMessage::Trust(u32::MAX), TrustTestMessage::Peer(0)] {
        for requested in [
            0,
            core::header_flags::COMPACT_LEN,
            core::header_flags::PACKED_STRUCT | core::header_flags::COMPACT_LEN,
            core::header_flags::PACKED_STRUCT
                | core::header_flags::COMPACT_LEN
                | core::header_flags::FIELD_BITSET,
        ] {
            let (bytes, flags) = {
                let _flags = core::DecodeFlagsGuard::enter(requested);
                norito::codec::encode_with_header_flags(&value)
            };
            let _flags = core::DecodeFlagsGuard::enter(flags);
            let (decoded, used) = core::decode_field_canonical::<TrustTestMessage>(&bytes).unwrap();
            assert_eq!(used, bytes.len());
            assert!(matches!((&value, &decoded),
                (TrustTestMessage::Trust(a), TrustTestMessage::Trust(b)) |
                (TrustTestMessage::Peer(a), TrustTestMessage::Peer(b)) if a == b));
            assert_eq!(
                TrustTestMessage::inbound_topic(&bytes, flags).unwrap(),
                Some(decoded.topic())
            );
            assert_eq!(
                TrustTestMessage::inbound_admission_class(&bytes, flags).unwrap(),
                decoded.admission_class()
            );
            let mut unknown = bytes.clone();
            unknown[..4].copy_from_slice(&2_u32.to_le_bytes());
            assert!(TrustTestMessage::inbound_topic(&unknown, flags).is_err());
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(TrustTestMessage::inbound_topic(&trailing, flags).is_err());
            assert!(TrustTestMessage::inbound_topic(&bytes[..bytes.len() - 1], flags).is_err());
            assert!(TrustTestMessage::inbound_topic(&bytes, flags | 0x80).is_err());
        }
    }
}
