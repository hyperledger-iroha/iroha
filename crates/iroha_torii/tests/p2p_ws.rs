//! Torii-level E2E test for the P2P WebSocket fallback route `/p2p`.
//! Requires building with `--features iroha_p2p/p2p_ws`.

#[cfg(feature = "p2p_ws")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p2p_ws_route_accepts_and_handshakes() {
    use iroha_config::parameters::actual::Network as NetCfg;
    use iroha_config_base::WithOrigin;
    use iroha_crypto::KeyPair;
    use iroha_data_model::ChainId;
    use iroha_p2p::{NetworkHandle, network::message::ClassifyTopic};
    use iroha_primitives::addr::socket_addr;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[derive(Clone, Debug, norito::codec::Decode, norito::codec::Encode)]
    struct Dummy;
    impl ClassifyTopic for Dummy {}

    // Start a P2P network that will accept inbound streams
    let kp = KeyPair::random();
    let chain_id = ChainId::from("test-chain");
    let (network, _child) = NetworkHandle::<Dummy>::start(
        kp,
        NetCfg {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            idle_timeout: std::time::Duration::from_millis(5000),
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            tls_enabled: false,
            tls_listen_address: None,
            prefer_ws_fallback: false,
            p2p_queue_cap_high: core::num::NonZeroUsize::new(128).unwrap(),
            p2p_queue_cap_low: core::num::NonZeroUsize::new(128).unwrap(),
            p2p_post_queue_cap: core::num::NonZeroUsize::new(64).unwrap(),
            p2p_subscriber_queue_cap: core::num::NonZeroUsize::new(128).unwrap(),
            happy_eyeballs_stagger: std::time::Duration::from_millis(50),
            addr_ipv6_first: false,
            lane_profile: iroha_config::parameters::actual::LaneProfile::Core,
            max_incoming: None,
            max_total_connections: None,
            accept_rate_per_ip_per_sec: None,
            accept_burst_per_ip: None,
            low_priority_rate_per_sec: None,
            low_priority_burst: None,
            low_priority_bytes_per_sec: None,
            low_priority_bytes_burst: None,
            allowlist_only: false,
            allow_keys: vec![],
            deny_keys: vec![],
            allow_cidrs: vec![],
            deny_cidrs: vec![],
        },
        Some(chain_id),
        None,
        None,
        iroha_futures::supervisor::ShutdownSignal::new(),
    )
    .await
    .expect("start p2p");

    // Minimal WS server that forwards the upgraded socket to Torii handler
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws_addr = listener.local_addr().unwrap();
    let net2 = network.clone();
    tokio::spawn(async move {
        let (stream, remote) = listener.accept().await.unwrap();
        let ws = accept_async(stream).await.unwrap();
        iroha_torii::handle_p2p_ws(ws, Some(net2), remote).await;
    });

    // Dial the server via WS fallback (client-side adapter appends /p2p)
    let endpoint = format!("{}:{}", ws_addr.ip(), ws_addr.port());
    let ws = iroha_p2p::transport::ws::connect_ws(&endpoint)
        .await
        .expect("ws client connect");
    drop(ws); // peer actor owns the stream
}
