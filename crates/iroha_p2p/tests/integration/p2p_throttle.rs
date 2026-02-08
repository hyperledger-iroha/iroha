//! Low-priority throttling tests for P2P.

use std::collections::HashSet;

use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake,
};
use iroha_config_base::WithOrigin;
use iroha_crypto::KeyPair;
use iroha_data_model::{ChainId, prelude::Peer};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{NetworkHandle, network::message::*};
use iroha_primitives::addr::socket_addr;
use norito::codec::{Decode, Encode};
use tokio::time::Duration;

#[derive(Clone, Debug, Decode, Encode)]
struct LoMsg(u32);

impl iroha_p2p::network::message::ClassifyTopic for LoMsg {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Other
    }
}

fn cfg(addr: iroha_primitives::addr::SocketAddr, rate: Option<u32>, burst: Option<u32>) -> Config {
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
        p2p_queue_cap_high: core::num::NonZeroUsize::new(256).unwrap(),
        p2p_queue_cap_low: core::num::NonZeroUsize::new(256).unwrap(),
        p2p_post_queue_cap: core::num::NonZeroUsize::new(128).unwrap(),
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
        low_priority_rate_per_sec: rate.and_then(core::num::NonZeroU32::new),
        low_priority_burst: burst.and_then(core::num::NonZeroU32::new),
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
async fn low_priority_posts_are_throttled() {
    let chain = ChainId::from("test_chain");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let a1 = socket_addr!(127.0.0.1:12_036);
    let a2 = socket_addr!(127.0.0.1:12_037);

    // Enable per-peer low-priority token bucket with 1 msg/sec, burst 1
    let started1 = NetworkHandle::<LoMsg>::start(
        kp1.clone(),
        cfg(a1.clone(), Some(1), Some(1)),
        Some(chain.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await;
    let (net1, _c1) = match started1 {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let started2 = NetworkHandle::<LoMsg>::start(
        kp2.clone(),
        cfg(a2.clone(), None, None),
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
    let p1 = Peer::new(a1.clone(), kp1.public_key().clone());
    let p2 = Peer::new(a2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), a2.clone())]));
    net2.update_topology(UpdateTopology([p1.id().clone()].into_iter().collect()));
    net2.update_peers_addresses(UpdatePeers(vec![(p1.id().clone(), a1.clone())]));

    // Wait for connect
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

    // Snapshot counter
    let before = iroha_p2p::network::low_post_throttled_count();

    // Fire a burst of low-priority posts
    for i in 0..20u32 {
        net1.post(Post {
            data: LoMsg(i),
            peer_id: p2.id().clone(),
            priority: Priority::Low,
        });
    }

    // Give time for processing and throttling to be observed
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = iroha_p2p::network::low_post_throttled_count();

    // Expect at least some throttling (first allowed, rest likely throttled within window)
    assert!(after > before, "expected throttling to increment counter");
}
