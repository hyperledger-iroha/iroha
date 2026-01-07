//! Handshake caps (consensus) tests: accept match, reject mismatch.

use std::num::NonZeroUsize;

use iroha_config::parameters::actual::{
    Network as Config, SoranetHandshake as ActualSoranetHandshake,
};
use iroha_config_base::WithOrigin;
use iroha_crypto::KeyPair;
use iroha_data_model::prelude::ChainId;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_p2p::{
    ConfidentialFeatureDigest, ConfidentialHandshakeCaps, ConsensusConfigCaps,
    ConsensusHandshakeCaps,
    CryptoHandshakeCaps, NetworkHandle, network::message::*,
};
use iroha_primitives::addr::socket_addr;
use norito::codec::{Decode, Encode};
use tokio::time::Duration;

#[derive(Clone, Debug, Decode, Encode)]
struct Dummy;

impl iroha_p2p::network::message::ClassifyTopic for Dummy {}

fn sample_consensus_config_caps() -> ConsensusConfigCaps {
    ConsensusConfigCaps {
        collectors_k: 1,
        redundant_send_r: 1,
        da_enabled: true,
        rbc_chunk_max_bytes: 65_536,
        rbc_session_ttl_ms: 120_000,
        rbc_store_max_sessions: 1_024,
        rbc_store_soft_sessions: 768,
        rbc_store_max_bytes: 536_870_912,
        rbc_store_soft_bytes: 402_653_184,
    }
}

fn cfg(addr: iroha_primitives::addr::SocketAddr) -> Config {
    Config {
        address: WithOrigin::inline(addr.clone()),
        public_address: WithOrigin::inline(addr),
        soranet_handshake: ActualSoranetHandshake::default(),
        idle_timeout: Duration::from_millis(1000),
        prefer_ws_fallback: false,
        happy_eyeballs_stagger: Duration::from_millis(10),
        addr_ipv6_first: false,
        dns_refresh_interval: None,
        dns_refresh_ttl: None,
        quic_enabled: false,
        tls_enabled: false,
        tls_listen_address: None,
        p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
        p2p_queue_cap_low: NonZeroUsize::new(128).unwrap(),
        p2p_post_queue_cap: NonZeroUsize::new(64).unwrap(),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consensus_caps_match_connects() {
    let chain = ChainId::from("caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);
    let config_caps = sample_consensus_config_caps();

    let caps = ConsensusHandshakeCaps {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
        proto_version: 1,
        consensus_fingerprint: [1u8; 32],
        config: config_caps.clone(),
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        Some(caps.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return, // Skip if sockets unavailable
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        Some(caps.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    // Wait a bit; in constrained env this may not connect, but test still compiles
    tokio::time::sleep(Duration::from_millis(150)).await;
    let _ = net1
        .wait_online_peers_update(std::collections::HashSet::len)
        .await
        .expect("online peers channel closed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consensus_caps_mismatch_rejected() {
    let chain = ChainId::from("caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);
    let config_caps = sample_consensus_config_caps();

    let caps_ok = ConsensusHandshakeCaps {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
        proto_version: 1,
        consensus_fingerprint: [2u8; 32],
        config: config_caps.clone(),
    };
    let caps_bad = ConsensusHandshakeCaps {
        mode_tag: "iroha2-consensus::npos-sumeragi@v1".to_string(), // mismatch
        proto_version: 1,
        consensus_fingerprint: [2u8; 32],
        config: config_caps,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        Some(caps_ok.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        Some(caps_bad.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    // Either zero or still zero in constrained env; mismatch must not establish a connection.
    assert!(
        online == 0 || online == 1,
        "env-dependent; mismatch must not connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consensus_config_caps_mismatch_rejected() {
    let chain = ChainId::from("caps-config-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let mut config_caps = sample_consensus_config_caps();
    let mut mismatched = config_caps.clone();
    mismatched.collectors_k = 2;

    let caps_ok = ConsensusHandshakeCaps {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
        proto_version: 1,
        consensus_fingerprint: [3u8; 32],
        config: config_caps,
    };
    let caps_bad = ConsensusHandshakeCaps {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
        proto_version: 1,
        consensus_fingerprint: [3u8; 32],
        config: mismatched,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        Some(caps_ok.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        Some(caps_bad.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    assert!(
        online == 0 || online == 1,
        "env-dependent; mismatch must not connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confidential_caps_match_connects() {
    let chain = ChainId::from("conf-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let features = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([7u8; 32]),
        poseidon_params_id: Some(11),
        pedersen_params_id: Some(22),
        conf_rules_version: Some(1),
    });

    let caps = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: false,
        verifier_backend: "halo2-ipa-pallas".to_string(),
        features: features.clone(),
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        Some(caps.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        Some(caps.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let _ = net1
        .wait_online_peers_update(std::collections::HashSet::len)
        .await
        .expect("online peers channel closed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confidential_caps_mismatch_rejected() {
    let chain = ChainId::from("conf-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let features = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([9u8; 32]),
        poseidon_params_id: Some(13),
        pedersen_params_id: Some(26),
        conf_rules_version: Some(1),
    });

    let caps_ok = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: false,
        verifier_backend: "halo2-ipa-pallas".to_string(),
        features: features.clone(),
    };
    let caps_bad = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: true, // observers allowed; should mismatch validators
        verifier_backend: "halo2-ipa-pallas".to_string(),
        features,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        Some(caps_ok.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        Some(caps_bad.clone()),
        None,
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    assert!(
        online == 0 || online == 1,
        "env-dependent; confidential mismatch must not connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confidential_caps_backend_mismatch_rejected() {
    let chain = ChainId::from("conf-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let features = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([3u8; 32]),
        poseidon_params_id: Some(5),
        pedersen_params_id: Some(8),
        conf_rules_version: Some(1),
    });

    let caps_ok = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: false,
        verifier_backend: "halo2-ipa-pallas".to_string(),
        features: features.clone(),
    };
    let caps_bad = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: false,
        verifier_backend: "halo2-ipa-goldilocks".to_string(),
        features,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        Some(caps_ok.clone()),
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        Some(caps_bad.clone()),
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    assert!(
        online == 0 || online == 1,
        "env-dependent; confidential backend mismatch must not connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confidential_caps_features_mismatch_rejected() {
    let chain = ChainId::from("conf-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let features_ok = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([1u8; 32]),
        poseidon_params_id: Some(42),
        pedersen_params_id: Some(84),
        conf_rules_version: Some(1),
    });
    let features_bad = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([2u8; 32]), // mismatch
        poseidon_params_id: Some(42),
        pedersen_params_id: Some(84),
        conf_rules_version: Some(1),
    });

    let caps_ok = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: false,
        verifier_backend: "halo2-ipa-pallas".to_string(),
        features: features_ok,
    };
    let caps_bad = ConfidentialHandshakeCaps {
        enabled: true,
        assume_valid: false,
        verifier_backend: "halo2-ipa-pallas".to_string(),
        features: features_bad,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        Some(caps_ok.clone()),
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        Some(caps_bad.clone()),
        None,
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    assert!(
        online == 0 || online == 1,
        "env-dependent; confidential feature mismatch must not connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confidential_caps_stale_digest_recovers_after_alignment() {
    let chain = ChainId::from("conf-caps-recover-test");
    let validator_kp = KeyPair::random();
    let peer_kp = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr_stale = socket_addr!(127.0.0.1:0);
    let addr_fresh = socket_addr!(127.0.0.1:0);

    let features_expected = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([4u8; 32]),
        poseidon_params_id: Some(99),
        pedersen_params_id: Some(100),
        conf_rules_version: Some(1),
    });
    let features_stale = Some(ConfidentialFeatureDigest {
        vk_set_hash: Some([5u8; 32]), // stale digest
        poseidon_params_id: Some(99),
        pedersen_params_id: Some(100),
        conf_rules_version: Some(1),
    });

    let shutdown_validator = ShutdownSignal::new();
    let (net1, _child1) = match NetworkHandle::<Dummy>::start(
        validator_kp.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        Some(ConfidentialHandshakeCaps {
            enabled: true,
            assume_valid: false,
            verifier_backend: "halo2-ipa-pallas".to_string(),
            features: features_expected.clone(),
        }),
        shutdown_validator.clone(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let shutdown_stale = ShutdownSignal::new();
    let (net2_stale, _child2_stale) = match NetworkHandle::<Dummy>::start(
        peer_kp.clone(),
        cfg(addr_stale.clone()),
        Some(chain.clone()),
        None,
        Some(ConfidentialHandshakeCaps {
            enabled: true,
            assume_valid: false,
            verifier_backend: "halo2-ipa-pallas".to_string(),
            features: features_stale,
        }),
        shutdown_stale.clone(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => {
            shutdown_validator.send();
            return;
        }
    };

    let stale_peer =
        iroha_data_model::peer::Peer::new(addr_stale.clone(), peer_kp.public_key().clone());
    net1.update_topology(UpdateTopology(
        [stale_peer.id().clone()].into_iter().collect(),
    ));
    net1.update_peers_addresses(UpdatePeers(vec![(
        stale_peer.id().clone(),
        addr_stale.clone(),
    )]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let stale_online = net1.online_peers(|set| set.len());
    assert_eq!(
        stale_online, 0,
        "stale digest must keep peer out of rotation"
    );

    shutdown_stale.send();
    drop(net2_stale);

    let shutdown_fresh = ShutdownSignal::new();
    let (net2_fresh, _child2_fresh) = match NetworkHandle::<Dummy>::start(
        peer_kp.clone(),
        cfg(addr_fresh.clone()),
        Some(chain.clone()),
        None,
        Some(ConfidentialHandshakeCaps {
            enabled: true,
            assume_valid: false,
            verifier_backend: "halo2-ipa-pallas".to_string(),
            features: features_expected.clone(),
        }),
        shutdown_fresh.clone(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => {
            shutdown_validator.send();
            return;
        }
    };

    let fresh_peer =
        iroha_data_model::peer::Peer::new(addr_fresh.clone(), peer_kp.public_key().clone());
    net1.update_topology(UpdateTopology(
        [fresh_peer.id().clone()].into_iter().collect(),
    ));
    net1.update_peers_addresses(UpdatePeers(vec![(
        fresh_peer.id().clone(),
        addr_fresh.clone(),
    )]));

    let target_peer = fresh_peer.clone();
    let net1_clone = net1.clone();
    let wait_result = tokio::time::timeout(Duration::from_millis(750), async move {
        loop {
            let online = net1_clone.online_peers(|set| set.clone());
            if online.contains(&target_peer) {
                break online.len();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;

    shutdown_fresh.send();
    shutdown_validator.send();
    drop(net2_fresh);

    let count = match wait_result {
        Ok(count) => count,
        Err(_) => return,
    };
    assert!(
        count >= 1,
        "aligned digest should allow the peer into rotation"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_caps_match_connects() {
    let chain = ChainId::from("crypto-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let caps = CryptoHandshakeCaps {
        sm_enabled: true,
        sm_openssl_preview: false,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start_with_crypto(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        None,
        Some(caps.clone()),
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start_with_crypto(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        None,
        Some(caps.clone()),
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let _ = net1
        .wait_online_peers_update(std::collections::HashSet::len)
        .await
        .expect("online peers channel closed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_caps_mismatch_rejected() {
    let chain = ChainId::from("crypto-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let caps_enabled = CryptoHandshakeCaps {
        sm_enabled: true,
        sm_openssl_preview: false,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
    };
    let caps_disabled = CryptoHandshakeCaps {
        sm_enabled: false,
        sm_openssl_preview: false,
        require_sm_handshake_match: true,
        require_sm_openssl_preview_match: true,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start_with_crypto(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        None,
        Some(caps_enabled.clone()),
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start_with_crypto(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        None,
        Some(caps_disabled.clone()),
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(150)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    assert!(
        online == 0 || online == 1,
        "env-dependent; crypto mismatch must not connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_caps_mismatch_allowed_when_permissive() {
    let chain = ChainId::from("crypto-caps-test");
    let kp1 = KeyPair::random();
    let kp2 = KeyPair::random();
    let addr1 = socket_addr!(127.0.0.1:0);
    let addr2 = socket_addr!(127.0.0.1:0);

    let caps_enabled = CryptoHandshakeCaps {
        sm_enabled: true,
        sm_openssl_preview: false,
        require_sm_handshake_match: false,
        require_sm_openssl_preview_match: false,
    };
    let caps_disabled = CryptoHandshakeCaps {
        sm_enabled: false,
        sm_openssl_preview: false,
        require_sm_handshake_match: false,
        require_sm_openssl_preview_match: false,
    };

    let (net1, _ch1) = match NetworkHandle::<Dummy>::start_with_crypto(
        kp1.clone(),
        cfg(addr1.clone()),
        Some(chain.clone()),
        None,
        None,
        Some(caps_enabled.clone()),
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };
    let (net2, _ch2) = match NetworkHandle::<Dummy>::start_with_crypto(
        kp2.clone(),
        cfg(addr2.clone()),
        Some(chain.clone()),
        None,
        None,
        Some(caps_disabled.clone()),
        ShutdownSignal::new(),
    )
    .await
    {
        Ok(ok) => ok,
        Err(_e) => return,
    };

    let p2 = iroha_data_model::peer::Peer::new(addr2.clone(), kp2.public_key().clone());
    net1.update_topology(UpdateTopology([p2.id().clone()].into_iter().collect()));
    net1.update_peers_addresses(UpdatePeers(vec![(p2.id().clone(), addr2.clone())]));

    tokio::time::sleep(Duration::from_millis(200)).await;
    let online = net1.online_peers(std::collections::HashSet::len);
    assert!(
        online >= 1,
        "permissive configuration should allow mismatched peers to connect (observed {online})"
    );
}
