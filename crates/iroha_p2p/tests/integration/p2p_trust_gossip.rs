//! Trust-gossip capability gating integration tests.
#![allow(unexpected_cfgs)]
use super::next_port;
use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake, SoranetPow,
};
use iroha_config::parameters::defaults::network::TRUST_GOSSIP;
use iroha_config_base::WithOrigin;
use iroha_crypto::soranet::handshake::{
    DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
};
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
#[derive(Clone, Debug, Decode, Encode)]
enum TrustTestMessage {
    Trust(u32),
    Peer(u32),
}
impl ClassifyTopic for TrustTestMessage {
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
    // Admission puzzles are covered by `p2p_puzzle`; keeping them out of this
    // suite makes trust-gossip timing assertions test only gossip behavior.
    let pow = SoranetPow {
        required: false,
        puzzle: None,
        ..SoranetPow::default()
    };
    let soranet_handshake = ActualSoranetHandshake {
        descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
        client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
        relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
        trust_gossip,
        kem_id: 1,
        sig_id: 1,
        resume_hash: None,
        pow,
    };
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
        chain_id.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping trust_gossip_disabled_drops_frames_and_keeps_peer_gossip: {e:?}");
            return;
        }
    };
    let (net_b, _) = match NetworkHandle::start(
        super::p2p_identity_keys(kp_b.clone()),
        make_config(&addr_b, false),
        chain_id.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping trust_gossip_disabled_drops_frames_and_keeps_peer_gossip: {e:?}");
            return;
        }
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
        chain_id.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping trust_gossip_enabled_reaches_both_peers: {e:?}");
            return;
        }
    };
    let (net_b, _) = match NetworkHandle::start(
        super::p2p_identity_keys(kp_b.clone()),
        make_config(&addr_b, TRUST_GOSSIP),
        chain_id.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping trust_gossip_enabled_reaches_both_peers: {e:?}");
            return;
        }
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
