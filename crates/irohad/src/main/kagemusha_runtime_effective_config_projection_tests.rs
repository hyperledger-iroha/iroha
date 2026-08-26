#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the focused projection test keeps one shared authenticated genesis fixture across every binding and rejection assertion"
)]
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
        version: iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord::VERSION,
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

struct SnapshotRuntimeInstallFixture {
    config: Config,
    genesis: GenesisBlock,
    snapshot: iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord,
    cadence: std::time::Duration,
    projection: iroha_data_model::offline::KagemushaV4RuntimeEffectiveConfigProjectionV1,
    validator_keys: Vec<iroha_crypto::KeyPair>,
}

fn snapshot_runtime_install_fixture() -> SnapshotRuntimeInstallFixture {
    let mut fixture = offline_semantic_genesis_fixture([]);
    let signed = iroha_core::sumeragi::signed_genesis_validator_pops(&fixture.genesis)
        .expect("signed validator authority");
    let advertised_peers = signed
        .keys()
        .enumerate()
        .map(|(index, id)| {
            iroha_data_model::peer::Peer::new(
                format!("127.0.0.1:{}", 18_000 + index)
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
    let projection =
        kagemusha_runtime_effective_config_projection::build_kagemusha_runtime_effective_config_projection_v1(
            &fixture.config,
            &fixture.genesis,
            &bootstrap,
        )
        .expect("validator effective projection")
        .projection()
        .clone();
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
        version: iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord::VERSION,
        context: snapshot_context,
        validator_set_pops: bootstrap.proofs_of_possession().to_vec(),
    };
    snapshot.validate().expect("valid snapshot lineage fixture");
    SnapshotRuntimeInstallFixture {
        config: fixture.config,
        genesis: fixture.genesis,
        snapshot,
        cadence: std::time::Duration::from_millis(fixture.cadence_ms),
        projection,
        validator_keys,
    }
}

fn snapshot_validator_seal(
    projection: iroha_data_model::offline::KagemushaV4RuntimeEffectiveConfigProjectionV1,
    network_id: iroha_data_model::NetworkId,
    signer: &iroha_crypto::KeyPair,
) -> iroha_data_model::offline::KagemushaV4ValidatorQualificationSealV1 {
    use iroha_data_model::offline::{
        KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA, KagemushaExactBytesDigestV1,
        KagemushaV4PromotionBindingV1, KagemushaV4ValidatorQualificationSealBodyV1,
        KagemushaV4ValidatorQualificationSealV1,
    };

    let exact = |bytes: &[u8]| {
        KagemushaExactBytesDigestV1::from_bytes(bytes).expect("nonempty exact-byte fixture")
    };
    let controller =
        iroha_crypto::KeyPair::from_seed(vec![0x91; 32], iroha_crypto::Algorithm::Ed25519);
    let body = KagemushaV4ValidatorQualificationSealBodyV1 {
        schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        binding: KagemushaV4PromotionBindingV1 {
            promotion_controller: controller.public_key().clone(),
            promotion_reservation: exact(b"snapshot promotion reservation"),
            promotion_id: [0x92; 32],
            network_id,
            reviewed_source_closure_descriptor_sha256: [0x93; 32],
            manifest_sha256: [0x94; 32],
            release_record_sha256: [0x95; 32],
            release_policy_source: exact(b"snapshot release policy"),
            device_attestation_policy_norito: exact(b"snapshot device policy"),
            signed_genesis: exact(b"snapshot signed genesis"),
            catalog_consensus_policy_digest: [0x96; 32],
            execution_policy_hash: iroha_crypto::Hash::prehashed(
                projection.genesis_context.execution_policy_hash,
            ),
        },
        validator_id: iroha_data_model::peer::PeerId::new(signer.public_key().clone()),
        iroha3d_executable: exact(b"snapshot iroha3d"),
        flattened_toml_config_source: exact(b"snapshot flattened config"),
        runtime_effective_config: projection,
        catalog_qualification_seal: exact(b"snapshot catalog seal"),
    };
    KagemushaV4ValidatorQualificationSealV1::try_sign(body, signer)
        .expect("valid validator qualification seal fixture")
}

fn assert_runtime_digest_absent(state: &State, sentinel: u8) {
    state
        .install_kagemusha_runtime_effective_config_sha256([sentinel; 32])
        .expect("failed startup must leave the runtime digest uninstalled");
}

#[test]
fn authenticated_snapshot_with_valid_local_seal_installs_runtime_digest() {
    let _registry_guard = instruction_registry_test_guard();
    iroha_genesis::init_instruction_registry();
    let mut fixture = snapshot_runtime_install_fixture();
    fixture
        .config
        .settlement
        .offline
        .kagemusha_validator_qualification_seal_path =
        Some("/fixture/local-validator-seal.norito".into());
    let (state, _kura) = genesis_staging_state_for_test(&fixture.config, &fixture.genesis);
    let seal = snapshot_validator_seal(
        fixture.projection.clone(),
        fixture.snapshot.context.network_id,
        &fixture.config.common.key_pair,
    );
    kagemusha_startup::install_runtime_effective_config_with_validator_seal_reader(
        &fixture.config,
        &state,
        Some(&fixture.snapshot),
        fixture.cadence,
        None,
        |_| Ok(seal),
    )
    .expect("the exact local snapshot seal installs its runtime digest");
    let expected = fixture
        .projection
        .consensus_sha256()
        .expect("valid fixture projection digest");
    let mut different = expected;
    different[0] ^= 1;
    assert!(
        state
            .install_kagemusha_runtime_effective_config_sha256(different)
            .is_err(),
        "a different digest must fail after startup installed the projection"
    );
    state
        .install_kagemusha_runtime_effective_config_sha256(expected)
        .expect("the installed digest remains idempotent");
}

#[test]
fn authenticated_snapshot_rejects_wrong_local_peer_without_installing() {
    let _registry_guard = instruction_registry_test_guard();
    iroha_genesis::init_instruction_registry();
    let mut fixture = snapshot_runtime_install_fixture();
    fixture
        .config
        .settlement
        .offline
        .kagemusha_validator_qualification_seal_path =
        Some("/fixture/wrong-peer-seal.norito".into());
    let (state, _kura) = genesis_staging_state_for_test(&fixture.config, &fixture.genesis);
    let other = fixture
        .validator_keys
        .iter()
        .find(|key| key.public_key() != fixture.config.common.peer.id.public_key())
        .expect("another qualified validator");
    let seal = snapshot_validator_seal(
        fixture.projection,
        fixture.snapshot.context.network_id,
        other,
    );
    let error = kagemusha_startup::install_runtime_effective_config_with_validator_seal_reader(
        &fixture.config,
        &state,
        Some(&fixture.snapshot),
        fixture.cadence,
        None,
        |_| Ok(seal),
    )
    .expect_err("a seal for another validator must fail closed");
    assert!(error.contains("different local peer"));
    assert_runtime_digest_absent(&state, 0xa1);
}

#[test]
fn authenticated_snapshot_rejects_projection_mismatch_without_installing() {
    let _registry_guard = instruction_registry_test_guard();
    iroha_genesis::init_instruction_registry();
    let mut fixture = snapshot_runtime_install_fixture();
    fixture
        .config
        .settlement
        .offline
        .kagemusha_validator_qualification_seal_path =
        Some("/fixture/mismatched-projection-seal.norito".into());
    let (state, _kura) = genesis_staging_state_for_test(&fixture.config, &fixture.genesis);
    let mut mismatched = fixture.projection;
    mismatched.kagemusha_max_decoded_bytes += 1;
    let seal = snapshot_validator_seal(
        mismatched,
        fixture.snapshot.context.network_id,
        &fixture.config.common.key_pair,
    );
    let error = kagemusha_startup::install_runtime_effective_config_with_validator_seal_reader(
        &fixture.config,
        &state,
        Some(&fixture.snapshot),
        fixture.cadence,
        None,
        |_| Ok(seal),
    )
    .expect_err("a stale or different sealed projection must fail closed");
    assert!(error.contains("effective snapshot runtime differs"));
    assert_runtime_digest_absent(&state, 0xa2);
}

#[test]
fn authenticated_snapshot_without_configured_seal_does_not_install() {
    let _registry_guard = instruction_registry_test_guard();
    iroha_genesis::init_instruction_registry();
    let fixture = snapshot_runtime_install_fixture();
    assert!(
        fixture
            .config
            .settlement
            .offline
            .kagemusha_validator_qualification_seal_path
            .is_none()
    );
    let (state, _kura) = genesis_staging_state_for_test(&fixture.config, &fixture.genesis);
    kagemusha_startup::install_runtime_effective_config_with_validator_seal_reader(
        &fixture.config,
        &state,
        Some(&fixture.snapshot),
        fixture.cadence,
        None,
        |_| panic!("an absent configured seal must not invoke the reader"),
    )
    .expect("snapshot startup without a configured local seal remains inert");
    assert_runtime_digest_absent(&state, 0xa3);
}
