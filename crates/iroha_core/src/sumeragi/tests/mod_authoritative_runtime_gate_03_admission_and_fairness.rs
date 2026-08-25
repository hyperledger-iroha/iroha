#[test]
fn ordinary_selector_preserves_certified_response_before_timeout_vote() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let response = v2_certified_body_response(0, 0, 1);
    let round = match &response {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
            ..
        }) => response.manifest.round,
        _ => unreachable!("certified response fixture is a v2 envelope"),
    };
    let mut timeout = v2_timeout_vote();
    match &mut timeout {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(vote),
            ..
        }) => vote.round = round,
        _ => unreachable!("timeout fixture is a v2 envelope"),
    }
    let _gate_directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            response,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            timeout, validator,
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    {
        let state = ingress.state.lock();
        let earliest = state
            .leader_wire_lifecycles
            .values()
            .filter(|record| record.status == super::FairV2IngressLeaderWireStatus::Ingress)
            .min_by_key(|record| record.token.scheduler_ordinal)
            .expect("one leader-wire barrier is active");
        assert_eq!(
            earliest.token.identity.phase,
            super::FairV2IngressLeaderWirePhase::CertifiedResponse
        );
    }
    let is_timeout_vote = |inbound: &InboundBlockMessage| {
        matches!(
            inbound.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        )
    };
    assert!(
        ingress
            .try_recv_if_checked_retiring_obsolete(is_timeout_vote)
            .expect("ordinary selection preserves the response barrier")
            .is_none()
    );
    let (mut selected, disposition) = ingress
        .try_recv_if_checked_retiring_obsolete(|inbound| !is_timeout_vote(inbound))
        .expect("the lifecycle selector preserves the checked dequeue")
        .expect("the exact certified response remains first");
    assert_eq!(disposition, super::FairV2IngressDequeueDisposition::Admit);
    assert!(!is_timeout_vote(&selected));
    let ownership = selected
        .take_ingress_ownership()
        .expect("the selected certified response retains exact ingress ownership");
    assert!(ownership.validate_exact());
    let response_runtime = ownership
        .leader_wire_runtime_receipt()
        .expect("the certified response crosses the durable runtime handoff");
    ingress
        .mark_leader_wire_volatile_terminal(response_runtime)
        .expect("retire the consumed certified response");
    assert_eq!(ingress.len(), 1, "the later TimeoutVote remains queued");

    let (mut selected, disposition) = ingress
        .try_recv_if_checked_retiring_obsolete(is_timeout_vote)
        .expect("the strict selector remains live after response retirement")
        .expect("the TimeoutVote becomes eligible only after its predecessor");
    assert_eq!(disposition, super::FairV2IngressDequeueDisposition::Admit);
    assert!(is_timeout_vote(&selected));
    let ownership = selected
        .take_ingress_ownership()
        .expect("the selected TimeoutVote retains exact ingress ownership");
    assert!(ownership.validate_exact());
    let timeout_runtime = ownership
        .leader_wire_runtime_receipt()
        .expect("the TimeoutVote crosses the ordinary durable runtime handoff");
    ingress
        .mark_leader_wire_volatile_terminal(timeout_runtime)
        .expect("retire the consumed TimeoutVote");
    assert_eq!(ingress.len(), 0);
}

#[test]
fn restored_productive_retry_stays_behind_an_earlier_certified_request_carrier() {
    let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
    assert_eq!(fixture.token.admission_ordinal(), 7);
    assert!(matches!(
        fixture
            .ingress
            .try_push(v2_certified_body_request_inbound(&fixture.validator)),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let target_ordinal = fixture
        .ingress
        .state
        .lock()
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| fair_v2_ingress_is_certified_body_request(&entry.inbound))
        .expect("certified request owns its fresh physical occurrence")
        .admission_ordinal;
    assert!(target_ordinal > fixture.token.admission_ordinal());
    assert!(matches!(
        fixture
            .ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                fixture.message.clone(),
                fixture.validator.clone(),
            )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let retry_ordinal = fixture
        .ingress
        .state
        .lock()
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
        .expect("restored productive lifecycle regained one physical carrier")
        .admission_ordinal;
    assert!(
        retry_ordinal > target_ordinal,
        "a retained lifecycle token cannot reuse its old ordinal as a new physical position"
    );
    assert!(
        fixture
            .ingress
            .try_recv_if(|inbound| !fair_v2_ingress_is_certified_body_request(inbound))
            .is_none(),
        "the later productive retry cannot cross the certified target cutoff"
    );
    let target = fixture
        .ingress
        .try_recv_if(fair_v2_ingress_is_certified_body_request)
        .expect("the refrozen leader prefix admits the earlier certified carrier");
    assert_eq!(target.sender(), &fixture.validator);
    let retry = fixture
        .ingress
        .try_recv_if(|_| true)
        .expect("the productive retry drains after its frozen predecessor");
    assert!(
        retry
            .ingress_ownership()
            .is_some_and(|ownership| { ownership.leader_wire_token() == Some(&fixture.token) })
    );
}
#[test]
fn restored_productive_retry_freezes_the_current_physical_source_prefix() {
    let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
    let earlier = v2_commit_certificate_request(0, &fixture.validator);
    assert!(matches!(
        fixture
            .ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                earlier,
                fixture.validator.clone(),
            )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let earlier_ordinal = fixture
        .ingress
        .state
        .lock()
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| entry.leader_wire_token.is_none())
        .expect("ordinary traffic owns its physical occurrence")
        .admission_ordinal;
    assert!(matches!(
        fixture
            .ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                fixture.message.clone(),
                fixture.validator.clone(),
            )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let retry_ordinal = fixture
        .ingress
        .state
        .lock()
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
        .expect("restored lifecycle acquired one fresh carrier")
        .admission_ordinal;
    assert!(earlier_ordinal < retry_ordinal);
    assert!(
        fixture
            .ingress
            .try_recv_if(|inbound| {
                inbound
                    .ingress_ownership()
                    .is_some_and(|ownership| ownership.leader_wire_token().is_some())
            })
            .is_none(),
        "a predicate which rejects the predecessor cannot select the leader-wire target"
    );
    let first = fixture
        .ingress
        .try_recv_if(|_| true)
        .expect("the replay-frozen physical predecessor drains first");
    assert!(matches!(
        first.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::CommitCertificateRequest(_),
            ..
        })
    ));
    let replay = fixture
        .ingress
        .try_recv_if(|_| true)
        .expect("the exact replay drains after its frozen source prefix");
    assert!(
        replay
            .ingress_ownership()
            .is_some_and(|ownership| { ownership.leader_wire_token() == Some(&fixture.token) })
    );
}
#[test]
fn restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire() {
    let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
    let round = match &fixture.message {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
            ..
        }) => proposal.round,
        _ => unreachable!("restart fixture carries a proposal"),
    };
    let mut earlier_message = v2_vote(wire::GlobalPhase::Prepare);
    let BlockMessage::V2(earlier_envelope) = &mut earlier_message else {
        unreachable!("vote fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Vote(earlier_vote) = &mut earlier_envelope.payload else {
        unreachable!("vote fixture carries a vote");
    };
    earlier_vote.round = round;
    earlier_vote.proposal_round = round;
    assert!(matches!(
        fixture
            .ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                earlier_message,
                fixture.alternate_validator.clone(),
            )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let (earlier_token, earlier_physical_ordinal) = {
        let state = fixture.ingress.state.lock();
        let entry = state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| {
                entry
                    .leader_wire_token
                    .as_ref()
                    .is_some_and(|token| token != &fixture.token)
            })
            .expect("fresh vote owns one leader-wire carrier");
        (
            entry
                .leader_wire_token
                .clone()
                .expect("selected entry has a leader-wire token"),
            entry.admission_ordinal,
        )
    };
    assert!(
        earlier_token.scheduler_ordinal > fixture.token.scheduler_ordinal,
        "the fresh lifecycle has a newer logical identity"
    );
    assert!(matches!(
        fixture
            .ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                fixture.message.clone(),
                fixture.validator.clone(),
            )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let replay_physical_ordinal = fixture
        .ingress
        .state
        .lock()
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
        .expect("restored lifecycle owns one replay carrier")
        .admission_ordinal;
    assert!(
        earlier_physical_ordinal < replay_physical_ordinal,
        "the replay carrier is physically newer despite its older logical identity"
    );
    assert_eq!(
        fixture
            .gate
            .ingress_scheduler_ordinals()
            .expect("read exact durable Ingress owner set"),
        std::collections::BTreeSet::from([
            fixture.token.scheduler_ordinal,
            earlier_token.scheduler_ordinal,
        ])
    );
    {
        // Round-robin history is independent of physical admission order.
        // Put the replay source first to ensure only the physical barrier,
        // rather than incidental ready-source order, protects the earlier
        // carrier.
        let mut state = fixture.ingress.state.lock();
        state.ready = std::collections::VecDeque::from([
            super::FairV2IngressSource::Validator(fixture.validator.clone()),
            super::FairV2IngressSource::Validator(fixture.alternate_validator.clone()),
        ]);
        fixture.ingress.debug_assert_consistent(&state);
    }
    assert!(
        fixture
            .ingress
            .try_recv_if(|inbound| {
                inbound
                    .ingress_ownership()
                    .is_some_and(|ownership| ownership.leader_wire_token() == Some(&fixture.token))
            })
            .is_none(),
        "the physically later replay cannot be selected merely because its logical ordinal is older"
    );
    let mut first = fixture
        .ingress
        .try_recv_if(|_| true)
        .expect("the physically earlier leader-wire carrier drains first");
    let mut first_ownership = first
        .take_ingress_ownership()
        .expect("leader-wire carrier retains ingress ownership");
    assert_eq!(
        first_ownership.leader_wire_token(),
        Some(&earlier_token),
        "physical order, not retained logical order, selects the owner"
    );
    fixture
        .ingress
        .bind_leader_wire_runtime_ownership(&mut first_ownership)
        .expect("bind the selected fresh lifecycle");
    let second = fixture
        .ingress
        .try_recv_if(|_| true)
        .expect("the older logical replay drains on the next turn");
    assert!(
        second
            .ingress_ownership()
            .is_some_and(|ownership| { ownership.leader_wire_token() == Some(&fixture.token) })
    );
}
#[test]
fn restored_productive_retry_ordinal_exhaustion_keeps_the_owner_dormant() {
    let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
    fixture.ingress.state.lock().last_admission_ordinal = u64::MAX;
    assert!(matches!(
        fixture
            .ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                fixture.message.clone(),
                fixture.validator.clone(),
            )),
        Err(super::FairV2IngressPushError::FailStop(_))
    ));
    {
        let state = fixture.ingress.state.lock();
        assert!(
            !state.open,
            "physical ordinal exhaustion fails admission closed"
        );
        assert_eq!(state.len, 0, "no carrier was admitted");
        let record = state
            .leader_wire_lifecycles
            .get(&fixture.token.slot)
            .expect("restored lifecycle remains retained");
        assert_eq!(record.status, super::FairV2IngressLeaderWireStatus::Dormant);
        assert_eq!(record.token, fixture.token);
    }
    assert_eq!(
        fixture
            .gate
            .earliest_ingress_scheduler_ordinal()
            .expect("read dormant durable selector"),
        None,
        "ordinal exhaustion cannot publish a carrierless scheduler owner"
    );
}
#[test]
fn full_ingress_does_not_persist_a_carrierless_leader_wire_barrier() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let layout = minimal_rs16_layout();
    let proposal_message = v2_maximum_structural_proposal_wire(layout, 1);
    let BlockMessage::V2(proposal_envelope) = &proposal_message else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
        unreachable!("proposal fixture carries Proposal");
    };
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, proposal.round, 2);
    let mut occurrence = 0_u64;
    loop {
        let request = InboundBlockMessage::from_authenticated_peer(
            v2_commit_certificate_request(occurrence, &validator),
            validator.clone(),
        );
        match ingress.try_push(request) {
            Ok(super::FairV2IngressPushDisposition::Enqueued) => {
                occurrence = occurrence
                    .checked_add(1)
                    .expect("bounded ingress fills before u64 exhaustion");
            }
            Err(super::FairV2IngressPushError::Full(_)) => break,
            _ => panic!("unexpected filler admission result"),
        }
    }
    assert_ne!(occurrence, 0, "the test must materialize a physical prefix");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal_message.clone(),
            validator.clone(),
        )),
        Err(super::FairV2IngressPushError::Full(_))
    ));
    assert!(
        ingress.state.lock().leader_wire_lifecycles.is_empty(),
        "ordinary backpressure must not leave a durable off-queue barrier"
    );
    while ingress.try_recv_if(|_| true).is_some() {}
    assert!(
        matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                v2_commit_certificate_request(occurrence, &validator),
                validator.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ),
        "unrelated traffic must remain admissible after the rejected packet disappears"
    );
    assert!(ingress.try_recv_if(|_| true).is_some());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal_message,
            validator,
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles.len(),
        1,
        "the exact lifecycle begins only with its physically owned carrier"
    );
}
#[test]
fn sealed_height_retirement_parks_late_productive_ingress_before_volatile_release() {
    let ingress = Arc::new(super::FairV2Ingress::new(64, 1 << 20, 1 << 18, 0, 0));
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let proposal_message = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 1);
    let BlockMessage::V2(proposal_envelope) = &proposal_message else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
        unreachable!("proposal fixture carries Proposal");
    };
    let round = proposal.round;
    let directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            v2_commit_certificate_request(0, &validator),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal_message,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let (gate, token) = {
        let state = ingress.state.lock();
        assert_eq!(state.len, 2, "the rollover race retains both raw carriers");
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .expect("the exact height gate remains bound");
        let token = state
            .leader_wire_lifecycles
            .values()
            .find(|record| record.status == super::FairV2IngressLeaderWireStatus::Ingress)
            .expect("the productive carrier owns one durable Ingress lifecycle")
            .token
            .clone();
        (gate, token)
    };

    ingress
        .retire_leader_wire_lifecycle_gate(&gate)
        .expect("sealed rollover parks the admitted productive carrier");
    {
        let state = ingress.state.lock();
        assert!(!state.open);
        assert_eq!(state.len, 0);
        assert_eq!(state.bytes, 0);
        assert!(state.ready.is_empty());
        assert!(state.pending_wire_owners.is_empty());
        assert!(state.leader_wire_lifecycles.is_empty());
        assert!(state.leader_wire_lifecycle_gate.is_none());
    }
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("inspect sealed selector ownership"),
        None,
        "a parked carrier cannot survive as an active cold-start selector owner"
    );
    drop(gate);
    drop(ingress);

    let owner = [0xA6; 32];
    let roster = [validator].into_iter().collect();
    let capacity =
        super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(1, 2)
            .expect("finite leader-wire geometry");
    let recovery_authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (reopened, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &directory.path().join("safety.wal"),
        round.context_id,
        round.height,
        owner,
        roster,
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("cold-open the gate after sealed rollover");
    assert_eq!(restore.records().len(), 1);
    assert_eq!(restore.records()[0].token(), &token);
    assert_eq!(
        restore.records()[0].status(),
        super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    );
    assert_eq!(restore.last_admission_ordinal(), token.admission_ordinal());
    assert_eq!(
        restore.scheduler_ordinal_high_watermark(),
        token.scheduler_ordinal()
    );
    assert_eq!(
        reopened
            .earliest_ingress_scheduler_ordinal()
            .expect("inspect cold selector ownership"),
        None,
        "a parked sidecar carrier cannot reopen as active ingress ownership"
    );
}

#[test]
fn sealed_height_retirement_crash_after_dormant_fsync_reopens_without_a_carrier() {
    let ingress = Arc::new(super::FairV2Ingress::new(64, 1 << 20, 1 << 18, 0, 0));
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let proposal_message = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 1);
    let BlockMessage::V2(proposal_envelope) = &proposal_message else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
        unreachable!("proposal fixture carries Proposal");
    };
    let round = proposal.round;
    let directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal_message,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let (gate, carriers, token) = {
        let state = ingress.state.lock();
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .expect("the exact height gate remains bound");
        let carriers = state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .filter_map(|entry| entry.leader_wire_token.as_ref())
            .map(|token| (token.slot.clone(), token.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let token = carriers
            .values()
            .next()
            .expect("the productive proposal retains one physical carrier")
            .clone();
        (gate, carriers, token)
    };
    let retirement = gate
        .park_sealed_ingress(carriers)
        .expect("publish the Dormant cut before the injected crash");
    // Inject the process cut before the infallible volatile-clear tail. Both the
    // fair queue and its in-memory mirror disappear with the process owner.
    retirement.abandon_at_crash_cut();
    drop(ingress);
    drop(gate);

    let owner = [0xA6; 32];
    let roster = [validator].into_iter().collect();
    let capacity =
        super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(1, 2)
            .expect("finite leader-wire geometry");
    let recovery_authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (reopened, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &directory.path().join("safety.wal"),
        round.context_id,
        round.height,
        owner,
        roster,
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("cold-open the post-fsync crash cut");
    assert_eq!(restore.records().len(), 1);
    assert_eq!(restore.records()[0].token(), &token);
    assert_eq!(
        restore.records()[0].status(),
        super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    );
    assert_eq!(
        reopened
            .earliest_ingress_scheduler_ordinal()
            .expect("inspect post-crash selector ownership"),
        None
    );
}

#[test]
fn sealed_height_retirement_persistence_failure_keeps_the_exact_carrier_bound() {
    let ingress = Arc::new(super::FairV2Ingress::new(64, 1 << 20, 1 << 18, 0, 0));
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let proposal_message = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 1);
    let BlockMessage::V2(proposal_envelope) = &proposal_message else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
        unreachable!("proposal fixture carries Proposal");
    };
    let directory = bind_test_leader_wire_gate(&ingress, &validator, proposal.round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal_message,
            validator,
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let (gate, token) = {
        let state = ingress.state.lock();
        let record = state
            .leader_wire_lifecycles
            .values()
            .next()
            .expect("the admitted proposal owns its exact lifecycle");
        (
            state
                .leader_wire_lifecycle_gate
                .as_ref()
                .cloned()
                .expect("the exact gate remains bound"),
            record.token.clone(),
        )
    };
    let snapshot = directory.path().join("safety.wal.leader-wire-lifecycles");
    std::fs::remove_file(&snapshot).expect("remove the test snapshot");
    std::fs::create_dir(&snapshot).expect("block atomic snapshot replacement");
    assert!(
        ingress.retire_leader_wire_lifecycle_gate(&gate).is_err(),
        "the volatile carrier cannot clear before Dormant fsync"
    );
    {
        let state = ingress.state.lock();
        assert!(!state.open, "persistence failure must remain fail closed");
        assert_eq!(state.len, 1);
        assert!(
            state
                .leader_wire_lifecycle_gate
                .as_ref()
                .is_some_and(|bound| {
                    super::serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(
                        bound, &gate,
                    )
                })
        );
        assert_eq!(
            state.leader_wire_lifecycles[&token.slot].status,
            super::FairV2IngressLeaderWireStatus::Ingress
        );
        assert!(
            state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .any(|entry| entry.leader_wire_token.as_ref() == Some(&token))
        );
    }
    assert_eq!(
        gate.restore().expect("inspect rolled-back gate").records()[0].status(),
        super::serviced_candidate_store::LeaderWireLifecycleStatus::Ingress
    );
    std::fs::remove_dir(&snapshot).expect("restore the test publication target");
    ingress
        .retire_leader_wire_lifecycle_gate(&gate)
        .expect("the retained authority retries the exact retirement");
}

#[test]
fn sealed_height_retirement_parks_ingress_without_consuming_runtime_owners() {
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let proposal = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 1);
    let BlockMessage::V2(proposal_envelope) = &proposal else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal_payload) = &proposal_envelope.payload
    else {
        unreachable!("proposal fixture carries Proposal");
    };
    let round = proposal_payload.round;
    let mut timeout = v2_timeout_vote();
    let BlockMessage::V2(timeout_envelope) = &mut timeout else {
        unreachable!("timeout fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote) = &mut timeout_envelope.payload
    else {
        unreachable!("timeout fixture carries TimeoutVote");
    };
    timeout_vote.round = round;
    let timeout_bytes = encoded_v2_len(&timeout);
    let ingress = Arc::new(super::FairV2Ingress::new(
        64,
        1 << 20,
        1 << 18,
        timeout_bytes,
        0,
    ));
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let selected = ingress
        .try_recv_if(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Proposal(_),
                    ..
                })
            )
        })
        .expect("the proposal crosses into exact Runtime ownership");
    assert!(
        selected
            .ingress_ownership()
            .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some())
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            timeout, validator,
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let gate = ingress
        .state
        .lock()
        .leader_wire_lifecycle_gate
        .as_ref()
        .cloned()
        .expect("the mixed Runtime and Ingress owners retain their gate");

    ingress
        .retire_leader_wire_lifecycle_gate(&gate)
        .expect("retirement parks only the physical Ingress owner");
    let restore = gate.restore().expect("inspect the exact mixed durable cut");
    assert_eq!(restore.records().len(), 2);
    assert_eq!(
        restore
            .records()
            .iter()
            .filter(|record| {
                record.status()
                    == super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
            })
            .count(),
        1,
        "only the queued timeout returns to Dormant"
    );
    assert_eq!(
        restore
            .records()
            .iter()
            .filter(|record| {
                record.status()
                    == super::serviced_candidate_store::LeaderWireLifecycleStatus::Runtime
            })
            .count(),
        1,
        "the unrelated downstream Runtime owner remains untouched"
    );
}

#[test]
fn sealed_height_retirement_requires_all_three_queued_token_projections() {
    let ingress = Arc::new(super::FairV2Ingress::new(64, 1 << 20, 1 << 18, 0, 0));
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let proposal = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 1);
    let BlockMessage::V2(proposal_envelope) = &proposal else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal_payload) = &proposal_envelope.payload
    else {
        unreachable!("proposal fixture carries Proposal");
    };
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, proposal_payload.round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal, validator,
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let (gate, token) = {
        let mut state = ingress.state.lock();
        let gate = state
            .leader_wire_lifecycle_gate
            .as_ref()
            .cloned()
            .expect("the exact gate remains bound");
        let entry = state
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .next()
            .expect("one productive entry remains queued");
        let token = entry
            .leader_wire_token
            .take()
            .expect("the side field owns the productive token");
        (gate, token)
    };
    assert!(
        ingress.retire_leader_wire_lifecycle_gate(&gate).is_err(),
        "the side-field token cannot disagree with the two sealed evidence carriers"
    );
    {
        let mut state = ingress.state.lock();
        let entry = state
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .next()
            .expect("the rejected retirement retains the entry");
        entry.leader_wire_token = Some(token.clone());
        Arc::make_mut(&mut entry.inbound)
            .ingress_ownership
            .as_mut()
            .expect("the inbound envelope retains its evidence")
            .leader_wire_token = None;
    }
    assert!(
        ingress.retire_leader_wire_lifecycle_gate(&gate).is_err(),
        "the inbound token cannot disagree with the side field and immutable snapshot"
    );
    {
        let mut state = ingress.state.lock();
        let entry = state
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .next()
            .expect("the second rejected retirement retains the entry");
        Arc::make_mut(&mut entry.inbound)
            .ingress_ownership
            .as_mut()
            .expect("the inbound envelope retains its evidence")
            .leader_wire_token = Some(token.clone());
        Arc::make_mut(&mut entry.ownership_snapshot).leader_wire_token = None;
    }
    assert!(
        ingress.retire_leader_wire_lifecycle_gate(&gate).is_err(),
        "the immutable snapshot cannot disagree with both physical projections"
    );
    {
        let mut state = ingress.state.lock();
        let entry = state
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .next()
            .expect("the third rejected retirement retains the entry");
        Arc::make_mut(&mut entry.ownership_snapshot).leader_wire_token = Some(token);
    }
    ingress
        .retire_leader_wire_lifecycle_gate(&gate)
        .expect("three exact token projections permit the sealed retirement");
}
#[test]
fn delayed_proposal_keeps_first_chunk_lossless_without_a_global_orphan_barrier() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    ingress.close();
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    ingress
        .configure_roster([validator.clone()])
        .expect("one-validator fair-ingress geometry");
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = 2;
    let layout = minimal_rs16_layout();
    let proposal_message = v2_maximum_structural_proposal_wire(layout, 1);
    let BlockMessage::V2(proposal_envelope) = &proposal_message else {
        unreachable!("proposal fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
        unreachable!("proposal fixture carries Proposal");
    };
    let manifest_hash = HashOf::new(&proposal.manifest);
    let round = proposal.round;
    let directory = TempDir::new().expect("temporary leader-wire directory");
    let wal_path = directory.path().join("safety.wal");
    let owner = [0xA5; 32];
    let roster = [validator.clone()].into_iter().collect();
    let capacity =
        super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(1, 2)
            .expect("finite leader-wire geometry");
    let recovery_authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (gate, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &wal_path,
        round.context_id,
        round.height,
        owner,
        roster,
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open exact leader-wire gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            round.context_id,
            round.height,
        )
        .expect("bind exact leader-wire gate");
    ingress.open().expect("open bound fair ingress");
    let chunk_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash,
            index: 0,
            bytes: vec![0x5A],
            sender: 0,
            signature: vec![0xC3],
        }),
    ));
    assert!(
        matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                chunk_message.clone(),
                validator.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ),
        "a chunk reordered before Proposal must reach the bounded worker orphan lifecycle"
    );
    assert!(
        matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                chunk_message.clone(),
                validator.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ),
        "an exact physical retransmission must retain one ingress owner"
    );
    assert!(
        ingress.state.lock().leader_wire_lifecycles.is_empty(),
        "an unbound chunk must not mint a Byzantine-pinnable global scheduler owner"
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            proposal_message,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(
        matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                chunk_message,
                validator,
            )),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ),
        "global exact-wire coalescing must run before the now-bindable chunk can mint a rank"
    );
    assert!(
        ingress
            .state
            .lock()
            .leader_wire_lifecycles
            .values()
            .all(|record| {
                record.token.identity.phase != super::FairV2IngressLeaderWirePhase::Chunk
            }),
        "a Proposal must not retrofit a durable lifecycle onto the already queued proofless chunk"
    );
    let chunk = ingress
        .try_recv_if(|_| true)
        .expect("the frozen physical predecessor chunk drains first");
    assert_eq!(payload_chunk_index(&chunk), Some(0));
    assert!(
        chunk
            .ingress_ownership()
            .is_some_and(|ownership| ownership.leader_wire_token().is_none()),
        "the proofless orphan episode retains fair ownership without exact rank ownership"
    );
    let proposal = ingress
        .try_recv_if(|_| true)
        .expect("the exact Proposal drains after its frozen predecessor");
    assert!(matches!(
        proposal.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(_),
            ..
        })
    ));
    assert!(
        proposal
            .ingress_ownership()
            .is_some_and(|ownership| ownership.leader_wire_token().is_some()),
        "manifest-bound Proposal begins the durable exact lifecycle"
    );
}
#[test]
fn retained_vote_does_not_hide_matching_proposal_or_transport_completion() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let vote_message = v2_vote(wire::GlobalPhase::Prepare);
    let (vote_round, proposal_round, subject) = match &vote_message {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) => (vote.round, vote.proposal_round, vote.subject),
        _ => unreachable!("vote fixture carries a v2 Vote"),
    };
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, vote_round, 2);
    let layout = minimal_rs16_layout();
    let mut proposal_message = v2_maximum_structural_proposal_wire(layout, 1);
    let manifest_hash = match &mut proposal_message {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
            ..
        }) => {
            proposal.round = proposal_round;
            proposal.subject = subject;
            proposal.manifest.round = proposal_round;
            proposal.manifest.subject = subject;
            HashOf::new(&proposal.manifest)
        }
        _ => unreachable!("proposal fixture carries a v2 Proposal"),
    };
    let chunk_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash,
            index: 0,
            bytes: vec![0xA5],
            sender: 0,
            signature: vec![0x5A],
        }),
    ));
    for message in [vote_message.clone(), proposal_message, chunk_message] {
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message,
                validator.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
    }
    let proposal = ingress
        .try_recv_if(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Proposal(_),
                    ..
                })
            )
        })
        .expect("a matching Proposal bypasses the retained Vote that it unblocks");
    assert!(matches!(
        proposal.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(_),
            ..
        })
    ));
    assert_eq!(ingress.state.lock().len, 2);
    let chunk = ingress
        .try_recv_if(super::fair_v2_ingress_is_transport_completion)
        .expect("body completion bypasses retained reducer-control ownership");
    assert_eq!(payload_chunk_index(&chunk), Some(0));
    assert_eq!(ingress.state.lock().len, 1);
    let retained_vote = ingress
        .try_recv_if(|_| true)
        .expect("the retained Vote remains owned after its dependencies drain");
    assert_eq!(retained_vote.message().encode(), vote_message.encode());
    assert_eq!(ingress.state.lock().len, 0);
}
#[test]
fn retained_vote_does_not_hide_timeout_certificate_that_closes_its_view() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let mut vote_message = v2_vote(wire::GlobalPhase::Prepare);
    let vote_round = match &mut vote_message {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) => {
            vote.round.view = 1;
            vote.proposal_round.view = 1;
            vote.round
        }
        _ => unreachable!("vote fixture carries a v2 Vote"),
    };
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, vote_round, 1);
    let mut timeout_certificate = v2_timeout_certificate(vote_round.view);
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ..
    }) = &mut timeout_certificate
    else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    certificate.round = vote_round;
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            vote_message.clone(),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let vote_token = ingress
        .state
        .lock()
        .leader_wire_lifecycles
        .values()
        .next()
        .expect("the retained Vote owns the first leader-wire lifecycle")
        .token
        .clone();
    let mut stale_timeout_certificate = timeout_certificate.clone();
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(stale),
        ..
    }) = &mut stale_timeout_certificate
    else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    stale.round.view = vote_round.view - 1;
    let mut later_timeout_certificate = timeout_certificate.clone();
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(later),
        ..
    }) = &mut later_timeout_certificate
    else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    later.round.view = vote_round.view + 1;
    assert!(!super::fair_v2_ingress_timeout_control_advances_owner(
        &vote_token,
        &InboundBlockMessage::from_authenticated_peer(stale_timeout_certificate, validator.clone()),
    ));
    assert!(super::fair_v2_ingress_timeout_control_advances_owner(
        &vote_token,
        &InboundBlockMessage::from_authenticated_peer(later_timeout_certificate, validator.clone()),
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            timeout_certificate,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let installed_timeout = ingress
        .try_recv_if(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
                    ..
                })
            )
        })
        .expect("a TC can reach verification when the selected Vote is body-blocked");
    assert!(matches!(
        installed_timeout.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
            ..
        })
    ));
    assert_eq!(ingress.state.lock().len, 1);
    let retained_vote = ingress
        .try_recv_if(|_| true)
        .expect("the superseded Vote remains exactly owned until normal retirement");
    assert_eq!(retained_vote.message().encode(), vote_message.encode());
    assert_eq!(ingress.state.lock().len, 0);
}
#[test]
fn certified_view_cut_admits_the_strict_same_round_timeout_upgrade() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let roster = validator_peers(4);
    let first_origin = roster
        .first()
        .expect("four-peer leader-wire roster")
        .clone();
    let upgrade_origin = roster.get(1).expect("four-peer leader-wire roster").clone();
    let stale_origin = roster.get(2).expect("four-peer leader-wire roster").clone();
    let thin = v2_timeout_certificate(1);
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ..
    }) = &thin
    else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    let round = certificate.round;
    let _directory = bind_test_leader_wire_gate_with_roster(&ingress, &roster, round, 2);
    let upgrade = v2_locked_timeout_certificate(round.view);
    assert_ne!(thin.encode(), upgrade.encode());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            thin,
            first_origin
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let certified_view_cut = |durable_view| {
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            [0xA6; 32],
            durable_view,
            false,
        )
    };
    let opened_view = round.view.checked_add(1).expect("fixture view advances");
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(certified_view_cut(opened_view))
            .expect("publish the certified view cut that installing TC(V) produces"),
        0
    );
    match ingress.try_push(InboundBlockMessage::from_authenticated_peer(
        upgrade,
        upgrade_origin,
    )) {
        Ok(super::FairV2IngressPushDisposition::Enqueued) => {}
        Ok(super::FairV2IngressPushDisposition::Coalesced) => {
            panic!("a distinct upgrade certificate is not an exact retransmission")
        }
        Err(super::FairV2IngressPushError::Rejected(rejection)) => panic!(
            "the strict same-round timeout upgrade was rejected: {:?}",
            rejection.reason
        ),
        Err(super::FairV2IngressPushError::Full(_)) => {
            panic!("the strict same-round timeout upgrade was refused for capacity")
        }
        Err(
            super::FairV2IngressPushError::Closed(_) | super::FairV2IngressPushError::FailStop(_),
        ) => panic!("the strict same-round timeout upgrade fail-stopped fair ingress"),
    }
    let later_view = opened_view.checked_add(1).expect("fixture view advances");
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(certified_view_cut(later_view))
            .expect("publish the next certified view cut"),
        0
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            v2_timeout_certificate(round.view),
            stale_origin
        )),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
}

#[test]
fn same_origin_timeout_upgrade_replaces_the_installed_terminal_certificate() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let thin = v2_timeout_certificate(1);
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ..
    }) = &thin
    else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    let round = certificate.round;
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            thin,
            validator.clone()
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut installed = ingress
        .try_recv_if(|_| true)
        .expect("the thin certificate reaches verification");
    let mut ownership = installed
        .take_ingress_ownership()
        .expect("the thin certificate retains ingress ownership");
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind the thin certificate runtime owner");
    let runtime = ownership
        .leader_wire_runtime_receipt()
        .expect("runtime receipt is installed");
    ingress
        .mark_leader_wire_volatile_terminal(runtime)
        .expect("publish the installed-certificate tombstone");
    let opened_view = round.view.checked_add(1).expect("fixture view advances");
    let next = super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
        round.context_id,
        round.height,
        [0xA6; 32],
        opened_view,
        false,
    );
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(next)
            .expect("publish the certified view cut that installing TC(V) produces"),
        0
    );
    let upgrade = v2_locked_timeout_certificate(round.view);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            upgrade,
            validator.clone()
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let state = ingress.state.lock();
    let replacement = state
        .leader_wire_lifecycles
        .values()
        .next()
        .expect("the upgrade owns the released timeout slot");
    assert_eq!(replacement.token.identity.view, round.view);
    assert_eq!(
        replacement.status,
        super::FairV2IngressLeaderWireStatus::Ingress
    );
}

#[test]
fn same_origin_timeout_upgrade_waits_on_the_active_predecessor() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let thin = v2_timeout_certificate(1);
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ..
    }) = &thin
    else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    let round = certificate.round;
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            thin,
            validator.clone()
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let upgrade = v2_locked_timeout_certificate(round.view);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            upgrade, validator
        )),
        Err(super::FairV2IngressPushError::Full(_))
    ));
    assert!(
        ingress.state.lock().open,
        "a same-round upgrade must wait without fail-stop"
    );
}

#[test]
fn certified_fence_escape_crosses_retained_control_reservation() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let mut vote_message = v2_vote(wire::GlobalPhase::Prepare);
    let vote_round = match &mut vote_message {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) => {
            vote.round.view = 1;
            vote.proposal_round.view = 1;
            vote.round
        }
        _ => unreachable!("vote fixture carries a v2 Vote"),
    };
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, vote_round, 1);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            vote_message.clone(),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut commit_response = v2_commit_certificate_response(7, &validator);
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::CommitCertificateResponse(response),
        ..
    }) = &mut commit_response
    else {
        unreachable!("commit-certificate fixture carries a v2 response");
    };
    response.certificate.round = vote_round;
    response.certificate.proposal_round = vote_round;
    assert!(super::fair_v2_ingress_is_certified_fence_escape(
        &InboundBlockMessage::from_authenticated_peer(commit_response.clone(), validator.clone()),
    ));
    let vote_token = ingress
        .state
        .lock()
        .leader_wire_lifecycles
        .values()
        .next()
        .expect("the retained Vote owns the control barrier")
        .token
        .clone();
    assert!(
        super::fair_v2_ingress_certified_fence_escape_advances_owner(
            &vote_token,
            &InboundBlockMessage::from_authenticated_peer(
                commit_response.clone(),
                validator.clone()
            ),
        )
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            commit_response,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let escaped = ingress
        .try_recv_if(super::fair_v2_ingress_is_certified_fence_escape)
        .expect("CommitQC discovery crosses the retained control reservation");
    assert!(matches!(
        escaped.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::CommitCertificateResponse(_),
            ..
        })
    ));
    assert_eq!(ingress.state.lock().len, 1);
    let timeout = v2_timeout_certificate(vote_round.view);
    assert!(super::fair_v2_ingress_is_certified_fence_escape(
        &InboundBlockMessage::from_authenticated_peer(timeout, validator.clone()),
    ));
    assert!(!super::fair_v2_ingress_is_certified_fence_escape(
        &InboundBlockMessage::from_authenticated_peer(v2_timeout_vote(), validator.clone()),
    ));
    assert!(!super::fair_v2_ingress_is_certified_fence_escape(
        &InboundBlockMessage::from_authenticated_peer(
            v2_vote(wire::GlobalPhase::Prepare),
            validator.clone(),
        ),
    ));
    let mut wrong_version = v2_timeout_certificate(vote_round.view);
    let BlockMessage::V2(message) = &mut wrong_version else {
        unreachable!("timeout fixture carries a v2 envelope");
    };
    message.protocol_version = wire::PROTOCOL_VERSION.saturating_add(1);
    assert!(!super::fair_v2_ingress_is_certified_fence_escape(
        &InboundBlockMessage::from_authenticated_peer(wrong_version, validator),
    ));
    let retained_vote = ingress
        .try_recv_if(|_| true)
        .expect("certified escape does not replace the retained Vote owner");
    assert_eq!(retained_vote.message().encode(), vote_message.encode());
    assert_eq!(ingress.state.lock().len, 0);
}
#[test]
fn retained_vote_does_not_hide_timeout_vote_needed_to_close_its_view() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let mut vote_message = v2_vote(wire::GlobalPhase::Prepare);
    let vote_round = match &mut vote_message {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) => {
            vote.round.view = 1;
            vote.proposal_round.view = 1;
            vote.round
        }
        _ => unreachable!("vote fixture carries a v2 Vote"),
    };
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, vote_round, 1);
    let mut timeout_vote = v2_timeout_vote();
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutVote(timeout),
        ..
    }) = &mut timeout_vote
    else {
        unreachable!("timeout fixture carries a v2 TimeoutVote");
    };
    timeout.round = vote_round;
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            vote_message.clone(),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let vote_token = ingress
        .state
        .lock()
        .leader_wire_lifecycles
        .values()
        .next()
        .expect("the retained Vote owns the first leader-wire lifecycle")
        .token
        .clone();
    for (view, expected) in [
        (vote_round.view - 1, false),
        (vote_round.view, true),
        (vote_round.view + 1, false),
    ] {
        let mut candidate = timeout_vote.clone();
        let BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(timeout),
            ..
        }) = &mut candidate
        else {
            unreachable!("timeout fixture carries a v2 TimeoutVote");
        };
        timeout.round.view = view;
        assert_eq!(
            super::fair_v2_ingress_timeout_control_advances_owner(
                &vote_token,
                &InboundBlockMessage::from_authenticated_peer(candidate, validator.clone()),
            ),
            expected,
            "only an exact-view timeout share can cross the blocked Vote owner"
        );
    }
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            timeout_vote,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let admitted_timeout_vote = ingress
        .try_recv_if(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                    ..
                })
            )
        })
        .expect("a timeout share can reach verification while the direct Vote is body-blocked");
    assert!(matches!(
        admitted_timeout_vote.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
            ..
        })
    ));
    assert_eq!(ingress.state.lock().len, 1);
    let retained_vote = ingress
        .try_recv_if(|_| true)
        .expect("the timed-out Vote remains exactly owned until normal retirement");
    assert_eq!(retained_vote.message().encode(), vote_message.encode());
    assert_eq!(ingress.state.lock().len, 0);
}
#[test]
fn ingress_stays_closed_until_replay_owner_acknowledges_ready() {
    let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
    let sender = authenticated_peer_for_test();
    handle.ingress_ready.store(false, Ordering::Release);
    assert!(!handle.try_incoming_block_message_from(sender.clone(), v2_message()));
    assert!(receiver.try_recv().is_none());
    handle.ingress_ready.store(true, Ordering::Release);
    assert!(handle.try_incoming_block_message_from(sender, v2_message()));
    assert!(receiver.try_recv().is_some());
}
#[test]
fn authenticated_lane_drain_votes_enter_the_bounded_live_relay_queue() {
    let (handle, _receiver, relay_receiver) = test_sumeragi_handle(1);
    handle.ingress_ready.store(true, Ordering::Release);
    let keypair = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
        .expect("generate BLS-normal lane-drain signer");
    let signer = PeerId::new(keypair.public_key().clone());
    let validator_set = vec![signer.clone()];
    let vote = crate::lane_consensus::LaneDrainVoteV1::new_signed(
        LaneDrainCertificateBodyV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                    iroha_data_model::block::BlockHeader,
                >::from_untyped_unchecked(
                    Hash::new(b"live-drain-ingress-genesis"),
                )),
                lane_id: LaneId::new(7),
                dataspace_id: DataSpaceId::new(9),
                lane_incarnation: Hash::new(b"live-drain-ingress-incarnation"),
                close_global_height: 3,
                initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(7),
                    DataSpaceId::new(9),
                    Hash::new(b"live-drain-ingress-incarnation"),
                    0,
                    None,
                ),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                LaneId::new(7),
                DataSpaceId::new(9),
                Hash::new(b"live-drain-ingress-incarnation"),
                0,
                None,
            ),
        },
        signer.clone(),
        keypair.private_key(),
    )
    .expect("sign valid lane-drain vote");
    assert!(handle.try_incoming_lane_drain_vote(signer.clone(), vote.clone()));
    let LaneRelayMessage::DrainVote {
        sender,
        vote: queued_vote,
    } = relay_receiver
        .try_recv()
        .expect("valid drain vote reaches the bounded relay queue")
    else {
        panic!("valid drain vote changed relay message kind");
    };
    assert_eq!(sender, signer);
    assert_eq!(queued_vote, vote);
    let mismatched_sender = PeerId::new(KeyPair::random().public_key().clone());
    assert!(!handle.try_incoming_lane_drain_vote(mismatched_sender, vote.clone()));
    assert!(relay_receiver.try_recv().is_err());
    let mut tampered = vote;
    tampered.bls_signature[0] ^= 0x01;
    assert!(!handle.try_incoming_lane_drain_vote(signer, tampered));
    assert!(relay_receiver.try_recv().is_err());
}
#[test]
fn v2_ingress_is_bounded_and_never_blocks_a_network_caller() {
    let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
    let sender = authenticated_peer_for_test();
    handle.ingress_ready.store(true, Ordering::Release);
    assert!(handle.try_incoming_block_message_from(sender.clone(), v2_message()));
    assert!(
        !handle.try_incoming_block_message_from(sender.clone(), v2_auxiliary_prepare(1)),
        "a distinct message at saturated capacity must reject promptly and rely on retransmission"
    );
    let _ = receiver.try_recv().expect("drain the bounded v2 queue");
    assert!(handle.try_incoming_block_message_from(sender, v2_message()));
}
#[test]
fn saturated_v2_ingress_returns_the_exact_owned_message_for_retry() {
    let (handle, receiver, _relay_receiver) = test_sumeragi_handle_with_source_geometry(4, Some(1));
    let sender = validator_peers(1).pop().expect("sender fixture");
    assert!(matches!(
        handle.try_incoming_block_message_from_owned(sender.clone(), v2_message()),
        super::SumeragiIngressDisposition::Accepted
    ));
    let retry =
        handle.try_incoming_block_message_from_owned(sender.clone(), v2_auxiliary_prepare(1));
    let super::SumeragiIngressDisposition::Retry(inbound) = retry else {
        panic!("saturated ingress must return caller ownership");
    };
    assert_eq!(inbound.sender(), &sender);
    assert_eq!(vote_height(&inbound), Some(2));
    let _ = receiver
        .try_recv()
        .expect("release bounded ingress capacity");
    assert!(matches!(
        handle.try_incoming_block_message_owned(inbound),
        super::SumeragiIngressDisposition::Accepted
    ));
}
#[test]
fn direct_envelopes_bind_both_identity_roles() {
    let sender = validator_peers(1).pop().expect("sender fixture");
    let direct = InboundBlockMessage::from_authenticated_peer(v2_message(), sender.clone());
    assert_eq!(direct.sender(), &sender);
    assert_eq!(direct.via(), &sender);
}
#[test]
fn atomic_lane_certificate_uses_the_shared_progress_owner() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(1);
    let sender = authenticated_peer_for_test();
    let certificate = lane_block_certificate(71);
    let expected = certificate.encode();
    assert_eq!(
        FairV2IngressClass::classify(&InboundBlockMessage::from_authenticated_peer(
            certificate.clone(),
            sender.clone(),
        )),
        FairV2IngressClass::Progress
    );
    assert!(matches!(
        handle.try_incoming_block_message_owned(InboundBlockMessage::from_authenticated_peer(
            certificate,
            sender,
        )),
        super::SumeragiIngressDisposition::Accepted
    ));
    let retained = ingress
        .try_recv()
        .expect("shared fair ingress retains the lane certificate");
    assert_eq!(retained.message().encode(), expected);
}
#[test]
fn oversized_atomic_lane_certificate_is_returned_exactly() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(1);
    let sender = authenticated_peer_for_test();
    let mut certificate = lane_block_certificate(72);
    let BlockMessage::LaneBlockCertificate(envelope) = &mut certificate else {
        unreachable!("fixture is an atomic lane certificate")
    };
    envelope.commit_qc.bls_aggregate_signature =
        vec![0xA5; super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES];
    let expected = certificate.encode();
    assert!(expected.len() > super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES);
    let disposition = handle.try_incoming_block_message_owned(
        InboundBlockMessage::from_authenticated_peer(certificate, sender),
    );
    let super::SumeragiIngressDisposition::Rejected(retained) = disposition else {
        panic!("oversized lane certificate must be rejected with exact ownership")
    };
    assert_eq!(retained.message().encode(), expected);
    assert!(ingress.try_recv().is_none());
}
#[test]
fn saturated_lane_ingress_returns_the_exact_owned_message_for_retry() {
    let (handle, _receiver, relay_receiver) = test_sumeragi_handle(1);
    let first = MergeCommitteeSignature {
        version: iroha_data_model::merge::MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: 7,
        view: 1,
        signer: 0,
        message_digest: Hash::new(b"first retained lane item"),
        bls_sig: vec![0xA5],
        leader_candidate_body: None,
    };
    let second = MergeCommitteeSignature {
        version: iroha_data_model::merge::MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: 7,
        view: 2,
        signer: 0,
        message_digest: Hash::new(b"second retained lane item"),
        bls_sig: vec![0x5A],
        leader_candidate_body: None,
    };
    assert!(matches!(
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::MergeSignature(first)),
        super::SumeragiIngressDisposition::Accepted
    ));
    let retry =
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::MergeSignature(second));
    let super::SumeragiIngressDisposition::Retry(message) = retry else {
        panic!("saturated lane ingress must return caller ownership");
    };
    let super::LaneRelayMessage::MergeSignature(retained) = &message else {
        panic!("retry must preserve the exact lane message variant");
    };
    assert_eq!(retained.view, 2);
    assert_eq!(retained.bls_sig, vec![0x5A]);
    let _ = relay_receiver
        .try_recv()
        .expect("release bounded lane ingress capacity");
    assert!(matches!(
        handle.try_incoming_lane_relay_owned(message),
        super::SumeragiIngressDisposition::Accepted
    ));
}
#[test]
fn sidecar_allocations_defer_historical_roster_proof_to_bounded_lane_owner() {
    use crate::merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarCloseV1,
        CertifiedMergeSidecarMessage, CertifiedMergeSidecarRequestV1,
        CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
        CertifiedMergeSidecarStreamEpochV1,
    };
    use std::num::NonZeroU64;
    let ingress_capacity = super::fair_v2_ingress_required_capacity(1, None)
        .expect("one-validator ingress geometry is representable");
    assert_eq!(ingress_capacity, 5);
    let (handle, ingress, relay_receiver) = test_sumeragi_handle(ingress_capacity);
    let mut peers = validator_peers(3);
    let roster_requester = peers.remove(0);
    let outsider = peers.remove(0);
    let hub = peers.remove(0);
    ingress.close();
    ingress
        .configure_roster([roster_requester.clone()])
        .expect("one frozen sidecar requester fits the ingress geometry");
    ingress.open().expect("open the frozen sidecar roster");
    let request_for = |requester: &PeerId| {
        let mut request = CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch: CertifiedMergeSidecarStreamEpochV1(NonZeroU64::MIN),
            semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(NonZeroU64::MIN),
            closed_through: 0,
            request_id: Hash::prehashed([0; Hash::LENGTH]),
            entry_hash: HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(
                b"early sidecar roster gate",
            )),
            encoded_len: 1,
            epoch_id: 1,
            reference_digest: Hash::new(b"early sidecar roster reference"),
            requester: requester.clone(),
            responder: roster_requester.clone(),
        };
        request.request_id = request.canonical_request_id();
        request
    };
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 8);
    let outsider_request = request_for(&outsider);
    let outsider_route = routes.mint_via(outsider.clone(), hub.clone());
    let admitted =
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
            sender: outsider.clone(),
            reply_route: Some(outsider_route),
            message: CertifiedMergeSidecarMessage::Request(outsider_request.clone()),
        });
    assert!(matches!(
        admitted,
        super::SumeragiIngressDisposition::Accepted
    ));
    assert!(matches!(
        relay_receiver
            .try_recv()
            .expect("serialized adapter receives the exact historical proof candidate"),
        super::LaneRelayMessage::CertifiedMergeSidecar {
            sender,
            reply_route: Some(route),
            message: CertifiedMergeSidecarMessage::Request(request),
        } if sender == outsider
            && request == outsider_request
            && route.is_authenticated_via(&hub)
            && route.semantic_target() == &outsider
    ));
    let mut outsider_close = CertifiedMergeSidecarCloseV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(NonZeroU64::MIN),
        closed_through: 1,
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: outsider.clone(),
        responder: roster_requester.clone(),
    };
    outsider_close.close_id = outsider_close.canonical_close_id();
    let expected_outsider_close = outsider_close.clone();
    let outsider_close_route = routes.mint_via(outsider.clone(), hub.clone());
    assert!(matches!(
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
            sender: outsider.clone(),
            reply_route: Some(outsider_close_route),
            message: CertifiedMergeSidecarMessage::Close(outsider_close),
        },),
        super::SumeragiIngressDisposition::Accepted
    ));
    assert!(matches!(
        relay_receiver
            .try_recv()
            .expect("serialized adapter receives the historical close candidate"),
        super::LaneRelayMessage::CertifiedMergeSidecar {
            sender,
            reply_route: Some(route),
            message: CertifiedMergeSidecarMessage::Close(close),
        } if sender == outsider
            && close == expected_outsider_close
            && route.is_authenticated_via(&hub)
            && route.semantic_target() == &outsider
    ));
    let mismatched_request = request_for(&outsider);
    let roster_route = routes.mint_via(roster_requester.clone(), hub.clone());
    assert!(matches!(
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
            sender: roster_requester.clone(),
            reply_route: Some(roster_route),
            message: CertifiedMergeSidecarMessage::Request(mismatched_request),
        },),
        super::SumeragiIngressDisposition::Rejected(_)
    ));
    assert!(
        matches!(
            relay_receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ),
        "a roster transport identity cannot allocate for another semantic requester"
    );
    let outsider_request = request_for(&outsider);
    let wrong_target_route = routes.mint_via(roster_requester.clone(), hub.clone());
    assert!(matches!(
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
            sender: outsider.clone(),
            reply_route: Some(wrong_target_route),
            message: CertifiedMergeSidecarMessage::Request(outsider_request),
        },),
        super::SumeragiIngressDisposition::Rejected(_)
    ));
    assert!(
        matches!(
            relay_receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ),
        "a reply route for another semantic peer cannot reach the proof owner"
    );
    let roster_request = request_for(&roster_requester);
    let roster_route = routes.mint_via(roster_requester.clone(), hub.clone());
    assert!(matches!(
        handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
            sender: roster_requester.clone(),
            reply_route: Some(roster_route),
            message: CertifiedMergeSidecarMessage::Request(roster_request.clone()),
        },),
        super::SumeragiIngressDisposition::Accepted
    ));
    assert!(matches!(
        relay_receiver
            .try_recv()
            .expect("a roster requester may use an authenticated non-roster relay"),
        super::LaneRelayMessage::CertifiedMergeSidecar {
            sender,
            reply_route: Some(route),
            message: CertifiedMergeSidecarMessage::Request(request),
        } if sender == roster_requester
            && request == roster_request
            && route.is_authenticated_via(&hub)
    ));
}
#[test]
fn restart_required_ingress_rejects_before_queue_mutation() {
    let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
    let sender = authenticated_peer_for_test();
    handle.output_guard.activate_restart_required();
    assert!(handle.restart_required());
    assert!(!handle.try_incoming_block_message_from(sender, v2_message()));
    assert!(
        receiver.try_recv().is_none(),
        "restart-required admission must not mutate the bounded ingress queue"
    );
}
fn validator_peers(count: u8) -> Vec<PeerId> {
    (0..count)
        .map(|seed| {
            PeerId::new(
                KeyPair::try_from_seed(
                    vec![seed.saturating_add(1); 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
                .expect("derive deterministic ingress peer")
                .public_key()
                .clone(),
            )
        })
        .collect()
}
#[test]
fn byzantine_v2_source_cannot_consume_honest_ingress_reservations_or_service_turns() {
    // The exact N=4, H=2 corridor needs 28 slots. Add one deliberate
    // ordinary-pressure slot so this test can retain two attacker items
    // while still proving that a third cannot consume any protected slot.
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle_with_source_geometry(29, Some(2));
    let validators = validator_peers(4);
    let attacker = validators[0].clone();
    let outsider = validator_peers(5).pop().expect("outsider fixture");
    ingress.close();
    ingress
        .configure_roster(validators.clone())
        .expect("four validators and their protected slots fit");
    ingress.open().expect("open configured roster");
    for index in 0..2 {
        assert!(
            handle.try_incoming_block_message_from(attacker.clone(), v2_auxiliary_prepare(index),)
        );
    }
    assert!(
        !handle.try_incoming_block_message_from(attacker.clone(), v2_auxiliary_prepare(2),),
        "attacker cannot consume ordinary, progress, or TimeoutVote slots reserved for empty validator lanes"
    );
    for honest in validators.iter().skip(1) {
        assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));
    }
    assert!(handle.try_incoming_block_message_from(outsider.clone(), v2_message()));
    assert_eq!(ingress.len(), 6);
    let first_cycle = (0..5)
        .map(|_| {
            ingress
                .try_recv()
                .expect("one ready source per fair service turn")
                .into_message_and_sender()
                .1
        })
        .collect::<Vec<_>>();
    assert_eq!(
        first_cycle,
        vec![
            attacker,
            validators[1].clone(),
            validators[2].clone(),
            validators[3].clone(),
            outsider,
        ]
    );
    assert_eq!(ingress.len(), 1, "only the attacker's second item remains");
}
#[test]
fn relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin() {
    const RELAYED_ORIGINS: usize = 32;
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(23);
    let validators = validator_peers(4);
    let via = validators[0].clone();
    let lane_origin = validators[1].clone();
    let origins = validator_peers(64)
        .into_iter()
        .skip(validators.len())
        .take(RELAYED_ORIGINS)
        .collect::<Vec<_>>();
    ingress.close();
    ingress
        .configure_roster(validators.clone())
        .expect("four validator owners fit");
    ingress.open().expect("open configured roster");
    let mut accepted = 0_usize;
    for (index, origin) in origins.iter().enumerate() {
        let inbound = InboundBlockMessage::from_transport(
            v2_auxiliary_prepare(u64::try_from(index).expect("fixture index fits u64")),
            origin.clone(),
            via.clone(),
        );
        match handle.try_incoming_block_message_owned(inbound) {
            super::SumeragiIngressDisposition::Accepted => accepted += 1,
            super::SumeragiIngressDisposition::Retry(retained) => {
                assert_eq!(retained.sender(), origin);
                assert_eq!(retained.via(), &via);
            }
            disposition => panic!("unexpected relayed-origin disposition: {disposition:?}"),
        }
    }
    assert_eq!(
        accepted, 2,
        "semantic-origin churn must remain inside one validator lane instead of multiplying its reserved slots"
    );
    {
        let state = ingress.state.lock();
        let nonempty = state
            .lanes
            .iter()
            .filter(|(_, lane)| !lane.entries.is_empty())
            .map(|(source, _)| source.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            nonempty,
            vec![super::FairV2IngressSource::Validator(via.clone())]
        );
        assert_eq!(
            state.ready,
            std::collections::VecDeque::from([nonempty[0].clone()])
        );
    }
    assert!(
        handle.try_incoming_block_message_from(validators[2].clone(), v2_message()),
        "one relayed via cannot consume a responsive validator's reserved owner"
    );
    let first = ingress
        .try_recv()
        .expect("oldest relayed origin owns the via's first fair turn");
    assert_eq!(first.sender(), &origins[0]);
    assert_eq!(first.via(), &via);
    let responsive = ingress
        .try_recv()
        .expect("responsive validator follows after one via turn");
    assert_eq!(responsive.sender(), &validators[2]);
    let second = ingress
        .try_recv()
        .expect("the via retains its second admitted origin");
    assert_eq!(second.sender(), &origins[1]);
    assert!(ingress.try_recv().is_none());
    assert!(matches!(
        handle.try_incoming_block_message_owned(InboundBlockMessage::from_transport(
            lane_block_certificate(73),
            lane_origin.clone(),
            via.clone(),
        )),
        super::SumeragiIngressDisposition::Accepted
    ));
    let inbound = ingress
        .try_recv()
        .expect("relayed lane certificate reaches serialized validation");
    assert_eq!(inbound.sender(), &lane_origin);
    assert_eq!(inbound.via(), &via);
    let (message, sender) = inbound.into_message_and_sender();
    assert_eq!(sender, lane_origin);
    assert!(matches!(message, BlockMessage::LaneBlockCertificate(_)));
}
#[test]
fn kura_replica_advert_requires_exact_signed_direct_keeper_ownership() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let keeper_key = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
        .expect("generate BLS-normal Kura replica keeper key");
    let keeper = PeerId::new(keeper_key.public_key().clone());
    let mut advert = super::message::KuraReplicaAdvertV1 {
        version: super::message::KURA_REPLICA_ADVERT_VERSION_V1,
        network_id: crate::sumeragi::synthetic_network_id("fair-ingress-kura-advert"),
        height: 13,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replica-block")),
        executed_block_wire_len: 4096,
        executed_block_wire_hash: Hash::new(b"replica-wire"),
        finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(b"replica-finality")),
        keeper_index: 0,
        keeper: keeper.clone(),
        signature: Vec::new(),
    };
    advert.signature =
        iroha_crypto::Signature::new(keeper_key.private_key(), &advert.signature_preimage())
            .payload()
            .to_vec();
    let message = BlockMessage::KuraReplicaAdvert(advert.clone());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_transport(
            message.clone(),
            keeper.clone(),
            keeper.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let admitted = ingress
        .try_recv()
        .expect("direct signed keeper advert owns one fair-ingress carrier");
    assert!(admitted.ingress_ownership().is_some_and(|ownership| {
        ownership.validate_exact()
            && ownership.matches_message(admitted.message())
            && ownership.matches_semantic_origin(&keeper)
    }));
    let other = PeerId::new(KeyPair::random().public_key().clone());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_transport(
            message.clone(),
            other.clone(),
            keeper.clone(),
        )),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_transport(
            message,
            keeper.clone(),
            other,
        )),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    advert.signature[0] ^= 0x80;
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_transport(
            BlockMessage::KuraReplicaAdvert(advert),
            keeper.clone(),
            keeper,
        )),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    assert_eq!(ingress.len(), 0, "rejected adverts retain no queue owner");
}
