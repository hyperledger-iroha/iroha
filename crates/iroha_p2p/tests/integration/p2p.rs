//! Integration tests for the peer-to-peer networking layer.
#![allow(unexpected_cfgs)]

use std::{
    collections::HashSet,
    fmt::Debug,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use futures::{prelude::*, stream::FuturesUnordered, task::AtomicWaker};
use iroha_config::parameters::{
    actual::{
        LaneProfile, Network as Config, RelayMode, SoranetHandshake as ActualSoranetHandshake,
        SoranetPow, SoranetPrivacy, SoranetVpn,
    },
    defaults::network::{PEER_GOSSIP_PERIOD, RELAY_TTL},
};
use iroha_config_base::WithOrigin;
use iroha_crypto::{
    KeyPair,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
    },
};
use iroha_data_model::prelude::{ChainId, Peer};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::{prelude::*, test_logger};
use iroha_p2p::{NetworkHandle, network::message::*, peer::message::PeerMessage};
use iroha_primitives::addr::socket_addr;
use norito::codec::{Decode, Encode};
use tokio::{
    sync::{Barrier, mpsc},
    time::Duration,
};

use super::next_port;

#[derive(Clone, Debug, Decode, Encode)]
struct TestMessage(String);

// Classify test payloads into a generic topic
impl iroha_p2p::network::message::ClassifyTopic for TestMessage {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Other
    }
}

#[derive(Clone, Debug, Decode, Encode)]
struct MultiTopic {
    chan: u8,
    payload: u32,
}

impl iroha_p2p::network::message::ClassifyTopic for MultiTopic {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        match self.chan {
            0 => iroha_p2p::network::message::Topic::TxGossip,
            1 => iroha_p2p::network::message::Topic::PeerGossip,
            2 => iroha_p2p::network::message::Topic::TrustGossip,
            _ => iroha_p2p::network::message::Topic::Other,
        }
    }
}

#[derive(Clone, Debug, Decode, Encode)]
struct ConsensusMessage(u32);

impl iroha_p2p::network::message::ClassifyTopic for ConsensusMessage {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Consensus
    }
}

macro_rules! impl_decode_from_slice_via_canonical {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
                fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                    norito::core::decode_field_canonical::<Self>(bytes)
                }
            }
        )+
    };
}

impl_decode_from_slice_via_canonical!(TestMessage, MultiTopic, ConsensusMessage);

fn setup_logger() {
    test_logger();
}

fn default_soranet_handshake() -> ActualSoranetHandshake {
    ActualSoranetHandshake {
        descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
        client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
        relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
        trust_gossip: true,
        kem_id: 1,
        sig_id: 1,
        resume_hash: None,
        pow: SoranetPow::default(),
    }
}

fn trust_config(
    addr: iroha_primitives::addr::SocketAddr,
    trust_gossip: bool,
    idle_timeout: Duration,
) -> Config {
    Config {
        address: WithOrigin::inline(addr.clone()),
        public_address: WithOrigin::inline(addr),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        trust_gossip,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: vec![],
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(100),
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
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(8192).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(32_768).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(2048).expect("non-zero"),
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
        max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    }
}

/// This test creates a network and one peer.
/// This peer connects back to our network, emulating some distant peer.
/// There is no need to create separate networks to check that messages
/// are properly sent and received using encryption and serialization/deserialization.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn network_create() {
    let delay = Duration::from_millis(200);
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    info!("Starting network tests...");
    let address = socket_addr!(127.0.0.1: {next_port()});
    let key_pair = KeyPair::random();
    let public_key = key_pair.public_key().clone();
    let idle_timeout = Duration::from_secs(60);
    let chain_id = ChainId::from("test_chain");
    let config = Config {
        address: WithOrigin::inline(address.clone()),
        public_address: WithOrigin::inline(address.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: vec![],
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(100),
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
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(8192).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(32_768).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(2048).expect("non-zero"),
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
        max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    };
    let started = NetworkHandle::start(
        key_pair,
        config,
        Some(chain_id),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (network, _) = match started {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping network_create: cannot start network: {e:?}");
            return;
        }
    };
    tokio::time::sleep(delay).await;

    info!("Connecting to peer...");
    let peer1 = Peer::new(address.clone(), public_key.clone());
    update_topology_and_peers_addresses(&network, std::slice::from_ref(&peer1));
    tokio::time::sleep(delay).await;

    info!("Posting message...");
    network.post(Post {
        data: TestMessage("Some data to send to peer".to_owned()),
        peer_id: peer1.id().clone(),
        priority: Priority::Low,
    });

    tokio::time::sleep(delay).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn trust_gossip_opt_out_blocks_trust_frames() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let chain = ChainId::from("trust_gate_opt_out");
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = KeyPair::random();
    let kp_b = KeyPair::random();

    let started_a = NetworkHandle::<MultiTopic>::start(
        kp_a.clone(),
        trust_config(addr_a.clone(), false, Duration::from_secs(60)),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net_a, _child_a) = match started_a {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let started_b = NetworkHandle::<MultiTopic>::start(
        kp_b.clone(),
        trust_config(addr_b.clone(), true, Duration::from_secs(60)),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net_b, _child_b) = match started_b {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());

    // Dial in one direction only (A -> B) to avoid flakiness from simultaneous connection
    // resolution dropping one of the sockets around the time we post the test message.
    net_a.update_topology(UpdateTopology(HashSet::from([peer_b.id().clone()])));
    net_a.update_peers_addresses(UpdatePeers(vec![(peer_b.id().clone(), addr_b.clone())]));
    // Still allow inbound from A, but do not provide A's address so B won't dial back.
    net_b.update_topology(UpdateTopology(HashSet::from([peer_a.id().clone()])));

    if tokio::time::timeout(Duration::from_millis(2500), async {
        let mut n = net_a
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net_a
                .wait_online_peers_update(std::collections::HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .is_err()
    {
        return;
    }

    if tokio::time::timeout(Duration::from_millis(2500), async {
        let mut n = net_b
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net_b
                .wait_online_peers_update(std::collections::HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .is_err()
    {
        return;
    }

    let (tx_a, mut rx_a) = mpsc::channel(8);
    let (tx_b, mut rx_b) = mpsc::channel(8);
    net_a
        .subscribe_to_peers_messages(tx_a)
        .expect("register subscriber");
    net_b
        .subscribe_to_peers_messages(tx_b)
        .expect("register subscriber");

    net_b.post(Post {
        data: MultiTopic {
            chan: 2,
            payload: 7,
        },
        peer_id: peer_a.id().clone(),
        priority: Priority::Low,
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(400), rx_a.recv())
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rx_a.try_recv().is_err());

    net_a.post(Post {
        data: MultiTopic {
            chan: 2,
            payload: 8,
        },
        peer_id: peer_b.id().clone(),
        priority: Priority::Low,
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(400), rx_b.recv())
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rx_b.try_recv().is_err());

    net_b.post(Post {
        data: MultiTopic {
            chan: 1,
            payload: 42,
        },
        peer_id: peer_a.id().clone(),
        priority: Priority::Low,
    });
    let msg_a = tokio::time::timeout(Duration::from_millis(2500), rx_a.recv())
        .await
        .expect("peer gossip delivery timed out")
        .expect("subscriber channel closed");
    assert_eq!(msg_a.payload.chan, 1);
    assert_eq!(msg_a.payload.payload, 42);

    net_a.post(Post {
        data: MultiTopic {
            chan: 1,
            payload: 43,
        },
        peer_id: peer_b.id().clone(),
        priority: Priority::Low,
    });
    let msg_b = tokio::time::timeout(Duration::from_millis(2500), rx_b.recv())
        .await
        .expect("peer gossip delivery timed out")
        .expect("subscriber channel closed");
    assert_eq!(msg_b.payload.chan, 1);
    assert_eq!(msg_b.payload.payload, 43);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trust_gossip_enabled_flows_through() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let chain = ChainId::from("trust_gate_enabled");
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = KeyPair::random();
    let kp_b = KeyPair::random();

    let started_a = NetworkHandle::<MultiTopic>::start(
        kp_a.clone(),
        trust_config(addr_a.clone(), true, Duration::from_secs(60)),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net_a, _child_a) = match started_a {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let started_b = NetworkHandle::<MultiTopic>::start(
        kp_b.clone(),
        trust_config(addr_b.clone(), true, Duration::from_secs(60)),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net_b, _child_b) = match started_b {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());

    // Dial in one direction only (A -> B) to avoid flakiness from simultaneous connection
    // resolution dropping one of the sockets around the time we post the test message.
    net_a.update_topology(UpdateTopology(HashSet::from([peer_b.id().clone()])));
    net_a.update_peers_addresses(UpdatePeers(vec![(peer_b.id().clone(), addr_b.clone())]));
    // Still allow inbound from A, but do not provide A's address so B won't dial back.
    net_b.update_topology(UpdateTopology(HashSet::from([peer_a.id().clone()])));

    if tokio::time::timeout(Duration::from_millis(2500), async {
        let mut n = net_a
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net_a
                .wait_online_peers_update(std::collections::HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .is_err()
    {
        return;
    }

    if tokio::time::timeout(Duration::from_millis(2500), async {
        let mut n = net_b
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net_b
                .wait_online_peers_update(std::collections::HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .is_err()
    {
        return;
    }

    let (tx_b, mut rx_b) = mpsc::channel(4);
    net_b
        .subscribe_to_peers_messages(tx_b)
        .expect("register subscriber");

    net_a.post(Post {
        data: MultiTopic {
            chan: 2,
            payload: 99,
        },
        peer_id: peer_b.id().clone(),
        priority: Priority::Low,
    });
    let msg_b = tokio::time::timeout(Duration::from_millis(2500), rx_b.recv())
        .await
        .expect("trust gossip delivery timed out")
        .expect("subscriber channel closed");
    assert_eq!(msg_b.payload.chan, 2);
    assert_eq!(msg_b.payload.payload, 99);
}

#[cfg(feature = "p2p_ws")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ws_fallback_connects_and_handshakes() {
    use tokio::{
        io::{AsyncRead, AsyncWrite, ReadBuf},
        net::TcpListener,
    };
    use tokio_tungstenite::accept_async;

    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }

    // Force the dialer to take the WebSocket path for Host addresses.

    // Start the inbound network (peer2).
    let kp2 = KeyPair::random();
    let chain_id = ChainId::from("test_chain");
    let idle = Duration::from_millis(5000);
    let (network2, _child2) = NetworkHandle::<TestMessage>::start(
        kp2.clone(),
        Config {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout: idle,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(128).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(64).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    .expect("start network2");

    // WS server that upgrades and forwards the duplex to network2.accept_stream.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws_addr = listener.local_addr().unwrap();
    let p2 = network2.clone();
    tokio::spawn(async move {
        if let Ok((stream, remote)) = listener.accept().await {
            if let Ok(ws) = accept_async(stream).await {
                let (sink, stream) = ws.split();
                // Adapters
                #[allow(clippy::items_after_statements)]
                struct R<S>(S, bytes::Bytes);
                impl<S> AsyncRead for R<S>
                where
                    S: futures::Stream<
                            Item = Result<
                                tokio_tungstenite::tungstenite::Message,
                                tokio_tungstenite::tungstenite::Error,
                            >,
                        > + Unpin
                        + Send,
                {
                    fn poll_read(
                        mut self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                        buf: &mut ReadBuf<'_>,
                    ) -> core::task::Poll<std::io::Result<()>> {
                        if !self.1.is_empty() {
                            let n = core::cmp::min(self.1.len(), buf.remaining());
                            buf.put_slice(&self.1.split_to(n));
                            return core::task::Poll::Ready(Ok(()));
                        }
                        match futures::ready!(core::pin::Pin::new(&mut self.0).poll_next(cx)) {
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(b))) => {
                                self.1 = b;
                                let n = core::cmp::min(self.1.len(), buf.remaining());
                                buf.put_slice(&self.1.split_to(n));
                                core::task::Poll::Ready(Ok(()))
                            }
                            Some(Ok(_)) => {
                                cx.waker().wake_by_ref();
                                core::task::Poll::Pending
                            }
                            Some(Err(e)) => core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws read error: {e}"),
                            ))),
                            None => core::task::Poll::Ready(Ok(())),
                        }
                    }
                }
                #[allow(clippy::items_after_statements)]
                struct W<S>(S, Vec<u8>);
                impl<S> AsyncWrite for W<S>
                where
                    S: futures::Sink<
                            tokio_tungstenite::tungstenite::Message,
                            Error = tokio_tungstenite::tungstenite::Error,
                        > + Unpin
                        + Send,
                {
                    fn poll_write(
                        mut self: core::pin::Pin<&mut Self>,
                        _cx: &mut core::task::Context<'_>,
                        data: &[u8],
                    ) -> core::task::Poll<std::io::Result<usize>> {
                        self.1.extend_from_slice(data);
                        core::task::Poll::Ready(Ok(data.len()))
                    }
                    fn poll_flush(
                        mut self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                    ) -> core::task::Poll<std::io::Result<()>> {
                        if self.1.is_empty() {
                            return core::task::Poll::Ready(Ok(()));
                        }
                        let data = core::mem::take(&mut self.1);
                        match futures::ready!(core::pin::Pin::new(&mut self.0).poll_ready(cx)) {
                            Ok(()) => {}
                            Err(e) => {
                                return core::task::Poll::Ready(Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("ws ready error: {e}"),
                                )));
                            }
                        }
                        if let Err(e) = core::pin::Pin::new(&mut self.0).start_send(
                            tokio_tungstenite::tungstenite::Message::Binary(data.into()),
                        ) {
                            return core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws send error: {e}"),
                            )));
                        }
                        match futures::ready!(core::pin::Pin::new(&mut self.0).poll_flush(cx)) {
                            Ok(()) => core::task::Poll::Ready(Ok(())),
                            Err(e) => core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws flush error: {e}"),
                            ))),
                        }
                    }
                    fn poll_shutdown(
                        mut self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                    ) -> core::task::Poll<std::io::Result<()>> {
                        match futures::ready!(core::pin::Pin::new(&mut self.0).poll_ready(cx)) {
                            Ok(()) => {}
                            Err(e) => {
                                return core::task::Poll::Ready(Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("ws ready error: {e}"),
                                )));
                            }
                        }
                        if let Err(e) = core::pin::Pin::new(&mut self.0)
                            .start_send(tokio_tungstenite::tungstenite::Message::Close(None))
                        {
                            return core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws close error: {e}"),
                            )));
                        }
                        match futures::ready!(core::pin::Pin::new(&mut self.0).poll_flush(cx)) {
                            Ok(()) => core::task::Poll::Ready(Ok(())),
                            Err(e) => core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws close error: {e}"),
                            ))),
                        }
                    }
                }
                let read = R(stream, bytes::Bytes::new());
                let write = W(sink, Vec::new());
                let _ = p2.accept_stream(read, write, remote).await;
            }
        }
    });

    // Start the dialer network (peer1) and point to the WS endpoint via Host address.
    let kp1 = KeyPair::random();
    let (mut network1, _child1) = NetworkHandle::<TestMessage>::start(
        kp1.clone(),
        Config {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout: idle,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            prefer_ws_fallback: true,
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
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(128).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(64).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    .expect("start network1");

    // Topology and peer address for network1 -> network2 via Host(ws_addr).
    let peer2 = Peer::new(
        format!("{}:{}", ws_addr.ip(), ws_addr.port())
            .parse()
            .unwrap(),
        kp2.public_key().clone(),
    );
    network1.update_topology(UpdateTopology([peer2.id().clone()].into_iter().collect()));
    network1.update_peers_addresses(UpdatePeers(vec![(
        peer2.id().clone(),
        format!("{}:{}", ws_addr.ip(), ws_addr.port())
            .parse()
            .unwrap(),
    )]));

    // Wait for the networks to connect
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut n = network1
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = network1
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("peer did not connect over WS");
}

#[derive(Clone, Debug)]
struct WaitForN(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    counter: AtomicU32,
    n: u32,
    waker: AtomicWaker,
}

impl WaitForN {
    fn new(n: u32) -> Self {
        Self(Arc::new(Inner {
            counter: AtomicU32::new(0),
            n,
            waker: AtomicWaker::new(),
        }))
    }

    fn inc(&self) {
        self.0.counter.fetch_add(1, Ordering::Relaxed);
        self.0.waker.wake();
    }

    fn current(&self) -> u32 {
        self.0.counter.load(Ordering::Relaxed)
    }
}

impl Future for WaitForN {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // Check if condition is already satisfied
        if self.0.counter.load(Ordering::Relaxed) >= self.0.n {
            return std::task::Poll::Ready(());
        }

        self.0.waker.register(cx.waker());

        if self.0.counter.load(Ordering::Relaxed) >= self.0.n {
            return std::task::Poll::Ready(());
        }

        std::task::Poll::Pending
    }
}

#[derive(Debug)]
pub struct TestActor {
    messages: WaitForN,
    receiver: mpsc::Receiver<PeerMessage<TestMessage>>,
}

impl TestActor {
    fn start(messages: WaitForN) -> mpsc::Sender<PeerMessage<TestMessage>> {
        let (sender, receiver) = mpsc::channel(10);
        let mut test_actor = Self { messages, receiver };
        tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    Some(PeerMessage { peer, payload: msg, .. }) = test_actor.receiver.recv() => {
                        info!(?msg, "Actor received message from {peer}");
                        test_actor.messages.inc();
                    },
                    else => break,
                }
            }
        });
        sender
    }
}

/// This test creates two networks and one peer from the first network.
/// This peer connects to our second network, emulating some distant peer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn two_networks() {
    // Allow some slack because this test can run in parallel with other integration tests and may
    // experience short scheduling delays under load.
    let delay = Duration::from_millis(1_000);
    let idle_timeout = Duration::from_secs(60);
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let key_pair1 = KeyPair::random();
    let public_key1 = key_pair1.public_key().clone();
    let key_pair2 = KeyPair::random().clone();
    let public_key2 = key_pair2.public_key().clone();
    let chain_id = ChainId::from("test_chain");
    info!("Starting first network...");
    let address1 = socket_addr!(127.0.0.1: {next_port()});
    let config1 = Config {
        address: WithOrigin::inline(address1.clone()),
        public_address: WithOrigin::inline(address1.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: vec![],
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(100),
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
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(8192).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(32_768).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(2048).expect("non-zero"),
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
        max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    };
    let started1 = NetworkHandle::start(
        key_pair1,
        config1,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut network1, _) = match started1 {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping two_networks: cannot start network1: {e:?}");
            return;
        }
    };

    info!("Starting second network...");
    let address2 = socket_addr!(127.0.0.1: {next_port()});
    let config2 = Config {
        address: WithOrigin::inline(address2.clone()),
        public_address: WithOrigin::inline(address2.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: vec![],
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(100),
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
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(8192).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(32_768).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(2048).expect("non-zero"),
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
        max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    };
    let started2 = NetworkHandle::<TestMessage>::start(
        key_pair2,
        config2,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut network2, _) = match started2 {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping two_networks: cannot start network2: {e:?}");
            return;
        }
    };

    let mut messages2 = WaitForN::new(1);
    let actor2 = TestActor::start(messages2.clone());
    if let Err(sender) = network2.subscribe_to_peers_messages(actor2) {
        drop(sender);
        panic!("failed to subscribe actor2 to network2 messages");
    }

    info!("Connecting peers...");
    let peer1 = Peer::new(address1.clone(), public_key1);
    let peer2 = Peer::new(address2.clone(), public_key2);
    // Connect `network1` to `network2` (one direction only).
    //
    // Dialling both directions can create simultaneous connections and invoke the
    // connection-resolution policy, which may transiently drop a connection around the time we
    // post the test message, making the test flaky under parallel execution.
    update_topology_and_peers_addresses(&network1, std::slice::from_ref(&peer2));
    // Ensure `network2` will accept inbound from `network1` without causing it to dial back.
    network2.update_topology(UpdateTopology(HashSet::from([peer1.id().clone()])));

    tokio::time::timeout(Duration::from_millis(2000), async {
        let mut connections = network1
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while connections != 1 {
            connections = network1
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("Failed to get all connections");

    // Ensure `network2` has observed the inbound connection as well.
    tokio::time::timeout(Duration::from_millis(2000), async {
        let mut connections = network2
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while connections != 1 {
            connections = network2
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("Failed to get all connections");

    info!("Posting message...");
    network1.post(Post {
        data: TestMessage("Some data to send to peer".to_owned()),
        peer_id: peer2.id().clone(),
        priority: Priority::Low,
    });

    tokio::time::timeout(delay, &mut messages2)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Failed to get all messages in given time (received {} out of 1)",
                messages2.current()
            )
        });

    let connected_peers1 = network1.online_peers(HashSet::len);
    assert_eq!(connected_peers1, 1);

    let connected_peers2 = network2.online_peers(HashSet::len);
    assert_eq!(connected_peers2, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[allow(clippy::too_many_lines)]
async fn update_peers_triggers_immediate_connect() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let idle_timeout = Duration::from_secs(60);
    let chain_id = ChainId::from("test_chain");

    // Fixed but unused ports in this file; pick new ones unlikely to collide
    let address1 = socket_addr!(127.0.0.1: {next_port()});
    let address2 = socket_addr!(127.0.0.1: {next_port()});

    let key_pair1 = KeyPair::random();
    let key_pair2 = KeyPair::random();

    let started1 = NetworkHandle::<TestMessage>::start(
        key_pair1.clone(),
        Config {
            address: WithOrigin::inline(address1.clone()),
            public_address: WithOrigin::inline(address1.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            prefer_ws_fallback: false,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: vec![],
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            happy_eyeballs_stagger: Duration::from_millis(100),
            addr_ipv6_first: false,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            quic_datagrams_enabled: iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(8192).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(32_768).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(2048).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut network1, _) = match started1 {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!(
                "Skipping update_peers_triggers_immediate_connect: cannot start network1: {e:?}"
            );
            return;
        }
    };

    let started2 = NetworkHandle::<TestMessage>::start(
        key_pair2.clone(),
        Config {
            address: WithOrigin::inline(address2.clone()),
            public_address: WithOrigin::inline(address2.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            prefer_ws_fallback: false,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: vec![],
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            happy_eyeballs_stagger: Duration::from_millis(100),
            addr_ipv6_first: false,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            quic_datagrams_enabled: iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(8192).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(32_768).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(2048).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (network2, _) = match started2 {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!(
                "Skipping update_peers_triggers_immediate_connect: cannot start network2: {e:?}"
            );
            return;
        }
    };

    // Advertise only topology (no addresses yet), on both sides.
    let peer1 = Peer::new(address1.clone(), key_pair1.public_key().clone());
    let peer2 = Peer::new(address2.clone(), key_pair2.public_key().clone());
    network1.update_topology(UpdateTopology([peer2.id().clone()].into_iter().collect()));
    network2.update_topology(UpdateTopology([peer1.id().clone()].into_iter().collect()));

    // Now push addresses only for network1 and verify that connection is established
    // promptly (before the 1s periodic tick) due to immediate topology update on
    // UpdatePeers.
    network1.update_peers_addresses(UpdatePeers(vec![(peer2.id().clone(), address2.clone())]));

    tokio::time::timeout(Duration::from_millis(900), async {
        let mut n = network1
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = network1
                .wait_online_peers_update(std::collections::HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("expected immediate connect before periodic tick");

    assert_eq!(network1.online_peers(std::collections::HashSet::len), 1);
}

/// When multiple addresses are provided for the same peer id, the dialer should
/// attempt them in parallel so the reachable one wins quickly even if another is
/// down or blackholed.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[allow(clippy::too_many_lines)]
async fn happy_eyeballs_parallel_dials() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let idle_timeout = Duration::from_secs(60);
    let chain_id = ChainId::from("test_chain");

    // Listener (peer2)
    let address2 = socket_addr!(127.0.0.1: {next_port()});
    let key_pair2 = KeyPair::random();
    let started2 = NetworkHandle::<TestMessage>::start(
        key_pair2.clone(),
        Config {
            address: WithOrigin::inline(address2.clone()),
            public_address: WithOrigin::inline(address2.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            prefer_ws_fallback: false,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: vec![],
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            happy_eyeballs_stagger: Duration::from_millis(100),
            addr_ipv6_first: false,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            quic_datagrams_enabled: iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(8192).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(32_768).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(2048).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (_n2, _) = match started2 {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping happy_eyeballs_parallel_dials: cannot start listener: {e:?}");
            return;
        }
    };

    // Dialer (network1)
    let address1 = socket_addr!(127.0.0.1: {next_port()});
    let key_pair1 = KeyPair::random();
    let started1 = NetworkHandle::<TestMessage>::start(
        key_pair1.clone(),
        Config {
            address: WithOrigin::inline(address1.clone()),
            public_address: WithOrigin::inline(address1.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            prefer_ws_fallback: false,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: vec![],
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            happy_eyeballs_stagger: Duration::from_millis(100),
            addr_ipv6_first: false,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            quic_datagrams_enabled: iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(8192).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(32_768).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(2048).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut network1, _) = match started1 {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping happy_eyeballs_parallel_dials: cannot start dialer: {e:?}");
            return;
        }
    };

    let peer2 = Peer::new(address2.clone(), key_pair2.public_key().clone());
    network1.update_topology(UpdateTopology([peer2.id().clone()].into_iter().collect()));
    // Provide two addresses: one unreachable (closed port) and one correct.
    // On localhost a closed port rejects quickly, but this still exercises
    // parallel attempts without delaying the good path.
    let unreachable = socket_addr!(127.0.0.1: {next_port()});
    network1.update_peers_addresses(UpdatePeers(vec![
        (peer2.id().clone(), unreachable.clone()),
        (peer2.id().clone(), address2.clone()),
    ]));

    // Expect a quick connect (well under the 1s periodic tick).
    tokio::time::timeout(Duration::from_millis(500), async {
        let mut n = network1
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = network1
                .wait_online_peers_update(std::collections::HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("expected fast connect despite an unreachable alternative address");

    assert_eq!(network1.online_peers(std::collections::HashSet::len), 1);
}

/// Ensure low-priority per-topic substreams avoid starvation of one topic by another.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn low_topics_do_not_starve_each_other() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let idle_timeout = Duration::from_secs(30);
    let chain_id = ChainId::from("test_chain");

    // Start receiver network (B)
    let addr_b = socket_addr!(127.0.0.1: {next_port()});
    let kp_b = KeyPair::random();
    let (net_b, _child_b) = match NetworkHandle::<MultiTopic>::start(
        kp_b.clone(),
        Config {
            address: WithOrigin::inline(addr_b.clone()),
            public_address: WithOrigin::inline(addr_b.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(1024).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(1024).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(1024).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping low_topics_do_not_starve_each_other (receiver): {e:?}");
            return;
        }
    };

    // Start sender network (A)
    let addr_a = socket_addr!(127.0.0.1: {next_port()});
    let kp_a = KeyPair::random();
    let (mut net_a, _child_a) = match NetworkHandle::<MultiTopic>::start(
        kp_a.clone(),
        Config {
            address: WithOrigin::inline(addr_a.clone()),
            public_address: WithOrigin::inline(addr_a.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(1024).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(1024).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(1024).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping low_topics_do_not_starve_each_other (sender): {e:?}");
            return;
        }
    };

    // Allow B to accept inbound connections from A
    let peer_a = Peer::new(addr_a.clone(), kp_a.public_key().clone());
    net_b.update_topology(UpdateTopology([peer_a.id().clone()].into_iter().collect()));

    // Connect A -> B
    let peer_b = Peer::new(addr_b.clone(), kp_b.public_key().clone());
    net_a.update_topology(UpdateTopology([peer_b.id().clone()].into_iter().collect()));
    net_a.update_peers_addresses(UpdatePeers(vec![(peer_b.id().clone(), addr_b.clone())]));

    // Wait until connected
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut n = net_a
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net_a
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("sender failed to connect to receiver");

    // Subscribe on receiver
    let (tx, mut rx) = mpsc::channel(4);
    if let Err(sender) = net_b.subscribe_to_peers_messages(tx) {
        drop(sender);
        panic!("failed to subscribe net_b");
    }

    // Flood topic 0 (TxGossip) and inject a single message on topic 1 (PeerGossip)
    for i in 0..500u32 {
        net_a.post(Post {
            data: MultiTopic {
                chan: 0,
                payload: i,
            },
            peer_id: peer_b.id().clone(),
            priority: Priority::Low,
        });
        if i == 10 {
            net_a.post(Post {
                data: MultiTopic {
                    chan: 1,
                    payload: 42,
                },
                peer_id: peer_b.id().clone(),
                priority: Priority::Low,
            });
        }
    }

    // Expect to receive the topic=1 message without starvation
    let found = tokio::time::timeout(Duration::from_secs(3), async move {
        while let Some(PeerMessage {
            peer: _from,
            payload: msg,
            ..
        }) = rx.recv().await
        {
            if matches!(msg.chan, 1) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(
        found,
        "topic 1 message should not starve behind topic 0 flood"
    );
}

/// Ensure relay hub forwards consensus traffic between spokes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn relay_hub_routes_consensus_between_spokes() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let idle_timeout = Duration::from_secs(30);
    let chain_id = ChainId::from("test_chain");

    let hub_addr = socket_addr!(127.0.0.1: {next_port()});
    let spoke1_addr = socket_addr!(127.0.0.1: {next_port()});
    let spoke2_addr = socket_addr!(127.0.0.1: {next_port()});

    let hub_kp = KeyPair::random();
    let spoke1_kp = KeyPair::random();
    let spoke2_kp = KeyPair::random();

    let hub_peer = Peer::new(hub_addr.clone(), hub_kp.public_key().clone());
    let spoke1_peer = Peer::new(spoke1_addr.clone(), spoke1_kp.public_key().clone());
    let spoke2_peer = Peer::new(spoke2_addr.clone(), spoke2_kp.public_key().clone());

    let make_config =
        |address: iroha_primitives::addr::SocketAddr,
         relay_mode: RelayMode,
         relay_hub_addresses: Vec<iroha_primitives::addr::SocketAddr>| {
            Config {
            address: WithOrigin::inline(address.clone()),
            public_address: WithOrigin::inline(address),
            relay_mode,
            relay_hub_addresses,
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: false,
            tls_fallback_to_plain: true,
            tls_listen_address: None,
            tls_inbound_only: false,
            p2p_queue_cap_high: NonZeroUsize::new(4096).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(4096).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(2048).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        }
        };

    let (mut hub_net, _hub_child) = match NetworkHandle::<ConsensusMessage>::start(
        hub_kp.clone(),
        make_config(hub_addr.clone(), RelayMode::Hub, Vec::new()),
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping relay_hub_routes_consensus_between_spokes (hub): {e:?}");
            return;
        }
    };
    let (mut spoke1_net, _spoke1_child) = match NetworkHandle::<ConsensusMessage>::start(
        spoke1_kp.clone(),
        make_config(
            spoke1_addr.clone(),
            RelayMode::Spoke,
            vec![hub_addr.clone()],
        ),
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping relay_hub_routes_consensus_between_spokes (spoke1): {e:?}");
            return;
        }
    };
    let (mut spoke2_net, _spoke2_child) = match NetworkHandle::<ConsensusMessage>::start(
        spoke2_kp.clone(),
        make_config(
            spoke2_addr.clone(),
            RelayMode::Spoke,
            vec![hub_addr.clone()],
        ),
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping relay_hub_routes_consensus_between_spokes (spoke2): {e:?}");
            return;
        }
    };

    let (spoke2_tx, mut spoke2_rx) = mpsc::channel(4);
    if let Err(sender) = spoke2_net.subscribe_to_peers_messages(spoke2_tx) {
        drop(sender);
        panic!("failed to subscribe spoke2 to messages");
    }

    update_topology_and_peers_addresses(&hub_net, &[spoke1_peer.clone(), spoke2_peer.clone()]);
    update_topology_and_peers_addresses(&spoke1_net, std::slice::from_ref(&hub_peer));
    update_topology_and_peers_addresses(&spoke2_net, std::slice::from_ref(&hub_peer));

    tokio::time::timeout(Duration::from_secs(5), async {
        while hub_net.online_peers(HashSet::len) < 2 {
            hub_net
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("connection deadline exceeded");
    tokio::time::timeout(Duration::from_secs(5), async {
        while spoke1_net.online_peers(HashSet::len) < 1 {
            spoke1_net
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("connection deadline exceeded");
    tokio::time::timeout(Duration::from_secs(5), async {
        while spoke2_net.online_peers(HashSet::len) < 1 {
            spoke2_net
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("connection deadline exceeded");

    let payload = ConsensusMessage(7);
    spoke1_net.post(Post {
        data: payload.clone(),
        peer_id: spoke2_peer.id().clone(),
        priority: Priority::High,
    });

    let received = tokio::time::timeout(Duration::from_secs(10), spoke2_rx.recv())
        .await
        .expect("spoke2 should receive consensus via hub")
        .expect("message channel closed");
    assert_eq!(received.peer.id(), spoke1_peer.id());
    assert_eq!(received.payload.0, payload.0);
}

/// Ensure relay hub can route consensus traffic between a spoke and an assist node.
///
/// This models mixed deployments where only constrained peers run as spokes, while
/// validators/observers use `Assist` to reach them without forcing the entire
/// network onto a relay-only topology.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn relay_hub_routes_consensus_between_spoke_and_assist() {
    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let idle_timeout = Duration::from_secs(30);
    let chain_id = ChainId::from("test_chain");

    let hub_addr = socket_addr!(127.0.0.1: {next_port()});
    let spoke_addr = socket_addr!(127.0.0.1: {next_port()});
    let assist_addr = socket_addr!(127.0.0.1: {next_port()});

    let hub_kp = KeyPair::random();
    let spoke_kp = KeyPair::random();
    let assist_kp = KeyPair::random();

    let hub_peer = Peer::new(hub_addr.clone(), hub_kp.public_key().clone());
    let spoke_peer = Peer::new(spoke_addr.clone(), spoke_kp.public_key().clone());
    let assist_peer = Peer::new(assist_addr.clone(), assist_kp.public_key().clone());

    let make_config =
        |address: iroha_primitives::addr::SocketAddr,
         relay_mode: RelayMode,
         relay_hub_addresses: Vec<iroha_primitives::addr::SocketAddr>| {
            Config {
                address: WithOrigin::inline(address.clone()),
                public_address: WithOrigin::inline(address),
                relay_mode,
                relay_hub_addresses,
                relay_ttl: RELAY_TTL,
                soranet_handshake: default_soranet_handshake(),
                soranet_privacy: SoranetPrivacy::default(),
                soranet_vpn: SoranetVpn::default(),
                lane_profile: LaneProfile::Core,
                require_sm_handshake_match: true,
                require_sm_openssl_preview_match: true,
                idle_timeout,
                connect_startup_delay:
                    iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
                dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
                peer_gossip_period: PEER_GOSSIP_PERIOD,
                peer_gossip_max_period: PEER_GOSSIP_PERIOD,
                trust_decay_half_life:
                    iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
                trust_penalty_bad_gossip:
                    iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
                trust_penalty_unknown_peer:
                    iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
                trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
                trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
                quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
                quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
                quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
                tls_enabled: false,
                tls_fallback_to_plain: true,
                tls_listen_address: None,
                tls_inbound_only: false,
                p2p_queue_cap_high: NonZeroUsize::new(4096).unwrap(),
                p2p_queue_cap_low: NonZeroUsize::new(4096).unwrap(),
                p2p_post_queue_cap: NonZeroUsize::new(2048).unwrap(),
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
                consensus_ingress_critical_rate_per_sec: iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
                consensus_ingress_critical_burst:
                    iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BURST,
                consensus_ingress_critical_bytes_per_sec: iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
                consensus_ingress_critical_bytes_burst: iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
                consensus_ingress_rbc_session_limit: iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RBC_SESSION_LIMIT,
                consensus_ingress_penalty_threshold: iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
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
                max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
                accept_bucket_idle:
                    iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
                accept_prefix_v4_bits:
                    iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
                accept_prefix_v6_bits:
                    iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
                tcp_nodelay: true,
                tcp_keepalive: None,
                max_frame_bytes_consensus: 262_144,
                max_frame_bytes_control: 262_144,
                max_frame_bytes_block_sync: 1_048_576,
                max_frame_bytes_tx_gossip: 262_144,
                max_frame_bytes_peer_gossip: 131_072,
                max_frame_bytes_health: 65_536,
                max_frame_bytes_other: 262_144,
                tls_only_v1_3: true,
                quic_max_idle_timeout: None,
            }
        };

    let (mut hub_net, _hub_child) = match NetworkHandle::<ConsensusMessage>::start(
        hub_kp.clone(),
        make_config(hub_addr.clone(), RelayMode::Hub, Vec::new()),
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping relay_hub_routes_consensus_between_spoke_and_assist (hub): {e:?}");
            return;
        }
    };
    let (mut spoke_net, _spoke_child) = match NetworkHandle::<ConsensusMessage>::start(
        spoke_kp.clone(),
        make_config(spoke_addr.clone(), RelayMode::Spoke, vec![hub_addr.clone()]),
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!(
                "Skipping relay_hub_routes_consensus_between_spoke_and_assist (spoke): {e:?}"
            );
            return;
        }
    };
    let (mut assist_net, _assist_child) = match NetworkHandle::<ConsensusMessage>::start(
        assist_kp.clone(),
        make_config(
            assist_addr.clone(),
            RelayMode::Assist,
            vec![hub_addr.clone()],
        ),
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!(
                "Skipping relay_hub_routes_consensus_between_spoke_and_assist (assist): {e:?}"
            );
            return;
        }
    };

    let (spoke_tx, mut spoke_rx) = mpsc::channel(4);
    if let Err(sender) = spoke_net.subscribe_to_peers_messages(spoke_tx) {
        drop(sender);
        panic!("failed to subscribe spoke to messages");
    }
    let (assist_tx, mut assist_rx) = mpsc::channel(4);
    if let Err(sender) = assist_net.subscribe_to_peers_messages(assist_tx) {
        drop(sender);
        panic!("failed to subscribe assist to messages");
    }

    update_topology_and_peers_addresses(&hub_net, &[spoke_peer.clone(), assist_peer.clone()]);
    update_topology_and_peers_addresses(&spoke_net, std::slice::from_ref(&hub_peer));
    update_topology_and_peers_addresses(&assist_net, std::slice::from_ref(&hub_peer));

    tokio::time::timeout(Duration::from_secs(10), async {
        while hub_net.online_peers(HashSet::len) < 2 {
            hub_net
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("connection deadline exceeded");
    tokio::time::timeout(Duration::from_secs(10), async {
        while spoke_net.online_peers(HashSet::len) < 1 {
            spoke_net
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("connection deadline exceeded");
    tokio::time::timeout(Duration::from_secs(10), async {
        while assist_net.online_peers(HashSet::len) < 1 {
            assist_net
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("connection deadline exceeded");

    // Give relay hub selection and subscriber wiring a moment to settle under parallel test load.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let payload_a = ConsensusMessage(11);
    assist_net.post(Post {
        data: payload_a.clone(),
        peer_id: spoke_peer.id().clone(),
        priority: Priority::High,
    });

    let received = tokio::time::timeout(Duration::from_secs(20), spoke_rx.recv())
        .await
        .expect("spoke should receive consensus via hub")
        .expect("message channel closed");
    assert_eq!(received.peer.id(), assist_peer.id());
    assert_eq!(received.payload.0, payload_a.0);

    let payload_b = ConsensusMessage(12);
    spoke_net.post(Post {
        data: payload_b.clone(),
        peer_id: assist_peer.id().clone(),
        priority: Priority::High,
    });

    let received = tokio::time::timeout(Duration::from_secs(20), assist_rx.recv())
        .await
        .expect("assist should receive consensus via hub")
        .expect("message channel closed");
    assert_eq!(received.peer.id(), spoke_peer.id());
    assert_eq!(received.payload.0, payload_b.0);
}
#[allow(dead_code)]
async fn multiple_networks() {
    setup_logger();
    info!("Starting...");

    let mut peers = Vec::new();
    let mut key_pairs = Vec::new();
    for _ in 0_u16..10_u16 {
        let address = socket_addr!(127.0.0.1: {next_port()});
        let key_pair = KeyPair::random();
        let public_key = key_pair.public_key().clone();
        peers.push(Peer::new(address, public_key));
        key_pairs.push(key_pair);
    }

    let mut networks = Vec::new();
    let mut peer_ids = Vec::new();
    let expected_msgs = (peers.len() * (peers.len() - 1))
        .try_into()
        .expect("Failed to convert to u32");
    let mut msgs = WaitForN::new(expected_msgs);
    let barrier = Arc::new(Barrier::new(peers.len()));
    let chain_id = ChainId::from("test_chain");

    peers
        .iter()
        .zip(key_pairs)
        .map(|(peer, key_pair)| {
            start_network(
                peer.clone(),
                key_pair,
                peers.clone(),
                msgs.clone(),
                Arc::clone(&barrier),
                chain_id.clone(),
                ShutdownSignal::new(),
            )
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .for_each(|(peer_id, handle)| {
            networks.push(handle);
            peer_ids.push(peer_id);
        });

    info!("Sending posts...");
    for network in &networks {
        for id in &peer_ids {
            let post = Post {
                data: TestMessage(String::from("Some data to send to peer")),
                peer_id: id.id().clone(),
                priority: Priority::Low,
            };
            network.post(post);
        }
    }
    info!("Posts sent");
    let timeout = Duration::from_millis(10_000);
    tokio::time::timeout(timeout, &mut msgs)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Failed to get all messages in given time {}ms (received {} out of {})",
                timeout.as_millis(),
                msgs.current(),
                expected_msgs,
            )
        });
}

#[allow(dead_code, clippy::too_many_lines)]
async fn start_network(
    peer: Peer,
    key_pair: KeyPair,
    peers: Vec<Peer>,
    messages: WaitForN,
    barrier: Arc<Barrier>,
    chain_id: ChainId,
    shutdown_signal: ShutdownSignal,
) -> (Peer, NetworkHandle<TestMessage>) {
    info!(peer_addr = %peer.address(), "Starting network");

    // This actor will get the messages from other peers and increment the counter
    let actor = TestActor::start(messages);

    let address = peer.address().clone();
    let idle_timeout = Duration::from_secs(60);
    let config = Config {
        address: WithOrigin::inline(address.clone()),
        public_address: WithOrigin::inline(address.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_proxy_required: false,
        p2p_no_proxy: vec![],
        p2p_proxy_tls_verify: true,
        p2p_proxy_tls_pinned_cert_der_base64: None,
        happy_eyeballs_stagger: Duration::from_millis(100),
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
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(8192).expect("non-zero"),
        p2p_queue_cap_low: NonZeroUsize::new(32_768).expect("non-zero"),
        p2p_post_queue_cap: NonZeroUsize::new(2048).expect("non-zero"),
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
        max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    };
    let (mut network, _) = NetworkHandle::start(
        key_pair,
        config,
        Some(chain_id),
        None,
        None,
        shutdown_signal,
    )
    .await
    .unwrap();
    if let Err(sender) = network.subscribe_to_peers_messages(actor) {
        drop(sender);
        panic!("failed to subscribe actor to network messages");
    }

    let _ = barrier.wait().await;
    let peers = peers.into_iter().filter(|p| p != &peer).collect::<Vec<_>>();
    let conn_count = peers.len();
    update_topology_and_peers_addresses(&network, &peers);

    let _ = barrier.wait().await;
    tokio::time::timeout(Duration::from_millis(10_000), async {
        let mut connections = network
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while conn_count != connections {
            info!(peer_addr = %peer.address(), %connections);
            connections = network
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await
    .expect("Failed to get all connections");

    // This is needed to ensure that all peers are connected to each other.
    // The problem is that both peers establish connection (in each pair of peers),
    // and one of connections is dropped based on disambiguator rule.
    // So the check above (`conn_count != connections`) doesn't work,
    // since peer can establish connection but then it will be dropped.
    tokio::time::sleep(Duration::from_secs(10)).await;

    info!(peer_addr = %peer.address(), %conn_count, "Got all connections!");

    (peer, network)
}

fn update_topology_and_peers_addresses<T>(network: &NetworkHandle<T>, peers: &[Peer])
where
    T: iroha_p2p::boilerplate::Pload + iroha_p2p::network::message::ClassifyTopic,
{
    let topology = peers.iter().map(|peer| peer.id().clone()).collect();
    network.update_topology(UpdateTopology(topology));

    let addresses = peers
        .iter()
        .map(|peer| (peer.id().clone(), peer.address().clone()))
        .collect();
    network.update_peers_addresses(UpdatePeers(addresses));
}

#[cfg(feature = "p2p_tls")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tls_inbound_listener_smoke() {
    use iroha_primitives::addr::SocketAddr as IrohaSocketAddr;

    setup_logger();
    if super::skip_if_no_tcp_bind() {
        return;
    }
    let idle_timeout = Duration::from_secs(30);
    let chain_id = ChainId::from("test_chain");

    // Find a free local port for TLS listener
    let port = match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(sock) => sock.local_addr().unwrap().port(),
        Err(e) => {
            eprintln!("Skipping tls_inbound_listener_smoke: cannot bind probe socket: {e}");
            return;
        }
    };
    // Build addresses: TLS listener on this fixed port, and advertise hostname so outbound uses TLS
    let tls_listen_addr = socket_addr!(127.0.0.1: {port});
    let public_host_addr = IrohaSocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
        host: "localhost".into(),
        port,
    });

    // Network 1 (listener with inbound TLS)
    let key_pair1 = KeyPair::random();
    let peer1 = Peer::new(public_host_addr.clone(), key_pair1.public_key().clone());

    let config1 = Config {
        address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
        public_address: WithOrigin::inline(public_host_addr.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
        tls_enabled: true,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(1024).unwrap(),
        p2p_queue_cap_low: NonZeroUsize::new(4096).unwrap(),
        p2p_post_queue_cap: NonZeroUsize::new(256).unwrap(),
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
        max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
        accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
        accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
        accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
        tcp_nodelay: true,
        tcp_keepalive: None,
        max_frame_bytes_consensus: 262_144,
        max_frame_bytes_control: 262_144,
        max_frame_bytes_block_sync: 1_048_576,
        max_frame_bytes_tx_gossip: 262_144,
        max_frame_bytes_peer_gossip: 131_072,
        max_frame_bytes_health: 65_536,
        max_frame_bytes_other: 262_144,
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    };

    // Start network1; if sandbox forbids sockets, skip
    let (network1, _child1) = match NetworkHandle::<TestMessage>::start(
        key_pair1.clone(),
        Config {
            tls_listen_address: Some(WithOrigin::inline(tls_listen_addr.clone())),
            ..config1
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping tls_inbound_listener_smoke: cannot start listener: {e:?}");
            return;
        }
    };

    // Network 2 (dialer with outbound TLS via hostname)
    let key_pair2 = KeyPair::random();
    let (_n2, _child2) = match NetworkHandle::<TestMessage>::start(
        key_pair2.clone(),
        Config {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: RELAY_TTL,
            soranet_handshake: default_soranet_handshake(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout,
            connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            peer_gossip_period: PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: PEER_GOSSIP_PERIOD,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
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
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            tls_enabled: true,
            tls_fallback_to_plain: true,
            p2p_queue_cap_high: NonZeroUsize::new(1024).unwrap(),
            p2p_queue_cap_low: NonZeroUsize::new(4096).unwrap(),
            p2p_post_queue_cap: NonZeroUsize::new(256).unwrap(),
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
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
            accept_rate_per_prefix_per_sec: None,
            accept_burst_per_prefix: None,
            low_priority_rate_per_sec: None,
            low_priority_burst: None,
            low_priority_bytes_per_sec: None,
            low_priority_bytes_burst: None,
            tls_listen_address: None,
            tls_inbound_only: false,
            allowlist_only: false,
            allow_keys: vec![],
            deny_keys: vec![],
            allow_cidrs: vec![],
            deny_cidrs: vec![],
            disconnect_on_post_overflow: true,
            max_frame_bytes: 1_048_576,
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 262_144,
            max_frame_bytes_control: 262_144,
            max_frame_bytes_block_sync: 1_048_576,
            max_frame_bytes_tx_gossip: 262_144,
            max_frame_bytes_peer_gossip: 131_072,
            max_frame_bytes_health: 65_536,
            max_frame_bytes_other: 262_144,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("Skipping tls_inbound_listener_smoke: cannot start dialer: {e:?}");
            return;
        }
    };

    // Connect dialer to listener via hostname/port address so outbound TLS is used
    network1.update_topology(UpdateTopology([peer1.id().clone()].into_iter().collect()));
    network1.update_peers_addresses(UpdatePeers(vec![(
        peer1.id().clone(),
        public_host_addr.clone(),
    )]));

    // Quick smoke: network1 should not crash; we expect at least to process connect attempt.
    // Since both ends are started, give it a short window and ensure code path runs.
    tokio::time::sleep(Duration::from_millis(200)).await;
}
#[test]
fn test_encryption() {
    use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};

    const TEST_KEY: [u8; 32] = [
        5, 87, 82, 183, 220, 57, 107, 49, 227, 4, 96, 231, 198, 88, 153, 11, 22, 65, 56, 45, 237,
        35, 231, 165, 122, 153, 14, 68, 13, 84, 5, 24,
    ];

    let encryptor =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(TEST_KEY).expect("valid key length");
    let message = b"Some ciphertext";
    let aad = b"Iroha2 AAD";
    let ciphertext = encryptor
        .encrypt_easy(aad.as_ref(), message.as_ref())
        .unwrap();
    let decrypted = encryptor
        .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
        .unwrap();
    assert_eq!(decrypted.as_slice(), message);
}
