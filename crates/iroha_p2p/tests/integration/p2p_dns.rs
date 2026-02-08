//! DNS refresh metric test.

use std::collections::HashSet;

use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake,
};
use iroha_config_base::WithOrigin;
use iroha_crypto::KeyPair;
use iroha_data_model::{ChainId, prelude::Peer};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::NetworkHandle;
use iroha_primitives::addr::{SocketAddr as IrohaSocketAddr, SocketAddrHost, socket_addr};
use tokio::time::Duration;

#[derive(Clone, Debug, norito::codec::Decode, norito::codec::Encode)]
struct TestMsg;

impl iroha_p2p::network::message::ClassifyTopic for TestMsg {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Other
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn interval_dns_refresh_increments_counter() {
    let chain = ChainId::from("test_chain");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();

    // Bind real TCP listeners on ephemeral ports; advertise via Host("localhost", port)
    let l1 = socket_addr!(127.0.0.1:0);
    let l2 = socket_addr!(127.0.0.1:0);
    let port1 = match std::net::TcpListener::bind(l1.to_string()) {
        Ok(s) => s.local_addr().unwrap().port(),
        Err(_e) => return,
    };
    let port2 = match std::net::TcpListener::bind(l2.to_string()) {
        Ok(s) => s.local_addr().unwrap().port(),
        Err(_e) => return,
    };
    drop((port1, port2)); // just probing ports

    let host1 = IrohaSocketAddr::Host(SocketAddrHost {
        host: "localhost".into(),
        port: port1,
    });
    let host2 = IrohaSocketAddr::Host(SocketAddrHost {
        host: "localhost".into(),
        port: port2,
    });

    let cfg1 = Config {
        address: WithOrigin::inline(socket_addr!(127.0.0.1: {port1})),
        public_address: WithOrigin::inline(host1.clone()),
        soranet_handshake: ActualSoranetHandshake::default(),
        idle_timeout: Duration::from_millis(1500),
        prefer_ws_fallback: false,
        happy_eyeballs_stagger: Duration::from_millis(50),
        addr_ipv6_first: false,
        dns_refresh_interval: Some(Duration::from_millis(100)),
        dns_refresh_ttl: None,
        quic_enabled: false,
        quic_datagrams_enabled: iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
        quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
        quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
        quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
        tls_enabled: false,
        tls_listen_address: None,
        p2p_queue_cap_high: core::num::NonZeroUsize::new(128).unwrap(),
        p2p_queue_cap_low: core::num::NonZeroUsize::new(128).unwrap(),
        p2p_post_queue_cap: core::num::NonZeroUsize::new(64).unwrap(),
        p2p_subscriber_queue_cap: iroha_config::parameters::defaults::network::P2P_SUBSCRIBER_QUEUE_CAP,
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
    };
    let cfg2 = Config {
        public_address: WithOrigin::inline(host2.clone()),
        address: WithOrigin::inline(socket_addr!(127.0.0.1: {port2})),
        ..cfg1.clone()
    };

    let started1 = NetworkHandle::<TestMsg>::start(
        kp1.clone(),
        cfg1,
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net1, _c1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let started2 = NetworkHandle::<TestMsg>::start(
        kp2.clone(),
        cfg2,
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _c2) = match started2 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    // Connect
    let p1 = Peer::new(host1.clone(), kp1.public_key().clone());
    let p2 = Peer::new(host2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), host2.clone())]));
    net2.update_topology(UpdateTopology([p1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), host1.clone())]));

    // Wait for initial connect
    let _ = tokio::time::timeout(Duration::from_millis(1500), async {
        let mut n = net1
            .wait_online_peers_update(HashSet::len)
            .await
            .expect("online peers channel closed");
        while n < 1 {
            n = net1
                .wait_online_peers_update(HashSet::len)
                .await
                .expect("online peers channel closed");
        }
    })
    .await;

    let before = iroha_p2p::network::dns_refresh_count();
    tokio::time::sleep(Duration::from_millis(350)).await;
    let after = iroha_p2p::network::dns_refresh_count();

    assert!(
        after > before,
        "expected interval-based DNS refresh to increment counter"
    );
}
