//! Exact topic and negotiated frame-cap enforcement on real P2P transports.
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_p2p::tests::integration::p2p_caps::BigMsg")]
#[derive(Clone, Decode, Encode)]
struct BigMsg {
    topic: u8,
    data: Vec<u8>,
}
// Diagnostics must not print multi-megabyte synthetic payloads on admission failure.
impl core::fmt::Debug for BigMsg {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BigMsg")
            .field("topic", &self.topic)
            .field("data_bytes", &self.data.len())
            .finish()
    }
}
impl ClassifyTopic for BigMsg {
    fn progress_reconstruction(&self) -> ProgressReconstruction {
        // The cap probes retain an identical payload through the observation;
        // replay has no stateful effect in this synthetic receiver.
        ProgressReconstruction::Retransmit
    }
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
        use norito::core;
        core::validate_header_flags(flags)?;
        let _flags = core::DecodeFlagsGuard::enter(flags);
        // Inspect the declared two-field layout without allocating the blob.
        let (topic, vector) = if flags & core::header_flags::PACKED_STRUCT == 0 {
            let (first, prefix) = core::read_len_from_slice_with_flags(payload, flags)?;
            if first != 1 {
                return Err(core::Error::LengthMismatch);
            }
            let topic = *payload.get(prefix).ok_or(core::Error::LengthMismatch)?;
            let rest = payload
                .get(prefix.checked_add(1).ok_or(core::Error::LengthMismatch)?..)
                .ok_or(core::Error::LengthMismatch)?;
            let (second, prefix) = core::read_len_from_slice_with_flags(rest, flags)?;
            if prefix.checked_add(second) != Some(rest.len()) {
                return Err(core::Error::LengthMismatch);
            }
            (topic, &rest[prefix..])
        } else if flags & core::header_flags::FIELD_BITSET == 0 {
            let (offsets, data) = payload
                .split_at_checked(24)
                .ok_or(core::Error::LengthMismatch)?;
            let offset = |n: usize| -> Result<usize, core::Error> {
                usize::try_from(u64::from_le_bytes(
                    offsets[n * 8..(n + 1) * 8]
                        .try_into()
                        .map_err(|_| core::Error::LengthMismatch)?,
                ))
                .map_err(|_| core::Error::LengthMismatch)
            };
            if offset(0)? != 0 || offset(1)? != 1 || offset(2)? != data.len() {
                return Err(core::Error::LengthMismatch);
            }
            let (&topic, vector) = data.split_first().ok_or(core::Error::LengthMismatch)?;
            (topic, vector)
        } else {
            let (&bitset, data) = payload.split_first().ok_or(core::Error::LengthMismatch)?;
            // u8 is fixed-width and Vec<u8> owns its sequence count. Neither
            // field has a hybrid size header in the canonical derive layout.
            if bitset != 0 {
                return Err(core::Error::LengthMismatch);
            }
            let (&topic, vector) = data.split_first().ok_or(core::Error::LengthMismatch)?;
            (topic, vector)
        };
        // Vec<u8> always encodes a fixed-u64 count followed by raw bytes.
        // Check that inner extent as well as the enclosing field/table extent.
        let (count, prefix) = core::inspect_seq_len_slice(vector)?;
        if prefix.checked_add(count) != Some(vector.len()) {
            return Err(core::Error::LengthMismatch);
        }
        Ok(Some(match topic {
            0 => Topic::Consensus,
            1 => Topic::Control,
            2 => Topic::BlockSync,
            3 => Topic::TxGossip,
            4 => Topic::PeerGossip,
            5 => Topic::Health,
            6 => Topic::Connect,
            _ => Topic::Other,
        }))
    }

    fn topic(&self) -> Topic {
        match self.topic {
            0 => Topic::Consensus,
            1 => Topic::Control,
            2 => Topic::BlockSync,
            3 => Topic::TxGossip,
            4 => Topic::PeerGossip,
            5 => Topic::Health,
            6 => Topic::Connect,
            _ => Topic::Other,
        }
    }
    fn subscriber_route(&self) -> SubscriberRoute {
        if self.topic == 6 {
            SubscriberRoute::Connect
        } else {
            SubscriberRoute::General
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
        max_frame_bytes_connect: topic_cap,
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
fn asymmetric_config(addr: &SocketAddr, public: &SocketAddr, plaintext: usize) -> Config {
    let mut cfg = make_config(
        addr,
        public,
        plaintext + iroha_config::parameters::defaults::network::DEFAULT_AEAD_FRAME_OVERHEAD_BYTES,
        1024,
    );
    cfg.max_total_connections = NonZeroUsize::new(4);
    cfg.max_frame_bytes_consensus = plaintext;
    cfg.max_frame_bytes_block_sync = plaintext;
    cfg
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn execution_transport_carries_large_connect_and_gossip_frames_without_widening_health() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;
    let network_id = super::test_network_id("execution_transport_frames");
    let key1 = super::random_node_key_pair();
    let key2 = super::random_node_key_pair();
    let addr1 = super::next_addr();
    let addr2 = super::next_addr();
    let shutdown1 = ShutdownSignal::new();
    let shutdown2 = ShutdownSignal::new();
    let config = |addr: &SocketAddr| {
        let mut cfg = make_config(addr, addr, 17 * 1024 * 1024, 32_768);
        cfg.max_total_connections = NonZeroUsize::new(4);
        cfg.max_frame_bytes_block_sync = 16 * 1024 * 1024;
        cfg.max_frame_bytes_connect = 8 * 1024 * 1024;
        cfg.max_frame_bytes_tx_gossip = 8 * 1024 * 1024;
        cfg
    };
    let (receiver, _receiver_child) = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(key1.clone()),
        config(&addr1),
        network_id,
        None,
        None,
        shutdown1.clone(),
    )
    .await
    .expect("execution receiver must start; this test does not silently skip startup failures");
    let (sender, _sender_child) = NetworkHandle::<BigMsg>::start(
        super::p2p_identity_keys(key2.clone()),
        config(&addr2),
        network_id,
        None,
        None,
        shutdown2.clone(),
    )
    .await
    .expect("execution sender must start");
    let peer1 = Peer::new(addr1.clone(), key1.public_key().clone());
    let peer2 = Peer::new(addr2.clone(), key2.public_key().clone());
    receiver.update_topology(UpdateTopology(HashSet::from([peer2.id().clone()])));
    sender.update_topology(UpdateTopology(HashSet::from([peer1.id().clone()])));
    sender.update_peers_addresses(UpdatePeers(vec![(peer1.id().clone(), addr1)]));
    assert!(
        wait_for_both_online(
            &receiver,
            &sender,
            Duration::from_secs(10),
            Duration::from_millis(25)
        )
        .await,
        "both actual authenticated P2P sessions must become online"
    );
    let (connect_tx, mut connect_rx) = tokio::sync::mpsc::channel(2);
    let (gossip_tx, mut gossip_rx) = tokio::sync::mpsc::channel(2);
    receiver
        .subscribe_to_peers_messages_with_filter(
            connect_tx,
            iroha_p2p::network::SubscriberFilter::topics_for_route(
                [Topic::Connect],
                SubscriberRoute::Connect,
            ),
        )
        .expect("dedicated Connect subscriber");
    receiver
        .subscribe_to_peers_messages_with_filter(
            gossip_tx,
            iroha_p2p::network::SubscriberFilter::topics_for_route(
                [Topic::TxGossip],
                SubscriberRoute::General,
            ),
        )
        .expect("ordinary gossip subscriber");
    for (topic, inbox) in [(6, &mut connect_rx), (3, &mut gossip_rx)] {
        let data = vec![topic; 4 * 1024 * 1024];
        let post = Post {
            data: BigMsg {
                topic,
                data: data.clone(),
            },
            peer_id: peer1.id().clone(),
            priority: Priority::Low,
        };
        // Connect and TxGossip are best effort. The reliable API must return
        // their exact owner, rather than pretending they entered that queue.
        let post = match sender.post_recoverable(post, None) {
            Err(NetworkActorAdmissionError::Rejected {
                message,
                reason: NetworkActorAdmissionRejection::NotReliableProgress,
            }) => message,
            other => {
                panic!("best-effort frame must retain its owner at the reliable API: {other:?}")
            }
        };
        assert_eq!(post.data.topic, topic);
        assert!(
            post.data.data == data,
            "reliable rejection must return the exact complete blob"
        );
        assert_eq!(post.peer_id, *peer1.id());
        assert_eq!(post.priority, Priority::Low);
        sender.post(post);
        let received = tokio::time::timeout(Duration::from_secs(10), inbox.recv())
            .await
            .expect("large frame crosses authenticated P2P transport")
            .expect("subscriber receives complete frame");
        assert_eq!(received.payload.topic, topic);
        assert!(
            received.payload.data == data,
            "subscriber receives the exact complete blob"
        );
    }
    assert_eq!(sender.outbound_topic_frame_cap(Topic::Health), 32_768);
    let health_cap_before = iroha_p2p::network::cap_violations_health();
    sender.post(Post {
        data: BigMsg {
            topic: 5,
            data: vec![0; 64 * 1024],
        },
        peer_id: peer1.id().clone(),
        priority: Priority::Low,
    });
    assert!(
        iroha_p2p::network::cap_violations_health() > health_cap_before,
        "best-effort Health must fail exact actor-byte admission before enqueue"
    );
    shutdown1.send();
    shutdown2.send();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn topic_cap_violation_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;
    let chain = super::test_network_id("test_chain");
    let kp1 = super::random_node_key_pair();
    let kp2 = super::random_node_key_pair();
    let a1 = super::next_addr();
    let a2 = super::next_addr();
    // Keep the exact 1 KiB Consensus cap under test. The independent body
    // cap funds mandatory control/private reservations before connections.
    let cfg = |addr: SocketAddr| {
        let mut cfg = make_config(
            &addr,
            &addr,
            1024 * 1024
                + iroha_config::parameters::defaults::network::DEFAULT_AEAD_FRAME_OVERHEAD_BYTES,
            1024,
        );
        cfg.max_total_connections = NonZeroUsize::new(4);
        cfg.max_frame_bytes_block_sync = 1024 * 1024;
        cfg
    };
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
        Err(error) => panic!("explicit cap fixture must start: {error}"),
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
        Err(error) => panic!("explicit cap fixture must start: {error}"),
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
        panic!("both cap-fixture peers must authenticate before admission assertion");
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
    // Both geometries are valid. The mandatory authenticated offer binds the
    // smaller directional maximum before any data can be written.
    let listener_cfg = asymmetric_config(&listen_addr, &listen_addr, 128 * 1024);
    let dialer_cfg = asymmetric_config(&dialer_addr, &dialer_addr, 512 * 1024);
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
        Err(error) => panic!("explicit cap fixture must start: {error}"),
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
        Err(error) => panic!("explicit cap fixture must start: {error}"),
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
    // Require the direct TCP connection before exercising negotiated admission.
    let online = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if net_listener.online_peers(HashSet::len) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        online.is_ok(),
        "valid asymmetric peers must authenticate before the refusal control"
    );
    let start_cap = iroha_p2p::network::cap_violations_consensus();
    // Local actor admission fits, but mandatory peer-writer admission must
    // refuse this frame against the authenticated smaller remote maximum.
    let oversize = BigMsg {
        topic: 0,
        data: vec![0u8; 256 * 1024],
    };
    net_dialer
        .post_recoverable(
            Post {
                data: oversize.clone(),
                peer_id: peer_listener.id().clone(),
                priority: Priority::High,
            },
            None,
        )
        .expect("local actor must admit the frame before negotiated writer refusal");
    // The writer fences the refused post; no oversized data reaches the
    // remote decoder. The malformed-record unit controls cover receiver caps.
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
        "negotiated oversize must close the exact refused writer tenure"
    );
    let end_cap = iroha_p2p::network::cap_violations_consensus();
    assert_eq!(
        end_cap, start_cap,
        "negotiated writer refusal must precede remote topic decode/accounting",
    );
    // Dialer should eventually observe the connection closure as well.
    let dialer_closed = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_dialer.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        dialer_closed.is_ok(),
        "dialer must observe negotiated refusal"
    );
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
    let listener_cfg = asymmetric_config(&tls_listen, &public_host, 128 * 1024);
    let client_addr = super::next_addr();
    let dialer_cfg = asymmetric_config(&client_addr, &client_addr, 512 * 1024);
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
        Err(error) => panic!("explicit cap fixture must start: {error}"),
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
        Err(error) => panic!("explicit cap fixture must start: {error}"),
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
    // Require the authenticated connection before exercising negotiated admission.
    let online = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if net_listener.online_peers(HashSet::len) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        online.is_ok(),
        "valid asymmetric peers must authenticate before the refusal control"
    );
    let start_cap = iroha_p2p::network::cap_violations_consensus();
    // Send a payload exceeding listener's global frame cap but within the dialer's cap.
    let oversize = BigMsg {
        topic: 0,
        data: vec![0u8; 256 * 1024],
    };
    net_dialer
        .post_recoverable(
            Post {
                data: oversize.clone(),
                peer_id: peer_listener.id().clone(),
                priority: Priority::High,
            },
            None,
        )
        .expect("local actor must admit the frame before negotiated writer refusal");
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
    let dialer_closed = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            if net_dialer.online_peers(HashSet::len) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        dialer_closed.is_ok(),
        "dialer must observe negotiated refusal"
    );
}
#[cfg(feature = "quic")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn quic_global_frame_cap_disconnects() {
    let _cap_test_guard = FRAME_CAP_TEST_LOCK.lock().await;
    let chain = super::test_network_id("test_chain_quic");
    let start_cap = iroha_p2p::network::cap_violations_consensus();
    for plaintext in [128 * 1024, 512 * 1024] {
        let address = super::next_addr();
        let mut config = asymmetric_config(&address, &address, plaintext);
        config.quic_enabled = true;
        let result = NetworkHandle::<BigMsg>::start(
            super::p2p_identity_keys(super::random_node_key_pair()),
            config,
            chain,
            None,
            None,
            ShutdownSignal::new(),
        )
        .await;
        assert!(
            matches!(result, Err(iroha_p2p::Error::Io(ref error))
            if error.kind() == std::io::ErrorKind::InvalidInput && error.to_string().contains("network.quic_enabled=true")),
            "shipping policy must refuse QUIC before creating a connection or decoding data"
        );
    }
    assert_eq!(
        iroha_p2p::network::cap_violations_consensus(),
        start_cap,
        "unavailable transport must not manufacture a topic-cap observation"
    );
}

#[test]
fn cap_fixture_raw_topic_matches_canonical_layout_without_decoding_the_blob() {
    use norito::core;
    for topic in 0..=8 {
        for length in [0, 1, 256] {
            let value = BigMsg {
                topic,
                data: vec![7; length],
            };
            assert!(
                format!("{value:?}").len() < 80,
                "fixture diagnostics must remain bounded"
            );
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
                let (decoded, used) = core::decode_field_canonical::<BigMsg>(&bytes).unwrap();
                assert_eq!(used, bytes.len());
                assert_eq!(decoded.topic, value.topic);
                assert_eq!(decoded.data, value.data);
                assert_eq!(
                    BigMsg::inbound_topic(&bytes, flags).unwrap(),
                    Some(decoded.topic())
                );
                assert_eq!(
                    BigMsg::inbound_admission_class(&bytes, flags).unwrap(),
                    decoded.admission_class()
                );
                let mut trailing = bytes.clone();
                trailing.push(0);
                assert!(BigMsg::inbound_topic(&trailing, flags).is_err());
                assert!(BigMsg::inbound_topic(&bytes[..bytes.len() - 1], flags).is_err());
                assert!(BigMsg::inbound_topic(&bytes, flags | 0x80).is_err());
                // Keep the outer field/table untouched, corrupt only Vec's own count.
                let count_at = bytes.len() - length - 8;
                for count in [u64::try_from(length).unwrap() + 1, u64::MAX] {
                    let mut wrong_count = bytes.clone();
                    wrong_count[count_at..count_at + 8].copy_from_slice(&count.to_le_bytes());
                    assert!(BigMsg::inbound_topic(&wrong_count, flags).is_err());
                }
                if flags & core::header_flags::FIELD_BITSET != 0 {
                    let mut wrong_bitset = bytes.clone();
                    wrong_bitset[0] = 0b10;
                    assert!(BigMsg::inbound_topic(&wrong_bitset, flags).is_err());
                }
            }
        }
    }
}
