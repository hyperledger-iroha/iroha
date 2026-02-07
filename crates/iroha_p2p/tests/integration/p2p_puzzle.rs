//! Puzzle-gated handshake edge cases for P2P.

use std::{
    collections::HashSet,
    num::NonZeroU32,
    time::{Duration, Instant},
};

use iroha_config::parameters::{
    actual::{
        LaneProfile, Network as Config, RelayMode, SoranetHandshake as ActualSoranetHandshake,
        SoranetPow, SoranetPrivacy, SoranetPuzzle, SoranetVpn,
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
use iroha_data_model::{ChainId, prelude::Peer};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{
    NetworkHandle,
    network::message::{ClassifyTopic, Topic, UpdatePeers, UpdateTopology},
    peer,
};
use iroha_primitives::addr::socket_addr;
use norito::codec::{Decode, Encode};

use super::next_port;

#[derive(Clone, Debug, Decode, Encode)]
struct EmptyMsg;

impl ClassifyTopic for EmptyMsg {
    fn topic(&self) -> Topic {
        Topic::Other
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for EmptyMsg {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}

fn puzzle_handshake(difficulty: u8, memory_kib: u32) -> ActualSoranetHandshake {
    let mut handshake = ActualSoranetHandshake {
        descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
        client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
        relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
        trust_gossip: true,
        kem_id: 1,
        sig_id: 1,
        resume_hash: None,
        pow: SoranetPow::default(),
    };
    handshake.pow.required = true;
    handshake.pow.difficulty = difficulty;
    handshake.pow.max_future_skew = Duration::from_secs(300);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    handshake.pow.ticket_ttl = Duration::from_secs(120);
    handshake.pow.puzzle = Some(SoranetPuzzle {
        memory_kib: NonZeroU32::new(memory_kib).expect("non-zero puzzle memory"),
        time_cost: NonZeroU32::new(2).expect("non-zero time cost"),
        lanes: NonZeroU32::new(1).expect("non-zero lanes"),
    });
    handshake
}

fn config(addr: iroha_primitives::addr::SocketAddr, handshake: ActualSoranetHandshake) -> Config {
    let public_addr = addr.clone();
    Config {
        address: WithOrigin::inline(addr),
        public_address: WithOrigin::inline(public_addr),
        relay_mode: RelayMode::Disabled,
        relay_hub_address: None,
        relay_ttl: RELAY_TTL,
        soranet_handshake: handshake,
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout: Duration::from_secs(5),
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
        dns_refresh_interval: None,
        dns_refresh_ttl: None,
        quic_enabled: false,
        tls_enabled: false,
        tls_listen_address: None,
        prefer_ws_fallback: false,
        p2p_proxy: None,
        p2p_no_proxy: vec![],
        p2p_queue_cap_high: core::num::NonZeroUsize::new(1024).unwrap(),
        p2p_queue_cap_low: core::num::NonZeroUsize::new(1024).unwrap(),
        p2p_post_queue_cap: core::num::NonZeroUsize::new(256).unwrap(),
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
        happy_eyeballs_stagger: Duration::from_millis(50),
        addr_ipv6_first: false,
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
        allow_keys: Vec::new(),
        deny_keys: Vec::new(),
        allow_cidrs: Vec::new(),
        deny_cidrs: Vec::new(),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn puzzle_mismatch_rejects_handshake() {
    let chain = ChainId::from("puzzle_mismatch");
    let baseline_failures = peer::handshake_failure_count();

    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1: {next_port()});
    let addr2 = socket_addr!(127.0.0.1: {next_port()});

    let handshake_entry = puzzle_handshake(4, 8 * 1024);
    let handshake_exit = puzzle_handshake(7, 8 * 1024);

    let started1 = NetworkHandle::<EmptyMsg>::start(
        kp1.clone(),
        config(addr1.clone(), handshake_entry),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net1, _child1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let started2 = NetworkHandle::<EmptyMsg>::start(
        kp2.clone(),
        config(addr2.clone(), handshake_exit),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _child2) = match started2 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let peer1 = Peer::new(addr1, kp1.public_key().clone());
    let peer2 = Peer::new(addr2, kp2.public_key().clone());

    net1.update_topology(UpdateTopology([peer2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(
        peer2.id().clone(),
        peer2.address().clone(),
    )]));
    net2.update_topology(UpdateTopology([peer1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(
        peer1.id().clone(),
        peer1.address().clone(),
    )]));

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut observed_failure = false;
    while Instant::now() < deadline {
        if peer::handshake_failure_count() > baseline_failures {
            observed_failure = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        observed_failure,
        "expected puzzle mismatch to cause handshake failure"
    );
    assert_eq!(
        net1.online_peers(HashSet::len),
        0,
        "entry relay must not mark the mismatched peer online"
    );
    assert_eq!(
        net2.online_peers(HashSet::len),
        0,
        "exit relay must not mark the mismatched peer online"
    );
}
