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

#[test]
fn lifecycle_rebind_requires_materialized_manifest_source() {
    let nexus = iroha_config::parameters::actual::Nexus::default();
    let registry =
        LaneManifestRegistry::from_config(&nexus.lane_catalog, &nexus.governance, &nexus.registry);
    rebind_lane_manifests_for_lifecycle(&registry, &nexus.lane_catalog, &nexus.governance)
        .expect("materialized source rebinds to its exact catalog");
    let status_only = LaneManifestRegistry::from_statuses(BTreeMap::new());
    let error =
        rebind_lane_manifests_for_lifecycle(&status_only, &nexus.lane_catalog, &nexus.governance)
            .expect_err("status-only scaffolding cannot authorize a lifecycle transition");
    assert!(
        error.to_string().contains("materialized frozen source"),
        "unexpected lifecycle diagnostic: {error}"
    );
}

#[test]
fn materialized_manifest_install_rejects_status_only_without_publication() {
    run_catalog_test(|| {
        // Match production's unbound constructor before the first policy installation.
        let constructor_world = World::new();
        let state = State::try_new(
            constructor_world
                .sccp_committee_generations
                .allocation_budget()
                .clone(),
            constructor_world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            crate::telemetry::StateTelemetry::default(),
        )
        .unwrap();
        let nexus = state.nexus_snapshot();
        let original_manifests = state.lane_manifests.read().clone();
        let original_privacy = state.lane_privacy_registry.read().clone();
        let generation = state.state_view_generation();
        let status_only = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::new()));
        let error = state
            .install_materialized_lane_manifests_for_catalog(
                &status_only,
                &nexus.lane_catalog,
                &nexus.governance,
            )
            .expect_err("status-only registry cannot publish");
        assert!(error.to_string().contains("materialized frozen source"));
        assert_eq!(state.state_view_generation(), generation);
        assert!(Arc::ptr_eq(
            &state.lane_manifests.read(),
            &original_manifests
        ));
        assert!(LanePrivacyRegistry::ptr_eq(
            &state.lane_privacy_registry.read(),
            &original_privacy
        ));

        let materialized = Arc::new(LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &nexus.governance,
            &nexus.registry,
        ));
        state
            .install_materialized_lane_manifests_for_catalog(
                &materialized,
                &nexus.lane_catalog,
                &nexus.governance,
            )
            .expect("matching frozen source publishes");
        assert!(Arc::ptr_eq(&state.lane_manifests.read(), &materialized));
        assert_eq!(state.state_view_generation(), generation + 2);
        let installed_privacy = state.lane_privacy_registry.read().clone();
        let equivalent = Arc::new(materialized.rebind(&nexus.lane_catalog, &nexus.governance));
        state
            .install_materialized_lane_manifests_for_catalog(
                &equivalent,
                &nexus.lane_catalog,
                &nexus.governance,
            )
            .unwrap();
        assert_eq!(state.state_view_generation(), generation + 2);
        assert!(Arc::ptr_eq(&state.lane_manifests.read(), &materialized));
        assert!(LanePrivacyRegistry::ptr_eq(
            &state.lane_privacy_registry.read(),
            &installed_privacy
        ));

        // A genuinely different, valid initial source still uses an actual publication.
        let directory = tempfile::tempdir().unwrap();
        let alias = &nexus.lane_catalog.lanes()[0].alias;
        std::fs::write(
            directory.path().join(format!("{alias}.manifest.json")),
            norito::json::to_vec(&norito::json!({ "lane": alias })).unwrap(),
        )
        .unwrap();
        let changed = Arc::new(LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &nexus.governance,
            &iroha_config::parameters::actual::LaneRegistry {
                manifest_directory: Some(directory.path().to_path_buf()),
                ..Default::default()
            },
        ));
        state
            .install_materialized_lane_manifests_for_catalog(
                &changed,
                &nexus.lane_catalog,
                &nexus.governance,
            )
            .unwrap();
        assert_eq!(state.state_view_generation(), generation + 4);
        assert!(Arc::ptr_eq(&state.lane_manifests.read(), &changed));
        // Empty privacy has no physical shell. Source authority changed above;
        // both empty projections deliberately share the allocation-free identity.
        assert!(state.lane_privacy_registry.read().is_empty());
        assert!(installed_privacy.is_empty());
    });
}

#[test]
fn provisional_emergency_manifest_install_is_empty_and_one_shot() {
    run_catalog_test(|| {
        let mut strict_state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        assert!(
            strict_state
                .install_provisional_empty_lane_manifests_for_emergency_fast_pre_auth()
                .is_err(),
            "Strict startup must refuse the emergency pre-authentication installer"
        );
        let mut state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing_in_emergency_fast_mode(),
            LiveQueryStore::start_test(),
        );
        state
            .install_provisional_empty_lane_manifests_for_emergency_fast_pre_auth()
            .expect("unshared pre-authentication State admits one empty provisional install");
        let provisional = state.lane_manifests.read().clone();
        assert!(provisional.statuses().is_empty());
        assert!(
            provisional
                .validate_materialized_source_projection()
                .is_err()
        );
        let privacy = state.lane_privacy_registry.read().clone();
        let error = state
            .install_provisional_empty_lane_manifests_for_emergency_fast_pre_auth()
            .expect_err("emergency pre-authentication install must be one-shot");
        assert!(
            error
                .to_string()
                .contains("one-shot pre-authentication phase")
        );
        assert!(Arc::ptr_eq(&*state.lane_manifests.read(), &provisional));
        assert!(LanePrivacyRegistry::ptr_eq(&*state.lane_privacy_registry.read(), &privacy));
    });
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
    ExpiresAtNativeActivation,
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
    // These component transitions need the actual retained global schedule.
    // Structural handshake metadata alone never supplies proposal-height authority.
    let mut config =
        crate::sumeragi::test_chain::TestChainConfig::new(World::with([], accounts, []), 0);
    config.nexus = Some(nexus.clone());
    config.lane_manifests = Some(Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &nexus.registry,
    )));
    let genesis_account = AccountId::new(config.genesis_key.public_key().clone());
    let mode = config.consensus_mode;
    let prepared = crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect("prepare catalog fixture's original signed global genesis");
    let genesis = prepared.genesis.block().clone();
    // Acquire the unshared owner before any worker can retain it.
    let state = Arc::try_unwrap(prepared.state)
        .unwrap_or_else(|_| panic!("catalog fixture owns its unique unapplied State"));
    crate::sumeragi::startup::apply_genesis(
        &state,
        genesis.clone(),
        &genesis_account,
        mode.into(),
        None,
    )
    .expect("apply the original signed genesis and retain its exact schedule");
    assert_eq!(state.latest_block_hash_fast(), Some(genesis.hash()));
    assert_eq!(
        state.network_id,
        iroha_data_model::NetworkId::from_genesis_hash(genesis.hash())
    );
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
        // Runtime catalog additions activate a participant lane, whose peers
        // must carry live Committee keys rather than global Validator keys.
        let mut id = derive_committee_key_id(key.public_key());
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
            expiry_height: if index == 0 {
                match invalid {
                    InvalidMember::Expired => Some(3),
                    InvalidMember::ExpiresAtNativeActivation => Some(4),
                    _ => None,
                }
            } else {
                None
            },
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
    if matches!(invalid, InvalidMember::None) {
        for key in &keys {
            let peer = PeerId::new(key.public_key().clone());
            assert!(state.world.view().peers().iter().any(|p| p == &peer));
            assert!(peer_has_live_consensus_key_for_lane(
                &state.world.view(),
                &peer,
                3,
                LaneId::new(5),
            ));
        }
    }
    (state, keys)
}

fn catalog_payload(state: &State, keys: &[iroha_crypto::KeyPair]) -> NexusCatalogTransitionV1 {
    let hash = [0x67; 32];
    catalog_transition_for_testing(
        state,
        DataSpaceMetadata {
            id: DataSpaceId::from_hash(&hash),
            alias: "new-catalog-ds".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        hash,
        RuntimeLaneConfig {
            id: LaneId::new(5),
            alias: "new-catalog-lane".to_owned(),
            dataspace_id: DataSpaceId::from_hash(&hash),
            ..RuntimeLaneConfig::default()
        },
        &keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>(),
    )
}

/// Build an additive request from the current retained catalog and original committee peers.
/// The caller still submits this through normal accepted execution and certified publication.
pub(crate) fn catalog_transition_for_testing(
    state: &State,
    descriptor: DataSpaceMetadata,
    manifest_hash: [u8; 32],
    lane: RuntimeLaneConfig,
    peers: &[PeerId],
) -> NexusCatalogTransitionV1 {
    let nexus = state.nexus_snapshot();
    NexusCatalogTransitionV1 {
        version: NexusCatalogTransitionV1::VERSION,
        expected_catalog_hash: LaneLifecycleParameterV1::catalog_hash(&nexus.lane_catalog),
        expected_incarnation_root: lane_lifecycle_incarnation_root(
            &nexus.lane_catalog,
            &state.lane_incarnations_snapshot(),
        )
        .unwrap(),
        expected_runtime_catalog_hash: state.view().runtime_catalog_hash().unwrap(),
        dataspace_additions: vec![RuntimeDataSpaceAdditionV1 {
            descriptor,
            manifest_hash,
        }],
        manifest_additions: vec![catalog_manifest_for_peers(lane.id, &lane.alias, peers)],
        lane_additions: vec![lane],
    }
}

fn catalog_manifest(
    lane_id: LaneId,
    alias: &str,
    keys: &[iroha_crypto::KeyPair],
) -> RuntimeLaneManifestV1 {
    catalog_manifest_for_peers(
        lane_id,
        alias,
        &keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>(),
    )
}

fn catalog_manifest_for_peers(
    lane_id: LaneId,
    alias: &str,
    peers: &[PeerId],
) -> RuntimeLaneManifestV1 {
    let validators: Vec<_> = peers
        .iter()
        .map(|peer| {
            let validator = AccountId::new(peer.public_key().clone()).to_string();
            let peer_id = peer.to_string();
            norito::json!({ "validator": validator, "peer_id": peer_id })
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
fn runtime_catalog_fixture_retains_original_schedule_and_cannot_authorize_by_header() {
    run_catalog_test(|| {
        let (state, _) = catalog_fixture(InvalidMember::None);
        let original = state
            .kura_handle()
            .get_block(
                std::num::NonZeroUsize::new(1).unwrap(),
                &state.ivm_execution_budget(),
            )
            .expect("completed original State read")
            .expect("actual retained signed genesis");
        let header = catalog_test_header(&state);
        assert_eq!(header.prev_block_hash(), Some(original.hash()));
        assert!(header.creation_time() > original.header().creation_time());
        assert_eq!(
            crate::sumeragi::schedule::scheduled_committee(&state.world.view(), 2)
                .expect("original genesis owns H2 authority")
                .len(),
            4
        );
        assert_eq!(
            state.nexus_snapshot().lane_catalog.lanes().len(),
            5,
            "the complete configured physical baseline is retained"
        );
        let (missing, missing_keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&missing, &missing_keys);
        // Retain the actual signed parent and all four live participant keys,
        // but corrupt only the schedule being tested. A header cannot replace it.
        let mut world = missing.world.block();
        *world.consensus_schedule.get_mut() =
            crate::sumeragi::schedule::RetainedConsensusSchedule::default();
        world.commit();
        let before = norito::json::to_json(&missing.world).unwrap();
        let mut block = missing.block(catalog_test_header(&missing));
        let mut tx = block.transaction();
        let error = tx
            .stage_consensus_catalog_transition(&payload)
            .expect_err("an H2 header cannot supply an original consensus schedule");
        assert!(matches!(
            error,
            LaneLifecycleError::RuntimeCatalog(message)
                if message == "the stored consensus schedule is malformed"
        ));
        assert!(runtime_catalog_from_world(&tx.world).unwrap().is_none());
        assert!(tx.pending_lane_lifecycle.is_none());
        drop(tx);
        drop(block);
        assert_eq!(norito::json::to_json(&missing.world).unwrap(), before);
        assert_eq!(state.latest_block_hash_fast(), Some(original.hash()));
    });
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
            .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
                first.clone().into_custom_parameter().unwrap(),
            ))
            .expect("valid fixture parameters");
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
        let original_native_policy = crate::sumeragi::lanes::lane_policy(&state.world.view())
            .expect("completed original routing metadata read");
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
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
        let native = crate::sumeragi::lanes::lane_policy(&transaction.world)
            .expect("completed original routing metadata read")
            .expect("physical registration also stages its native lane policy");
        let fixed = native.fixed_lane(LaneId::new(5)).unwrap();
        assert_eq!(
            fixed.dataspace,
            payload.dataspace_additions[0].descriptor.id
        );
        assert_eq!(fixed.committee.len(), 4);
        crate::sumeragi::lanes::step::validate_policy(&native).unwrap();
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
        assert_eq!(
            crate::sumeragi::lanes::lane_policy(&block.world)
                .expect("completed original routing metadata read"),
            original_native_policy,
            "aborting the catalog transaction preserves the exact original native policy"
        );
    });
}

#[test]
fn runtime_catalog_activates_native_private_lane_and_routes_exact_dataspace() {
    run_catalog_test(|| {
        use crate::sumeragi::lanes::{
            lane_policy, merge::LaneStepInput, routing::RoutingInputs, step,
        };
        use iroha_data_model::{
            nexus::LaneVisibility,
            smart_contract::ContractAddress,
            transaction::{Executable, TransactionBuilder, executable::ContractInvocation},
        };
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let mut payload = catalog_payload(&state, &keys);
        payload.lane_additions[0].visibility = LaneVisibility::Restricted;
        let lane = payload.lane_additions[0].id;
        let dataspace = payload.lane_additions[0].dataspace_id;
        let mut block = state.block(catalog_test_header(&state));
        let params = block.world.parameters().sumeragi.clone();
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .unwrap();
        transaction.apply();
        step::advance(&mut block, &LaneStepInput::default()).unwrap();
        let mut policy = lane_policy(&block.world)
            .expect("completed original routing metadata read")
            .unwrap();
        assert_eq!(policy.lane_params, params);
        let record = block
            .world
            .sumeragi_lanes()
            .lane(lane)
            .expect("ordinary lane step creates instance");
        assert_eq!((record.created_at, record.active_from), (2, 4));
        assert_eq!(record.dataspace, dataspace);
        assert_eq!(record.committee, policy.fixed_lane(lane).unwrap().committee);
        assert!(record.closing.is_none());
        crate::sumeragi::lanes::lane_height_config(record).unwrap();
        assert_eq!(
            block
                .nexus
                .lane_catalog
                .lanes()
                .iter()
                .find(|entry| entry.id == lane)
                .unwrap()
                .visibility,
            LaneVisibility::Restricted
        );
        let authority = AccountId::new(keys[0].public_key().clone());
        let make_tx = |target| {
            let call = ContractInvocation {
                contract_address: ContractAddress::derive(&state.network_id, &authority, 0, target)
                    .unwrap(),
                expected_code_hash: Hash::new(b"catalog-routing-contract"),
                entrypoint: "call".to_owned(),
                arguments: None,
            };
            crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(
                TransactionBuilder::new(
                    state.network_id,
                    authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_executable(Executable::ContractCall(call))
                .sign(keys[0].private_key()),
            ))
        };
        // Even an explicit owner-wide route cannot capture registry/control-plane work
        // or change a concrete universal application target into private execution.
        policy
            .routes
            .push(iroha_data_model::sumeragi_lanes::SumeragiLaneRoute {
                lane,
                account: Some(authority.to_string()),
                instruction: None,
            });
        let inputs = RoutingInputs {
            root_scope: crate::sumeragi::lanes::routing::committed_root_scope(&block.world),
            policy: Some(&policy),
            lanes: block.world.sumeragi_lanes(),
            dataspaces: &block.nexus.dataspace_catalog,
            world: &block.world,
            ledger_time_ms: 0,
        };
        let private = make_tx(dataspace);
        assert_eq!(
            inputs
                .execution_route(&private, 4)
                .expect("completed original routing read"),
            None,
            "private work cannot escape to universal before activation is applied"
        );
        assert_eq!(
            inputs
                .execution_route(&private, 5)
                .expect("completed original routing read"),
            Some(crate::queue::RoutingDecision::new(lane, dataspace))
        );
        assert_eq!(
            inputs
                .execution_route(&make_tx(DataSpaceId::new(777)), 5)
                .expect("completed original routing read"),
            None
        );
        assert_eq!(
            inputs
                .execution_route(&make_tx(DataSpaceId::UNIVERSAL), 5)
                .expect("completed original routing read"),
            Some(crate::queue::RoutingDecision::new(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL
            )),
            "the same owner still routes its universal work to universal"
        );
        use iroha_data_model::{
            alias_setup::{
                AliasDataSpaceIntentV1, AliasDataspaceBootstrapGrantV1, AliasIntentV1,
                AliasLeaseAcquisitionV1, AliasQuoteGuardV1, ResolvedDataSpaceV1,
            },
            isi::{InstructionBox, SetParameter, alias_setup::EnsureAlias},
            parameter::Parameter,
        };
        let bootstrap =
            AliasDataspaceBootstrapGrantV1::try_new("new-catalog-ds", authority.clone()).unwrap();
        let controls: [InstructionBox; 3] = [
            SetParameter::new(Parameter::Custom(
                payload.clone().into_custom_parameter().unwrap(),
            ))
            .into(),
            SetParameter::new(Parameter::Custom(
                bootstrap.into_custom_parameter().unwrap(),
            ))
            .into(),
            EnsureAlias::new(
                AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                    dataspace: ResolvedDataSpaceV1::new(
                        "new-catalog-ds".parse().unwrap(),
                        dataspace,
                    ),
                    owner: authority.clone(),
                }),
                AliasLeaseAcquisitionV1::new(1, None),
                AliasQuoteGuardV1 {
                    expected_policy_version: 1,
                    expected_payment_asset:
                        iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
                            .parse()
                            .unwrap(),
                    max_amount: 1_u32.into(),
                    valid_until_ms: u64::MAX,
                },
            )
            .into(),
        ];
        for instruction in controls {
            let control = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(
                TransactionBuilder::new(
                    state.network_id,
                    authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([instruction])
                .sign(keys[0].private_key()),
            ));
            for height in 2..=5 {
                assert_eq!(
                    inputs
                        .execution_route(&control, height)
                        .expect("completed original routing read"),
                    Some(crate::queue::RoutingDecision::new(
                        LaneId::SINGLE,
                        DataSpaceId::UNIVERSAL,
                    )),
                    "all deployment phases stay global before and after native activation"
                );
            }
        }
        assert!(
            state.view().world().sumeragi_lanes().lane(lane).is_none(),
            "uncommitted block must not publish its native lane"
        );
    });
}

#[test]
fn runtime_catalog_rejects_native_lane_conflict_without_partial_state() {
    run_catalog_test(|| {
        use iroha_data_model::sumeragi_lanes::{
            SumeragiFixedLane, SumeragiLaneMember, SumeragiLanePolicy,
        };
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let mut policy = SumeragiLanePolicy::for_chain(
            block.world.parameters().sumeragi.clone(),
            iroha_sumeragi::availability::recommended_data_availability_layout(),
        );
        policy.fixed.push(SumeragiFixedLane {
            lane: LaneId::new(5),
            dataspace: DataSpaceId::new(99),
            committee: keys
                .iter()
                .map(|key| SumeragiLaneMember {
                    peer: PeerId::new(key.public_key().clone()),
                    pop: iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                })
                .collect(),
        });
        block
            .world
            .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
                policy.clone().into_custom_parameter(),
            ))
            .expect("valid fixture parameters");
        let before = block.world.parameters().clone();
        let mut transaction = block.transaction();
        transaction
            .stage_consensus_catalog_transition(&payload)
            .expect_err("conflicting native policy cannot be overwritten");
        assert_eq!(transaction.world.parameters(), &before);
        assert!(transaction.pending_lane_lifecycle.is_none());
        assert!(
            runtime_catalog_from_world(&transaction.world)
                .unwrap()
                .is_none()
        );
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
            InvalidMember::ExpiresAtNativeActivation,
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
            .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
                iroha_data_model::parameter::CustomParameter::new(
                    NexusRuntimeCatalogV1::parameter_id(),
                    iroha_primitives::json::Json::new(norito::json!({"version": 255})),
                ),
            ))
            .expect("valid fixture parameters");
        world.commit();
        assert!(runtime_catalog_from_world(&state.world.view()).is_err());
        assert!(runtime_catalog_root_from_world(&state.world.view()).is_err());
    });
}

include!("runtime_catalog_commit_tests.rs");

#[test]
fn runtime_catalog_preparation_preserves_cut_and_publication_allocates_nothing() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        let mut tx = block.transaction();
        tx.world.axt_authorization_transitioned.insert(DataSpaceId::new(10));
        let original = tx.world.parameters.get().clone();
        let canonical = tx.canonical_runtime.get().clone();
        let original_lanes_pointer = tx.canonical_runtime.lanes.as_ptr();
        let manifests = Arc::clone(&tx.lane_manifests);
        let privacy = tx.lane_privacy_registry.clone();
        let catalog = tx.nexus.lane_catalog.clone();
        let dataspaces = tx.world.dataspace_catalog.clone();
        let incarnations = tx.lane_incarnations.clone();
        let lineage = tx.lane_incarnation_lineage.clone();
        let activations = tx.lane_incarnation_activation_heights.clone();
        let transitioned = tx.world.axt_authorization_transitioned.clone();
        let pool = tx.world.sccp_committee_generations.allocation_budget().clone();
        let held = pool.reserved_bytes();

        // Abandoning complete preparation retains every original owner and cut.
        let abandoned = tx.prepare_consensus_catalog_transition(&payload).unwrap();
        assert!(pool.reserved_bytes() > held);
        drop(abandoned);
        assert_eq!(pool.reserved_bytes(), held);
        let prepared = tx.prepare_consensus_catalog_transition(&payload).unwrap();
        assert!(original.same_version(tx.world.parameters.get()));
        assert!(tx.world.parameters.touched_value().is_none());
        assert_eq!(tx.canonical_runtime.get(), &canonical);
        assert!(tx.canonical_runtime.touched_value().is_none());
        assert!(Arc::ptr_eq(&tx.lane_manifests, &manifests));
        assert!(LanePrivacyRegistry::ptr_eq(&tx.lane_privacy_registry, &privacy));
        assert_eq!(tx.nexus.lane_catalog, catalog);
        assert_eq!(tx.world.dataspace_catalog, dataspaces);
        assert_eq!(tx.lane_incarnations, incarnations);
        assert_eq!(tx.lane_incarnation_lineage, lineage);
        assert_eq!(tx.lane_incarnation_activation_heights, activations);
        assert_eq!(tx.world.axt_authorization_transitioned, transitioned);
        assert!(tx.pending_lane_lifecycle.is_none());

        let successor = prepared.parameters.clone();
        let successor_lanes_pointer = prepared.canonical_runtime.as_ref()
            .expect("the added lane changes canonical geometry").lanes.as_ptr();
        let successor_manifests = Arc::clone(&prepared.lane_manifests);
        let successor_privacy = prepared.lane_privacy_registry.clone();
        let prepared_held = pool.reserved_bytes();
        assert_eq!(
            crate::test_allocations::allocations_during(|| {
                tx.publish_consensus_catalog_transition(prepared);
            }),
            0,
            "the actual catalog publication suffix must move already prepared owners"
        );
        assert_eq!(pool.reserved_bytes(), prepared_held);
        assert!(successor.same_version(tx.world.parameters.get()));
        assert!(original.same_version(tx.world.parameters.get_before_transaction()));
        assert_eq!(tx.canonical_runtime.get_before_transaction(), &canonical);
        assert_eq!(tx.canonical_runtime.get_before_transaction().lanes.as_ptr(), original_lanes_pointer);
        assert_eq!(tx.canonical_runtime.lanes.as_ptr(), successor_lanes_pointer);
        assert!(Arc::ptr_eq(&tx.lane_manifests, &successor_manifests));
        assert!(LanePrivacyRegistry::ptr_eq(&tx.lane_privacy_registry, &successor_privacy));
        assert!(Arc::ptr_eq(
            &tx.pending_lane_lifecycle.as_ref().unwrap().updated_lane_manifests,
            &successor_manifests,
        ));
        assert_eq!(tx.world.axt_authorization_transitioned, transitioned);
        assert_eq!(tx.canonical_runtime.lanes, tx.nexus.lane_catalog.lanes());
        assert_eq!(tx.canonical_runtime.autoscale_sample_history, canonical.autoscale_sample_history);
        assert!(runtime_catalog_from_world(&tx.world).unwrap().is_some());
        drop(successor);
        drop(successor_privacy);
        drop(tx);
        assert_eq!(pool.reserved_bytes(), held);
        assert_eq!(crate::snapshot::canonical_staged_state_snapshot_bytes(&block), staged);
    });
}

#[test]
fn axt_incarnation_transition_preparation_preserves_existing_and_marks_exact_changes() {
    run_catalog_test(|| {
        let (state, _) = catalog_fixture(InvalidMember::None);
        let mut block = state.block(catalog_test_header(&state));
        let mut tx = block.transaction();
        for lane in 0..6 {
            tx.world.axt_policies.insert(DataSpaceId::new(u64::from(lane)), AxtPolicyEntry {
                manifest_root: [0x41; 32],
                target_lane: LaneId::new(lane),
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 0,
            });
        }
        tx.world.axt_authorization_transitioned.insert(DataSpaceId::new(10));
        let previous: BTreeMap<_, _> = [0, 1, 2, 4].into_iter()
            .map(|lane| (LaneId::new(lane), Hash::new([lane as u8])))
            .collect();
        let mut next = previous.clone();
        next.insert(LaneId::new(1), Hash::new([42]));
        next.remove(&LaneId::new(2));
        next.insert(LaneId::new(3), Hash::new([3]));
        let expected: BTreeSet<_> = [1, 2, 3, 10].into_iter().map(DataSpaceId::new).collect();
        assert_eq!(tx.world.prepare_axt_lane_incarnation_transitions(&previous, &next), expected);
        assert_eq!(tx.world.axt_authorization_transitioned, BTreeSet::from([DataSpaceId::new(10)]));
        tx.world.mark_axt_lane_incarnation_transitions(&previous, &next);
        assert_eq!(tx.world.axt_authorization_transitioned, expected);
        assert_eq!(tx.world.prepare_axt_lane_incarnation_transitions(&next, &next), expected);
        assert_eq!(crate::test_allocations::allocations_during(|| {
            tx.world.mark_axt_lane_incarnation_transitions(&next, &next);
        }), 0, "unchanged ordinary lifecycle keeps its original allocation-free path");
        assert_eq!(tx.world.axt_authorization_transitioned, expected);
    });
}

#[test]
fn runtime_catalog_second_parameter_refusal_keeps_complete_original_cut_and_retries() {
    run_catalog_test(|| {
        use crate::pipeline_gas_policy::RetainedStateParameters;
        use crate::test_allocations::refuse_layout_occurrence_during;
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let original_state = norito::json::to_json(&state.world).unwrap();
        let mut block = state.block(catalog_test_header(&state));
        for occurrence in [0, 1] {
            let mut tx = block.transaction();
            let original = tx.world.parameters.get().clone();
            let pool = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let held = pool.reserved_bytes();
            let catalog = tx.nexus.lane_catalog.clone();
            let dataspaces = tx.world.dataspace_catalog.clone();
            let manifests = Arc::clone(&tx.lane_manifests);
            let privacy = tx.lane_privacy_registry.clone();
            let canonical = tx.canonical_runtime.get().clone();
            let incarnations = tx.lane_incarnations.clone();
            let lineage = tx.lane_incarnation_lineage.clone();
            let activations = tx.lane_incarnation_activation_heights.clone();
            let transitioned = tx.world.axt_authorization_transitioned.clone();
            let events = (
                tx.world.external_event_buf.len(),
                tx.world.internal_event_buf.len(),
            );
            let (result, refused) = refuse_layout_occurrence_during(
                RetainedStateParameters::graph_allocation_layout_for_testing(),
                occurrence,
                || tx.stage_consensus_catalog_transition(&payload),
            );
            assert!(
                refused,
                "the real first/second parameter graph shell must be reached"
            );
            let Err(LaneLifecycleError::ParameterDeferred(reason)) = result else {
                panic!("preserve actual graph allocation refusal before catalog mutation");
            };
            assert!(reason.allocator_refusal().is_some());
            assert!(original.same_version(tx.world.parameters.get()));
            assert_eq!(tx.nexus.lane_catalog, catalog);
            assert_eq!(tx.world.dataspace_catalog, dataspaces);
            assert!(Arc::ptr_eq(&tx.lane_manifests, &manifests));
            assert!(LanePrivacyRegistry::ptr_eq(&tx.lane_privacy_registry, &privacy));
            assert_eq!(tx.canonical_runtime.get(), &canonical);
            assert!(tx.canonical_runtime.touched_value().is_none());
            assert!(tx.world.parameters.touched_value().is_none());
            assert_eq!(tx.lane_incarnations, incarnations);
            assert_eq!(tx.lane_incarnation_lineage, lineage);
            assert_eq!(tx.lane_incarnation_activation_heights, activations);
            assert_eq!(tx.world.axt_authorization_transitioned, transitioned);
            assert!(tx.pending_lane_lifecycle.is_none());
            assert_eq!(
                (
                    tx.world.external_event_buf.len(),
                    tx.world.internal_event_buf.len()
                ),
                events
            );
            drop(reason);
            assert_eq!(
                pool.reserved_bytes(),
                held,
                "both prepared graphs refund on refusal"
            );
        }
        {
            let mut retry = block.transaction();
            retry.stage_consensus_catalog_transition(&payload).unwrap();
            assert!(runtime_catalog_from_world(&retry.world).unwrap().is_some());
            let native = crate::sumeragi::lanes::lane_policy(&retry.world)
                .unwrap()
                .unwrap();
            assert!(native.fixed_lane(LaneId::new(5)).is_some());
            assert!(retry.pending_lane_lifecycle.is_some());
        }
        drop(block);
        assert_eq!(norito::json::to_json(&state.world).unwrap(), original_state);
    });
}

#[test]
fn actual_catalog_command_refusal_is_atomic_and_success_has_no_third_parameter_write() {
    run_catalog_test(|| {
        use crate::pipeline_gas_policy::RetainedStateParameters;
        use crate::smartcontracts::Execute as _;
        use crate::test_allocations::refuse_layout_occurrence_during;
        use iroha_data_model::{isi::SetParameter, parameter::Parameter};
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let command = payload.clone().into_custom_parameter().unwrap();
        let authority = AccountId::new(keys[0].public_key().clone());
        let mut block = state.block(catalog_test_header(&state));
        let original = block.world.parameters.get().clone();
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        let layout = RetainedStateParameters::graph_allocation_layout_for_testing();
        for occurrence in [0, 1] {
            let mut tx = block.transaction();
            let instruction = SetParameter::new(Parameter::Custom(command.clone()));
            let pool = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let held = pool.reserved_bytes();
            let events = (
                tx.world.external_event_buf.len(),
                tx.world.internal_event_buf.len(),
            );
            let (result, observed) = refuse_layout_occurrence_during(layout, occurrence, || {
                instruction.execute(&authority, &mut tx)
            });
            assert!(observed && result.is_err());
            let cause = tx
                .ensure_execution_ready()
                .expect_err("actual command keeps original sticky refusal");
            assert!(cause.allocator_refusal().is_some());
            assert!(cause.allocation_refusal().is_none());
            assert!(original.same_version(tx.world.parameters.get()));
            assert!(tx.pending_lane_lifecycle.is_none());
            assert!(!tx.lane_manifests.has_manifest(LaneId::new(5)));
            assert_eq!(
                (
                    tx.world.external_event_buf.len(),
                    tx.world.internal_event_buf.len()
                ),
                events
            );
            drop(cause);
            assert_eq!(pool.reserved_bytes(), held);
            drop(tx);
            assert_eq!(
                crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
                staged
            );
        }
        {
            let mut tx = block.transaction();
            let instruction = SetParameter::new(Parameter::Custom(command));
            let events = (
                tx.world.external_event_buf.len(),
                tx.world.internal_event_buf.len(),
            );
            let (result, third_attempted) = refuse_layout_occurrence_during(layout, 2, || {
                instruction.execute(&authority, &mut tx)
            });
            result.unwrap();
            assert!(
                !third_attempted,
                "a catalog command must not fall through to generic parameter storage"
            );
            assert!(!tx.is_execution_deferred());
            assert!(runtime_catalog_from_world(&tx.world).unwrap().is_some());
            assert!(
                crate::sumeragi::lanes::lane_policy(&tx.world)
                    .unwrap()
                    .unwrap()
                    .fixed_lane(LaneId::new(5))
                    .is_some()
            );
            assert!(
                tx.world
                    .parameters()
                    .custom()
                    .get(&NexusCatalogTransitionV1::parameter_id())
                    .is_none()
            );
            assert_eq!(
                (
                    tx.world.external_event_buf.len(),
                    tx.world.internal_event_buf.len()
                ),
                events
            );
        }
        assert_eq!(
            crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
            staged
        );
    });
}

#[test]
fn actual_catalog_command_decode_and_staging_budgets_preserve_refusal_and_retry() {
    run_catalog_test(|| {
        use crate::smartcontracts::Execute as _;
        use iroha_data_model::{isi::SetParameter, parameter::Parameter};
        use norito::core::{DecodeBudgetContext, DecodeLocalRefusalKind, DecodeResourceError};
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let command = payload.clone().into_custom_parameter().unwrap();
        let authority = AccountId::new(keys[0].public_key().clone());
        let unlimited =
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
        let oracle = DecodeBudgetContext::new(unlimited);
        let decoded = oracle
            .with(|| NexusCatalogTransitionV1::from_custom_parameter(&command))
            .unwrap()
            .unwrap();
        assert_eq!(decoded, payload);
        let command_bytes = usize::try_from(oracle.consumed_allocated_bytes()).unwrap();
        assert!(command_bytes > 0);
        drop(oracle);
        let allocation_limit = |bytes| {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        };
        // This exact limit permits the complete incoming command, including its
        // structural validation. The actual ISI must then preserve the original
        // refusal from the next model operation inside staging.
        assert!(
            norito::with_decode_limits_scope(allocation_limit(command_bytes), || {
                NexusCatalogTransitionV1::from_custom_parameter(&command)
            })
            .unwrap()
            .is_some()
        );
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        for (limits, work) in [
            (
                norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX),
                true,
            ),
            (allocation_limit(0), false),
            (allocation_limit(command_bytes), false),
        ] {
            let mut tx = block.transaction();
            let original = tx.world.parameters.get().clone();
            let pool = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let held = pool.reserved_bytes();
            let catalog = tx.nexus.lane_catalog.clone();
            let dataspaces = tx.world.dataspace_catalog.clone();
            let manifests = Arc::clone(&tx.lane_manifests);
            let events = (
                tx.world.external_event_buf.len(),
                tx.world.internal_event_buf.len(),
            );
            let instruction = SetParameter::new(Parameter::Custom(command.clone()));
            norito::with_decode_limits_scope(limits, || instruction.execute(&authority, &mut tx))
                .expect_err("actual catalog operation must preserve its original local refusal");
            let owner = tx
                .take_execution_deferral()
                .expect("sticky original catalog refusal");
            let original_refusal = owner
                .decode_refusal()
                .expect("captured original decoder")
                .clone();
            assert_eq!(
                original_refusal.kind(),
                DecodeLocalRefusalKind::EnclosingLimit
            );
            if work {
                assert!(matches!(
                    original_refusal.resource_error(),
                    DecodeResourceError::TotalElementsExceeded { limit: 0, .. }
                ));
            } else {
                assert!(
                    matches!(original_refusal.resource_error(), DecodeResourceError::TotalAllocationExceeded { limit, .. } if limit == limits.max_total_allocated_bytes() as u64)
                );
            }
            assert!(owner.allocation_refusal().is_none());
            assert!(owner.allocator_refusal().is_none());
            let returned = crate::execution_attempt::ExecutionDeferred::try_from_vm_error(
                owner.into_vm_error(),
            )
            .unwrap();
            assert_eq!(returned.decode_refusal(), Some(&original_refusal));
            assert!(
                tx.is_execution_deferred(),
                "moving the owner must not clear publication refusal"
            );
            assert!(original.same_version(tx.world.parameters.get()));
            assert_eq!(tx.nexus.lane_catalog, catalog);
            assert_eq!(tx.world.dataspace_catalog, dataspaces);
            assert!(Arc::ptr_eq(&tx.lane_manifests, &manifests));
            assert!(tx.pending_lane_lifecycle.is_none());
            assert_eq!(
                (
                    tx.world.external_event_buf.len(),
                    tx.world.internal_event_buf.len()
                ),
                events
            );
            assert_eq!(pool.reserved_bytes(), held);
            drop(tx);
            assert_eq!(
                crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
                staged
            );
            let mut retry = block.transaction();
            SetParameter::new(Parameter::Custom(command.clone()))
                .execute(&authority, &mut retry)
                .unwrap();
            assert!(!retry.is_execution_deferred());
            assert!(retry.pending_lane_lifecycle.is_some());
            assert!(runtime_catalog_from_world(&retry.world).unwrap().is_some());
            assert!(
                retry
                    .world
                    .parameters()
                    .custom()
                    .get(&NexusCatalogTransitionV1::parameter_id())
                    .is_none()
            );
            assert_eq!(
                (
                    retry.world.external_event_buf.len(),
                    retry.world.internal_event_buf.len()
                ),
                events
            );
            drop(retry);
            assert_eq!(
                crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
                staged
            );
        }
    });
}

#[test]
fn actual_catalog_invalid_json_version_and_protocol_bound_reject_without_deferral() {
    run_catalog_test(|| {
        use crate::smartcontracts::Execute as _;
        use iroha_data_model::{
            isi::{
                SetParameter,
                error::{InstructionExecutionError, InvalidParameterError},
            },
            nexus::NexusCatalogValidationError,
            parameter::{CustomParameter, Parameter},
        };
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let authority = AccountId::new(keys[0].public_key().clone());
        let mut unsupported = catalog_payload(&state, &keys);
        unsupported.version = 0;
        // Json owns canonical key ordering; the struct serializer's field order
        // is not itself a canonical Json input.
        let invalid_version = iroha_primitives::json::Json::new(unsupported);
        let entries =
            vec!["0"; iroha_data_model::nexus::MAX_NEXUS_RUNTIME_CATALOG_ENTRIES + 1].join(",");
        let intrinsic_bound = format!("{{\"oversized\":[{entries}]}}").parse().unwrap();
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        for (case, payload) in ["{}".parse().unwrap(), invalid_version, intrinsic_bound]
            .into_iter()
            .enumerate()
        {
            let custom = CustomParameter::new(NexusCatalogTransitionV1::parameter_id(), payload);
            let expected = NexusCatalogTransitionV1::from_custom_parameter(&custom).unwrap_err();
            match case {
                0 => assert!(matches!(&expected, NexusCatalogValidationError::Json(_))),
                1 => assert!(matches!(
                    &expected,
                    NexusCatalogValidationError::UnsupportedVersion(0)
                )),
                _ => assert!(matches!(
                    &expected,
                    NexusCatalogValidationError::Preflight(_)
                )),
            }
            let message = format!("invalid Nexus catalog transition payload: {expected}");
            let mut tx = block.transaction();
            let original = tx.world.parameters.get().clone();
            let events = (
                tx.world.external_event_buf.len(),
                tx.world.internal_event_buf.len(),
            );
            let error = SetParameter::new(Parameter::Custom(custom))
                .execute(&authority, &mut tx)
                .unwrap_err();
            assert!(
                matches!(error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(actual)) if actual == message)
            );
            assert!(!tx.is_execution_deferred());
            assert!(tx.ensure_execution_ready().is_ok());
            assert!(original.same_version(tx.world.parameters.get()));
            assert!(tx.pending_lane_lifecycle.is_none());
            assert_eq!(
                (
                    tx.world.external_event_buf.len(),
                    tx.world.internal_event_buf.len()
                ),
                events
            );
            drop(tx);
            assert_eq!(
                crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
                staged
            );
        }
    });
}

#[test]
fn protected_catalog_reader_refusal_keeps_original_staged_state_and_retries() {
    run_catalog_test(|| {
        use norito::core::{DecodeLocalRefusalKind, DecodeResourceError};
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        {
            let mut tx = block.transaction();
            tx.stage_consensus_catalog_transition(&payload).unwrap();
            let expected = runtime_catalog_from_world(&tx.world).unwrap().unwrap();
            let original = tx.world.parameters.get().clone();
            let catalog = tx.nexus.lane_catalog.clone();
            let dataspaces = tx.world.dataspace_catalog.clone();
            let manifests = Arc::clone(&tx.lane_manifests);
            let events = (
                tx.world.external_event_buf.len(),
                tx.world.internal_event_buf.len(),
            );
            let pool = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let held = pool.reserved_bytes();
            for (limits, work) in [
                (
                    norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX),
                    true,
                ),
                (
                    norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
                    false,
                ),
            ] {
                let error = norito::with_decode_limits_scope(limits, || {
                    runtime_catalog_from_world(&tx.world)
                })
                .unwrap_err();
                let LaneLifecycleError::ParameterDeferred(original_refusal) = error else {
                    panic!("retained catalog resource shortage must remain local");
                };
                let cause = original_refusal.decode_refusal().unwrap();
                assert_eq!(cause.kind(), DecodeLocalRefusalKind::EnclosingLimit);
                if work {
                    assert!(matches!(
                        cause.resource_error(),
                        DecodeResourceError::TotalElementsExceeded { limit: 0, .. }
                    ));
                } else {
                    assert!(matches!(
                        cause.resource_error(),
                        DecodeResourceError::TotalAllocationExceeded { limit: 0, .. }
                    ));
                }
                assert!(original.same_version(tx.world.parameters.get()));
                assert_eq!(tx.nexus.lane_catalog, catalog);
                assert_eq!(tx.world.dataspace_catalog, dataspaces);
                assert!(Arc::ptr_eq(&tx.lane_manifests, &manifests));
                assert!(tx.pending_lane_lifecycle.is_some());
                assert_eq!(
                    (
                        tx.world.external_event_buf.len(),
                        tx.world.internal_event_buf.len()
                    ),
                    events
                );
                assert_eq!(pool.reserved_bytes(), held);
                assert_eq!(
                    runtime_catalog_from_world(&tx.world).unwrap(),
                    Some(expected.clone())
                );
            }
        }
        assert_eq!(
            crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
            staged
        );
    });
}

#[test]
fn catalog_observer_rejects_inner_protocol_reconstructed_and_stale_resource_errors() {
    use crate::execution_attempt::ExecutionAttemptError;
    use iroha_data_model::nexus::NexusCatalogValidationError;
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let intrinsic = nexus_catalog_validation_attempt(|| {
        norito::with_decode_limits_scope(limits, || norito::core::reserve_decode_allocation(1))
            .map_err(NexusCatalogValidationError::Codec)
    });
    assert!(matches!(
        intrinsic,
        Err(ExecutionAttemptError::Rejected(
            NexusCatalogValidationError::Codec(norito::Error::ScopedDecodeResource(_))
        ))
    ));
    let stale = norito::with_decode_limits_scope(limits, || {
        norito::core::classify_decode_attempt(|| norito::core::reserve_decode_allocation(1))
    })
    .unwrap_err()
    .into_error();
    let reconstructed = norito::Error::TotalAllocationExceeded {
        attempted: 1,
        limit: 0,
    };
    for error in [stale, reconstructed] {
        let fields = error.decode_resource_error();
        let text = error.to_string();
        let result = norito::with_decode_limits_scope(limits, || {
            nexus_catalog_validation_attempt(|| {
                let _fresh = norito::core::reserve_decode_allocation(1).unwrap_err();
                Err::<(), _>(NexusCatalogValidationError::Codec(error))
            })
        });
        let Err(ExecutionAttemptError::Rejected(NexusCatalogValidationError::Codec(returned))) =
            result
        else {
            panic!("foreign or reconstructed fields cannot borrow the fresh refusal's authority");
        };
        assert_eq!(returned.decode_resource_error(), fields);
        assert_eq!(returned.to_string(), text);
    }
}


#[test]
fn runtime_catalog_publication_after_prior_cell_writes_allocates_nothing_and_refunds_intermediate() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        let mut tx = block.transaction();
        let pool = tx.world.sccp_committee_generations.allocation_budget().clone();
        let original = tx.world.parameters.get().clone();
        let canonical = tx.canonical_runtime.get().clone();
        let held = pool.reserved_bytes();
        // Build a distinct, fully admitted current owner through the existing
        // source-bound fixture path; publication must actually retire this graph.
        let intermediate = original.prepare_replace(original.get(), &pool).unwrap()
            .into_successor(&original, &pool).unwrap();
        assert!(!intermediate.same_version(&original));
        let intermediate_bytes = pool.reserved_bytes() - held;
        assert!(intermediate_bytes > 0);
        *tx.world.parameters.get_mut() = intermediate;
        let _ = tx.canonical_runtime.get_mut();
        let original_lanes = tx.canonical_runtime.get_before_transaction().lanes.as_ptr();
        assert!(tx.world.parameters.touched_value().is_some());
        assert!(tx.canonical_runtime.touched_value().is_some());
        let prepared = tx.prepare_consensus_catalog_transition(&payload).unwrap();
        let before_publication = pool.reserved_bytes();
        assert_eq!(crate::test_allocations::allocations_during(|| {
            tx.publish_consensus_catalog_transition(prepared);
        }), 0, "retiring an intermediate graph under the real World refund scope must not allocate");
        assert_eq!(pool.reserved_bytes(), before_publication - intermediate_bytes,
            "the intermediate is not retained by a census-only shadow owner");
        assert!(original.same_version(tx.world.parameters.get_before_transaction()));
        assert!(original.same_version(tx.world.parameters.get_before_block()));
        assert_eq!(tx.canonical_runtime.get_before_transaction(), &canonical);
        assert_eq!(tx.canonical_runtime.get_before_transaction().lanes.as_ptr(), original_lanes);
        assert!(tx.pending_lane_lifecycle.is_some());
        drop(tx);
        assert_eq!(pool.reserved_bytes(), held);
        assert_eq!(crate::snapshot::canonical_staged_state_snapshot_bytes(&block), staged);
    });
}


#[test]
fn actual_runtime_manifest_registry_refusal_preserves_catalog_cut_and_retries() {
    run_catalog_test(|| {
        use norito::core::{DecodeBudgetContext, DecodeLocalRefusalKind, DecodeResourceError};
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        let mut tx = block.transaction();
        // Use the exact effective geometry and cumulative manifests from real
        // successful preparation, with the original retained baseline registry.
        let prepared = tx.prepare_consensus_catalog_transition(&payload).unwrap();
        let additions = &prepared.pending_lane_lifecycle.runtime_catalog.as_ref().unwrap().manifests;
        let reconstruct = || runtime_catalog_manifest_registry(
            tx.lane_manifests.as_ref(),
            additions,
            &prepared.nexus.lane_catalog,
            &prepared.nexus.dataspace_catalog,
            &prepared.nexus.governance,
        );
        let unlimited = norito::DecodeLimits::new(
            usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX,
        );
        let oracle = DecodeBudgetContext::new(unlimited);
        let registry = oracle.with(reconstruct).expect("same actual loader succeeds");
        assert!(registry.has_same_authority_as(&prepared.lane_manifests));
        let bytes = usize::try_from(oracle.consumed_allocated_bytes()).unwrap();
        assert!(bytes > 0, "observe real manifest parser/writer allocations");
        drop(oracle);
        drop(registry);
        let original = tx.world.parameters.get().clone();
        let canonical = tx.canonical_runtime.get().clone();
        let catalog = tx.nexus.lane_catalog.clone();
        let dataspaces = tx.world.dataspace_catalog.clone();
        let manifests = Arc::clone(&tx.lane_manifests);
        let privacy = tx.lane_privacy_registry.clone();
        let pool = tx.world.sccp_committee_generations.allocation_budget().clone();
        let held = pool.reserved_bytes();
        for (limits, work) in [
            (norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX), true),
            (norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX), false),
            (norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes - 1, usize::MAX), false),
        ] {
            let result = norito::with_decode_limits_scope(limits, reconstruct);
            let Err(LaneLifecycleError::ParameterDeferred(owner)) = result else {
                panic!("the real manifest operation must retain its original local refusal");
            };
            let original_refusal = owner.decode_refusal().expect("actual codec owner").clone();
            assert_eq!(original_refusal.kind(), DecodeLocalRefusalKind::EnclosingLimit);
            if work {
                assert!(matches!(original_refusal.resource_error(),
                    DecodeResourceError::TotalElementsExceeded { limit: 0, .. }));
            } else {
                assert!(matches!(original_refusal.resource_error(),
                    DecodeResourceError::TotalAllocationExceeded { limit, .. }
                        if limit == limits.max_total_allocated_bytes() as u64));
            }
            assert!(owner.allocation_refusal().is_none());
            assert!(owner.allocator_refusal().is_none());
            let returned = crate::execution_attempt::ExecutionDeferred::try_from_vm_error(
                owner.into_vm_error(),
            ).unwrap();
            assert_eq!(returned.decode_refusal(), Some(&original_refusal));
            assert!(original.same_version(tx.world.parameters.get()));
            assert!(tx.world.parameters.touched_value().is_none());
            assert_eq!(tx.canonical_runtime.get(), &canonical);
            assert!(tx.canonical_runtime.touched_value().is_none());
            assert_eq!(tx.nexus.lane_catalog, catalog);
            assert_eq!(tx.world.dataspace_catalog, dataspaces);
            assert!(Arc::ptr_eq(&tx.lane_manifests, &manifests));
            assert!(LanePrivacyRegistry::ptr_eq(&tx.lane_privacy_registry, &privacy));
            assert!(tx.pending_lane_lifecycle.is_none());
            assert!(!tx.is_execution_deferred(), "read-only preparation does not fabricate an execution owner");
            assert_eq!(pool.reserved_bytes(), held);
            let retry = reconstruct().expect("identical sources succeed under fresh local capacity");
            assert!(retry.has_same_authority_as(&prepared.lane_manifests));
            assert!(retry.is_bound_to_catalog(&prepared.nexus.lane_catalog));
        }
        drop(prepared);
        tx.stage_consensus_catalog_transition(&payload).unwrap();
        assert!(tx.pending_lane_lifecycle.is_some());
        drop(tx);
        assert_eq!(crate::snapshot::canonical_staged_state_snapshot_bytes(&block), staged);
    });
}

#[test]
fn actual_runtime_manifest_registry_invalid_json_and_binding_reject_without_deferral() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let staged = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
        let tx = block.transaction();
        let prepared = tx.prepare_consensus_catalog_transition(&payload).unwrap();
        for manifest in [
            iroha_primitives::json::Json::new(norito::json!({"lane": 7})),
            catalog_manifest(LaneId::new(5), "wrong-effective-alias", &keys).manifest,
        ] {
            let mut additions = payload.manifest_additions.clone();
            additions[0].manifest = manifest;
            let result = runtime_catalog_manifest_registry(
                tx.lane_manifests.as_ref(),
                &additions,
                &prepared.nexus.lane_catalog,
                &prepared.nexus.dataspace_catalog,
                &prepared.nexus.governance,
            );
            assert!(matches!(result, Err(LaneLifecycleError::RuntimeCatalog(_))));
            assert!(!tx.is_execution_deferred());
            assert!(tx.pending_lane_lifecycle.is_none());
            assert!(tx.world.parameters.touched_value().is_none());
            assert!(tx.canonical_runtime.touched_value().is_none());
        }
        drop(prepared);
        drop(tx);
        assert_eq!(crate::snapshot::canonical_staged_state_snapshot_bytes(&block), staged);
    });
}

#[test]
fn materialized_privacy_admission_refuses_before_publication_and_reuses_installed_owner() {
    run_catalog_test(|| {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let nexus = state.nexus_snapshot();
        let directory = tempfile::tempdir().unwrap();
        let lane = &nexus.lane_catalog.lanes()[0];
        let alias = lane.alias.clone();
        let body = norito::json!({
            "lane": alias,
            "privacy_commitments": [{
                "id": 1, "scheme": "merkle", "merkle": {
                    "root": "1212121212121212121212121212121212121212121212121212121212121212",
                    "max_depth": 16,
                },
            }],
        });
        std::fs::write(
            directory.path().join(format!("{}.manifest.json", lane.alias)),
            norito::json::to_vec(&body).unwrap(),
        ).unwrap();
        let source = Arc::new(LaneManifestRegistry::from_config(
            &nexus.lane_catalog, &nexus.governance,
            &iroha_config::parameters::actual::LaneRegistry {
                manifest_directory: Some(directory.path().to_path_buf()), ..Default::default()
            },
        ));
        source.validate_active_coverage_for_catalog(&nexus.lane_catalog).unwrap();
        source.canonical_materialized_authority_preimage(&nexus.lane_catalog, &nexus.governance).unwrap();
        assert!(!source.status(lane.id).unwrap().privacy_commitments.is_empty());
        let pool = state.ivm_execution_budget();
        let limit = pool.limit_bytes();
        let held = pool.reserved_bytes();
        let generation = state.state_view_generation();
        let original_manifests = state.lane_manifests.read().clone();
        let original_privacy = state.lane_privacy_registry.read().clone();
        pool.set_limit_bytes(held);
        let error = state.install_materialized_lane_manifests_for_catalog(
            &source, &nexus.lane_catalog, &nexus.governance,
        ).unwrap_err();
        let LaneLifecycleError::ParameterDeferred(original) = error else { panic!("actual privacy refusal stays local") };
        assert!(matches!(original.allocation_refusal(), Some(iroha_allocation::AllocationRefusal::Capacity { .. })));
        assert_eq!(state.state_view_generation(), generation);
        assert!(Arc::ptr_eq(&state.lane_manifests.read(), &original_manifests));
        assert!(LanePrivacyRegistry::ptr_eq(&state.lane_privacy_registry.read(), &original_privacy));
        assert_eq!(pool.reserved_bytes(), held);
        pool.set_limit_bytes(limit);
        let installed = state.install_materialized_lane_manifests_for_catalog(
            &source, &nexus.lane_catalog, &nexus.governance,
        ).unwrap();
        assert!(installed.belongs_to(&pool));
        assert!(LanePrivacyRegistry::ptr_eq(&installed, &state.lane_privacy_registry.read()));
        assert_eq!(state.state_view_generation(), generation + 2);
        let installed_held = pool.reserved_bytes();
        pool.set_limit_bytes(installed_held);
        let equivalent = Arc::new(source.rebind(&nexus.lane_catalog, &nexus.governance));
        let handoff = state.install_materialized_lane_manifests_for_catalog(
            &equivalent, &nexus.lane_catalog, &nexus.governance,
        ).unwrap();
        let refreshed = state.install_lane_manifests_if_consensus_compatible(&equivalent).unwrap();
        assert!(LanePrivacyRegistry::ptr_eq(&installed, &handoff));
        assert!(LanePrivacyRegistry::ptr_eq(&installed, &refreshed));
        assert_eq!(pool.reserved_bytes(), installed_held, "startup and refresh share one graph");
        assert_eq!(state.state_view_generation(), generation + 2);
        pool.set_limit_bytes(limit);
    });
}

#[test]
fn prepared_privacy_owner_survives_transaction_apply_and_carrier_preparation_without_rebuild() {
    run_catalog_test(|| {
        let (state, keys) = catalog_fixture(InvalidMember::None);
        let payload = catalog_payload(&state, &keys);
        let mut block = state.block(catalog_test_header(&state));
        let pool = state.ivm_execution_budget();
        let mut tx = block.transaction();
        tx.stage_consensus_catalog_transition(&payload).unwrap();
        let prepared = tx.lane_privacy_registry.clone();
        assert!(!prepared.is_empty());
        assert!(prepared.belongs_to(&pool));
        assert!(LanePrivacyRegistry::ptr_eq(
            &prepared, &tx.pending_lane_lifecycle.as_ref().unwrap().updated_lane_privacy,
        ));
        tx.apply();
        assert!(LanePrivacyRegistry::ptr_eq(&prepared, &block.lane_privacy_registry));
        let pending = block.pending_autoscale_lifecycle.as_ref().unwrap();
        assert!(LanePrivacyRegistry::ptr_eq(&prepared, &pending.updated_lane_privacy));
        let effects = carrier_lifecycle_effects::PreparedLaneLifecycleEffects::prepare(pending, &block.nexus);
        // Its private owner is checked in that module's own test-only accessor.
        assert!(effects.privacy_is_for_test(&prepared));
        assert!(state.lane_privacy_registry.read().is_empty());
    });
}
