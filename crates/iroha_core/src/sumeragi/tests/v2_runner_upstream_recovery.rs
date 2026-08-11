// Upstream roster-duty and quiet-recovery regressions retained through the merge.

#[test]
fn global_voting_role_tracks_each_frozen_roster_without_losing_validator_processes() {
    let (context, keys) = context();
    let peer = PeerId::new(keys[0].public_key().clone());
    assert_eq!(
        local_validator_index(&context, &peer, NodeRole::Observer).expect("observer"),
        None
    );
    assert_eq!(
        local_validator_index(&context, &peer, NodeRole::Validator)
            .expect("roster member remains a global validator"),
        context
            .roster
            .iter()
            .position(|entry| entry.validator == peer)
            .map(|index| u32::try_from(index).expect("fixture roster index fits u32"))
    );
    assert_eq!(
        local_validator_index(
            &context,
            &PeerId::new(
                KeyPair::try_from_seed(vec![0x55; 32], Algorithm::BlsNormal)
                    .expect("deterministic non-member key")
                    .public_key()
                    .clone()
            ),
            NodeRole::Validator
        )
        .expect("a removed validator continues as a global observer"),
        None
    );
}

#[test]
fn initially_absent_configured_validator_claims_one_process_generation() {
    let (context, _) = context();
    let local_key = KeyPair::try_from_seed(vec![0x55; 32], Algorithm::BlsNormal)
        .expect("deterministic initially absent validator key");
    let local_peer = PeerId::new(local_key.public_key().clone());
    assert_eq!(
        local_validator_index(&context, &local_peer, NodeRole::Validator)
            .expect("configured validator may be absent from this frozen roster"),
        None
    );

    let kura = super::super::v2_lane_work::tests::locked_lane_work_test_kura(
        iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
    );
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind the immutable configured peer before generation acquisition");
    let claim = claim_runner_lifecycle_process_generation(
        NodeRole::Validator,
        kura.as_ref(),
        &context,
        &local_peer,
    )
    .expect("initial absence must not suppress generation acquisition")
    .expect("configured validator owns one lifecycle generation");
    let generation = claim.generation();
    assert_ne!(generation, 0);
    assert_eq!(claim.local_peer_id(), &local_peer);
    assert_eq!(
        claim.network_id(),
        context.network_id
    );

    let mut later_context = context.clone();
    later_context.roster.push(wire::ValidatorPower {
        validator: local_peer.clone(),
        power: 1,
    });
    later_context.quorum = wire::DualQuorum::from_roster(&later_context.roster)
        .expect("rotated-in roster has a valid dual quorum");
    assert!(
        local_validator_index(&later_context, &local_peer, NodeRole::Validator)
            .expect("the same configured process can rotate into a later roster")
            .is_some()
    );
    assert_eq!(
        claim.generation(),
        generation,
        "rotation-in reuses the already acquired process-lifetime claim"
    );
    assert_eq!(
        claim_runner_lifecycle_process_generation(
            NodeRole::Observer,
            kura.as_ref(),
            &later_context,
            &local_peer,
        )
        .expect("explicit observer path is non-mutating"),
        None
    );
}

#[test]
fn lane_production_duty_survives_successor_global_roster_removal() {
    let (context, _) = context();
    let tag = EventTag::new(context.height, 4, Generation::new(23));
    let directive =
        LocalProposalDirective::for_test(tag, context.leader(tag.view()), None, None, None);

    assert_eq!(
        local_consensus_duties(directive, None),
        LocalConsensusDuties {
            autonomous_lane_view: Some(tag.view()),
            global_validator: None,
        },
        "successor-global observer status must not suppress independently frozen lane-author work"
    );

    let subject = proposal_subject(b"lane production duty lock");
    let locked_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 2,
    };
    let locked = LocalProposalDirective::for_test(
        tag,
        context.leader(tag.view()),
        Some(locked_round),
        Some(subject),
        None,
    );
    assert_eq!(
        local_consensus_duties(locked, None).autonomous_lane_view,
        None,
        "a global lock still suppresses fresh lane payload production"
    );

    let decided = LocalProposalDirective::for_test(
        tag,
        context.leader(tag.view()),
        None,
        None,
        Some(subject),
    );
    assert_eq!(
        local_consensus_duties(decided, None).autonomous_lane_view,
        None,
        "a terminal global decision retires fresh lane payload production"
    );
}

#[test]
fn pre_submit_lane_binding_rejection_arms_one_non_empty_retry() {
    let (context, _) = context();
    let owner = proposal_owner(
        &context,
        EventTag::new(context.height, 3, Generation::new(19)),
        None,
        None,
    );
    let now = Instant::now();
    let mut state = LocalProposalState {
        attempted: Some(owner),
        candidate_work_wait: Some(CandidateWorkWait {
            owner,
            started_at: now,
            next_retry: now,
        }),
        ..LocalProposalState::default()
    };

    assert_eq!(
        state.handle_candidate_binding_rejection(owner),
        LocalValidationDisposition::RetryNonEmpty,
        "an unsubmitted lane-binding rejection must not stop the process"
    );
    assert_eq!(state.attempted, None);
    assert_eq!(state.non_empty_retry, Some(owner));
    assert!(state.candidate_work_wait.is_none());
    assert!(state.submitted.is_none());
    assert!(state.pending_events.is_none());
    assert!(state.global_selection.is_none());

    assert_eq!(
        state.handle_candidate_binding_rejection(owner),
        LocalValidationDisposition::FatalNonEmpty,
        "a rejected non-empty recovery retry still fails closed"
    );
}

#[test]
fn quiet_retransmission_tick_services_one_retained_historical_session() {
    let mut lane_work = super::super::v2_lane_work::tests::quiet_historical_recovery_fixture();
    assert!(lane_work.has_pending_historical_recovery());

    let outcome = service_historical_recovery_tick(&mut lane_work)
        .expect("quiet retransmission tick advances retained history");
    let HistoricalRecoveryServiceOutcome::Waiting(wait) = outcome else {
        panic!("missing canonical body must remain a typed quiet-network retry: {outcome:?}");
    };
    assert_eq!(
        wait.reason(),
        super::super::v2_lane_work::HistoricalRecoveryWaitReason::CanonicalBlockPending
    );
    assert!(wait.first_observation());
    assert!(
        lane_work.has_pending_historical_recovery(),
        "one bounded wait turn must retain the exact historical owner"
    );
}
