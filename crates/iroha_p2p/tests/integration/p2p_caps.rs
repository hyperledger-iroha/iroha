//! Topic cap enforcement tests. Skips gracefully if sockets are unavailable.

use std::{collections::HashSet, num::NonZeroUsize};

use iroha_config::parameters::{
    actual::{
        LaneProfile, Network as Config, RelayMode, SoranetHandshake as ActualSoranetHandshake,
        SoranetPow, SoranetPrivacy, SoranetVpn,
    },
    defaults::network::{PEER_GOSSIP_PERIOD, RELAY_TTL},
};
use iroha_config_base::WithOrigin;
use iroha_crypto::soranet::handshake::{
    DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
};
use iroha_data_model::prelude::Peer;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{
    NetworkHandle,
    network::{NetworkActorAdmissionError, NetworkActorAdmissionRejection, message::*},
};
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
use iroha_primitives::addr::SocketAddrHost;
use iroha_primitives::addr::{SocketAddr, socket_addr};
use norito::codec::{Decode, Encode};
#[cfg(feature = "p2p_ws")]
use tokio::net::TcpListener;
use tokio::time::Duration;

// These tests assert process-global cap counters, so their snapshots must not overlap.
static FRAME_CAP_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Debug, Decode, Encode)]
struct BigMsg {
    topic: u8,
    data: Vec<u8>,
}

impl ClassifyTopic for BigMsg {
    fn topic(&self) -> Topic {
        match self.topic {
            0 => Topic::Consensus,
            1 => Topic::Control,
            2 => Topic::BlockSync,
            3 => Topic::TxGossip,
            4 => Topic::PeerGossip,
            5 => Topic::Health,
            _ => Topic::Other,
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for BigMsg {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}

async fn wait_for_peer_state(
    network: &NetworkHandle<BigMsg>,
    should_be_online: bool,
    timeout: Duration,
    poll_interval: Duration,
) -> bool {
    tokio::time::timeout(timeout, async {
        loop {
            let is_online = network.online_peers(HashSet::len) > 0;
            if is_online == should_be_online {
                break;
            }
            tokio::time::sleep(poll_interval).await;
        }
    })
    .await
    .is_ok()
}

async fn wait_for_both_online(
    first: &NetworkHandle<BigMsg>,
    second: &NetworkHandle<BigMsg>,
    timeout: Duration,
    poll_interval: Duration,
) -> bool {
    wait_for_peer_state(first, true, timeout, poll_interval).await
        && wait_for_peer_state(second, true, timeout, poll_interval).await
}

async fn wait_for_consensus_cap_increase(start_cap: u64, timeout: Duration) -> Option<u64> {
    tokio::time::timeout(timeout, async {
        loop {
            let current = iroha_p2p::network::cap_violations_consensus();
            if current > start_cap {
                break current;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .ok()
}

#[cfg(feature = "p2p_ws")]
async fn forward_one_ws_connection(
    listener: TcpListener,
    network: NetworkHandle<BigMsg>,
) -> std::io::Result<()> {
    let (stream, remote) = listener.accept().await?;
    let (read, write) = super::ws_io::accept_bounded(stream).await?;
    if network
        .accept_stream(read, write, remote)
        .await
        .map_err(|error| {
            std::io::Error::other(format!("network actor websocket handoff failed: {error}"))
        })?
    {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "network actor rejected websocket stream handoff",
        ))
    }
}

#[cfg(feature = "p2p_ws")]
async fn assert_ws_global_cap_disconnects(
    listener: &NetworkHandle<BigMsg>,
    dialer: &NetworkHandle<BigMsg>,
    listener_peer: &Peer,
) {
    let start_cap = iroha_p2p::network::cap_violations_consensus();
    dialer.post(Post {
        data: BigMsg {
            topic: 0,
            data: vec![0_u8; 8 * 1024],
        },
        peer_id: listener_peer.id().clone(),
        priority: Priority::High,
    });

    assert!(
        wait_for_peer_state(
            listener,
            false,
            Duration::from_millis(1_500),
            Duration::from_millis(50),
        )
        .await,
        "listener should disconnect after WS frame cap violation"
    );
    assert_eq!(
        iroha_p2p::network::cap_violations_consensus(),
        start_cap,
        "global cap enforcement must precede topic cap accounting",
    );

    let _ = wait_for_peer_state(
        dialer,
        false,
        Duration::from_millis(1_000),
        Duration::from_millis(50),
    )
    .await;
}

fn default_soranet_handshake() -> ActualSoranetHandshake {
    // Frame-cap tests do not exercise admission puzzles; avoid coupling their
    // deadlines to Argon2 cost or host load.
    let pow = SoranetPow {
        required: false,
        puzzle: None,
        ..SoranetPow::default()
    };
    ActualSoranetHandshake {
        descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
        client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
        relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
        trust_gossip: true,
        kem_id: 1,
        sig_id: 1,
        resume_hash: None,
        pow,
    }
}

#[allow(clippy::too_many_lines)]
fn make_config(
    addr: &SocketAddr,
    public: &SocketAddr,
    max_frame_bytes: usize,
    topic_cap: usize,
) -> Config {
    Config {
        address: WithOrigin::inline(addr.clone()),
        public_address: WithOrigin::inline(public.clone()),
        relay_mode: RelayMode::Disabled,
        relay_hub_addresses: Vec::new(),
        relay_ttl: RELAY_TTL,
        soranet_handshake: default_soranet_handshake(),
        soranet_privacy: SoranetPrivacy::default(),
        soranet_vpn: SoranetVpn::default(),
        lane_profile: LaneProfile::Core,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
        idle_timeout: Duration::from_millis(2000),
        reply_writer_flush_timeout:
            iroha_config::parameters::defaults::network::REPLY_WRITER_FLUSH_TIMEOUT,
        connect_startup_delay: iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
        dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
        deferred_send_ttl: std::time::Duration::from_millis(
            iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
        ),
        deferred_send_max_per_peer:
            iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
        deferred_send_max_bytes_per_peer:
            iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_BYTES_PER_PEER,
        deferred_send_max_bytes_total:
            iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_BYTES_TOTAL,
        peer_gossip_period: PEER_GOSSIP_PERIOD,
        peer_gossip_max_period: PEER_GOSSIP_PERIOD,
        trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
        trust_penalty_bad_gossip:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
        trust_penalty_unknown_peer:
            iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
        trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
        debug_packet_loss_inbound_percent: 0,
        debug_packet_loss_outbound_percent: 0,
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
        scion: iroha_config::parameters::actual::ScionConfig::default(),
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
        p2p_queue_cap_low: NonZeroUsize::new(128).unwrap(),
        p2p_post_queue_cap: NonZeroUsize::new(128).unwrap(),
        p2p_outbound_frame_queue_max_high_bytes:
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_BYTES,
        p2p_outbound_frame_queue_max_low_bytes:
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_LOW_BYTES,
        p2p_outbound_frame_queue_max_high_frames:
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_FRAMES,
        p2p_outbound_frame_queue_max_low_frames:
            iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_LOW_FRAMES,
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
        max_frame_bytes,
        max_frame_bytes_consensus: topic_cap,
        max_frame_bytes_control: topic_cap,
        max_frame_bytes_block_sync: topic_cap,
        max_frame_bytes_tx_gossip: topic_cap,
        max_frame_bytes_peer_gossip: topic_cap,
        max_frame_bytes_health: topic_cap,
        max_frame_bytes_other: topic_cap,
        tcp_nodelay: true,
        tcp_keepalive: Some(Duration::from_secs(60)),
        tls_only_v1_3: true,
        quic_max_idle_timeout: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn topic_cap_violation_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;

    let chain = super::test_network_id("test_chain");
    let kp1 = super::random_node_key_pair();
    let kp2 = super::random_node_key_pair();
    let a1 = super::next_addr();
    let a2 = super::next_addr();

    // Small caps for all topics (1 KiB) with a larger global cap so the
    // per-topic consensus cap handles the oversized frame.
    let cfg = |addr: SocketAddr| make_config(&addr, &addr, 16 * 1024, 1024);

    let started1 = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp1.clone()),
        cfg(a1.clone()),
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net1, _c1) = match started1 {
        Ok(ok) => ok,
        Err(_) => return,
    };
    let started2 = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp2.clone()),
        cfg(a2.clone()),
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _c2) = match started2 {
        Ok(ok) => ok,
        Err(_) => return,
    };

    // Connect with a single outbound dial to avoid racing simultaneous
    // connection resolution with the cap-violation post below.
    let p2 = Peer::new(a2.clone(), kp2.public_key().clone());
    let p1 = Peer::new(a1.clone(), kp1.public_key().clone());
    net1.update_topology(UpdateTopology(HashSet::from([p2.id().clone()])));
    net2.update_topology(UpdateTopology(HashSet::from([p1.id().clone()])));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), a1.clone())]));

    // Wait for both views of the connection to be established.
    if !wait_for_both_online(
        &net1,
        &net2,
        Duration::from_millis(1_500),
        Duration::from_millis(50),
    )
    .await
    {
        return;
    }

    // Track the initial consensus cap counter so exact actor admission can be
    // shown to account for the rejected frame.
    let start_cap = iroha_p2p::network::cap_violations_consensus();

    // Submit a BigMsg exceeding the topic cap (Consensus cap=1 KiB, data=8 KiB).
    let big = BigMsg {
        topic: 0,
        data: vec![0u8; 8192],
    };
    let rejection = net2
        .post_recoverable(
            Post {
                data: big,
                peer_id: p1.id().clone(),
                priority: Priority::High,
            },
            None,
        )
        .expect_err("the oversized consensus frame must fail recoverable admission");
    assert!(matches!(
        rejection,
        NetworkActorAdmissionError::Rejected {
            reason: NetworkActorAdmissionRejection::FrameTooLarge,
            ..
        }
    ));
    let end_cap = wait_for_consensus_cap_increase(start_cap, Duration::from_millis(1_000))
        .await
        .expect("consensus cap violation counter should increase");

    // Exact outbound admission rejects the oversized canonical frame before
    // transferring ownership, and the consensus cap counter records it.
    assert!(
        end_cap > start_cap,
        "consensus cap violations should increment for oversized frame"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn tcp_global_frame_cap_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;

    let chain = super::test_network_id("test_chain_tcp");
    let kp_listener = super::random_node_key_pair();
    let kp_dialer = super::random_node_key_pair();

    // Reserve a concrete TCP port for the listener so the dialer can reach it reliably.
    let probe = match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(sock) => sock,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(err) => panic!("bind probe socket: {err}"),
    };
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let listen_addr = socket_addr!(127.0.0.1: {port});
    let dialer_addr = super::next_addr();

    // Listener enforces a tight global frame cap, dialer keeps a generous cap so outbound succeeds.
    let listener_cfg = make_config(&listen_addr, &listen_addr, 1_024, 16 * 1024);
    let dialer_cfg = make_config(&dialer_addr, &dialer_addr, 16 * 1024, 16 * 1024);

    let started_listener = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_listener.clone()),
        listener_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net_listener, _child_listener) = match started_listener {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let started_dialer = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_dialer.clone()),
        dialer_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net_dialer, _child_dialer) = match started_dialer {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let peer_listener = Peer::new(listen_addr.clone(), kp_listener.public_key().clone());
    let peer_dialer = Peer::new(dialer_addr.clone(), kp_dialer.public_key().clone());

    // The listener only needs topology membership to accept the dialer; omitting the dialer
    // address prevents a simultaneous outbound session from masking the tested disconnect.
    net_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));

    net_dialer.update_topology(UpdateTopology(HashSet::from([peer_listener.id().clone()])));
    net_dialer.update_peers_addresses(UpdatePeers(vec![(
        peer_listener.id().clone(),
        listen_addr.clone(),
    )]));

    // Wait for the direct TCP connection to establish; skip in sandboxed environments.
    let online = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if net_listener.online_peers(HashSet::len) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if online.is_err() {
        return;
    }

    let start_cap = iroha_p2p::network::cap_violations_consensus();

    // Send a payload larger than the listener's global frame cap but within the dialer's allowance.
    let oversize = BigMsg {
        topic: 0,
        data: vec![0u8; 8 * 1024],
    };
    net_dialer.post(Post {
        data: oversize,
        peer_id: peer_listener.id().clone(),
        priority: Priority::High,
    });

    // The listener should drop the session once the oversized frame is observed.
    let dropped = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_listener.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        dropped.is_ok(),
        "listener should disconnect after global frame cap violation"
    );

    let end_cap = iroha_p2p::network::cap_violations_consensus();
    assert_eq!(
        end_cap, start_cap,
        "global cap enforcement must run before topic cap accounting",
    );

    // Dialer should eventually observe the connection closure as well.
    let _ = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_dialer.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
}

#[cfg(feature = "p2p_tls")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tls_global_frame_cap_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;

    let chain = super::test_network_id("test_chain_tls");
    let kp_listener = super::random_node_key_pair();
    let kp_dialer = super::random_node_key_pair();

    // Reserve a TCP port for TLS listener (the same port is reused for QUIC-less TCP listener).
    let probe = match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(sock) => sock,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(err) => panic!("bind probe socket: {err}"),
    };
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let tls_listen = socket_addr!(127.0.0.1: {port});
    let public_host = SocketAddr::Host(SocketAddrHost {
        host: "localhost".into(),
        port,
    });

    // Listener enforces a small global frame cap, dialer uses a generous cap so outbound succeeds.
    let mut listener_cfg = make_config(&tls_listen, &public_host, 1024, 4096);
    listener_cfg.tls_enabled = true;
    listener_cfg.tls_listen_address = Some(WithOrigin::inline(tls_listen.clone()));

    let client_addr = super::next_addr();
    let mut dialer_cfg = make_config(&client_addr, &client_addr, 16 * 1024, 16 * 1024);
    dialer_cfg.tls_enabled = true;

    let started_listener = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_listener.clone()),
        listener_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net_listener, _child_listener) = match started_listener {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let started_dialer = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_dialer.clone()),
        dialer_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net_dialer, _child_dialer) = match started_dialer {
        Ok(ok) => ok,
        Err(_) => return,
    };

    // Exchange topology using hostname so the dialer attempts the TLS path.
    let peer_listener = Peer::new(public_host.clone(), kp_listener.public_key().clone());
    let peer_dialer = Peer::new(client_addr.clone(), kp_dialer.public_key().clone());

    // Keep this one-way so the oversized inbound frame closes the only listener-side session.
    net_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));

    net_dialer.update_topology(UpdateTopology(HashSet::from([peer_listener.id().clone()])));
    net_dialer.update_peers_addresses(UpdatePeers(vec![(
        peer_listener.id().clone(),
        public_host.clone(),
    )]));

    // Wait for connection establishment (skip if environment prevents it).
    let online = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if net_listener.online_peers(HashSet::len) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if online.is_err() {
        return;
    }

    let start_cap = iroha_p2p::network::cap_violations_consensus();

    // Send a payload exceeding listener's global frame cap but within the dialer's cap.
    let oversize = BigMsg {
        topic: 0,
        data: vec![0u8; 8 * 1024],
    };
    net_dialer.post(Post {
        data: oversize,
        peer_id: peer_listener.id().clone(),
        priority: Priority::High,
    });

    // Expect the listener to drop the connection after rejecting the oversized frame.
    let dropped = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_listener.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        dropped.is_ok(),
        "listener should disconnect after frame cap violation"
    );

    let end_cap = iroha_p2p::network::cap_violations_consensus();
    assert_eq!(
        end_cap, start_cap,
        "global frame cap enforcement should occur before topic caps are counted"
    );

    // Dialer should eventually observe zero peers once the listener drops the session.
    let _ = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_dialer.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
}

#[cfg(feature = "quic")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn quic_global_frame_cap_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;

    let chain = super::test_network_id("test_chain_quic");
    let kp_listener = super::random_node_key_pair();
    let kp_dialer = super::random_node_key_pair();

    // Reserve a UDP/TCP port for QUIC + TCP listener pair.
    let probe = match std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(sock) => sock,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "Skipping quic_global_frame_cap_disconnects: cannot bind UDP probe socket: {e}"
            );
            return;
        }
        Err(e) => panic!("bind probe udp socket: {e}"),
    };
    let port = probe.local_addr().expect("probe local addr").port();
    drop(probe);

    let listen_addr = socket_addr!(127.0.0.1: {port});
    let public_host = SocketAddr::Host(SocketAddrHost {
        host: "localhost".into(),
        port,
    });

    let mut listener_cfg = make_config(&listen_addr, &public_host, 1024, 4096);
    listener_cfg.quic_enabled = true;

    let client_addr = super::next_addr();
    let mut dialer_cfg = make_config(&client_addr, &client_addr, 16 * 1024, 16 * 1024);
    dialer_cfg.quic_enabled = true;

    let started_listener = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_listener.clone()),
        listener_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net_listener, _child_listener) = match started_listener {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let started_dialer = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_dialer.clone()),
        dialer_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net_dialer, _child_dialer) = match started_dialer {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let peer_listener = Peer::new(public_host.clone(), kp_listener.public_key().clone());
    let peer_dialer = Peer::new(client_addr.clone(), kp_dialer.public_key().clone());

    // Keep this one-way so the oversized inbound frame closes the only listener-side session.
    net_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));

    net_dialer.update_topology(UpdateTopology(HashSet::from([peer_listener.id().clone()])));
    net_dialer.update_peers_addresses(UpdatePeers(vec![(
        peer_listener.id().clone(),
        public_host.clone(),
    )]));

    let online = tokio::time::timeout(Duration::from_millis(1800), async {
        loop {
            if net_listener.online_peers(HashSet::len) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    })
    .await;
    if online.is_err() {
        return;
    }

    let start_cap = iroha_p2p::network::cap_violations_consensus();

    let oversize = BigMsg {
        topic: 0,
        data: vec![0u8; 8 * 1024],
    };
    net_dialer.post(Post {
        data: oversize,
        peer_id: peer_listener.id().clone(),
        priority: Priority::High,
    });

    let dropped = tokio::time::timeout(Duration::from_millis(1200), async {
        loop {
            if net_listener.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    })
    .await;
    assert!(
        dropped.is_ok(),
        "listener should disconnect after QUIC frame cap violation"
    );

    let end_cap = iroha_p2p::network::cap_violations_consensus();
    assert_eq!(
        end_cap, start_cap,
        "global frame enforcement must precede topic cap accounting"
    );

    let _ = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_dialer.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
}

#[cfg(feature = "p2p_ws")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ws_global_frame_cap_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;

    let chain = super::test_network_id("test_chain_ws");
    let kp_listener = super::random_node_key_pair();
    let kp_dialer = super::random_node_key_pair();

    let ws_listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "skipping ws_global_frame_cap_disconnects: loopback bind is forbidden: {err}"
            );
            return;
        }
        Err(err) => panic!("bind ws listener: {err}"),
    };
    let ws_addr = ws_listener.local_addr().expect("ws listener addr");

    let listener_addr = super::next_addr();
    let listener_cfg = make_config(&listener_addr, &listener_addr, 1_024, 16 * 1024);
    let (network_listener, _child_listener) = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_listener.clone()),
        listener_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    .expect("start websocket listener network");

    let forwarder = tokio::spawn(forward_one_ws_connection(
        ws_listener,
        network_listener.clone(),
    ));

    let dialer_addr = super::next_addr();
    let mut dialer_cfg = make_config(&dialer_addr, &dialer_addr, 16 * 1024, 16 * 1024);
    dialer_cfg.prefer_ws_fallback = true;
    let (net_dialer, _child_dialer) = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_dialer.clone()),
        dialer_cfg,
        chain.clone(),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    .expect("start websocket dialer network");

    // Listener only needs topology knowledge to accept the inbound session.
    let peer_dialer = Peer::new(dialer_addr.clone(), kp_dialer.public_key().clone());
    network_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));

    let listener_host: SocketAddr = format!("localhost:{}", ws_addr.port())
        .parse()
        .expect("host socket addr");
    let peer_listener = Peer::new(listener_host.clone(), kp_listener.public_key().clone());

    net_dialer.update_topology(UpdateTopology(HashSet::from([peer_listener.id().clone()])));
    net_dialer.update_peers_addresses(UpdatePeers(vec![(
        peer_listener.id().clone(),
        listener_host.clone(),
    )]));

    tokio::time::timeout(Duration::from_millis(2_000), forwarder)
        .await
        .expect("websocket accept/handshake task timed out")
        .expect("websocket accept/handshake task panicked")
        .expect("websocket handshake or network handoff failed");

    assert!(
        wait_for_peer_state(
            &network_listener,
            true,
            Duration::from_millis(2_000),
            Duration::from_millis(50),
        )
        .await,
        "websocket listener network did not observe the peer before the online timeout"
    );

    assert_ws_global_cap_disconnects(&network_listener, &net_dialer, &peer_listener).await;
}
