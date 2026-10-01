use super::*;
use iroha_data_model::nexus::{RuntimeDataSpaceRetirementV1, RuntimeLaneRetirementV1};

fn run_retirement_test(test: impl FnOnce() + Send + 'static) {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn retirement_fixture() -> (
    State,
    AccountId,
    iroha_data_model::nexus::NexusCatalogTransitionV1,
) {
    use iroha_data_model::{
        account::Account,
        sns::{NameControllerV1, NameRecordV1},
    };
    use iroha_model_base::metadata::Metadata;
    let key = iroha_crypto::KeyPair::try_from_seed(vec![81; 32], iroha_crypto::Algorithm::Ed25519)
        .unwrap();
    let owner = AccountId::new(key.public_key().clone());
    let selector = crate::sns::selector_for_dataspace_alias("is").unwrap();
    let dataspace = DataSpaceId::from_hash(&selector.name_hash());
    let lanes = LaneCatalog::new(
        NonZeroU32::new(8).unwrap(),
        vec![
            iroha_data_model::nexus::LaneConfig::default(),
            iroha_data_model::nexus::LaneConfig {
                id: LaneId::new(7),
                dataspace_id: dataspace,
                alias: "is".into(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let dataspaces = DataSpaceCatalog::new(vec![
        iroha_data_model::nexus::DataSpaceMetadata::default(),
        iroha_data_model::nexus::DataSpaceMetadata {
            id: dataspace,
            alias: "is".into(),
            description: Some("retained physical IS baseline".into()),
            fault_tolerance: 1,
        },
    ])
    .unwrap();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = lanes.clone();
    nexus.configured_lane_catalog = lanes;
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.dataspace_catalog = dataspaces.clone();
    nexus.configured_dataspace_catalog = dataspaces;
    nexus
        .routing_policy
        .rules
        .push(iroha_config::parameters::actual::LaneRoutingRule {
            lane: LaneId::new(7),
            dataspace: Some(dataspace),
            matcher: Default::default(),
        });
    let mut world = World::with([], [Account::new(owner.clone()).build(&owner)], []);
    let address = iroha_data_model::account::AccountAddress::from_account_id(&owner).unwrap();
    let lease = NameRecordV1::new(
        selector.clone(),
        owner.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        Metadata::default(),
    );
    world
        .smart_contract_state
        .insert(crate::sns::record_storage_key(&selector), lease.encode());
    let mut parameters = world.parameters.block();
    parameters.set_parameter(crate::sumeragi::lanes::routing::test_support::metadata(
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
    ));
    parameters.set_parameter(
        crate::sumeragi::lanes::routing::test_support::closed_native_lane_policy(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        ),
    );
    parameters.commit();
    let state = State::new_with_nexus_for_testing(
        world,
        nexus.clone(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    state.install_lane_manifests_for_testing(&Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &nexus.registry,
    )));
    let request = iroha_data_model::nexus::NexusCatalogTransitionV1 {
        version: 1,
        dataspace_additions: Vec::new(),
        lane_additions: Vec::new(),
        manifest_additions: Vec::new(),
        dataspace_retirements: vec![RuntimeDataSpaceRetirementV1 {
            dataspace_id: dataspace,
            alias: "is".into(),
            owner: owner.clone(),
            expected_ownership_generation: 1,
        }],
        lane_retirements: vec![LaneId::new(7)],
        expected_catalog_hash: LaneLifecycleParameterV1::catalog_hash(&nexus.lane_catalog),
        expected_incarnation_root: lane_lifecycle_incarnation_root(
            &nexus.lane_catalog,
            &state.lane_incarnations_snapshot(),
        )
        .unwrap(),
        expected_runtime_catalog_hash: None,
    };
    (state, owner, request)
}

#[test]
fn physical_retirement_removes_execution_and_keeps_exact_native_history() {
    run_retirement_test(|| {
        let (state, owner, request) = retirement_fixture();
        let original = state.nexus_snapshot();
        let original_incarnations = state.lane_incarnations_snapshot();
        let original_activation = state.lane_incarnation_activation_heights_snapshot();
        let original_sns = state
            .world
            .smart_contract_state
            .view()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(8).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&owner, &request)
            .unwrap();
        let runtime = runtime_catalog_from_world(&transaction.world)
            .unwrap()
            .unwrap();
        assert!(transaction.nexus.dataspace_catalog.by_alias("is").is_none());
        assert!(
            transaction
                .nexus
                .lane_catalog
                .lanes()
                .iter()
                .all(|lane| lane.id != LaneId::new(7))
        );
        assert!(transaction.nexus.routing_policy.rules.is_empty());
        assert_eq!(
            transaction.nexus.configured_lane_catalog,
            original.configured_lane_catalog
        );
        assert_eq!(
            transaction.nexus.configured_dataspace_catalog,
            original.configured_dataspace_catalog
        );
        assert_eq!(
            runtime.retired_lanes,
            vec![RuntimeLaneRetirementV1 {
                lane: original.lane_catalog.lanes()[1].clone(),
                incarnation: original_incarnations[&LaneId::new(7)],
                activation_height: original_activation[&LaneId::new(7)],
                retirement_height: 8
            }]
        );
        let update = &transaction
            .pending_lane_lifecycle
            .as_ref()
            .unwrap()
            .catalog_update;
        assert!(lane_config_entries_match(
            &update.previous_storage_geometry.config,
            &update.updated_storage_geometry.config
        ));
        assert_eq!(
            update.previous_storage_geometry.incarnations,
            update.updated_storage_geometry.incarnations
        );
        assert_eq!(
            update.previous_storage_geometry.activation_heights,
            update.updated_storage_geometry.activation_heights
        );
        assert_eq!(
            update.previous_lane_incarnation_lineage,
            update.updated_lane_incarnation_lineage
        );
        assert!(!update.lanes_to_reset.contains(&LaneId::new(7)));
        assert_eq!(
            transaction
                .world
                .smart_contract_state()
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
            original_sns
        );
        validate_physical_retirement_accepted_world(
            None,
            &runtime,
            &original,
            &transaction.world,
            update,
            8,
            0,
        )
        .unwrap();
        let restored = transaction
            .canonical_runtime
            .get()
            .nexus_projection_with_catalog(&original, Some(&runtime))
            .unwrap();
        assert_eq!(restored.lane_catalog, transaction.nexus.lane_catalog);
        assert!(restored.routing_policy.rules.is_empty());
    });
}

#[test]
fn physical_retirement_refuses_wrong_owner_generation_roots_and_incomplete_geometry_atomically() {
    run_retirement_test(|| {
        let (state, owner, request) = retirement_fixture();
        for bad in [
            iroha_data_model::nexus::NexusCatalogTransitionV1 {
                expected_runtime_catalog_hash: Some(Hash::new(b"stale")),
                ..request.clone()
            },
            iroha_data_model::nexus::NexusCatalogTransitionV1 {
                dataspace_retirements: vec![RuntimeDataSpaceRetirementV1 {
                    expected_ownership_generation: 2,
                    ..request.dataspace_retirements[0].clone()
                }],
                ..request.clone()
            },
            iroha_data_model::nexus::NexusCatalogTransitionV1 {
                lane_retirements: vec![LaneId::new(6)],
                ..request.clone()
            },
        ] {
            let mut block = state.block(BlockHeader::new(
                NonZeroU64::new(8).unwrap(),
                None,
                None,
                0,
                0,
            ));
            let mut transaction = block.transaction();
            let original = transaction.nexus.clone();
            assert!(
                transaction
                    .stage_consensus_catalog_transition(&owner, &bad)
                    .is_err()
            );
            assert_eq!(transaction.nexus.lane_catalog, original.lane_catalog);
            assert!(
                runtime_catalog_from_world(&transaction.world)
                    .unwrap()
                    .is_none()
            );
            assert!(transaction.pending_lane_lifecycle.is_none());
        }
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(8).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        let other = AccountId::new(iroha_crypto::KeyPair::random().public_key().clone());
        assert!(
            transaction
                .stage_consensus_catalog_transition(&other, &request)
                .is_err()
        );
        assert!(
            runtime_catalog_from_world(&transaction.world)
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn physical_retirement_waits_for_original_custody_release_in_the_global_clock() {
    run_retirement_test(|| {
        use iroha_data_model::sumeragi_lanes::{SumeragiLaneCustody, SumeragiLaneFrontier};
        let (state, _, request) = retirement_fixture();
        let targets = BTreeSet::from([request.dataspace_retirements[0].dataspace_id]);
        let lanes = BTreeSet::from([LaneId::new(7)]);
        let mut world = state.world.block();
        let custody = SumeragiLaneCustody {
            lane: LaneId::new(7),
            incarnation: [3; 32],
            instance: [4; 32],
            created_at: 2,
            // A large native frontier cannot advance the global release deadline.
            merged: SumeragiLaneFrontier {
                height: 1_000_000,
                ..Default::default()
            },
            signer_count: 4,
            signers: Vec::new().try_into().unwrap(),
            evidence_horizon: 3,
            slashing_delay: 2,
            retired_at: Some(5),
        };
        custody.validate().unwrap();
        world.sumeragi_lanes.get_mut().custody.push(custody.clone());
        assert!(
            ensure_physical_retirement_closed(&world, &targets, &lanes, 9)
                .unwrap_err()
                .to_string()
                .contains("released original")
        );
        ensure_physical_retirement_closed(&world, &targets, &lanes, 10).unwrap();
        assert_eq!(world.sumeragi_lanes().custody, vec![custody]);
    });
}

#[test]
fn physical_retirement_rechecks_later_block_obligations_before_publication() {
    run_retirement_test(|| {
        use iroha_data_model::asset::{AssetBalanceScope, AssetDefinitionId, AssetId};
        let (state, owner, request) = retirement_fixture();
        let original = state.nexus_snapshot();
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(8).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&owner, &request)
            .unwrap();
        let runtime = runtime_catalog_from_world(&transaction.world)
            .unwrap()
            .unwrap();
        let asset = AssetId::with_scope(
            AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("issuer", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            owner,
            AssetBalanceScope::Dataspace(request.dataspace_retirements[0].dataspace_id),
        );
        transaction.world.assets.insert(
            asset.clone(),
            iroha_data_model::common::Owned::new(iroha_primitives::numeric::Quantity::from(1_u32)),
        );
        let update = &transaction
            .pending_lane_lifecycle
            .as_ref()
            .unwrap()
            .catalog_update;
        let error = validate_physical_retirement_accepted_world(
            None,
            &runtime,
            &original,
            &transaction.world,
            update,
            8,
            0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("nonzero scoped balance"));
        assert!(transaction.world.assets().get(&asset).is_some());
        assert_eq!(
            update.previous_storage_geometry,
            update.updated_storage_geometry
        );
    });
}
