#[test]
fn restored_productive_retry_stays_behind_an_earlier_certified_request_carrier() {
    let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Reserved);
    assert_eq!(fixture.token.admission_ordinal(), 7);
    assert!(matches!(
        fixture.ingress.try_push(InboundBlockMessage::new(
            v2_certified_body_request(&fixture.validator),
            Some(fixture.validator.clone()),
        )),
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
        fixture.ingress.try_push(InboundBlockMessage::new(
            fixture.message.clone(),
            Some(fixture.validator.clone()),
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
    assert_eq!(target.sender(), Some(&fixture.validator));
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
        fixture.ingress.try_push(InboundBlockMessage::new(
            earlier,
            Some(fixture.validator.clone()),
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
        fixture.ingress.try_push(InboundBlockMessage::new(
            fixture.message.clone(),
            Some(fixture.validator.clone()),
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
        fixture.ingress.try_push(InboundBlockMessage::new(
            earlier_message,
            Some(fixture.alternate_validator.clone()),
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
        fixture.ingress.try_push(InboundBlockMessage::new(
            fixture.message.clone(),
            Some(fixture.validator.clone()),
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
        fixture.ingress.try_push(InboundBlockMessage::new(
            fixture.message.clone(),
            Some(fixture.validator.clone()),
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
