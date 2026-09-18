use crate::query::store::LiveQueryStore;
use iroha_data_model::nexus::{
    DataSpaceMetadata, LaneConfig as RuntimeLaneConfig, LaneLifecycleParameterV1,
    NexusCatalogTransitionV1, NexusRuntimeCatalogV1, RuntimeDataSpaceAdditionV1,
    RuntimeLaneManifestV1,
};

fn run_catalog_test(test: impl FnOnce() + Send + 'static) {
    let result = std::thread::Builder::new()
        .name("runtime-catalog-test".to_owned())
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .expect("spawn bounded-stack catalog test")
        .join();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[derive(Clone, Copy)]
enum InvalidMember {
    None,
    MissingAccount,
    MissingPeer,
    MissingKey,
    MissingPop,
    InvalidPop,
    WrongRole,
    Disabled,
    NotYetActive,
    Expired,
}

fn catalog_fixture(invalid: InvalidMember) -> (State, Vec<iroha_crypto::KeyPair>) {
    let keys: Vec<_> = (1_u8..=4)
        .map(|seed| {
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
                .expect("deterministic synthetic validator key")
        })
        .collect();
    let accounts: Vec<_> = keys
        .iter()
        .enumerate()
        .filter_map(|(index, key)| {
            if index == 0 && matches!(invalid, InvalidMember::MissingAccount) {
                return None;
            }
            let id = AccountId::new(key.public_key().clone());
            Some(iroha_data_model::account::Account::new(id.clone()).build(&id))
        })
        .collect();
    let lanes = LaneCatalog::new(
        NonZeroU32::new(5).unwrap(),
        (0..5)
            .map(|id| RuntimeLaneConfig {
                id: LaneId::new(id),
                alias: format!("existing-{id}"),
                dataspace_id: DataSpaceId::new(match id {
                    3 => 10,
                    4 => 12,
                    _ => 0,
                }),
                ..RuntimeLaneConfig::default()
            })
            .collect(),
    )
    .expect("five retained fixture lanes");
    let dataspaces = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(10),
            alias: "dpn".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(12),
            alias: "existing-private".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("retained fixture dataspaces");
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.configured_lane_catalog = lanes.clone();
    nexus.lane_catalog = lanes;
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.configured_dataspace_catalog = dataspaces.clone();
    nexus.dataspace_catalog = dataspaces;
    nexus.staking.public_validator_mode =
        iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
    let state = State::new_with_nexus_for_testing(
        World::with([], accounts, []),
        nexus.clone(),
        LiveQueryStore::start_test(),
    );
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &nexus.registry,
    )));
    let mut world = state.world.block();
    {
        let mut peers = world.peers_mut_for_testing().transaction();
        for (index, key) in keys.iter().enumerate() {
            if index != 0 || !matches!(invalid, InvalidMember::MissingPeer) {
                peers.push(PeerId::new(key.public_key().clone()));
            }
        }
        peers.apply();
    }
    for (index, key) in keys.iter().enumerate() {
        if index == 0 && matches!(invalid, InvalidMember::MissingKey) {
            continue;
        }
        let mut id = derive_validator_key_id(key.public_key());
        if index == 0 && matches!(invalid, InvalidMember::WrongRole) {
            id.role = ConsensusKeyRole::Endorsement;
        }
        let mut pop =
            Some(iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("synthetic BLS PoP"));
        if index == 0 {
            match invalid {
                InvalidMember::MissingPop => pop = None,
                InvalidMember::InvalidPop => pop.as_mut().unwrap()[0] ^= 1,
                _ => {}
            }
        }
        let record = ConsensusKeyRecord {
            id: id.clone(),
            public_key: key.public_key().clone(),
            pop,
            activation_height: if index == 0 && matches!(invalid, InvalidMember::NotYetActive) {
                4
            } else {
                0
            },
            expiry_height: (index == 0 && matches!(invalid, InvalidMember::Expired)).then_some(3),
            replaces: None,
            status: if index == 0 && matches!(invalid, InvalidMember::Disabled) {
                ConsensusKeyStatus::Disabled
            } else {
                ConsensusKeyStatus::Active
            },
        };
        world.consensus_keys.insert(id.clone(), record);
        world
            .consensus_keys_by_pk
            .insert(key.public_key().to_string(), vec![id]);
    }
    world.commit();
    (state, keys)
}

fn catalog_payload(state: &State, keys: &[iroha_crypto::KeyPair]) -> NexusCatalogTransitionV1 {
    let hash = [0x67; 32];
    let new_dataspace = DataSpaceId::from_hash(&hash);
    let nexus = state.nexus_snapshot();
    NexusCatalogTransitionV1 {
        version: 1,
        expected_catalog_hash: LaneLifecycleParameterV1::catalog_hash(&nexus.lane_catalog),
        expected_incarnation_root: lane_lifecycle_incarnation_root(
            &nexus.lane_catalog,
            &state.lane_incarnations_snapshot(),
        )
        .unwrap(),
        expected_runtime_catalog_hash: None,
        dataspace_additions: vec![RuntimeDataSpaceAdditionV1 {
            descriptor: DataSpaceMetadata {
                id: new_dataspace,
                alias: "new-catalog-ds".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            manifest_hash: hash,
        }],
        lane_additions: vec![RuntimeLaneConfig {
            id: LaneId::new(5),
            alias: "new-catalog-lane".to_owned(),
            dataspace_id: new_dataspace,
            ..RuntimeLaneConfig::default()
        }],
        manifest_additions: vec![catalog_manifest(LaneId::new(5), "new-catalog-lane", keys)],
    }
}

fn catalog_manifest(
    lane_id: LaneId,
    alias: &str,
    keys: &[iroha_crypto::KeyPair],
) -> RuntimeLaneManifestV1 {
    let validators: Vec<_> = keys
        .iter()
        .map(|key| {
            let validator = AccountId::new(key.public_key().clone()).to_string();
            let peer_id = PeerId::new(key.public_key().clone()).to_string();
            norito::json!({
                "validator": validator,
                "peer_id": peer_id,
            })
        })
        .collect();
    RuntimeLaneManifestV1 {
        lane_id,
        manifest: iroha_primitives::json::Json::new(norito::json!({
            "lane": alias, "version": 1, "validators": validators, "quorum": 3,
            "privacy_commitments": [{
                "id": 1, "scheme": "merkle",
                "merkle": {
                    "root": "1212121212121212121212121212121212121212121212121212121212121212",
                    "max_depth": 16,
                },
            }],
        })),
    }
}

#[test]
fn runtime_catalog_preflight_preserves_prior_additions_and_rejects_replacement() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut nexus = state.nexus_snapshot();
        let baseline_registry = state.lane_manifests.read().clone();
        let first = NexusRuntimeCatalogV1 {
            version: 1,
            baseline_dataspaces_hash: iroha_data_model::nexus::dataspace_catalog_hash(
                &nexus.configured_dataspace_catalog,
            ),
            baseline_manifests_hash: Hash::prehashed(
                baseline_registry.baseline_consensus_policy_digest(),
            ),
            dataspaces: payload.dataspace_additions,
            manifests: payload.manifest_additions,
        };
        let first_plan = iroha_data_model::nexus::LaneLifecyclePlan {
            additions: payload.lane_additions,
            retire: Vec::new(),
        };
        nexus.dataspace_catalog = runtime_catalog_transition_dataspaces(
            &nexus,
            &baseline_registry,
            &state.world.view(),
            &first,
            &first_plan,
        )
        .expect("first additive transition");
        nexus.lane_catalog = nexus.lane_catalog.apply_lifecycle(&first_plan).unwrap();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        let registry = baseline_registry
            .with_runtime_additions(
                &first.manifests,
                &nexus.lane_catalog,
                &nexus.dataspace_catalog,
                &nexus.governance,
            )
            .expect("accepted first native manifest");
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                first.clone().into_custom_parameter().unwrap(),
            ));
        world.commit();
        let second_plan = iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![RuntimeLaneConfig {
                id: LaneId::new(6),
                alias: "second-catalog-lane".to_owned(),
                dataspace_id: first.dataspaces[0].descriptor.id,
                ..RuntimeLaneConfig::default()
            }],
            retire: Vec::new(),
        };
        let mut second = first.clone();
        second.manifests.push(catalog_manifest(
            LaneId::new(6),
            "second-catalog-lane",
            &keys,
        ));
        assert_eq!(
            runtime_catalog_transition_dataspaces(
                &nexus,
                &registry,
                &state.world.view(),
                &second,
                &second_plan,
            )
            .unwrap(),
            nexus.dataspace_catalog,
            "adding another lane preserves physical dataspaces"
        );
        for case in 0..7 {
            let mut changed = second.clone();
            let mut changed_plan = second_plan.clone();
            match case {
                0 => changed.dataspaces.clear(),
                1 => changed.dataspaces[0].descriptor.description = Some("changed".to_owned()),
                2 => {
                    changed.manifests.remove(0);
                }
                3 => changed.manifests[0] = catalog_manifest(LaneId::new(5), "changed", &keys),
                4 => changed.baseline_manifests_hash = Hash::new(b"foreign manifest baseline"),
                5 => changed_plan.retire.push(LaneId::new(4)),
                _ => changed_plan.additions[0].alias = "existing-4".to_owned(),
            }
            assert!(
                runtime_catalog_transition_dataspaces(
                    &nexus,
                    &registry,
                    &state.world.view(),
                    &changed,
                    &changed_plan,
                )
                .is_err(),
                "catalog replacement case {case}"
            );
        }
        assert_eq!(
            runtime_catalog_from_world(&state.world.view()).unwrap(),
            Some(first)
        );
    });
}

#[test]
fn runtime_catalog_stages_dataspace_lane_manifest_atomically_with_four_live_pops() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let before = state.nexus_snapshot();
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .expect("complete atomic catalog transition");
        let runtime = runtime_catalog_from_world(&transaction.world)
            .unwrap()
            .expect("protected cumulative catalog");
        assert_eq!(runtime.dataspaces, payload.dataspace_additions);
        assert_eq!(runtime.manifests, payload.manifest_additions);
        assert_eq!(
            transaction.nexus.configured_dataspace_catalog,
            before.configured_dataspace_catalog
        );
        assert_eq!(
            &transaction.nexus.lane_catalog.lanes()[..5],
            before.lane_catalog.lanes()
        );
        assert_eq!(
            transaction.nexus.lane_catalog.lanes()[3].dataspace_id,
            DataSpaceId::new(10)
        );
        assert_eq!(
            transaction.nexus.lane_catalog.lanes()[4].dataspace_id,
            DataSpaceId::new(12)
        );
        assert!(transaction.lane_manifests.has_manifest(LaneId::new(5)));
        assert!(
            transaction
                .lane_manifests
                .status(LaneId::new(5))
                .unwrap()
                .manifest_path
                .is_none()
        );
        assert_eq!(
            runtime_catalog_root_from_world(&transaction.world).unwrap(),
            Some(runtime.canonical_hash().unwrap())
        );
        assert!(
            transaction
                .pending_lane_lifecycle
                .as_ref()
                .unwrap()
                .runtime_catalog
                .is_some()
        );
        assert!(matches!(
            transaction.stage_consensus_catalog_transition(&payload),
            Err(LaneLifecycleError::LifecycleAlreadyStaged)
        ));
        drop(transaction);
        assert_eq!(
            block.nexus.lane_catalog, before.lane_catalog,
            "aborted transaction cannot publish topology"
        );
        assert!(runtime_catalog_from_world(&block.world).unwrap().is_none());
    });
}

#[test]
fn runtime_catalog_rejects_ineligible_committee_without_partial_state() {
    run_catalog_test(|| {
        for invalid in [
            InvalidMember::MissingAccount,
            InvalidMember::MissingPeer,
            InvalidMember::MissingKey,
            InvalidMember::MissingPop,
            InvalidMember::InvalidPop,
            InvalidMember::WrongRole,
            InvalidMember::Disabled,
            InvalidMember::NotYetActive,
            InvalidMember::Expired,
        ] {
            let (state, keys) = catalog_fixture(invalid);
            let before = state.nexus_snapshot();
            let payload = catalog_payload(&state, &keys);
            let mut block = state.block(BlockHeader::new(
                NonZeroU64::new(2).unwrap(),
                None,
                None,
                0,
                0,
            ));
            let mut transaction = block.transaction();
            transaction
                .stage_consensus_catalog_transition(&payload)
                .expect_err("invalid authority cannot activate");
            assert_eq!(transaction.nexus.lane_catalog, before.lane_catalog);
            assert_eq!(
                transaction.nexus.dataspace_catalog,
                before.dataspace_catalog
            );
            assert!(
                runtime_catalog_from_world(&transaction.world)
                    .unwrap()
                    .is_none()
            );
            assert!(transaction.pending_lane_lifecycle.is_none());
            assert!(!transaction.lane_manifests.has_manifest(LaneId::new(5)));
        }
    });
}

#[test]
fn runtime_catalog_rejects_stale_roots_and_genesis_without_partial_state() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let original = catalog_payload(&state, &keys);
        for case in 0..4 {
            let mut payload = original.clone();
            match case {
                0 => payload.expected_catalog_hash = Hash::new(b"stale catalog"),
                1 => payload.expected_incarnation_root = Hash::new(b"stale incarnation"),
                2 => payload.expected_runtime_catalog_hash = Some(Hash::new(b"stale runtime")),
                _ => {}
            }
            let height = if case == 3 { 1 } else { 2 };
            let mut block = state.block(BlockHeader::new(
                NonZeroU64::new(height).unwrap(),
                None,
                None,
                0,
                0,
            ));
            let mut transaction = block.transaction();
            transaction
                .stage_consensus_catalog_transition(&payload)
                .expect_err("stale or pre-genesis catalog request");
            assert!(
                runtime_catalog_from_world(&transaction.world)
                    .unwrap()
                    .is_none()
            );
            assert!(transaction.pending_lane_lifecycle.is_none());
        }
    });
}

#[test]
fn runtime_catalog_accessor_rejects_malformed_protected_state_and_baseline_drift() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        assert!(
            runtime_catalog_from_world(&state.world.view())
                .unwrap()
                .is_none()
        );
        let baseline = state.nexus_snapshot().configured_dataspace_catalog;
        let payload = catalog_payload(&state, &keys);
        let runtime = NexusRuntimeCatalogV1 {
            version: 1,
            baseline_dataspaces_hash: iroha_data_model::nexus::dataspace_catalog_hash(&baseline),
            baseline_manifests_hash: Hash::prehashed(
                state
                    .lane_manifests
                    .read()
                    .baseline_consensus_policy_digest(),
            ),
            dataspaces: payload.dataspace_additions,
            manifests: payload.manifest_additions,
        };
        let effective = runtime_catalog_dataspaces(&baseline, Some(&runtime)).unwrap();
        assert!(effective.by_id(DataSpaceId::new(10)).is_some());
        assert!(effective.by_id(DataSpaceId::new(12)).is_some());
        let mut wrong = runtime;
        wrong.baseline_dataspaces_hash = Hash::new(b"foreign baseline");
        assert!(runtime_catalog_dataspaces(&baseline, Some(&wrong)).is_err());
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                iroha_data_model::parameter::CustomParameter::new(
                    NexusRuntimeCatalogV1::parameter_id(),
                    iroha_primitives::json::Json::new(norito::json!({"version": 255})),
                ),
            ));
        world.commit();
        assert!(runtime_catalog_from_world(&state.world.view()).is_err());
        assert!(runtime_catalog_root_from_world(&state.world.view()).is_err());
    });
}

include!("runtime_catalog_commit_tests.rs");
