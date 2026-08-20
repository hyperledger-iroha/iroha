#[test]
fn restored_carrierless_leader_wires_stay_dormant_until_exact_post_capacity_replay() {
    for cut in [
        RestoredLeaderWireCut::Reserved,
        RestoredLeaderWireCut::Ingress,
        RestoredLeaderWireCut::Runtime,
        RestoredLeaderWireCut::Volatile,
    ] {
        let fixture = restored_leader_wire_fixture(cut);
        let mut occurrence = 0_u64;
        loop {
            let unrelated = InboundBlockMessage::from_authenticated_peer(
                v2_commit_certificate_request(occurrence, &fixture.validator),
                fixture.validator.clone(),
            );
            match fixture.ingress.try_push(unrelated) {
                Ok(super::FairV2IngressPushDisposition::Enqueued) => {
                    occurrence = occurrence
                        .checked_add(1)
                        .expect("bounded ingress fills before u64 exhaustion");
                }
                Err(super::FairV2IngressPushError::Full(_)) => break,
                _ => panic!("unexpected unrelated admission for {cut:?}"),
            }
        }
        assert_ne!(
            occurrence, 0,
            "unrelated traffic must enter despite the dormant {cut:?} owner"
        );
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    fixture.message.clone(),
                    fixture.validator.clone(),
                )),
            Err(super::FairV2IngressPushError::Full(_))
        ));
        {
            let state = fixture.ingress.state.lock();
            assert!(state.open, "ordinary backpressure cannot fail-stop {cut:?}");
            let record = state
                .leader_wire_lifecycles
                .get(&fixture.token.slot)
                .expect("restored slot remains retained");
            assert_eq!(record.status, super::FairV2IngressLeaderWireStatus::Dormant);
            assert_eq!(record.token, fixture.token);
        }
        assert_eq!(
            fixture
                .gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read full replay selector"),
            None,
            "a Full exact replay cannot activate the dormant {cut:?} owner"
        );
        while fixture.ingress.try_recv_if(|_| true).is_some() {}
        let mut newer = fixture.message.clone();
        let BlockMessage::V2(envelope) = &mut newer else {
            unreachable!("leader-wire restart fixture is a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut envelope.payload else {
            unreachable!("leader-wire restart fixture carries Proposal");
        };
        proposal.round.view = proposal
            .round
            .view
            .checked_add(1)
            .expect("fixture view has a successor");
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    newer,
                    fixture.validator.clone(),
                )),
            Err(super::FairV2IngressPushError::Full(_))
        ));
        assert!(
            fixture.ingress.state.lock().open,
            "a newer conflicting identity must wait without fail-stop for {cut:?}"
        );
        assert!(
            matches!(
                fixture
                    .ingress
                    .try_push(InboundBlockMessage::from_authenticated_peer(
                        v2_commit_certificate_request(occurrence, &fixture.validator),
                        fixture.validator.clone(),
                    )),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ),
            "unrelated traffic must still bypass the dormant {cut:?} slot"
        );
        assert!(fixture.ingress.try_recv_if(|_| true).is_some());
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    fixture.message.clone(),
                    fixture.validator.clone(),
                )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            fixture
                .gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read activated replay selector"),
            Some(fixture.token.scheduler_ordinal())
        );
        {
            let state = fixture.ingress.state.lock();
            let record = state
                .leader_wire_lifecycles
                .get(&fixture.token.slot)
                .expect("exact retry retained its old slot");
            assert_eq!(record.status, super::FairV2IngressLeaderWireStatus::Ingress);
            assert_eq!(record.token, fixture.token);
        }
        let mut replay = fixture
            .ingress
            .try_recv_if(|_| true)
            .expect("exact replay owns a physical carrier");
        let mut ownership = replay
            .take_ingress_ownership()
            .expect("exact replay retains ingress ownership");
        assert_eq!(ownership.leader_wire_token(), Some(&fixture.token));
        fixture
            .ingress
            .bind_leader_wire_runtime_ownership(&mut ownership)
            .expect("exact replay rebinds its immutable runtime owner");
        let runtime = ownership
            .leader_wire_runtime_receipt()
            .expect("runtime receipt is installed");
        assert_eq!(runtime.token(), &fixture.token);
        if let Some(restored_owner) = fixture.runtime_owner {
            assert_eq!(runtime.owner(), restored_owner);
        }
        fixture
            .ingress
            .mark_leader_wire_volatile_terminal(runtime)
            .expect("publish replay tombstone");
        assert!(
            matches!(
                fixture
                    .ingress
                    .try_push(InboundBlockMessage::from_authenticated_peer(
                        fixture.message,
                        fixture.validator,
                    )),
                Ok(super::FairV2IngressPushDisposition::Coalesced)
            ),
            "the drained {cut:?} lifecycle cannot resurrect its old ingress stage"
        );
        assert!(fixture.ingress.try_recv_if(|_| true).is_none());
    }
}
#[test]
fn durable_view_cut_retires_carrierless_leader_wire_without_exact_retry() {
    for cut in [
        RestoredLeaderWireCut::Reserved,
        RestoredLeaderWireCut::Ingress,
        RestoredLeaderWireCut::Runtime,
        RestoredLeaderWireCut::Volatile,
    ] {
        let fixture = restored_leader_wire_fixture(cut);
        let next_view = fixture
            .token
            .identity
            .view
            .checked_add(1)
            .expect("fixture view has a successor");
        let mut newer = fixture.message.clone();
        let BlockMessage::V2(envelope) = &mut newer else {
            unreachable!("leader-wire restart fixture is a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut envelope.payload else {
            unreachable!("leader-wire restart fixture carries Proposal");
        };
        proposal.round.view = next_view;
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    newer.clone(),
                    fixture.validator.clone(),
                )),
            Err(super::FairV2IngressPushError::Full(_))
        ));
        let next =
            super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                fixture.token.identity.context_id,
                fixture.token.identity.height,
                [0xA7; 32],
                next_view,
                false,
            );
        assert_eq!(
            fixture
                .ingress
                .advance_leader_wire_recovery_cut(next)
                .expect("publish the live certified-view cut"),
            1,
            "{cut:?}"
        );
        assert!(
            fixture
                .ingress
                .state
                .lock()
                .leader_wire_lifecycles
                .is_empty(),
            "{cut:?}"
        );
        let restore = fixture.gate.restore().expect("inspect durable live cut");
        assert!(restore.records().is_empty(), "{cut:?}");
        assert_eq!(restore.last_admission_ordinal(), 7, "{cut:?}");
        assert_eq!(restore.scheduler_ordinal_high_watermark(), 41, "{cut:?}");
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    fixture.message,
                    fixture.validator.clone(),
                )),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    newer,
                    fixture.validator,
                )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let state = fixture.ingress.state.lock();
        let replacement = state
            .leader_wire_lifecycles
            .values()
            .next()
            .expect("current-view wire owns the released slot");
        assert_eq!(replacement.token.identity.view, next_view, "{cut:?}");
        assert!(replacement.token.admission_ordinal > 7, "{cut:?}");
        assert!(replacement.token.scheduler_ordinal > 41, "{cut:?}");
    }
}
#[test]
fn certified_view_cut_reopen_retires_restored_timeout_certificate() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    ingress.close();
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let alternate_validator = PeerId::new(KeyPair::random().public_key().clone());
    ingress
        .configure_roster([validator.clone(), alternate_validator.clone()])
        .expect("two-validator fair-ingress geometry");
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = 2;
    let message = v2_timeout_certificate(1);
    let BlockMessage::V2(envelope) = &message else {
        unreachable!("timeout fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) = &envelope.payload else {
        unreachable!("timeout fixture carries a v2 TimeoutCertificate");
    };
    let round = certificate.round;
    let wire_hash = CryptoHash::new(envelope.encode());
    let (identity, slot) = {
        let state = ingress.state.lock();
        match super::fair_v2_ingress_leader_wire_identity(&state, &message, &validator, wire_hash) {
            super::FairV2IngressLeaderWireDerivation::Exact { identity, slot } => (identity, slot),
            _ => panic!("timeout fixture must derive an exact leader-wire identity"),
        }
    };
    let token = super::FairV2IngressLeaderWireToken {
        source_class: identity.phase.source_class(),
        identity,
        slot,
        admission_ordinal: 7,
        scheduler_ordinal: 41,
    };
    let directory = TempDir::new().expect("temporary leader-wire restart directory");
    let wal_path = directory.path().join("safety.wal");
    let owner = [0xA7; 32];
    let roster = [validator.clone(), alternate_validator.clone()]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let capacity =
        super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(2, 2)
            .expect("finite leader-wire geometry");
    let authority_at = |durable_view, decision_durable| {
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            durable_view,
            decision_durable,
        )
    };
    let (gate, _) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &wal_path,
        round.context_id,
        round.height,
        owner,
        roster.clone(),
        capacity,
        2,
        authority_at(round.view, false),
        &[],
        &[],
    )
    .expect("open leader-wire restart fixture");
    gate.reserve(token.clone())
        .expect("reserve restart fixture token");
    gate.mark_ingress(&token)
        .expect("persist fixture ingress cut");
    drop(gate);
    let opened_view = round.view.checked_add(1).expect("fixture view advances");
    let (gate, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &wal_path,
        round.context_id,
        round.height,
        owner,
        roster,
        capacity,
        2,
        authority_at(opened_view, false),
        &[],
        &[],
    )
    .expect("reopen at the certified view that installing TC(V) produced");
    assert!(
        restore.records().is_empty(),
        "reopening past TC(V) must retire its restored lifecycle owner"
    );
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            round.context_id,
            round.height,
        )
        .expect("bind restored leader-wire gate");
    ingress.open().expect("open restored fair ingress");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(v2_timeout_certificate(0), alternate_validator.clone())),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(v2_timeout_certificate(opened_view), validator)),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(authority_at(opened_view, true))
            .expect("publish the durable Decision cut"),
        0
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(v2_timeout_certificate(opened_view), alternate_validator)),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
}
#[test]
fn durable_view_cut_drains_live_obsolete_carrier_despite_downstream_backpressure() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let layout = minimal_rs16_layout();
    let message = v2_maximum_structural_proposal_wire(layout, 1);
    let BlockMessage::V2(envelope) = &message else {
        unreachable!("live obsolete fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &envelope.payload else {
        unreachable!("live obsolete fixture carries Proposal");
    };
    let round = proposal.round;
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message.clone(),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(
        ingress
            .try_recv_if_checked_retiring_obsolete(|_| false)
            .expect("probe backpressured live carrier")
            .is_none(),
        "temporary downstream backpressure must retain a current-view carrier"
    );
    let next_view = round
        .view
        .checked_add(1)
        .expect("fixture view has a successor");
    let next = super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
        round.context_id,
        round.height,
        [0xA6; 32],
        next_view,
        false,
    );
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(next)
            .expect("publish certified live-view cut"),
        0,
        "a carrier-owning Ingress record must cross Runtime before retirement"
    );
    let (mut obsolete, disposition) = ingress
        .try_recv_if_checked_retiring_obsolete(|_| false)
        .expect("classify obsolete live carrier")
        .expect("WAL-obsolete carrier bypasses unchanged downstream capacity");
    assert_eq!(
        disposition,
        super::FairV2IngressDequeueDisposition::RetireObsolete
    );
    let ownership = obsolete
        .take_ingress_ownership()
        .expect("obsolete dequeue retains exact ownership");
    let runtime = ownership
        .leader_wire_runtime_receipt()
        .expect("obsolete dequeue durably binds Runtime");
    assert_eq!(runtime.token().view(), round.view);
    ingress
        .mark_obsolete_leader_wire_volatile_terminal(runtime)
        .expect("publish WAL-authorized volatile terminal");
    let mut replacement = message.clone();
    let BlockMessage::V2(envelope) = &mut replacement else {
        unreachable!("replacement fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut envelope.payload else {
        unreachable!("replacement fixture carries Proposal");
    };
    proposal.round.view = next_view;
    proposal.manifest.round.view = next_view;
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            replacement,
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message, validator
        )),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    let state = ingress.state.lock();
    let replacement = state
        .leader_wire_lifecycles
        .values()
        .next()
        .expect("current-view replacement owns the released slot");
    assert_eq!(
        replacement.status,
        super::FairV2IngressLeaderWireStatus::Ingress
    );
    assert_eq!(replacement.token.view(), next_view);
    assert!(replacement.token.admission_ordinal() > runtime.token().admission_ordinal());
    assert!(replacement.token.scheduler_ordinal() > runtime.token().scheduler_ordinal());
}
#[test]
fn certified_body_response_survives_view_and_decision_cuts_at_fair_ingress() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let message = v2_certified_body_response(1, 0, 8);
    let BlockMessage::V2(envelope) = &message else {
        unreachable!("certified response fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &envelope.payload else {
        unreachable!("certified response fixture carries its exact payload");
    };
    let round = response.manifest.round;
    let _directory = bind_test_leader_wire_gate(&ingress, &validator, round, 2);
    let advanced_view = round.view.checked_add(1).expect("fixture view advances");
    let view_cut =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            [0xA6; 32],
            advanced_view,
            false,
        );
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(view_cut)
            .expect("publish the live certified-view cut"),
        0
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message.clone(),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message.clone(),
            validator.clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Coalesced)
    ));
    let mut conflicting = message.clone();
    let BlockMessage::V2(envelope) = &mut conflicting else {
        unreachable!("conflicting fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &mut envelope.payload
    else {
        unreachable!("conflicting fixture carries a certified response");
    };
    response.body.push(0xFF);
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            conflicting,
            validator.clone(),
        )),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    assert!(
        ingress
            .try_recv_if_checked_retiring_obsolete(|_| false)
            .expect("probe the delayed historical response")
            .is_none(),
        "a view cut must not misclassify the response as an obsolete carrier"
    );
    let (mut drained, disposition) = ingress
        .try_recv_if_checked_retiring_obsolete(|_| true)
        .expect("drain the request-bound historical response")
        .expect("the response remains physically admissible");
    assert_eq!(disposition, super::FairV2IngressDequeueDisposition::Admit);
    let ownership = drained
        .take_ingress_ownership()
        .expect("historical response retains exact ingress ownership");
    let runtime = ownership
        .leader_wire_runtime_receipt()
        .expect("historical response binds exact runtime ownership");
    assert_eq!(
        runtime.token().identity.phase,
        super::FairV2IngressLeaderWirePhase::CertifiedResponse
    );
    assert_eq!(runtime.token().view(), round.view);
    ingress
        .mark_leader_wire_volatile_terminal(runtime)
        .expect("publish the exact response terminal");
    let decision_cut =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            [0xA6; 32],
            advanced_view,
            true,
        );
    assert_eq!(
        ingress
            .advance_leader_wire_recovery_cut(decision_cut)
            .expect("publish the durable Decision cut"),
        0
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message, validator
        )),
        Ok(super::FairV2IngressPushDisposition::Coalesced)
    ));
}
#[test]
fn durable_decision_cut_retires_and_closes_carrierless_leader_wire_height() {
    let fixture = restored_leader_wire_fixture(RestoredLeaderWireCut::Runtime);
    let next = super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
        fixture.token.identity.context_id,
        fixture.token.identity.height,
        [0xA7; 32],
        fixture.token.identity.view,
        true,
    );
    assert_eq!(
        fixture
            .ingress
            .advance_leader_wire_recovery_cut(next)
            .expect("publish the durable Decision cut"),
        1
    );
    let mut later = fixture.message.clone();
    let BlockMessage::V2(envelope) = &mut later else {
        unreachable!("leader-wire restart fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut envelope.payload else {
        unreachable!("leader-wire restart fixture carries Proposal");
    };
    proposal.round.view = proposal
        .round
        .view
        .checked_add(1)
        .expect("fixture view has a successor");
    for message in [fixture.message, later] {
        assert!(matches!(
            fixture
                .ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    message,
                    fixture.validator.clone(),
                )),
            Err(super::FairV2IngressPushError::Rejected(_))
        ));
    }
    assert!(fixture.ingress.state.lock().open);
    assert!(
        fixture
            .ingress
            .state
            .lock()
            .leader_wire_lifecycles
            .is_empty()
    );
}
