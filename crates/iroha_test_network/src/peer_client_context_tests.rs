// Context ownership controls use attached local peers without starting validators.
fn context_owner_fixture() -> (tempfile::TempDir, NetworkPeer) {
    let directory = tempdir().expect("peer context directory");
    let environment = Environment {
        dir: directory.path().to_path_buf(),
    };
    let peer = NetworkPeer::builder().build(&environment);
    peer.client_config
        .set(PeerClientConfig {
            chain: config::chain_id(),
            network_id: NetworkId::from_genesis_hash(
                HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                    CryptoHash::prehashed([0xA5; CryptoHash::LENGTH]),
                ),
            ),
            chain_discriminant: defaults::common::chain_discriminant(),
            api_token: None,
            policy: PeerClientPolicy {
                status_timeout: Duration::from_secs(37),
                request_timeout: Duration::from_secs(3),
                ttl: Duration::from_secs(160),
            },
        })
        .expect("attach fixture exactly once");
    (directory, peer)
}

#[test]
fn peer_context_concurrent_clones_share_one_retained_owner() {
    let (_directory, peer) = context_owner_fixture();
    assert!(peer.retained_client.get().is_none());
    let addresses = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let peer = peer.clone();
                scope.spawn(move || {
                    let client = peer.client();
                    assert_eq!(client.client().account(), &*ALICE_ID);
                    assert_eq!(client.client().operator_key_pair(), Some(&peer.key_pair));
                    assert_eq!(
                        client.client().endpoint().as_str(),
                        format!("{}/", peer.torii_url())
                    );
                    peer.retained_client() as *const Client as usize
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(addresses.iter().all(|address| *address == addresses[0]));
    assert_eq!(
        peer.retained_client() as *const Client as usize,
        addresses[0]
    );
    assert!(!peer.is_running());
}

#[test]
fn peer_context_keeps_attached_policy_despite_later_environment_changes() {
    let _guard = lock_env_guard(&CONFIG_ENV_GUARD);
    let (_directory, peer) = context_owner_fixture();
    let _changed = [
        EnvVarRestore::set("IROHA_TEST_CLIENT_STATUS_TIMEOUT_SECS", "999"),
        EnvVarRestore::set("IROHA_TEST_CLIENT_REQUEST_TIMEOUT_SECS", "888"),
        EnvVarRestore::set("IROHA_TEST_CLIENT_TTL_SECS", "2200"),
    ];
    let alice = peer.client();
    let bob = peer.client_for(
        &BOB_ID,
        iroha_test_samples::BOB_KEYPAIR.private_key().clone(),
    );
    for client in [&alice, &bob] {
        assert_eq!(
            client.client().transaction_status_timeout(),
            Duration::from_secs(37)
        );
        assert_eq!(
            client.client().torii_request_timeout(),
            Duration::from_secs(3)
        );
        assert_eq!(
            client.client().transaction_ttl(),
            Some(Duration::from_secs(160))
        );
        assert_eq!(
            client.client().network_id(),
            &peer.client_config.get().unwrap().network_id
        );
        assert_eq!(client.client().operator_key_pair(), Some(&peer.key_pair));
    }
    assert_eq!(alice.client().account(), &*ALICE_ID);
    assert_eq!(bob.client().account(), &*BOB_ID);
    assert_eq!(peer.client().client().account(), &*ALICE_ID);
}

#[test]
fn peer_context_does_not_share_authority_or_endpoint_between_peers() {
    let (_first_directory, first) = context_owner_fixture();
    let (_second_directory, second) = context_owner_fixture();
    let a = first.client();
    let b = second.client();
    assert_ne!(a.client().endpoint(), b.client().endpoint());
    assert_ne!(
        a.client().operator_key_pair(),
        b.client().operator_key_pair()
    );
    assert!(!std::ptr::eq(
        first.retained_client(),
        second.retained_client()
    ));
    let mut changed_builder = a.client().to_builder();
    changed_builder.torii_request_timeout = Duration::from_secs(99);
    assert_eq!(
        first.client().client().torii_request_timeout(),
        Duration::from_secs(3)
    );
}

#[test]
fn late_peer_context_preserves_signed_genesis_and_peer_ownership() -> Result<()> {
    let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(4));
    let genesis = network.genesis();
    let genesis_hash = genesis.0.hash();
    let roster = network
        .validators()
        .iter()
        .map(NetworkPeer::id)
        .collect::<Vec<_>>();
    let peer = NetworkPeer::builder().build(network.env());
    let base_config = peer.base_config_table();
    assert!(peer.client_config.get().is_none());
    network.attach_peer_client_context(&peer)?;
    let original = network.peers()[0].client_config.get().unwrap();
    let attached = peer.client_config.get().unwrap();
    assert_eq!(attached.chain, original.chain);
    assert_eq!(
        attached.network_id,
        NetworkId::from_genesis_hash(genesis_hash)
    );
    assert_eq!(attached.chain_discriminant, original.chain_discriminant);
    assert_eq!(
        attached.policy.status_timeout,
        original.policy.status_timeout
    );
    assert_eq!(
        attached.policy.request_timeout,
        original.policy.request_timeout
    );
    assert_eq!(attached.policy.ttl, original.policy.ttl);
    let client = peer.client();
    assert_eq!(client.client().network_id(), &network.network_id());
    assert_eq!(client.client().chain(), &network.chain_id());
    assert_eq!(client.client().operator_key_pair(), Some(&peer.key_pair));
    assert_eq!(
        client.client().endpoint().as_str(),
        format!("{}/", peer.torii_url())
    );
    assert_eq!(client.client().account(), &*ALICE_ID);
    assert_eq!(peer.base_config_table(), base_config);
    assert!(!peer.is_running());
    assert_eq!(
        network
            .validators()
            .iter()
            .map(NetworkPeer::id)
            .collect::<Vec<_>>(),
        roster
    );
    assert_eq!(network.genesis().0.hash(), genesis_hash);
    let signatures = genesis.0.signatures().collect::<Vec<_>>();
    assert_eq!(signatures.len(), 1);
    signatures[0].signature().verify_hash(
        network.genesis_key_pair.as_ref().unwrap().public_key(),
        genesis_hash,
    )?;
    let clone = peer.clone();
    assert!(Arc::ptr_eq(&clone.client_config, &peer.client_config));
    assert!(network.attach_peer_client_context(&clone).is_err());
    assert_eq!(clone.client().client().network_id(), &network.network_id());
    Ok(())
}

#[test]
fn late_peer_context_rejects_foreign_environment_without_binding() -> Result<()> {
    let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(4));
    let directory = tempdir()?;
    let environment = Environment {
        dir: directory.path().to_path_buf(),
    };
    let foreign = NetworkPeer::builder().build(&environment);
    assert!(network.attach_peer_client_context(&foreign).is_err());
    assert!(foreign.client_config.get().is_none());
    assert!(foreign.retained_client.get().is_none());
    Ok(())
}
