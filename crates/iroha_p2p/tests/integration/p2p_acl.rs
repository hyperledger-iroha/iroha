//! ACL and admission tests for P2P networking.

use std::collections::HashSet;

use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake,
};
use iroha_config_base::WithOrigin;
use iroha_crypto::KeyPair;
use iroha_data_model::{ChainId, prelude::Peer};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{NetworkHandle, network::message::*};
use iroha_primitives::addr::{SocketAddr as IrohaSocketAddr, SocketAddrHost, socket_addr};
use norito::codec::{Decode, Encode};
use tokio::time::Duration;

#[derive(Clone, Debug, Decode, Encode)]
struct TestMessage(&'static str);

impl iroha_p2p::network::message::ClassifyTopic for TestMessage {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Other
    }
}

fn base_cfg(addr: IrohaSocketAddr) -> Config {
    Config {
        address: WithOrigin::inline(addr.clone()),
        public_address: WithOrigin::inline(addr),
        soranet_handshake: ActualSoranetHandshake::default(),
        idle_timeout: Duration::from_millis(1500),
        prefer_ws_fallback: false,
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
        tls_listen_address: None,
        p2p_queue_cap_high: core::num::NonZeroUsize::new(128).unwrap(),
        p2p_queue_cap_low: core::num::NonZeroUsize::new(256).unwrap(),
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
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deny_key_blocks_connection() {
    let idle = Duration::from_millis(1500);
    let chain_id = ChainId::from("test_chain");

    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:12_030);
    let addr2 = socket_addr!(127.0.0.1:12_031);

    // Start network1 with denylist containing kp2
    let mut cfg1 = base_cfg(addr1.clone());
    cfg1.idle_timeout = idle;
    cfg1.deny_keys = vec![kp2.public_key().clone()];
    let started1 = NetworkHandle::<TestMessage>::start(
        kp1.clone(),
        cfg1,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net1, _child1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    }; // skip if sandbox forbids sockets

    // Start network2 default
    let mut cfg2 = base_cfg(addr2.clone());
    cfg2.idle_timeout = idle;
    let started2 = NetworkHandle::<TestMessage>::start(
        kp2.clone(),
        cfg2,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _child2) = match started2 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    // Wire topology
    let p1 = Peer::new(addr1.clone(), kp1.public_key().clone());
    let p2 = Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));
    net2.update_topology(UpdateTopology([p1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), addr1.clone())]));

    // Give time for attempted connect and ACL drop; ensure net1 never shows p2 online
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(net1.online_peers(HashSet::len), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn allowlist_only_permits_only_listed_key() {
    let idle = Duration::from_millis(1500);
    let chain_id = ChainId::from("test_chain");

    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:12_032);
    let addr2 = socket_addr!(127.0.0.1:12_033);

    // Start network1 allowlist_only: only kp2 allowed
    let mut cfg1 = base_cfg(addr1.clone());
    cfg1.idle_timeout = idle;
    cfg1.allowlist_only = true;
    cfg1.allow_keys = vec![kp2.public_key().clone()];
    let started1 = NetworkHandle::<TestMessage>::start(
        kp1.clone(),
        cfg1,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (mut net1, _child1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    // Start network2 default
    let mut cfg2 = base_cfg(addr2.clone());
    cfg2.idle_timeout = idle;
    let started2 = NetworkHandle::<TestMessage>::start(
        kp2.clone(),
        cfg2,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _child2) = match started2 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    // Wire topology
    let p1 = Peer::new(addr1.clone(), kp1.public_key().clone());
    let p2 = Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));
    net2.update_topology(UpdateTopology([p1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), addr1.clone())]));

    // Wait until net1 sees p2
    let _ = tokio::time::timeout(Duration::from_millis(2_000), async {
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
    assert_eq!(net1.online_peers(HashSet::len), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cidr_deny_blocks_inbound() {
    let idle = Duration::from_millis(1500);
    let chain_id = ChainId::from("test_chain");

    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:12_034);
    let addr2 = socket_addr!(127.0.0.1:12_035);

    // network1 denies the entire 127.0.0.0/8 range
    let mut cfg1 = base_cfg(addr1.clone());
    cfg1.idle_timeout = idle;
    cfg1.deny_cidrs = vec!["127.0.0.0/8".to_string()];
    let started1 = NetworkHandle::<TestMessage>::start(
        kp1.clone(),
        cfg1,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net1, _child1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    // network2 default
    let mut cfg2 = base_cfg(addr2.clone());
    cfg2.idle_timeout = idle;
    let started2 = NetworkHandle::<TestMessage>::start(
        kp2.clone(),
        cfg2,
        Some(chain_id.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net2, _child2) = match started2 {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    // Topology: attempt to connect from 127.0.0.1 which should be denied by CIDR
    let p2 = Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));
    // Also advertise back to ensure both directions attempted
    let p1 = Peer::new(addr1.clone(), kp1.public_key().clone());
    net2.update_topology(UpdateTopology([p1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), addr1.clone())]));

    // Give time; CIDR deny should prevent net1 from accepting inbound
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(net1.online_peers(HashSet::len), 0);
}
