#[test]
fn runtime_effective_projection_binds_frozen_pops_and_rejects_observers() {
    let _registry_guard = instruction_registry_test_guard();
    iroha_genesis::init_instruction_registry();
    let mut fixture = offline_semantic_genesis_fixture([]);
    let signed = iroha_core::sumeragi::signed_genesis_validator_pops(&fixture.genesis)
        .expect("signed validator authority");
    let advertised_peers = signed
        .keys()
        .enumerate()
        .map(|(index, id)| {
            iroha_data_model::peer::Peer::new(
                format!("127.0.0.1:{}", 17_000 + index)
                    .parse()
                    .expect("fixture address"),
                id.public_key().clone(),
            )
        })
        .collect::<Vec<_>>();
    let validator_keys = (0_u8..4)
        .map(|index| {
            iroha_crypto::KeyPair::try_from_seed(
                vec![0x50 + index; 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("deterministic validator key")
        })
        .collect::<Vec<_>>();
    let myself = iroha_data_model::peer::Peer::new(
        "0.0.0.0:16000".parse().expect("fixture bind address"),
        advertised_peers[0].id().public_key().clone(),
    );
    fixture.config.common.peer = myself.clone();
    fixture.config.common.key_pair = validator_keys
        .iter()
        .find(|key| key.public_key() == myself.id().public_key())
        .expect("local validator private key")
        .clone();
    fixture.config.network.public_address =
        WithOrigin::inline(advertised_peers[0].address().clone());
    fixture.config.common.trusted_peers =
        WithOrigin::inline(iroha_config::parameters::actual::TrustedPeers {
            myself,
            others: advertised_peers.iter().skip(1).cloned().collect(),
            pops: signed
                .iter()
                .map(|(id, pop)| (id.public_key().clone(), pop.clone()))
                .collect(),
        });
    fixture.config.sumeragi.role = iroha_config::parameters::actual::NodeRole::Validator;
    fixture.config.genesis.expected_hash = fixture.genesis.0.hash();
    let bootstrap = validate_genesis_execution_offline(
        &fixture.config,
        &fixture.genesis,
        &fixture.authority,
        fixture.mode,
        fixture.parameters,
        fixture.cadence_ms,
        load_configured_kagemusha_release_catalog(&fixture.config).expect("empty release catalog"),
    )
    .expect("fully validated genesis bootstrap");
    let verified = kagemusha_runtime_effective_config_projection::build_kagemusha_runtime_effective_config_projection_v1(
        &fixture.config,
        &fixture.genesis,
        &bootstrap,
    )
    .expect("validator effective projection");
    let projection = verified.projection();
    assert_eq!(projection.genesis_context, fixture.parameters);
    assert!(
        projection
            .validators
            .iter()
            .zip(bootstrap.proofs_of_possession())
            .all(|(validator, staged)| &validator.bls_pop == staged)
    );
    assert_eq!(
        projection.validators[0].public_address,
        advertised_peers[0].address().clone(),
        "the projection must seal the advertised endpoint, not the bind address",
    );
    let signed_genesis_projection = iroha_core::smartcontracts::isi::offline::VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive_from_signed_genesis(
        &fixture.config,
        &fixture.genesis,
    )
    .expect("signed genesis derives the same runtime projection");
    assert_eq!(projection, signed_genesis_projection.projection());

    let mut snapshot_context = bootstrap.context().clone();
    snapshot_context.height = 2;
    snapshot_context.epoch_end_height = snapshot_context.epoch_end_height.max(2);
    snapshot_context.parent_commit_qc = None;
    snapshot_context.nexus_amx_context_hash =
        iroha_crypto::Hash::new(b"post-genesis authenticated Nexus context");
    snapshot_context.snapshot_bootstrap = Some(
        iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
            snapshot_height: 1,
            snapshot_block_hash: fixture.genesis.0.hash(),
            snapshot_block_creation_time_ms: 1,
            snapshot_state_hash: iroha_crypto::Hash::new(b"authenticated snapshot state"),
        },
    );
    let snapshot = iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord {
        version:
            iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord::VERSION,
        context: snapshot_context,
        validator_set_pops: bootstrap.proofs_of_possession().to_vec(),
    };
    snapshot.validate().expect("valid snapshot lineage fixture");
    let snapshot_projection = iroha_core::smartcontracts::isi::offline::VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive_from_authenticated_snapshot(
        &fixture.config,
        &snapshot,
        std::time::Duration::from_millis(fixture.cadence_ms),
        projection.genesis_context,
    )
    .expect("authenticated snapshot derives the same runtime projection");
    assert_eq!(projection, snapshot_projection.projection());
    let mut leader_ordered_snapshot = snapshot;
    leader_ordered_snapshot.context.roster.reverse();
    leader_ordered_snapshot.validator_set_pops.reverse();
    leader_ordered_snapshot
        .validate()
        .expect("leader ordering preserves a valid snapshot roster");
    let leader_ordered_projection = iroha_core::smartcontracts::isi::offline::VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive_from_authenticated_snapshot(
        &fixture.config,
        &leader_ordered_snapshot,
        std::time::Duration::from_millis(fixture.cadence_ms),
        projection.genesis_context,
    )
    .expect("leader-ordered snapshot derives the canonical runtime projection");
    assert_eq!(projection, leader_ordered_projection.projection());

    let mut second_host = fixture.config.clone();
    let second_myself = iroha_data_model::peer::Peer::new(
        "0.0.0.0:16001".parse().expect("second bind address"),
        advertised_peers[1].id().public_key().clone(),
    );
    second_host.common.peer = second_myself.clone();
    second_host.common.key_pair = validator_keys
        .iter()
        .find(|key| key.public_key() == second_myself.id().public_key())
        .expect("second validator private key")
        .clone();
    second_host.network.public_address = WithOrigin::inline(advertised_peers[1].address().clone());
    second_host.common.trusted_peers =
        WithOrigin::inline(iroha_config::parameters::actual::TrustedPeers {
            myself: second_myself,
            others: advertised_peers
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != 1)
                .map(|(_, peer)| peer.clone())
                .collect(),
            pops: signed
                .iter()
                .map(|(id, pop)| (id.public_key().clone(), pop.clone()))
                .collect(),
        });
    let second_projection = kagemusha_runtime_effective_config_projection::build_kagemusha_runtime_effective_config_projection_v1(
        &second_host,
        &fixture.genesis,
        &bootstrap,
    )
    .expect("second validator effective projection");
    assert_eq!(projection, second_projection.projection());

    let mut observer = fixture.config.clone();
    observer.sumeragi.role = iroha_config::parameters::actual::NodeRole::Observer;
    assert!(
        kagemusha_runtime_effective_config_projection::build_kagemusha_runtime_effective_config_projection_v1(
            &observer,
            &fixture.genesis,
            &bootstrap,
        )
        .is_err()
    );
    let mut mismatched_pop = fixture.config.clone();
    let first_key = signed
        .keys()
        .next()
        .expect("four signed validators")
        .public_key()
        .clone();
    mismatched_pop
        .common
        .trusted_peers
        .value_mut()
        .pops
        .get_mut(&first_key)
        .expect("configured PoP")[0] ^= 1;
    assert!(
        kagemusha_runtime_effective_config_projection::build_kagemusha_runtime_effective_config_projection_v1(
            &mismatched_pop,
            &fixture.genesis,
            &bootstrap,
        )
        .is_err()
    );
    let outsider =
        iroha_crypto::KeyPair::try_from_seed(vec![0x7f; 32], iroha_crypto::Algorithm::BlsNormal)
            .expect("outsider BLS key");
    let outsider_pop =
        iroha_crypto::bls_normal_pop_prove(outsider.private_key()).expect("outsider PoP");
    let mut extra_pop = fixture.config;
    extra_pop
        .common
        .trusted_peers
        .value_mut()
        .pops
        .insert(outsider.public_key().clone(), outsider_pop);
    assert!(
        kagemusha_runtime_effective_config_projection::build_kagemusha_runtime_effective_config_projection_v1(
            &extra_pop,
            &fixture.genesis,
            &bootstrap,
        )
        .is_err()
    );
}
