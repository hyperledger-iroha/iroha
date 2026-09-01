// First-release exact lane-route committee regressions.

fn exact_manifest_authority_fixture(
    fault_tolerance: u32,
    validator_count: u8,
) -> (State, Vec<KeyPair>) {
    let state = blank_test_state();
    let mut nexus = state.nexus_snapshot();
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::UNIVERSAL,
        alias: "universal".to_owned(),
        description: None,
        fault_tolerance,
    }])
    .expect("exact-authority dataspace catalog");
    install_existing_nexus_geometry_for_test(&state, nexus);
    let keypairs = (1..=validator_count)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic exact-authority BLS key")
        })
        .collect::<Vec<_>>();
    seed_consensus_keys_with_pops(&state, &keypairs);
    install_lane_manifest_registry_for_keypairs(&state, &[LaneId::SINGLE], &keypairs);
    (state, keypairs)
}

fn exact_stake_authority_fixture(
    fault_tolerance: u32,
    validator_count: u8,
) -> (State, Vec<KeyPair>) {
    let state = blank_test_state();
    let mut nexus = state.nexus_snapshot();
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::UNIVERSAL,
        alias: "universal".to_owned(),
        description: None,
        fault_tolerance,
    }])
    .expect("exact stake-authority dataspace catalog");
    install_existing_nexus_geometry_for_test(&state, nexus);
    let keypairs = (1..=validator_count)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic exact stake-authority BLS key")
        })
        .collect::<Vec<_>>();
    seed_consensus_keys_with_pops(&state, &keypairs);
    let minimum_stake = state.nexus_snapshot().staking.min_validator_stake.clone();
    let mut world = state.world.block();
    for keypair in &keypairs {
        let validator = AccountId::new(keypair.public_key().clone());
        world.public_lane_validators.insert(
            (LaneId::SINGLE, validator.clone()),
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: validator.clone(),
                peer_id: PeerId::new(keypair.public_key().clone()),
                stake_account: validator,
                total_stake: minimum_stake.clone(),
                self_stake: minimum_stake.clone(),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_height: 1,
                deactivation_height: None,
                last_reward_epoch: None,
            },
        );
    }
    world.commit();
    (state, keypairs)
}

fn install_malformed_beacon_cursor(state: &State) {
    // A non-empty cursor disables the unit-fixture entropy fallback, while the
    // absent backing pulse makes any attempted seed resolution fail closed.
    let mut world = state.world.block();
    world.global_beacon_latest_pulse.insert(
        GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
        crate::beacon::GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0xA1; 32],
            seed: [0xB2; 32],
            height: 0,
            round: 0,
        },
    );
    world.commit();
}

fn resolve_universal_committee(
    state: &State,
) -> Result<LaneAuthorityCommittee, LaneAuthorityError> {
    state.resolve_lane_committee_at_height(
        LaneAuthorityRoute::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        1,
    )
}

fn private_settlement_authority_for_keys(
    state: &State,
    authority_height: u64,
    keypairs: &[KeyPair],
) -> iroha_data_model::nexus::PrivateSettlementCommitteeAuthorityV1 {
    let mut validator_rows = keypairs
        .iter()
        .map(|keypair| {
            (
                AccountId::new(keypair.public_key().clone()),
                PeerId::new(keypair.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("private-settlement authority PoP"),
            )
        })
        .collect::<Vec<_>>();
    validator_rows.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let validators = validator_rows
        .iter()
        .map(|(_, validator, _)| validator.clone())
        .collect::<Vec<_>>();
    let validator_pops = validator_rows
        .into_iter()
        .map(|(_, _, pop)| pop)
        .collect::<Vec<_>>();
    iroha_data_model::nexus::PrivateSettlementCommitteeAuthorityV1 {
        route: iroha_data_model::nexus::PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_id: LaneId::SINGLE,
            lane_incarnation: state
                .lane_incarnation_at_height(LaneId::SINGLE, authority_height)
                .expect("fixture lane incarnation is active"),
        },
        validator_set_hash: HashOf::new(&validators),
        validators,
        validator_pops,
    }
}

#[test]
fn private_settlement_authority_accepts_exact_state_anchored_f1_roster() {
    let (state, keypairs) = exact_manifest_authority_fixture(1, 4);
    let authority = private_settlement_authority_for_keys(&state, 1, &keypairs);

    crate::private_settlement::validate_private_settlement_committee_authority_v1(
        &state.view(),
        1,
        &authority,
    )
    .expect("exact four-validator f=1 authority must be accepted");
}

#[test]
fn private_settlement_authority_rejects_forged_and_reordered_rosters() {
    let (state, keypairs) = exact_manifest_authority_fixture(1, 4);
    let valid = private_settlement_authority_for_keys(&state, 1, &keypairs);

    let forged_keys = (0x41_u8..=0x44)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("forged test BLS key")
        })
        .collect::<Vec<_>>();
    let forged = private_settlement_authority_for_keys(&state, 1, &forged_keys);
    assert!(
        crate::private_settlement::validate_private_settlement_committee_authority_v1(
            &state.view(),
            1,
            &forged,
        )
        .is_err(),
        "four attacker-owned BLS keys must not become lane authority"
    );

    let mut reordered = valid;
    reordered.validators.swap(0, 1);
    reordered.validator_pops.swap(0, 1);
    reordered.validator_set_hash = HashOf::new(&reordered.validators);
    assert!(
        crate::private_settlement::validate_private_settlement_committee_authority_v1(
            &state.view(),
            1,
            &reordered,
        )
        .is_err(),
        "the receipt roster must preserve canonical state order"
    );
}

#[test]
fn private_settlement_authority_rejects_rotated_roster_and_stale_incarnation() {
    let (state, original_keys) = exact_manifest_authority_fixture(1, 4);
    let original = private_settlement_authority_for_keys(&state, 1, &original_keys);
    let replacement_keys = (0x51_u8..=0x54)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("replacement test BLS key")
        })
        .collect::<Vec<_>>();
    seed_consensus_keys_with_pops(&state, &replacement_keys);
    install_lane_manifest_registry_for_keypairs(&state, &[LaneId::SINGLE], &replacement_keys);
    assert!(
        crate::private_settlement::validate_private_settlement_committee_authority_v1(
            &state.view(),
            1,
            &original,
        )
        .is_err(),
        "a roster retired from the state authority source must be rejected"
    );
    let replacement = private_settlement_authority_for_keys(&state, 1, &replacement_keys);
    crate::private_settlement::validate_private_settlement_committee_authority_v1(
        &state.view(),
        1,
        &replacement,
    )
    .expect("the replacement authoritative roster must be accepted");

    let mut stale_incarnation = replacement;
    stale_incarnation.route.lane_incarnation = Hash::new(b"retired lane incarnation");
    assert!(
        crate::private_settlement::validate_private_settlement_committee_authority_v1(
            &state.view(),
            1,
            &stale_incarnation,
        )
        .is_err(),
        "authority from another lane incarnation must be rejected"
    );
}

#[test]
fn private_settlement_authority_rejects_non_f1_committee_geometry() {
    let (state, keypairs) = exact_manifest_authority_fixture(2, 7);
    let four_key_claim = private_settlement_authority_for_keys(&state, 1, &keypairs[..4]);

    assert!(
        crate::private_settlement::validate_private_settlement_committee_authority_v1(
            &state.view(),
            1,
            &four_key_claim,
        )
        .is_err(),
        "V1 must not accept a four-key subset of an f=2/seven-validator committee"
    );
}

#[test]
fn exact_lane_committee_rejects_f1_pools_of_one_and_three() {
    for available in [1_u8, 3] {
        let (state, _) = exact_manifest_authority_fixture(1, available);
        assert_eq!(
            resolve_universal_committee(&state),
            Err(LaneAuthorityError::UndersizedPool {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
                authority_height: 1,
                required: 4,
                actual: usize::from(available),
            })
        );
    }
}

#[test]
fn exact_lane_committee_accepts_four_and_stably_samples_larger_f1_pool() {
    let (state, four_keys) = exact_manifest_authority_fixture(1, 4);
    install_malformed_beacon_cursor(&state);
    let four = resolve_universal_committee(&state).expect("exact four-validator committee");
    assert_eq!(four.fault_tolerance(), 1);
    assert_eq!(four.validators().len(), 4);
    assert!(four.validators().windows(2).all(|pair| pair[0] < pair[1]));
    let expected_four = four_keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        four.validators().iter().cloned().collect::<BTreeSet<_>>(),
        expected_four
    );

    let (larger_state, larger_keys) = exact_manifest_authority_fixture(1, 9);
    let first = resolve_universal_committee(&larger_state).expect("sample larger pool");
    let second = resolve_universal_committee(&larger_state).expect("repeat larger-pool sample");
    assert_eq!(first, second);
    assert_eq!(first.validators().len(), 4);
    assert!(first.validators().windows(2).all(|pair| pair[0] < pair[1]));
    let eligible = larger_keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<BTreeSet<_>>();
    assert!(
        first
            .validators()
            .iter()
            .all(|peer| eligible.contains(peer))
    );

    let reversed_validators = larger_keys
        .iter()
        .rev()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    install_lane_manifest_registry(
        &larger_state,
        &[(LaneId::SINGLE, DataSpaceId::UNIVERSAL, reversed_validators)],
    );
    assert_eq!(
        resolve_universal_committee(&larger_state).expect("order-independent sample"),
        first,
        "manifest declaration order must not influence seeded committee membership"
    );

    let (malformed_larger_state, _) = exact_manifest_authority_fixture(1, 9);
    install_malformed_beacon_cursor(&malformed_larger_state);
    assert_eq!(
        resolve_universal_committee(&malformed_larger_state),
        Err(LaneAuthorityError::InvalidAuthoritySource {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            authority_height: 1,
        }),
        "an oversized pool must still require verified sampling entropy",
    );
}

#[test]
fn exact_stake_elected_committee_does_not_require_sampling_entropy() {
    let (state, keypairs) = exact_stake_authority_fixture(1, 4);
    install_malformed_beacon_cursor(&state);
    let committee = resolve_universal_committee(&state)
        .expect("exact stake-elected committee must not consult sampling entropy");
    let expected = keypairs
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        committee
            .validators()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected,
    );
}

#[test]
fn exact_lane_committee_selects_seven_for_f2() {
    let (state, _) = exact_manifest_authority_fixture(2, 11);
    let committee = resolve_universal_committee(&state).expect("f=2 committee");
    assert_eq!(committee.fault_tolerance(), 2);
    assert_eq!(committee.validators().len(), 7);
    assert!(
        committee
            .validators()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
}

#[test]
fn exact_lane_committee_rejects_same_lane_on_wrong_dataspace() {
    let (state, _) = exact_manifest_authority_fixture(1, 4);
    let other = DataSpaceId::new(9);
    let mut nexus = state.nexus_snapshot();
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: other,
            alias: "other".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("two-dataspace catalog");
    install_existing_nexus_geometry_for_test(&state, nexus);
    assert_eq!(
        state.resolve_lane_committee_at_height(LaneAuthorityRoute::new(LaneId::SINGLE, other), 1,),
        Err(LaneAuthorityError::InactiveRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: other,
            authority_height: 1,
        })
    );
}

#[test]
fn exact_lane_committee_rejects_manifest_dataspace_mismatch() {
    let (state, keys) = exact_manifest_authority_fixture(1, 4);
    let other = DataSpaceId::new(9);
    let mut nexus = state.nexus_snapshot();
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: other,
            alias: "other".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("two-dataspace catalog");
    install_existing_nexus_geometry_for_test(&state, nexus);
    let validators = keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect();
    install_lane_manifest_registry(&state, &[(LaneId::SINGLE, other, validators)]);
    assert!(matches!(
        resolve_universal_committee(&state),
        Err(LaneAuthorityError::InvalidAuthoritySource {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            authority_height: 1,
        })
    ));
}

#[test]
fn exact_stake_committee_reselects_then_fails_closed_across_peer_churn() {
    let state = blank_test_state();
    let keypairs = (0x31_u8..=0x35)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic stake-authority BLS key")
        })
        .collect::<Vec<_>>();
    seed_consensus_keys_with_pops(&state, &keypairs);
    for (index, keypair) in keypairs.iter().enumerate() {
        insert_active_public_lane_validator_for_test(
            &state,
            LaneId::SINGLE,
            &AccountId::new(keypair.public_key().clone()),
            keypair,
            1_000_000_u64.saturating_sub(u64::try_from(index).expect("small index")),
        );
    }
    let first = resolve_universal_committee(&state).expect("initial stake committee");
    assert_eq!(first.validators().len(), 4);
    remove_world_peer_for_test(&state, &first.validators()[0]);
    let replacement = resolve_universal_committee(&state).expect("replacement stake committee");
    assert_eq!(replacement.validators().len(), 4);
    assert_ne!(replacement.validators(), first.validators());
    remove_world_peer_for_test(&state, &replacement.validators()[0]);
    assert!(matches!(
        resolve_universal_committee(&state),
        Err(LaneAuthorityError::UndersizedPool {
            required: 4,
            actual: 3,
            ..
        })
    ));
}

#[test]
fn exact_stake_committee_rejects_split_dataspace_projections() {
    let state = blank_test_state();
    let second_lane = LaneId::new(1);
    let mut nexus = state.nexus_snapshot();
    nexus.lane_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: second_lane,
                alias: "universal-sibling".to_owned(),
                dataspace_id: DataSpaceId::UNIVERSAL,
                ..LaneConfig::default()
            },
        ],
    )
    .expect("two-lane shared-dataspace catalog");
    install_existing_nexus_geometry_for_test(&state, nexus);
    let keypairs = (0x41_u8..=0x44)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic split-projection BLS key")
        })
        .collect::<Vec<_>>();
    seed_consensus_keys_with_pops(&state, &keypairs);
    for (index, keypair) in keypairs.iter().enumerate() {
        let lane_id = if index < 2 {
            LaneId::SINGLE
        } else {
            second_lane
        };
        insert_active_public_lane_validator_for_test(
            &state,
            lane_id,
            &AccountId::new(keypair.public_key().clone()),
            keypair,
            1_000_000,
        );
    }
    assert!(matches!(
        resolve_universal_committee(&state),
        Err(LaneAuthorityError::InvalidAuthoritySource { .. })
    ));
}

#[test]
fn exact_lane_committee_rejects_one_of_one_geometry_and_autoscale_pin() {
    let invalid_geometry = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::UNIVERSAL,
        alias: "universal".to_owned(),
        description: None,
        fault_tolerance: 0,
    }])
    .expect_err("f=0 would create a forbidden one-of-one committee");
    assert!(matches!(
        invalid_geometry,
        iroha_data_model::nexus::DataSpaceCatalogError::InvalidFaultTolerance {
            id: DataSpaceId::UNIVERSAL,
            fault_tolerance: 0,
        }
    ));

    let autoscale_state = blank_test_state();
    let lane_id = LaneId::new(1);
    let one_key = KeyPair::try_from_seed(vec![0x55; 32], Algorithm::BlsNormal)
        .expect("deterministic one-member autoscale key");
    let lane = autoscale_elastic_catalog_lane_with_committee_for_test(
        lane_id,
        1,
        std::slice::from_ref(&one_key),
    );
    install_autoscale_elastic_catalog_for_test(&autoscale_state, lane);
    assert_eq!(
        autoscale_state.resolve_lane_committee_at_height(
            LaneAuthorityRoute::new(lane_id, DataSpaceId::UNIVERSAL),
            1,
        ),
        Err(LaneAuthorityError::InvalidAutoscalePin {
            lane_id,
            dataspace_id: DataSpaceId::UNIVERSAL,
            required: 4,
            actual: 1,
        })
    );
}

#[test]
fn exact_autoscale_committee_keeps_its_four_member_incarnation_pin() {
    let state = blank_test_state();
    let lane_id = LaneId::new(1);
    let keypairs = (0x61_u8..=0x64)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic autoscale pin BLS key")
        })
        .collect::<Vec<_>>();
    let lane = autoscale_elastic_catalog_lane_with_committee_for_test(lane_id, 1, &keypairs);
    install_autoscale_elastic_catalog_for_test(&state, lane);
    let route = LaneAuthorityRoute::new(lane_id, DataSpaceId::UNIVERSAL);
    let pinned = state
        .resolve_lane_committee_at_height(route, 1)
        .expect("exact autoscale pin");
    assert_eq!(pinned.validators().len(), 4);
    seed_consensus_keys_with_pops(&state, &keypairs);
    remove_world_peer_for_test(&state, &pinned.validators()[0]);
    assert_eq!(
        state
            .resolve_lane_committee_at_height(route, 1)
            .expect("immutable pin survives live churn"),
        pinned
    );
}
