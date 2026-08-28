//! Topic cap enforcement tests. Skips gracefully if sockets are unavailable.
use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake,
};
use iroha_config::parameters::defaults::network::TRUST_GOSSIP;
use iroha_data_model::prelude::Peer;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{
    NetworkHandle,
    network::{NetworkActorAdmissionError, NetworkActorAdmissionRejection, message::*},
};
use iroha_primitives::addr::SocketAddrHost;
use iroha_primitives::addr::{SocketAddr, socket_addr};
use norito::codec::{Decode, Encode};
use std::{collections::HashSet, num::NonZeroUsize};
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
fn default_soranet_handshake() -> ActualSoranetHandshake {
    // Keep admission inexpensive so these tests continue to measure frame-cap
    // behavior. `test_network_config` isolates replay state.
    super::low_cost_test_soranet_handshake()
}
fn make_config(
    addr: &SocketAddr,
    public: &SocketAddr,
    max_frame_bytes: usize,
    topic_cap: usize,
) -> Config {
    Config {
        happy_eyeballs_stagger: Duration::from_millis(50),
        p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
        p2p_queue_cap_low: NonZeroUsize::new(128).unwrap(),
        p2p_post_queue_cap: NonZeroUsize::new(128).unwrap(),
        max_frame_bytes,
        tcp_keepalive: Some(Duration::from_secs(60)),
        max_frame_bytes_consensus: topic_cap,
        max_frame_bytes_control: topic_cap,
        max_frame_bytes_block_sync: topic_cap,
        max_frame_bytes_tx_gossip: topic_cap,
        max_frame_bytes_peer_gossip: topic_cap,
        max_frame_bytes_health: topic_cap,
        max_frame_bytes_other: topic_cap,
        ..super::test_network_config(
            addr.clone(),
            public.clone(),
            Duration::from_millis(2000),
            default_soranet_handshake(),
            TRUST_GOSSIP,
        )
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
        chain,
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
        chain,
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
        chain,
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
        chain,
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
    let listener_cfg = make_config(&tls_listen, &public_host, 1024, 4096);
    let client_addr = super::next_addr();
    let dialer_cfg = make_config(&client_addr, &client_addr, 16 * 1024, 16 * 1024);
    let started_listener = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(kp_listener.clone()),
        listener_cfg,
        chain,
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
        chain,
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
        chain,
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
        chain,
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
