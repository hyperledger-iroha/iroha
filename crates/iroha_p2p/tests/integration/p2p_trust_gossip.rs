//! Trust-gossip capability gating integration tests.
#![allow(unexpected_cfgs)]

use std::{collections::HashSet, num::NonZeroUsize};

use iroha_config::parameters::{
    actual::{
        LaneProfile, Network as Config, RelayMode, SoranetHandshake as ActualSoranetHandshake,
        SoranetPow, SoranetPrivacy, SoranetVpn,
    },
    defaults::network::{
        ACCEPT_BUCKET_IDLE, ACCEPT_PREFIX_V4_BITS, ACCEPT_PREFIX_V6_BITS, MAX_ACCEPT_BUCKETS,
        PEER_GOSSIP_PERIOD, RELAY_TTL, TRUST_DECAY_HALF_LIFE, TRUST_GOSSIP, TRUST_MIN_SCORE,
        TRUST_PENALTY_BAD_GOSSIP, TRUST_PENALTY_UNKNOWN_PEER,
    },
};
use iroha_config_base::WithOrigin;
use iroha_crypto::{
    KeyPair,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
    },
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
use tokio::{sync::mpsc, time::Duration};

use super::next_port;

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

#[allow(clippy::too_many_lines)]
fn make_config(addr: &SocketAddr, trust_gossip: bool) -> Config {
    Config {
        address: WithOrigin::inline(addr.clone()),
        public_address: WithOrigin::inline(addr.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: ActualSoranetHandshake {
            descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
            client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
            relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
            trust_gossip,
            kem_id: 1,
            sig_id: 1,
            resume_hash: None,
            pow: SoranetPow::default(),
        },
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout: Duration::from_secs(10),
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        deferred_send_ttl: std::time::Duration::from_millis(
            iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
        ),
        deferred_send_max_per_peer:
            iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip: TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer: TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: TRUST_MIN_SCORE,
        trust_gossip,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: vec![],
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(50),
        addr_ipv6_first: false,
        dns_refresh_interval: None,
        dns_refresh_ttl: None,
        quic_enabled: false,
        quic_datagrams_enabled: iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
        quic_datagram_max_payload_bytes:
            iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
        quic_datagram_receive_buffer_bytes:
            iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
        quic_datagram_send_buffer_bytes:
            iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
        scion: iroha_config::parameters::actual::ScionConfig::default(),
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(4096).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(4096).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(1024).expect("non-zero"),
        p2p_subscriber_queue_cap:
            iroha_config::parameters::defaults::network::P2P_SUBSCRIBER_QUEUE_CAP,
        consensus_ingress_rate_per_sec:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RATE_PER_SEC,
        consensus_ingress_burst:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BURST,
        consensus_ingress_bytes_per_sec:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_PER_SEC,
        consensus_ingress_bytes_burst:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_BURST,
        consensus_ingress_critical_rate_per_sec:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
        consensus_ingress_critical_burst:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BURST,
        consensus_ingress_critical_bytes_per_sec:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
        consensus_ingress_critical_bytes_burst:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
        consensus_ingress_rbc_session_limit:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RBC_SESSION_LIMIT,
        consensus_ingress_penalty_threshold:
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
        consensus_ingress_penalty_window: Duration::from_millis(
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_WINDOW_MS,
        ),
        consensus_ingress_penalty_cooldown: Duration::from_millis(
            iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS,
        ),
        max_incoming: None,
        max_total_connections: None,
        accept_rate_per_ip_per_sec: None,
        accept_burst_per_ip: None,
        max_accept_buckets: MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: ACCEPT_PREFIX_V6_BITS,
        accept_rate_per_prefix_per_sec: None,
        accept_burst_per_prefix: None,
        low_priority_rate_per_sec: None,
        low_priority_burst: None,
        low_priority_bytes_per_sec: None,
        low_priority_bytes_burst: None,
        allowlist_only: false,
        allow_keys: vec![],
        deny_keys: vec![],
        allow_cidrs: vec![],
        deny_cidrs: vec![],
        disconnect_on_post_overflow: true,
        max_frame_bytes: 1_048_576,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tcp_nodelay: true,
        tcp_keepalive: None,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
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
    let chain_id = None;
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = KeyPair::random();
    let kp_b = KeyPair::random();

    let (net_a, _) = match NetworkHandle::start(
        kp_a.clone(),
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
        kp_b.clone(),
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

    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());
    connect_topology(&net_a, &net_b, &peer_a, &peer_b);
    wait_for_peer(&net_a).await;
    wait_for_peer(&net_b).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

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
    let chain_id = None;
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = KeyPair::random();
    let kp_b = KeyPair::random();

    let (net_a, _) = match NetworkHandle::start(
        kp_a.clone(),
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
        kp_b.clone(),
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

    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());
    connect_topology(&net_a, &net_b, &peer_a, &peer_b);
    wait_for_peer(&net_a).await;
    wait_for_peer(&net_b).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

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
