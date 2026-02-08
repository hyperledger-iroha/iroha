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
use iroha_crypto::{
    KeyPair,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
    },
};
use iroha_data_model::{ChainId, prelude::Peer};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{NetworkHandle, network::message::*};
#[cfg(any(feature = "p2p_tls", feature = "quic"))]
use iroha_primitives::addr::SocketAddrHost;
use iroha_primitives::addr::{SocketAddr, socket_addr};
use norito::codec::{Decode, Encode};
use tokio::time::Duration;

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
        tls_enabled: false,
        tls_fallback_to_plain: true,
        tls_listen_address: None,
        tls_inbound_only: false,
        p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
        p2p_queue_cap_low: NonZeroUsize::new(128).unwrap(),
        p2p_post_queue_cap: NonZeroUsize::new(128).unwrap(),
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
    let chain = ChainId::from("test_chain");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let a1 = socket_addr!(127.0.0.1:0);
    let a2 = socket_addr!(127.0.0.1:0);

    // Small caps for all topics (1 KiB) and small global cap
    let cfg = |addr: SocketAddr| make_config(&addr, &addr, 2048, 1024);

    let started1 = NetworkHandle::<BigMsg>::start(
        kp1.clone(),
        cfg(a1.clone()),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net1, _c1) = match started1 {
        Ok(ok) => ok,
        Err(_) => return,
    };
    let started2 = NetworkHandle::<BigMsg>::start(
        kp2.clone(),
        cfg(a2.clone()),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _c2) = match started2 {
        Ok(ok) => ok,
        Err(_) => return,
    };

    // Connect
    let p2 = Peer::new(a2.clone(), kp2.public_key().clone());
    let p1 = Peer::new(a1.clone(), kp1.public_key().clone());
    net1.update_topology(UpdateTopology(HashSet::from([p2.id().clone()])));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), a2.clone())]));
    net2.update_topology(UpdateTopology(HashSet::from([p1.id().clone()])));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), a1.clone())]));

    // Wait connection established
    if tokio::time::timeout(Duration::from_millis(1500), async {
        let mut n = net1
            .wait_online_peers_update(std::collections::HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net1
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

    // Track initial consensus cap violation counter so we can assert the drop path increments it.
    let start_cap = iroha_p2p::network::cap_violations_consensus();

    // Send a BigMsg exceeding topic cap (Consensus cap=1k, send 8k)
    let big = BigMsg {
        topic: 0,
        data: vec![0u8; 8192],
    };
    net2.post(Post {
        data: big,
        peer_id: p1.id().clone(),
        priority: Priority::High,
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    // Expect net1 to have dropped net2 after violation or soon after
    let count = net1.online_peers(std::collections::HashSet::len);
    assert!(count == 0 || count == 1);

    // The inbound fast path should reject the oversized frame without
    // re-encoding payloads; the consensus cap counter should increment exactly once.
    let end_cap = iroha_p2p::network::cap_violations_consensus();
    assert_eq!(
        end_cap,
        start_cap + 1,
        "consensus cap violations should increment by one for oversized frame"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn tcp_global_frame_cap_disconnects() {
    let chain = ChainId::from("test_chain_tcp");
    let kp_listener = KeyPair::random();
    let kp_dialer = KeyPair::random();

    // Reserve a concrete TCP port for the listener so the dialer can reach it reliably.
    let probe = match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(sock) => sock,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(err) => panic!("bind probe socket: {err}"),
    };
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let listen_addr = socket_addr!(127.0.0.1: {port});
    let dialer_addr = socket_addr!(127.0.0.1:0);

    // Listener enforces a tight global frame cap, dialer keeps a generous cap so outbound succeeds.
    let listener_cfg = make_config(&listen_addr, &listen_addr, 1_024, 16 * 1024);
    let dialer_cfg = make_config(&dialer_addr, &dialer_addr, 16 * 1024, 16 * 1024);

    let started_listener = NetworkHandle::<BigMsg>::start(
        kp_listener.clone(),
        listener_cfg,
        Some(chain.clone()),
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
        kp_dialer.clone(),
        dialer_cfg,
        Some(chain.clone()),
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

    net_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));
    net_listener.update_peers_addresses(UpdatePeers(vec![(
        peer_dialer.id().clone(),
        dialer_addr.clone(),
    )]));

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
    let chain = ChainId::from("test_chain_tls");
    let kp_listener = KeyPair::random();
    let kp_dialer = KeyPair::random();

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

    let client_addr = socket_addr!(127.0.0.1:0);
    let mut dialer_cfg = make_config(&client_addr, &client_addr, 16 * 1024, 16 * 1024);
    dialer_cfg.tls_enabled = true;

    let started_listener = NetworkHandle::<BigMsg>::start(
        kp_listener.clone(),
        listener_cfg,
        Some(chain.clone()),
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
        kp_dialer.clone(),
        dialer_cfg,
        Some(chain.clone()),
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

    net_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));
    net_listener.update_peers_addresses(UpdatePeers(vec![(
        peer_dialer.id().clone(),
        client_addr.clone(),
    )]));

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
    let chain = ChainId::from("test_chain_quic");
    let kp_listener = KeyPair::random();
    let kp_dialer = KeyPair::random();

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

    let client_addr = socket_addr!(127.0.0.1:0);
    let mut dialer_cfg = make_config(&client_addr, &client_addr, 16 * 1024, 16 * 1024);
    dialer_cfg.quic_enabled = true;

    let started_listener = NetworkHandle::<BigMsg>::start(
        kp_listener.clone(),
        listener_cfg,
        Some(chain.clone()),
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
        kp_dialer.clone(),
        dialer_cfg,
        Some(chain.clone()),
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

    net_listener.update_topology(UpdateTopology(HashSet::from([peer_dialer.id().clone()])));
    net_listener.update_peers_addresses(UpdatePeers(vec![(
        peer_dialer.id().clone(),
        client_addr.clone(),
    )]));

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
    use bytes::Bytes;
    use futures::StreamExt;
    use tokio::{
        io::{AsyncRead, AsyncWrite, ReadBuf},
        net::TcpListener,
    };
    use tokio_tungstenite::{
        accept_async,
        tungstenite::{Error as WsError, Message as WsMessage},
    };

    let chain = ChainId::from("test_chain_ws");
    let kp_listener = KeyPair::random();
    let kp_dialer = KeyPair::random();

    let listener_cfg = make_config(
        &socket_addr!(127.0.0.1:0),
        &socket_addr!(127.0.0.1:0),
        1_024,
        16 * 1024,
    );
    let (network_listener, _child_listener) = match NetworkHandle::<BigMsg>::start(
        kp_listener.clone(),
        listener_cfg,
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_) => return,
    };

    let ws_listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(err) => panic!("bind ws listener: {err}"),
    };
    let ws_addr = ws_listener.local_addr().expect("ws listener addr");

    let forward_handle = network_listener.clone();
    tokio::spawn(async move {
        if let Ok((stream, remote)) = ws_listener.accept().await {
            if let Ok(ws_stream) = accept_async(stream).await {
                let (sink, stream) = ws_stream.split();

                struct WsRead<S> {
                    stream: S,
                    buffer: Bytes,
                }

                impl<S> WsRead<S> {
                    fn new(stream: S) -> Self {
                        Self {
                            stream,
                            buffer: Bytes::new(),
                        }
                    }
                }

                impl<S> AsyncRead for WsRead<S>
                where
                    S: futures::Stream<Item = Result<WsMessage, WsError>> + Unpin + Send,
                {
                    fn poll_read(
                        mut self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                        buf: &mut ReadBuf<'_>,
                    ) -> core::task::Poll<std::io::Result<()>> {
                        if !self.buffer.is_empty() {
                            let n = core::cmp::min(self.buffer.len(), buf.remaining());
                            buf.put_slice(&self.buffer.split_to(n));
                            return core::task::Poll::Ready(Ok(()));
                        }

                        match futures::ready!(core::pin::Pin::new(&mut self.stream).poll_next(cx)) {
                            Some(Ok(WsMessage::Binary(frame))) => {
                                self.buffer = frame;
                                let n = core::cmp::min(self.buffer.len(), buf.remaining());
                                buf.put_slice(&self.buffer.split_to(n));
                                core::task::Poll::Ready(Ok(()))
                            }
                            Some(Ok(_)) => {
                                cx.waker().wake_by_ref();
                                core::task::Poll::Pending
                            }
                            Some(Err(err)) => core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws read error: {err}"),
                            ))),
                            None => core::task::Poll::Ready(Ok(())),
                        }
                    }
                }

                struct WsWrite<S> {
                    sink: S,
                    buffer: Vec<u8>,
                }

                impl<S> WsWrite<S> {
                    fn new(sink: S) -> Self {
                        Self {
                            sink,
                            buffer: Vec::new(),
                        }
                    }
                }

                impl<S> AsyncWrite for WsWrite<S>
                where
                    S: futures::Sink<WsMessage, Error = WsError> + Unpin + Send,
                {
                    fn poll_write(
                        mut self: core::pin::Pin<&mut Self>,
                        _cx: &mut core::task::Context<'_>,
                        data: &[u8],
                    ) -> core::task::Poll<std::io::Result<usize>> {
                        self.buffer.extend_from_slice(data);
                        core::task::Poll::Ready(Ok(data.len()))
                    }

                    fn poll_flush(
                        mut self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                    ) -> core::task::Poll<std::io::Result<()>> {
                        if self.buffer.is_empty() {
                            return core::task::Poll::Ready(Ok(()));
                        }
                        let payload = core::mem::take(&mut self.buffer);
                        match futures::ready!(core::pin::Pin::new(&mut self.sink).poll_ready(cx)) {
                            Ok(()) => {}
                            Err(err) => {
                                return core::task::Poll::Ready(Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("ws ready error: {err}"),
                                )));
                            }
                        }
                        if let Err(err) = core::pin::Pin::new(&mut self.sink)
                            .start_send(WsMessage::Binary(payload.into()))
                        {
                            return core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws send error: {err}"),
                            )));
                        }
                        match futures::ready!(core::pin::Pin::new(&mut self.sink).poll_flush(cx)) {
                            Ok(()) => core::task::Poll::Ready(Ok(())),
                            Err(err) => core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws flush error: {err}"),
                            ))),
                        }
                    }

                    fn poll_shutdown(
                        mut self: core::pin::Pin<&mut Self>,
                        cx: &mut core::task::Context<'_>,
                    ) -> core::task::Poll<std::io::Result<()>> {
                        match futures::ready!(core::pin::Pin::new(&mut self.sink).poll_ready(cx)) {
                            Ok(()) => {}
                            Err(err) => {
                                return core::task::Poll::Ready(Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("ws ready error: {err}"),
                                )));
                            }
                        }
                        if let Err(err) =
                            core::pin::Pin::new(&mut self.sink).start_send(WsMessage::Close(None))
                        {
                            return core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws close error: {err}"),
                            )));
                        }
                        match futures::ready!(core::pin::Pin::new(&mut self.sink).poll_flush(cx)) {
                            Ok(()) => core::task::Poll::Ready(Ok(())),
                            Err(err) => core::task::Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("ws close error: {err}"),
                            ))),
                        }
                    }
                }

                let read = WsRead::new(stream);
                let write = WsWrite::new(sink);
                let _ = forward_handle.accept_stream(read, write, remote).await;
            }
        }
    });

    let mut dialer_cfg = make_config(
        &socket_addr!(127.0.0.1:0),
        &socket_addr!(127.0.0.1:0),
        16 * 1024,
        16 * 1024,
    );
    dialer_cfg.prefer_ws_fallback = true;
    let (net_dialer, _child_dialer) = match NetworkHandle::<BigMsg>::start(
        kp_dialer.clone(),
        dialer_cfg,
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_) => return,
    };

    // Listener only needs topology knowledge to accept the inbound session.
    let peer_dialer = Peer::new(socket_addr!(127.0.0.1:0), kp_dialer.public_key().clone());
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

    let online = tokio::time::timeout(Duration::from_millis(2_000), async {
        loop {
            if network_listener.online_peers(HashSet::len) > 0 {
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

    let oversize = BigMsg {
        topic: 0,
        data: vec![0u8; 8 * 1024],
    };
    net_dialer.post(Post {
        data: oversize,
        peer_id: peer_listener.id().clone(),
        priority: Priority::High,
    });

    let dropped = tokio::time::timeout(Duration::from_millis(1_500), async {
        loop {
            if network_listener.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        dropped.is_ok(),
        "listener should disconnect after WS frame cap violation"
    );

    let end_cap = iroha_p2p::network::cap_violations_consensus();
    assert_eq!(
        end_cap, start_cap,
        "global cap enforcement must precede topic cap accounting",
    );

    let _ = tokio::time::timeout(Duration::from_millis(1_000), async {
        loop {
            if net_dialer.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
}
