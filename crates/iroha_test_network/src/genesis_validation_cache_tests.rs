#[test]
fn validated_genesis_cache_reuses_exact_block_and_network_identity() {
    use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};

    let _profile = ChainDiscriminantGuard::enter(369);
    let before_build = config::genesis_preexecution_count();
    let network = build_with_isolated_permit(
        NetworkBuilder::new()
            .with_peers(4)
            .with_npos_consensus()
            .with_config_layer(|layer| {
                layer
                    .write("chain", "fc56984b-2be7-431d-840e-21514d1883f0")
                    .write("chain_discriminant", 369_i64);
            }),
    );
    let after_build = config::genesis_preexecution_count();
    assert!(
        after_build > before_build,
        "the fixture must perform real native pre-execution"
    );
    let validated = network
        .validated_genesis
        .as_ref()
        .expect("file-free network has a validation cache")
        .get()
        .expect("builder must validate before publishing network identity");
    let expected_wire = validated
        .block
        .0
        .encode_wire()
        .expect("canonical genesis wire");
    let expected_id = NetworkId::from_genesis_hash(validated.block.0.hash());
    {
        let _foreign = ChainDiscriminantGuard::enter(777);
        for _ in 0..3 {
            assert_eq!(network.genesis().0.encode_wire().unwrap(), expected_wire);
            assert_eq!(network.network_id(), expected_id);
            assert_eq!(
                chain_discriminant(),
                777,
                "cached reads must not leak a profile scope"
            );
        }
    }
    assert_eq!(
        config::genesis_preexecution_count(),
        after_build,
        "genesis and network identity getters must not repeat native pre-execution"
    );

    // A failed exact policy check must not turn a raw block into a published cache.
    let topology = network
        .peers()
        .iter()
        .map(NetworkPeer::id)
        .collect::<Vec<_>>();
    for corrupt_nexus in [true, false] {
        let cache = OnceLock::new();
        let mut bad = validated.staged_hashes;
        if corrupt_nexus {
            bad.nexus_amx = CryptoHash::new(b"wrong staged Nexus/AMX binding");
        } else {
            bad.execution_policy = CryptoHash::new(b"wrong staged execution policy binding");
        }
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.get_or_init(|| {
                ValidatedNetworkGenesis::new(
                    validated.block.clone(),
                    bad,
                    &network.consensus_profile,
                    &topology,
                )
            });
        }));
        assert!(failure.is_err());
        assert!(
            cache.get().is_none(),
            "failed validation must leave the cache unpublished"
        );
    }
    let mut wrong_topology = topology.clone();
    wrong_topology.pop();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            validated.assert_matches(&network.consensus_profile, &wrong_topology);
        }))
        .is_err(),
        "cached reads must retain the exact signed voting roster check"
    );
}

#[test]
fn file_backed_genesis_keeps_fresh_preexecution_validation() {
    use iroha_data_model::account::address::ChainDiscriminantGuard;

    let _profile = ChainDiscriminantGuard::enter(defaults::common::chain_discriminant());
    for (directory_field, custom) in [("manifest_directory", false), ("cache_directory", true)] {
        let directory = tempfile::tempdir().expect("owned empty public manifest directory");
        let path = directory.path().to_string_lossy().into_owned();
        let raw_custom = Arc::new(std::sync::Mutex::new(None));
        let mut builder = NetworkBuilder::new()
            .with_peers(4)
            .with_config_layer(move |layer| {
                layer.write(["nexus", "registry", directory_field], path.clone());
            });
        if custom {
            let retained = Arc::clone(&raw_custom);
            builder = builder.with_genesis_block(move |topology, pops| {
                let raw = unexecuted_genesis_factory_with_post_topology(
                    Vec::new(),
                    Vec::new(),
                    topology,
                    pops,
                );
                *retained.lock().expect("raw custom genesis") = Some(raw.clone());
                raw
            });
        }
        let mut network = build_with_isolated_permit(builder);
        assert!(
            network.validated_genesis.is_none(),
            "file-backed policies must not use the immutable fast path"
        );
        if custom {
            // The public builder normally augments custom genesis before returning.
            // Restore its real callback output to exercise the getter's retained
            // augmentation path twice: an auxiliary cache must not bypass staging.
            let raw = raw_custom
                .lock()
                .expect("raw custom genesis")
                .take()
                .unwrap();
            assert!(!genesis_has_exactly_one_consensus_handshake(
                &raw,
                &consensus_handshake_parameter(&network.consensus_profile),
            ));
            network.cached_genesis = OnceLock::new();
            network
                .cached_genesis
                .set(raw)
                .expect("restore raw custom fixture");
        }
        let before = config::genesis_preexecution_count();
        let block = network.genesis();
        let after_genesis = config::genesis_preexecution_count();
        assert!(
            after_genesis > before,
            "the cached raw block still requires native staging"
        );
        assert_eq!(
            network.network_id(),
            NetworkId::from_genesis_hash(block.0.hash())
        );
        assert!(
            config::genesis_preexecution_count() > after_genesis,
            "every file-backed lookup, including repeated custom augmentation, must stage again"
        );
    }
}
