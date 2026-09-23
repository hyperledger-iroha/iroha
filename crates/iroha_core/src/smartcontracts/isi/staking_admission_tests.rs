// Exact candidate-consent, account-authority, and transaction rollback controls.
fn signed_candidate(
    stx: &StateTransaction<'_, '_>,
    validator: &AccountId,
    peer_key: &KeyPair,
    stake: u64,
    lane_id: LaneId,
) -> RegisterPublicLaneCandidate {
    let mut registration = RegisterPublicLaneValidator::new(
        lane_id,
        validator.clone(),
        PeerId::new(peer_key.public_key().clone()),
        validator.clone(),
        Quantity::from(stake),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(stake)),
    );
    let registered = stx
        .world
        .peers
        .iter()
        .any(|peer| peer == &registration.peer_id);
    let key_height = if registered || stx._curr_block.is_genesis() {
        stx.block_height()
    } else {
        stx.block_height()
            + stx
                .world
                .parameters
                .get()
                .sumeragi
                .key_activation_lead_blocks
    };
    let activation_height = next_unfrozen_election_height(
        key_height,
        stx.world
            .sumeragi_npos_parameters()
            .expect("NPoS schedule")
            .epoch_length_blocks
            .get(),
    )
    .expect("election height");
    registration.monetary_plan.precondition =
        PublicLaneMonetaryPreconditionV1::Registration(PublicLaneMonetaryRegistrationV1 {
            activation_height,
        });
    let authorization = PublicLaneCandidateAuthorization::new(
        *stx.network_id(),
        registration.clone(),
        activation_height,
    );
    RegisterPublicLaneCandidate {
        registration,
        activation_height,
        proof_of_possession: iroha_crypto::bls_normal_pop_prove(peer_key.private_key())
            .expect("candidate PoP"),
        peer_signature: iroha_crypto::SignatureOf::try_new(peer_key.private_key(), &authorization)
            .expect("candidate consent"),
    }
}

#[test]
fn initial_executor_candidate_bonds_after_key_lead_without_joining_current_topology() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 6);
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, _, escrow, definition) = prepare_accounts(&mut stx);
    stx.world
        .parameters
        .get_mut()
        .sumeragi
        .key_activation_lead_blocks = 7;
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let candidate = signed_candidate(&stx, &validator, &peer_key, 1000, LaneId::new(42));
    let peer = candidate.registration.peer_id.clone();
    let prior_topology = stx.commit_topology.get().clone();
    crate::executor::Executor::Initial
        .execute_instruction(&mut stx, &validator, candidate.into())
        .expect("ordinary stake owner can register a candidate without peer administration");
    let record = stx
        .world
        .public_lane_validators
        .get(&(LaneId::new(42), validator.clone()))
        .expect("candidate record");
    assert_eq!(record.activation_height, 13);
    assert_eq!(
        record.status,
        PublicLaneValidatorStatus::PendingActivation(13)
    );
    assert!(!validator_election_eligible_at_height(record, 12));
    assert!(validator_election_eligible_at_height(record, 13));
    assert_eq!(stx.commit_topology.get(), &prior_topology);
    assert!(!stx.commit_topology.iter().any(|voter| voter == &peer));
    assert_eq!(
        peer_consensus_key_gate_for_lane(&stx.world, &peer, 2, LaneId::new(42)),
        ConsensusKeyGate::NotYetActive
    );
    assert_eq!(
        peer_consensus_key_gate_for_lane(&stx.world, &peer, 13, LaneId::new(42)),
        ConsensusKeyGate::Live
    );
    assert!(
        !crate::state::peer_has_live_consensus_key_for_role(
            &stx.world,
            &peer,
            13,
            ConsensusKeyRole::Validator
        ),
        "participant enrollment cannot create a global voting key"
    );
    let escrow_asset = stx
        .world
        .assets
        .get(&AssetId::new(definition, escrow))
        .expect("stake escrow");
    assert_eq!(escrow_asset.as_ref(), &Quantity::from(1000_u64));
}

#[test]
fn candidate_rejects_other_authority_tampered_consent_and_invalid_pop() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, intruder, _, _) = prepare_accounts(&mut stx);
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let candidate = signed_candidate(&stx, &validator, &peer_key, 1000, LaneId::new(42));
    let peer = candidate.registration.peer_id.clone();
    assert!(candidate.clone().execute(&intruder, &mut stx).is_err());
    let mut tampered = candidate.clone();
    tampered.registration.initial_stake = Quantity::from(2000_u64);
    assert!(tampered.execute(&validator, &mut stx).is_err());
    let mut invalid_pop = candidate;
    invalid_pop.proof_of_possession = vec![0; 96];
    assert!(invalid_pop.execute(&validator, &mut stx).is_err());
    assert!(!stx.world.peers.iter().any(|id| id == &peer));
    assert!(
        stx.world
            .public_lane_validators
            .get(&(LaneId::new(42), validator))
            .is_none()
    );
}

#[test]
fn failed_candidate_transaction_rolls_back_peer_key_and_stake() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let (validator, escrow, definition) = {
        let mut stx = block.transaction();
        let (validator, _, escrow, definition) = prepare_accounts(&mut stx);
        stx.apply();
        (validator, escrow, definition)
    };
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::new(peer_key.public_key().clone());
    {
        let mut stx = block.transaction();
        let candidate = signed_candidate(&stx, &validator, &peer_key, 100_000, LaneId::new(42));
        assert!(
            candidate.execute(&validator, &mut stx).is_err(),
            "insufficient funds"
        );
        assert!(
            !stx.world.peers.iter().any(|id| id == &peer),
            "failed instruction must not publish an unbonded peer"
        );
        // Failed transaction is deliberately dropped rather than applied.
    }
    let stx = block.transaction();
    assert!(!stx.world.peers.iter().any(|id| id == &peer));
    assert!(
        stx.world
            .consensus_keys_by_pk()
            .get(&peer.public_key().to_string())
            .is_none()
    );
    assert!(
        stx.world
            .public_lane_validators
            .get(&(LaneId::new(42), validator))
            .is_none()
    );
    assert!(
        stx.world
            .assets
            .get(&AssetId::new(definition, escrow))
            .is_none()
    );
}

#[test]
fn activation_requires_validator_authority_after_genesis() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, intruder, _, _) = prepare_accounts(&mut stx);
    RegisterPublicLaneValidator::new(
        LaneId::SINGLE,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        Quantity::from(1000_u64),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(1000_u64)),
    )
    .execute(&validator, &mut stx)
    .expect("registration");
    let instruction = ActivatePublicLaneValidator::new(LaneId::SINGLE, validator.clone());
    let error = instruction
        .clone()
        .execute(&intruder, &mut stx)
        .expect_err("foreign activation");
    assert!(error.to_string().contains("authority must match validator"));
    let error = instruction
        .execute(&validator, &mut stx)
        .expect_err("future boundary");
    assert!(error.to_string().contains("activation height not reached"));
}

#[test]
fn distinct_peer_binding_requires_consent_even_for_the_validator_owner() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, delegator, _, _) = prepare_accounts(&mut stx);
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::new(peer_key.public_key().clone());
    stx.world.peers.push(peer.clone());
    stx.commit_topology.get_mut().push(peer.clone());
    seed_validator_consensus_key(&mut stx, &peer, ConsensusKeyStatus::Active);
    let key_id = crate::state::derive_validator_key_id(peer.public_key());
    stx.world
        .consensus_keys
        .get_mut(&key_id)
        .expect("live key")
        .pop = Some(iroha_crypto::bls_normal_pop_prove(peer_key.private_key()).expect("PoP"));
    let candidate = signed_candidate(&stx, &validator, &peer_key, 1000, LaneId::SINGLE);
    let error = candidate
        .registration
        .clone()
        .execute(&validator, &mut stx)
        .expect_err("plain registration cannot squat another consensus identity");
    assert!(error.to_string().contains("network-bound consent"));
    let error = candidate
        .clone()
        .execute(&validator, &mut stx)
        .expect_err("consent cannot add a previously ineligible global candidate");
    assert!(error.to_string().contains("prepared epoch key transition"));
    // A retained eligible record in another lane already contributes this peer
    // to the global pool. The new binding is redundant for global election.
    stx.world.public_lane_validators.insert(
        (LaneId::new(42), ALICE_ID.clone()),
        PublicLaneValidatorRecord {
            lane_id: LaneId::new(42),
            validator: ALICE_ID.clone(),
            peer_id: peer,
            stake_account: ALICE_ID.clone(),
            total_stake: 1000_u64.into(),
            self_stake: 1000_u64.into(),
            metadata: Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_height: 1,
            deactivation_height: None,
            last_reward_epoch: None,
        },
    );
    let mut other = stx
        .world
        .public_lane_validators
        .get(&(LaneId::new(42), ALICE_ID.clone()))
        .expect("backing record")
        .clone();
    other.lane_id = LaneId::SINGLE;
    other.validator = delegator.clone();
    other.stake_account = delegator.clone();
    other.peer_id = validator_peer_id(&delegator);
    stx.commit_topology.get_mut().push(other.peer_id.clone());
    stx.world
        .public_lane_validators
        .insert((LaneId::SINGLE, delegator), other);
    candidate
        .execute(&validator, &mut stx)
        .expect("signed redundant binding preserves the already-eligible global peer");
}

#[test]
fn pending_rebind_requires_network_bound_replacement_peer_consent() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 6);
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, _, _, _) = prepare_accounts(&mut stx);
    RegisterPublicLaneValidator::new(
        LaneId::SINGLE,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        Quantity::from(1000_u64),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(1000_u64)),
    )
    .execute(&validator, &mut stx)
    .expect("register owner-bound peer");
    let replacement_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let replacement = PeerId::new(replacement_key.public_key().clone());
    stx.world.peers.push(replacement.clone());
    seed_validator_consensus_key(&mut stx, &replacement, ConsensusKeyStatus::Active);
    let rebind =
        RebindPublicLaneValidatorPeer::new(LaneId::SINGLE, validator.clone(), replacement.clone());
    let error = rebind
        .clone()
        .execute(&validator, &mut stx)
        .expect_err("unsigned distinct peer");
    assert!(error.to_string().contains("network-bound consent"));
    let payload = PublicLanePeerBindingAuthorization::new(
        *stx.network_id(),
        LaneId::SINGLE,
        validator.clone(),
        replacement.clone(),
        7,
        validator_peer_id(&validator),
    );
    let mut stale_payload = payload.clone();
    stale_payload.activation_height += 6;
    let stale_signature =
        iroha_crypto::SignatureOf::try_new(replacement_key.private_key(), &stale_payload)
            .expect("future-tenure signature");
    let error = rebind
        .clone()
        .with_peer_signature(stale_signature)
        .execute(&validator, &mut stx)
        .expect_err("consent from another validator tenure");
    assert!(error.to_string().contains("replacement peer signature"));
    let mut wrong_previous = payload.clone();
    wrong_previous.previous_peer_id = replacement.clone();
    let stale_signature =
        iroha_crypto::SignatureOf::try_new(replacement_key.private_key(), &wrong_previous)
            .expect("wrong-binding signature");
    assert!(
        rebind
            .clone()
            .with_peer_signature(stale_signature)
            .execute(&validator, &mut stx)
            .is_err()
    );
    let signature = iroha_crypto::SignatureOf::try_new(replacement_key.private_key(), &payload)
        .expect("replacement consent");
    rebind
        .with_peer_signature(signature)
        .execute(&validator, &mut stx)
        .expect("consenting replacement");
    assert_eq!(
        stx.world
            .public_lane_validators
            .get(&(LaneId::SINGLE, validator))
            .expect("validator")
            .peer_id,
        replacement
    );
}

#[test]
fn fresh_global_candidate_fails_closed_before_any_peer_or_custody_write() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, _, escrow, definition) = prepare_accounts(&mut stx);
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let candidate = signed_candidate(&stx, &validator, &peer_key, 1000, LaneId::SINGLE);
    let error = candidate
        .execute(&validator, &mut stx)
        .expect_err("global roster has no prepared epoch key transition");
    assert!(error.to_string().contains("prepared epoch key transition"));
    assert!(
        !stx.world
            .peers
            .iter()
            .any(|peer| peer.public_key() == peer_key.public_key())
    );
    assert!(
        stx.world
            .consensus_keys_by_pk()
            .get(&peer_key.public_key().to_string())
            .is_none()
    );
    assert!(
        stx.world
            .public_lane_validators
            .get(&(LaneId::SINGLE, validator))
            .is_none()
    );
    assert!(
        stx.world
            .assets
            .get(&AssetId::new(definition, escrow))
            .is_none()
    );
}

#[test]
fn candidate_requires_committed_schedule_and_rejects_expired_tenure_consent() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, _, _, _) = prepare_accounts(&mut stx);
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let candidate = signed_candidate(&stx, &validator, &peer_key, 1000, LaneId::new(42));
    let mut stale = candidate.clone();
    stale.activation_height += 1;
    let error = stale
        .execute(&validator, &mut stx)
        .expect_err("consent for another boundary");
    assert!(
        error
            .to_string()
            .contains("consent targets activation height")
    );
    stx.world
        .parameters
        .get_mut()
        .custom
        .remove(&SumeragiNposParameters::parameter_id());
    let error = candidate
        .execute(&validator, &mut stx)
        .expect_err("missing committed schedule");
    assert!(
        error
            .to_string()
            .contains("committed NPoS election parameters")
    );
    assert!(
        !stx.world
            .peers
            .iter()
            .any(|peer| peer.public_key() == peer_key.public_key())
    );
}

#[test]
fn participant_binding_cannot_smuggle_global_validator_outside_prepared_topology() {
    let state = setup_state();
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, _, escrow, definition) = prepare_accounts(&mut stx);
    stx.commit_topology
        .get_mut()
        .push(validator_peer_id(&validator));
    let peer_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::new(peer_key.public_key().clone());
    let _ = stx.world.peers.push(peer.clone());
    seed_validator_consensus_key(&mut stx, &peer, ConsensusKeyStatus::Active);
    let candidate = signed_candidate(&stx, &validator, &peer_key, 1000, LaneId::new(42));
    let error = candidate
        .execute(&validator, &mut stx)
        .expect_err("nonzero lane must not bypass global transition gate");
    assert!(error.to_string().contains("prepared epoch key transition"));
    assert!(
        stx.world
            .public_lane_validators
            .get(&(LaneId::new(42), validator))
            .is_none()
    );
    assert!(
        stx.world
            .assets
            .get(&AssetId::new(definition, escrow))
            .is_none()
    );
}

#[test]
fn global_pool_guard_preserves_safe_unbonds_and_rejects_exit_or_minimum_crossing() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 6);
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, delegator, _, _) = prepare_accounts(&mut stx);
    let lane_id = LaneId::new(42);
    stx.nexus.staking.min_validator_stake = 1000_u64.into();
    stx.nexus.staking.unbonding_delay = Duration::ZERO;
    RegisterPublicLaneValidator::new(
        lane_id,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        2000_u64.into(),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(2000_u64)),
    )
    .execute(&validator, &mut stx)
    .expect("seed retained validator before topology freeze");
    stx.commit_topology
        .get_mut()
        .push(validator_peer_id(&validator));
    let release_at_ms = stx.block_unix_timestamp_ms();
    let error = ExitPublicLaneValidator {
        lane_id,
        validator: validator.clone(),
        release_at_ms,
    }
    .execute(&validator, &mut stx)
    .expect_err("cross-lane global voter cannot exit without transition");
    assert!(error.to_string().contains("prepared epoch key transition"));
    let unbond = |staker: &AccountId, amount, label| SchedulePublicLaneUnbond {
        lane_id,
        validator: validator.clone(),
        staker: staker.clone(),
        amount: Quantity::from(amount),
        request_id: Hash::new(label),
        release_at_ms,
    };
    unbond(&validator, 1000_u64, "global-safe-self-unbond")
        .execute(&validator, &mut stx)
        .expect("self withdrawal retaining exact minimum");
    let error = unbond(&validator, 1_u64, "global-unsafe-self-unbond")
        .execute(&validator, &mut stx)
        .expect_err("self withdrawal below minimum changes global eligibility");
    assert!(error.to_string().contains("prepared epoch key transition"));
    BondPublicLaneStake {
        monetary_plan: fixture_bond_plan(
            &stx,
            lane_id,
            &validator,
            &delegator,
            Quantity::from(100_u64),
        ),
        lane_id,
        validator: validator.clone(),
        staker: delegator.clone(),
        amount: 100_u64.into(),
        metadata: Metadata::default(),
    }
    .execute(&delegator, &mut stx)
    .expect("delegation does not change global seat selection");
    unbond(&delegator, 100_u64, "global-safe-delegator-unbond")
        .execute(&delegator, &mut stx)
        .expect("delegator may withdraw without changing minimum self stake");
    let key = (lane_id, validator.clone());
    let record = stx
        .world
        .public_lane_validators
        .get_mut(&key)
        .expect("validator");
    record.activation_height = 1;
    record.deactivation_height = Some(2);
    record.status = PublicLaneValidatorStatus::Exited;
    unbond(&validator, 1000_u64, "global-ended-tenure-unbond")
        .execute(&validator, &mut stx)
        .expect("already finalized tenure may drain remaining custody");
    assert!(
        stx.world
            .public_lane_validators
            .get(&key)
            .expect("retained custody")
            .self_stake
            .is_zero()
    );
}

#[test]
fn global_pool_guard_rejects_bond_restoring_under_minimum_global_candidate() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 6);
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, _, _, _) = prepare_accounts(&mut stx);
    let lane_id = LaneId::SINGLE;
    RegisterPublicLaneValidator::new(
        lane_id,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        1000_u64.into(),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(1000_u64)),
    )
    .execute(&validator, &mut stx)
    .expect("seed retained validator");
    stx.commit_topology
        .get_mut()
        .push(validator_peer_id(&validator));
    // A governance minimum increase is external to ordinary owner staking.
    stx.nexus.staking.min_validator_stake = 2000_u64.into();
    let error = BondPublicLaneStake {
        monetary_plan: fixture_bond_plan(
            &stx,
            lane_id,
            &validator,
            &validator,
            Quantity::from(1000_u64),
        ),
        lane_id,
        validator: validator.clone(),
        staker: validator.clone(),
        amount: 1000_u64.into(),
        metadata: Metadata::default(),
    }
    .execute(&validator, &mut stx)
    .expect_err("owner cannot restore global eligibility without prepared keys");
    assert!(error.to_string().contains("prepared epoch key transition"));
    assert_eq!(
        stx.world
            .public_lane_validators
            .get(&(lane_id, validator))
            .expect("validator")
            .self_stake,
        Quantity::from(1000_u64)
    );
}

#[test]
fn global_pool_guard_checks_later_tenure_union_and_allows_permanent_redundancy() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 6);
    let mut block = state.block(block_header_with_height(2));
    let mut stx = block.transaction();
    let (validator, delegator, _, _) = prepare_accounts(&mut stx);
    let lane_id = LaneId::SINGLE;
    RegisterPublicLaneValidator::new(
        lane_id,
        validator.clone(),
        validator_peer_id(&validator),
        validator.clone(),
        1000_u64.into(),
        Metadata::default(),
        fixture_registration_plan(&stx, &validator, Quantity::from(1000_u64)),
    )
    .execute(&validator, &mut stx)
    .expect("seed retained validator");
    stx.commit_topology
        .get_mut()
        .push(validator_peer_id(&validator));
    let key = (lane_id, validator.clone());
    let mut redundant = stx
        .world
        .public_lane_validators
        .get(&key)
        .expect("record")
        .clone();
    let mut lane_cover = redundant.clone();
    lane_cover.validator = delegator.clone();
    lane_cover.stake_account = delegator.clone();
    lane_cover.peer_id = validator_peer_id(&delegator);
    stx.commit_topology
        .get_mut()
        .push(lane_cover.peer_id.clone());
    stx.world
        .public_lane_validators
        .insert((lane_id, delegator), lane_cover);
    redundant.lane_id = LaneId::new(42);
    redundant.validator = ALICE_ID.clone();
    redundant.stake_account = ALICE_ID.clone();
    redundant.deactivation_height = Some(13);
    stx.world
        .public_lane_validators
        .insert((LaneId::new(42), ALICE_ID.clone()), redundant);
    let mut replacement = stx
        .world
        .public_lane_validators
        .get(&key)
        .expect("record")
        .clone();
    replacement.deactivation_height = Some(7);
    assert_eq!(
        crate::state::epoch_validator_candidate_peer_ids_from_world(
            &stx.world,
            stx.commit_topology.iter().cloned(),
            7,
            &stx.nexus,
            None
        ),
        crate::state::epoch_validator_candidate_peer_ids_from_world(
            &stx.world,
            stx.commit_topology.iter().cloned(),
            7,
            &stx.nexus,
            Some(&replacement)
        )
    );
    let error = ensure_global_candidate_pool_preserved(&stx, &replacement, "test_exit")
        .expect_err("redundancy that ends later cannot hide future removal");
    assert!(error.to_string().contains("prepared epoch key transition"));
    stx.world
        .public_lane_validators
        .get_mut(&(LaneId::new(42), ALICE_ID.clone()))
        .expect("redundant record")
        .deactivation_height = None;
    ensure_global_candidate_pool_preserved(&stx, &replacement, "test_exit")
        .expect("permanent same-peer and lane redundancy preserves every future pool");
}

#[test]
fn election_interval_union_rounds_boundaries_and_ignores_finished_custody() {
    assert_eq!(
        normalized_future_election_intervals([(1, Some(2)), (8, Some(20)), (19, None)], 7, 6),
        vec![(13, u128::from(u64::MAX) + 1)]
    );
    assert_eq!(
        normalized_future_election_intervals([(7, Some(13)), (13, Some(19))], 7, 6),
        vec![(7, 19)]
    );
}
