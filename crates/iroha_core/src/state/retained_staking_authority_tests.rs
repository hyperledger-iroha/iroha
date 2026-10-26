// Staking selection reads one caller-retained monetary cut without local mirrors.

#[test]
fn new_autoscale_committee_uses_original_world_minimum_before_pinning() {
    let lane_id = LaneId::new(1);
    let base = LaneConfig::default();
    let lane = autoscale_elastic_lane_config_from_base(lane_id, &base, 1).unwrap();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.autoscale.enabled = true;
    nexus.autoscale.min_lane_id = NonZeroU32::new(1).unwrap();
    nexus.autoscale.max_lane_id_exclusive = NonZeroU32::new(2).unwrap();
    nexus.staking.public_validator_mode = LaneValidatorMode::StakeElected;
    nexus.lane_catalog = LaneCatalog::new(NonZeroU32::new(2).unwrap(), vec![base, lane]).unwrap();
    nexus.lane_config = DerivedLaneConfig::from_catalog(&nexus.lane_catalog);
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"authored-autoscale-minimum")),
    );
    let manifests = LaneManifestRegistry::default();
    for minimum in [None, Some(100), Some(101)] {
        let mut world = World::default();
        if let Some(minimum) = minimum {
            set_staking_minimum_for_test(&mut world, minimum);
        }
        let mut block = world.block();
        let mut expected = Vec::new();
        for seed in 81..85 {
            let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap();
            let peer = seed_active_public_lane_validator(&mut block, &key, LaneId::SINGLE, 100);
            seed_consensus_key_with_role(
                &mut block,
                &peer,
                ConsensusKeyRole::Committee,
                ConsensusKeyStatus::Active,
                0,
            );
            expected.push(peer);
        }
        block.commit();
        expected.sort();
        let selected = State::derive_new_autoscale_lane_committee_from_sources(
            &world.view(),
            &network,
            lane_id,
            &manifests,
            &nexus,
            1,
        );
        if minimum == Some(100) {
            assert_eq!(
                selected, expected,
                "an exact four-member original stake pool needs no beacon"
            );
        } else {
            assert!(
                selected.is_empty(),
                "missing or insufficient original stake cannot create a committee"
            );
        }
    }
}

fn monetary_selection_fixture(minimum: Option<u64>) -> World {
    let mut world = World::default();
    if let Some(minimum) = minimum {
        set_staking_minimum_for_test(&mut world, minimum);
    }
    let mut block = world.block();
    for (seed, stake) in [(71, 100), (72, 200), (73, 300)] {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("deterministic original-cut validator");
        seed_active_public_lane_validator(&mut block, &key, LaneId::SINGLE, stake);
    }
    block.commit();
    world
}

fn assert_monetary_selection_count(world: &impl WorldReadOnly, expected: usize) {
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.staking.public_validator_mode = LaneValidatorMode::StakeElected;
    let epoch = epoch_validator_candidate_peer_ids_from_world(world, Vec::new(), 1, &nexus, None);
    assert_eq!(
        epoch.len(),
        expected,
        "epoch threshold uses the supplied World cut"
    );
    let mut inputs = lane_authority::inputs_from_nexus(
        LaneId::SINGLE,
        LaneValidatorMode::StakeElected,
        &nexus,
        1,
    )
    .expect("active fixture route");
    let manifests = LaneManifestRegistry::default();
    let peers =
        lane_authority::peer_pool_with_inputs(world, LaneId::SINGLE, &manifests, &inputs, 1)
            .expect("valid original monetary source");
    let accounts = lane_authority::validator_accounts_with_inputs(
        world,
        LaneId::SINGLE,
        &manifests,
        &inputs,
        1,
    );
    assert_eq!(peers.len(), expected);
    assert_eq!(accounts.len(), expected);
    assert_eq!(
        peers.into_iter().collect::<BTreeSet<_>>(),
        epoch.iter().cloned().collect()
    );
    // Exercise the data-space-wide branch with the same retained source,
    // including an empty sibling projection, rather than only the direct lane.
    inputs.staking_authority_lanes.insert(LaneId::new(1));
    let shared =
        lane_authority::peer_pool_with_inputs(world, LaneId::SINGLE, &manifests, &inputs, 1)
            .expect("one nonempty canonical staking projection");
    assert_eq!(
        shared.into_iter().collect::<BTreeSet<_>>(),
        epoch.into_iter().collect()
    );
    assert_eq!(
        lane_authority::validator_accounts_with_inputs(
            world,
            LaneId::SINGLE,
            &manifests,
            &inputs,
            1,
        )
        .len(),
        expected
    );
}

#[test]
fn monetary_selection_requires_initialized_original_policy() {
    let world = monetary_selection_fixture(None);
    let view = world.view();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.staking.public_validator_mode = LaneValidatorMode::StakeElected;
    assert!(
        epoch_validator_candidate_peer_ids_from_world(&view, Vec::new(), 1, &nexus, None,)
            .is_empty(),
        "live stake and keys cannot synthesize missing monetary policy"
    );
    let mut inputs = lane_authority::inputs_from_nexus(
        LaneId::SINGLE,
        LaneValidatorMode::StakeElected,
        &nexus,
        1,
    )
    .unwrap();
    let manifests = LaneManifestRegistry::default();
    assert!(
        lane_authority::peer_pool_with_inputs(&view, LaneId::SINGLE, &manifests, &inputs, 1)
            .is_err()
    );
    assert!(
        lane_authority::validator_accounts_with_inputs(
            &view,
            LaneId::SINGLE,
            &manifests,
            &inputs,
            1
        )
        .is_empty()
    );
    inputs.staking_authority_lanes.insert(LaneId::new(1));
    assert!(
        lane_authority::peer_pool_with_inputs(&view, LaneId::SINGLE, &manifests, &inputs, 1)
            .is_err()
    );
    assert!(
        lane_authority::validator_accounts_with_inputs(
            &view,
            LaneId::SINGLE,
            &manifests,
            &inputs,
            1
        )
        .is_empty()
    );
}

#[test]
fn monetary_selection_keeps_retained_threshold_across_pending_and_published_update() {
    let world = monetary_selection_fixture(Some(200));
    let original = world.view();
    assert_monetary_selection_count(&original, 2);
    let mut successor = original
        .parameters()
        .require_staking_policy()
        .unwrap()
        .clone();
    successor.min_validator_stake = 300_u64.into();
    let mut block = world.block();
    let mut transaction = block
        .try_transaction(
            #[cfg(feature = "telemetry")]
            None,
            iroha_config::parameters::actual::LaneConfig::default(),
            0,
        )
        .expect("original World pool admits the transaction checkpoint");
    transaction
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            successor.into_custom_parameter().unwrap(),
        ))
        .expect("admit complete successor before publication");
    assert_monetary_selection_count(&transaction, 1);
    assert_monetary_selection_count(&original, 2);
    transaction.apply();
    assert_monetary_selection_count(&original, 2);
    block.commit();
    assert_monetary_selection_count(&original, 2);
    assert_monetary_selection_count(&world.view(), 1);
}

#[test]
fn pinned_autoscale_authority_does_not_reapply_current_monetary_threshold() {
    let world = monetary_selection_fixture(None);
    let view = world.view();
    let nexus = iroha_config::parameters::actual::Nexus::default();
    let mut inputs = lane_authority::inputs_from_nexus(
        LaneId::SINGLE,
        LaneValidatorMode::StakeElected,
        &nexus,
        1,
    )
    .unwrap();
    let pin = view.peers().iter().cloned().collect::<Vec<_>>();
    inputs.bounded.autoscale_validator_set = Some(pin.clone());
    assert_eq!(
        lane_authority::peer_pool_with_inputs(
            &view,
            LaneId::SINGLE,
            &LaneManifestRegistry::default(),
            &inputs,
            1,
        )
        .expect("retained incarnation authority remains exact"),
        pin
    );
}
