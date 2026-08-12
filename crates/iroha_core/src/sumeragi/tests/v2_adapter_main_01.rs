#[test]
fn restored_body_available_terminal_retirement_is_persistent_before_token_release() {
    assert_restored_stage_seven_retirement_does_not_resurrect(0xB8, true, false, false);
    assert_restored_stage_seven_retirement_does_not_resurrect(0xB9, true, true, false);
    assert_restored_stage_seven_retirement_does_not_resurrect(0xBA, true, false, true);
    assert_restored_stage_seven_retirement_does_not_resurrect(0xBB, false, false, false);
    assert_restored_stage_seven_retirement_does_not_resurrect(0xBD, false, false, false);
}

#[test]
fn live_producer_owner_cannot_replace_immutable_identity() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let event = reducer::Event::TimeoutElapsed {
        tag: adapter.current_tag(),
    };
    let candidate = adapter
        .serviced_candidate(&event, DeferredPriority::Completion, None, None)
        .expect("timeout has a producer stage");
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"live producer owner"), 1)
        .expect("bind first live owner");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve first live owner")
        .expect("tracked source reserves an address");
    let address = reservation.address;
    let original = adapter.producer_continuations[&address].clone();
    assert!(
        adapter.restored_dormant_producer_continuations.is_empty(),
        "same-process reservations are never restart-dormant"
    );

    adapter.clear_selected_producer_lifecycle();
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"forged equal-rank owner"), 1)
        .expect("bind a distinct equal-rank owner");
    assert!(matches!(
        adapter.reserve_selected_producer_continuation(Some(candidate)),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
    assert_eq!(adapter.producer_continuations[&address], original);
    assert_eq!(
        adapter.durable_producer_continuations.get(&address),
        Some(&original),
        "rejected live replacement changes no durable alias"
    );
}

#[test]
fn restored_producer_rejects_a_mismatched_replay_identity_without_mutation() {
    let directory = TempDir::new().expect("temporary directory");
    let candidate;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"stored producer owner"), 1)
            .expect("bind stored owner");
        adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("persist stored owner")
            .expect("tracked source reserves an address");
    }

    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restore dormant producer");
    assert!(startup.is_empty());
    let (address, original) = restarted
        .producer_continuations
        .iter()
        .next()
        .map(|(address, record)| (*address, record.clone()))
        .expect("restored producer exists");
    restarted
        .bind_selected_producer_lifecycle(Hash::new(b"replayed producer owner"), 2)
        .expect("bind replay owner");

    assert!(matches!(
        restarted.reserve_selected_producer_continuation(Some(candidate)),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
    assert_eq!(restarted.producer_continuations[&address], original);
    assert_eq!(
        restarted.durable_producer_continuations.get(&address),
        Some(&original)
    );
    assert!(
        restarted
            .restored_dormant_producer_continuations
            .contains(&address),
        "a rejected identity replacement cannot claim the dormant alias"
    );
}

#[test]
fn conditional_transport_service_reserves_and_coalesces_a_producer_lifecycle() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let leader = adapter.wire_context.leader(adapter.current_tag().view());
    let message = proposal(&adapter.wire_context, leader, subject(0x6D));
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"conditional transport source"), 17)
        .expect("bind exact transport owner");
    let outcome = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(message))
        .expect("service authenticated proposal");
    let handoff = outcome
        .producer_handoff()
        .expect("transport service retains a producer handoff");
    assert_eq!(
        handoff.source_class(),
        ProducerContinuationSourceClass::ConditionalTransport
    );
    assert_eq!(handoff.identity().admission_ordinal(), 17);
    let address = handoff.identity().address();
    assert_eq!(
        adapter.producer_continuations[&address].status(),
        ProducerContinuationStatus::Reserved
    );
    adapter
        .acknowledge_producer_handoff(
            handoff,
            ProducerContinuationHandoffEvidence::ConcreteSuccessor,
        )
        .expect("physical runtime successor acknowledges transport service");
    assert_eq!(
        adapter.producer_continuations[&address].status(),
        ProducerContinuationStatus::Terminal
    );
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&address),
        "volatile transport completion cannot become a restart-stable terminal"
    );
}

#[test]
fn retired_empty_handoff_terminalizes_once_and_exact_replay_coalesces() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let event = reducer::Event::TimeoutElapsed {
        tag: adapter.current_tag(),
    };
    let candidate = adapter
        .serviced_candidate(&event, DeferredPriority::Completion, None, None)
        .expect("timeout has a producer stage");
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"retired empty handoff"), 18)
        .expect("bind exact local owner");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve before source retirement")
        .expect("tracked source reserves an address");
    let handoff = adapter
        .record_serviced_candidate(Some(candidate), false, false, Some(reservation))
        .expect("drain source without a concrete successor")
        .expect("drained source retains its exact reservation");
    let address = handoff.identity().address();
    assert_eq!(
        adapter
            .producer_handoff_evidence(handoff, false)
            .expect("classify empty handoff"),
        ProducerContinuationHandoffEvidence::VolatileTerminal
    );
    let terminal = adapter
        .acknowledge_producer_handoff(
            handoff,
            ProducerContinuationHandoffEvidence::VolatileTerminal,
        )
        .expect("retired empty handoff terminalizes");
    assert_eq!(terminal.identity(), handoff.identity());
    assert_eq!(
        adapter.producer_continuations[&address].status(),
        ProducerContinuationStatus::Terminal
    );
    assert_eq!(adapter.producer_continuations.len(), 1);
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&address),
        "process-local retirement must not be upgraded to restart-stable evidence"
    );

    let replay = adapter
        .step(event)
        .expect("coalesce exact retransmission after drain");
    assert_eq!(
        replay.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert!(
        replay.producer_handoff().is_none(),
        "the drained identity cannot mint a second producer lifecycle"
    );
    assert_eq!(adapter.producer_continuations.len(), 1);
    assert_eq!(
        adapter.producer_continuations[&address].status(),
        ProducerContinuationStatus::Terminal,
        "exact replay cannot resurrect the retired old stage"
    );
}

#[test]
fn every_producer_stage_has_an_explicit_replay_parent_contract() {
    let classified = ServicedCandidateStage::ALL
        .map(|stage| (stage, producer_parent_replay_source_for_stage(stage)));
    assert_eq!(classified.len(), SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE);
    assert_eq!(
        classified,
        [
            (
                ServicedCandidateStage::LocalProposalReady,
                ProducerParentReplaySource::DurableBodyPipeline,
            ),
            (
                ServicedCandidateStage::ProposalReceived,
                ProducerParentReplaySource::ConditionalResponsiveTransport,
            ),
            (
                ServicedCandidateStage::VoteReceived,
                ProducerParentReplaySource::ConditionalResponsiveTransport,
            ),
            (
                ServicedCandidateStage::QuorumCertificateReceived,
                ProducerParentReplaySource::ConditionalResponsiveTransport,
            ),
            (
                ServicedCandidateStage::TimeoutVoteReceived,
                ProducerParentReplaySource::ConditionalResponsiveTransport,
            ),
            (
                ServicedCandidateStage::TimeoutCertificateReceived,
                ProducerParentReplaySource::ConditionalResponsiveTransport,
            ),
            (
                ServicedCandidateStage::TimeoutElapsed,
                ProducerParentReplaySource::SafetyWal,
            ),
            (
                ServicedCandidateStage::BodyAvailable,
                ProducerParentReplaySource::VolatileBodyReconstruction,
            ),
            (
                ServicedCandidateStage::BodyStored,
                ProducerParentReplaySource::DurableBodyPipeline,
            ),
            (
                ServicedCandidateStage::ValidationCompleted,
                ProducerParentReplaySource::DurableBodyPipeline,
            ),
            (
                ServicedCandidateStage::ApplicationCompleted,
                ProducerParentReplaySource::DurableDecision,
            ),
        ]
    );
    for stage in ServicedCandidateStage::ALL {
        let expected = matches!(
            stage,
            ServicedCandidateStage::LocalProposalReady
                | ServicedCandidateStage::TimeoutElapsed
                | ServicedCandidateStage::BodyStored
                | ServicedCandidateStage::ValidationCompleted
                | ServicedCandidateStage::ApplicationCompleted
        );
        assert_eq!(
            producer_parent_is_locally_reconstructible(stage),
            expected,
            "only an independently durable local parent may reserve"
        );
    }
}

#[test]
fn speculative_producer_rollback_restores_free_and_terminal_slots() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let event = reducer::Event::TimeoutElapsed {
        tag: adapter.current_tag(),
    };
    let first = adapter
        .serviced_candidate(&event, DeferredPriority::Completion, None, None)
        .expect("timeout has a producer stage");
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"first source"), 1)
        .expect("bind first source");
    let inserted = adapter
        .reserve_selected_producer_continuation(Some(first))
        .expect("reserve free slot")
        .expect("tracked source reserves");
    let address = inserted.address;
    assert_eq!(inserted.change, ProducerReservationChange::Inserted);
    let original = adapter.producer_continuations[&address].clone();
    adapter.clear_selected_producer_lifecycle();
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"first source"), 1)
        .expect("bind the exact physical retry");
    let coalesced = adapter
        .reserve_selected_producer_continuation(Some(first))
        .expect("coalesce the same logical request")
        .expect("tracked retry retains its original address");
    assert_eq!(coalesced.address, address);
    assert_eq!(coalesced.change, ProducerReservationChange::Unchanged);
    assert_eq!(adapter.producer_continuations[&address], original);
    adapter
        .rollback_producer_reservation(Some(coalesced))
        .expect("roll back coalesced reservation");
    adapter
        .rollback_producer_reservation(Some(inserted))
        .expect("roll back inserted reservation");
    assert!(!adapter.producer_continuations.contains_key(&address));

    adapter.clear_selected_producer_lifecycle();
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"first source"), 1)
        .expect("rebind first source");
    let inserted = adapter
        .reserve_selected_producer_continuation(Some(first))
        .expect("reserve first owner")
        .expect("tracked source reserves");
    adapter
        .terminalize_producer_continuation(Some(inserted.address))
        .expect("terminalize incumbent");
    let terminal = adapter.producer_continuations[&inserted.address].clone();

    let replacement_key = ServicedCandidateKey::new(
        adapter.wire_context.id(),
        adapter.wire_context.height,
        adapter.fingerprints.node.into(),
        adapter.wire_context.leader(1),
        1,
        Some([0x47; 32]),
        0,
        ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
        DeferredEventKind::TimeoutElapsed.code(),
        [0x47; 32],
    );
    let replacement = (replacement_key, 1, ServicedCandidatePolicy::Suppress);
    adapter.clear_selected_producer_lifecycle();
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"replacement source"), 2)
        .expect("bind replacement source");
    let replaced = adapter
        .reserve_selected_producer_continuation(Some(replacement))
        .expect("replace terminal slot")
        .expect("tracked replacement reserves");
    assert!(matches!(
        replaced.change,
        ProducerReservationChange::ReplacedTerminal { .. }
    ));
    adapter
        .rollback_producer_reservation(Some(replaced))
        .expect("roll back terminal replacement");
    assert_eq!(adapter.producer_continuations[&address], terminal);
}

#[test]
fn process_only_producer_replacement_rollback_stays_volatile_across_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x48);

    adapter
        .rollback_producer_reservation(Some(replacement.reservation))
        .expect("roll back process-only terminal replacement");
    assert_eq!(
        adapter.producer_continuations[&replacement.address],
        replacement.incumbent
    );
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&replacement.address),
        "rollback cannot publish the process-only predecessor"
    );

    drop(adapter);
    assert_process_only_predecessor_absent_after_restart(&directory);
}

#[test]
fn process_only_producer_replacement_release_stays_volatile_across_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x49);

    adapter
        .release_unrecorded_producer(Some(replacement.reservation))
        .expect("release process-only terminal replacement");
    assert_eq!(
        adapter.producer_continuations[&replacement.address],
        replacement.incumbent
    );
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&replacement.address),
        "release cannot publish the process-only predecessor"
    );

    drop(adapter);
    assert_process_only_predecessor_absent_after_restart(&directory);
}

#[test]
fn durable_decision_release_does_not_restore_stale_process_only_predecessor() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x4B);
    adapter.clear_selected_producer_lifecycle();

    let decided_subject = subject(0x4C);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (_, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let mut decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS-normal key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    authenticate_qc(&mut decision, &keys);
    adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision,
            )),
        ))
        .expect("install the durable Decision and reclaim producer ownership");
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    assert!(adapter.restored_dormant_producer_continuations.is_empty());
    assert!(adapter.deferred_producer_continuations.is_empty());
    assert!(adapter.pending_producer_handoffs.is_empty());
    let reclaimed_snapshot = std::fs::read(adapter.serviced_candidate_store_path_for_test())
        .expect("read canonical reclaimed owner snapshot");

    adapter
        .release_unrecorded_producer(Some(replacement.reservation))
        .expect("discard stale pre-Decision undo token");
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    assert!(adapter.restored_dormant_producer_continuations.is_empty());
    assert!(adapter.deferred_producer_continuations.is_empty());
    assert!(adapter.pending_producer_handoffs.is_empty());
    assert_eq!(
        std::fs::read(adapter.serviced_candidate_store_path_for_test())
            .expect("reread canonical reclaimed owner snapshot"),
        reclaimed_snapshot,
        "a stale pre-Decision undo token cannot republish reclaimed ownership"
    );

    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision retry"), 3)
        .expect("bind post-Decision retry");
    assert!(
        adapter
            .reserve_selected_producer_continuation(Some(replacement.candidate))
            .expect("canonical post-Decision retry remains serviceable")
            .is_none(),
        "the durable Decision remains the sole restart owner"
    );
    assert!(!adapter.fail_closed);

    drop(adapter);
    let (restarted, _) = open_test(&directory).expect("replay the durable Decision");
    assert!(restarted.reducer.durable_state().decision().is_some());
    assert!(restarted.serviced_candidates_decision_reclaimed);
    assert!(restarted.serviced_candidates.is_empty());
    assert!(restarted.durable_serviced_candidates.is_empty());
    assert!(restarted.producer_continuations.is_empty());
    assert!(restarted.durable_producer_continuations.is_empty());
    assert!(restarted.restored_dormant_producer_continuations.is_empty());
    assert!(restarted.deferred_producer_continuations.is_empty());
    assert!(restarted.pending_producer_handoffs.is_empty());
}

#[test]
fn process_only_producer_replacement_handoff_does_not_resurrect_predecessor() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x4A);

    let handoff = adapter
        .record_serviced_candidate(
            Some(replacement.candidate),
            false,
            false,
            Some(replacement.reservation),
        )
        .expect("stage volatile replacement handoff")
        .expect("replacement retains an exact handoff");
    adapter
        .acknowledge_producer_handoff(
            handoff,
            ProducerContinuationHandoffEvidence::VolatileTerminal,
        )
        .expect("acknowledge volatile replacement handoff");
    assert_eq!(
        adapter.producer_continuations[&replacement.address].status(),
        ProducerContinuationStatus::Terminal
    );
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&replacement.address),
        "non-durable acknowledgement cannot resurrect the process-only predecessor"
    );

    drop(adapter);
    assert_process_only_predecessor_absent_after_restart(&directory);
}

#[test]
fn retiring_busy_local_parent_releases_unacknowledged_producer_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };
    let subject = subject(0x4A);
    let payload = [0x4A, 2];
    let manifest = encode_payload(&adapter.wire_context, round, subject, &payload)
        .expect("encode producer-retirement payload")
        .manifest()
        .clone();
    adapter
        .defer_body_pipeline_stage_for_test(
            tag,
            &manifest,
            DeferredBodyPipelineStageForTest::BodyStored,
        )
        .expect("stage exact local parent");
    let (admission_ordinal, candidate) = {
        let input = adapter
            .deferred_completions
            .back()
            .expect("deferred local parent");
        (
            input.admission_ordinal,
            adapter
                .serviced_candidate(
                    &input.event,
                    input.priority,
                    input.completion_evidence.as_ref(),
                    input.authenticated_wire_identity.as_deref(),
                )
                .expect("body-store completion has a serviced identity"),
        )
    };
    adapter
        .bind_selected_producer_lifecycle(
            Hash::new(b"retired busy local parent"),
            admission_ordinal,
        )
        .expect("bind exact lifecycle");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve before adapter ownership")
        .expect("local durable parent reserves");
    let address = reservation.address;
    adapter
        .deferred_producer_continuations
        .insert(admission_ordinal, reservation);

    adapter
        .retire_deferred_body_pipeline_completions(tag, round, subject)
        .expect("persist exact local-parent retirement before queue release");

    assert!(adapter.deferred_completions.is_empty());
    assert!(
        !adapter
            .deferred_producer_continuations
            .contains_key(&admission_ordinal)
    );
    assert!(
        !adapter.producer_continuations.contains_key(&address),
        "goal-reaching retirement cannot manufacture successor acknowledgement"
    );
}

#[test]
fn failed_busy_parent_retirement_retains_queue_and_durable_owner() {
    let directory = TempDir::new().expect("temporary deferred-retirement directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };
    let subject = subject(0x4B);
    let manifest = encode_payload(&adapter.wire_context, round, subject, &[0x4B, 2])
        .expect("encode deferred-retirement payload")
        .manifest()
        .clone();
    adapter
        .defer_body_pipeline_stage_for_test(
            tag,
            &manifest,
            DeferredBodyPipelineStageForTest::BodyStored,
        )
        .expect("stage exact deferred producer");
    let (admission_ordinal, candidate) = {
        let input = adapter
            .deferred_completions
            .back()
            .expect("deferred producer input");
        (
            input.admission_ordinal,
            adapter
                .serviced_candidate(
                    &input.event,
                    input.priority,
                    input.completion_evidence.as_ref(),
                    input.authenticated_wire_identity.as_deref(),
                )
                .expect("deferred body-store producer identity"),
        )
    };
    adapter
        .bind_selected_producer_lifecycle(
            Hash::new(b"failed deferred producer retirement"),
            admission_ordinal,
        )
        .expect("bind exact deferred lifecycle");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve deferred producer")
        .expect("deferred producer has one durable reservation");
    let address = reservation.address;
    adapter
        .deferred_producer_continuations
        .insert(admission_ordinal, reservation);

    let path = adapter
        .serviced_candidate_store_path_for_test()
        .to_path_buf();
    let snapshot = std::fs::read(&path).expect("read producer snapshot before sabotage");
    std::fs::remove_file(&path).expect("remove producer snapshot");
    std::fs::create_dir(&path).expect("replace producer snapshot with a directory");
    assert!(matches!(
        adapter.retire_deferred_body_pipeline_completions(tag, round, subject),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
    assert!(adapter.fail_closed);
    assert_eq!(adapter.deferred_completions.len(), 1);
    assert!(
        adapter
            .deferred_producer_continuations
            .contains_key(&admission_ordinal)
    );
    assert_eq!(
        adapter.producer_continuations.get(&address),
        adapter.durable_producer_continuations.get(&address),
        "failed publication restores both producer aliases before returning"
    );

    std::fs::remove_dir(&path).expect("remove sabotaged producer directory");
    std::fs::write(&path, snapshot).expect("restore pre-retirement producer snapshot");
    drop(adapter);
    let (restarted, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart after failed deferred retirement");
    assert!(
        restarted
            .restored_dormant_producer_continuations
            .contains(&address),
        "failed retirement must reopen the exact producer instead of losing work"
    );
}

#[test]
fn restart_frontier_retains_all_four_stages_of_the_protected_body_pipeline() {
    let directory = TempDir::new().expect("temporary protected-frontier directory");
    let expected_addresses;
    let expected_stage_codes = BTreeSet::from([
        ServicedCandidateStage::LocalProposalReady as u8,
        ServicedCandidateStage::BodyAvailable as u8,
        ServicedCandidateStage::BodyStored as u8,
        ServicedCandidateStage::ValidationCompleted as u8,
    ]);
    let protected_target;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: tag.view(),
        };
        let body_subject = subject(0x4C);
        let manifest = encode_payload(&adapter.wire_context, round, body_subject, &[0x4C, 2])
            .expect("encode protected producer payload")
            .manifest()
            .clone();
        let mut addresses = BTreeSet::new();
        for (stage, lifecycle_marker, lifecycle_ordinal) in [
            (
                DeferredBodyPipelineStageForTest::LocalProposalReady,
                0x40,
                41,
            ),
            (DeferredBodyPipelineStageForTest::BodyAvailable, 0x41, 42),
            (DeferredBodyPipelineStageForTest::BodyStored, 0x42, 43),
            (
                DeferredBodyPipelineStageForTest::ValidationSucceeded,
                0x43,
                44,
            ),
        ] {
            adapter
                .defer_body_pipeline_stage_for_test(tag, &manifest, stage)
                .expect("stage protected body producer");
            let (deferred_ordinal, candidate) = {
                let input = adapter
                    .deferred_completions
                    .back()
                    .expect("protected producer input");
                (
                    input.admission_ordinal,
                    adapter
                        .serviced_candidate(
                            &input.event,
                            input.priority,
                            input.completion_evidence.as_ref(),
                            input.authenticated_wire_identity.as_deref(),
                        )
                        .expect("protected body stage has a producer identity"),
                )
            };
            adapter
                .bind_selected_producer_lifecycle(
                    Hash::new([0x4C, lifecycle_marker]),
                    lifecycle_ordinal,
                )
                .expect("bind one protected producer lifecycle");
            let reservation = adapter
                .reserve_selected_producer_continuation(Some(candidate))
                .expect("persist protected producer stage")
                .expect("protected body stage reserves one address");
            assert!(addresses.insert(reservation.address));
            assert!(
                adapter
                    .deferred_producer_continuations
                    .insert(deferred_ordinal, reservation)
                    .is_none()
            );
            adapter.clear_selected_producer_lifecycle();
        }
        assert_eq!(addresses.len(), expected_stage_codes.len());

        let (_, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS-normal key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let mut prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: body_subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![1, 2, 3],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut prepare, &keys);
        let timeout_signers = vec![1, 2, 3];
        let timeout_preimage = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer: timeout_signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let timeout_shares = timeout_signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small fixture signer")].private_key(),
                    &timeout_preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let timeout_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate protected frontier timeout votes");
        let timeout = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare),
                signers: timeout_signers,
                aggregate_signature: timeout_signature,
            }],
        };
        adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ))
            .expect("durably install the protected lock and successor view");
        assert_eq!(adapter.current_tag().view(), tag.view() + 1);
        let locked = adapter
            .reducer
            .durable_state()
            .locked()
            .expect("TC installs its highest PrepareQC as the durable lock");
        protected_target = *locked.subject().as_bytes();
        assert_eq!(locked.proposal_round().view(), tag.view());
        expected_addresses = addresses;
    }

    let (restarted, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the protected four-stage producer frontier");
    assert_eq!(restarted.current_tag().view(), 1);
    assert_eq!(
        restarted.restored_dormant_producer_continuations, expected_addresses,
        "restart must retain every and only protected body-pipeline stage"
    );
    assert_eq!(
        restarted
            .producer_continuations
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        expected_addresses
    );
    assert_eq!(
        restarted
            .durable_producer_continuations
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        expected_addresses
    );
    let restored_stage_codes = restarted
        .producer_continuations
        .values()
        .map(|record| {
            assert_eq!(record.status(), ProducerContinuationStatus::Reserved);
            assert_eq!(record.identity().candidate().source_view(), 0);
            assert_eq!(
                record.identity().candidate().target(),
                Some(protected_target)
            );
            record.identity().stage()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(restored_stage_codes, expected_stage_codes);
    assert_eq!(
        restarted
            .dormant_local_fifo_reservations()
            .expect("project protected Local stages into FIFO reservations")
            .len(),
        3,
        "BodyAvailable remains fetch-backed while the other three protected stages retain local FIFO slots"
    );
    assert!(!restarted.fail_closed);
}

#[test]
fn restart_frontier_rejects_reserved_producer_beyond_the_durable_view() {
    let directory = TempDir::new().expect("temporary future-producer directory");
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let current = adapter.current_tag();
        let future = reducer::EventTag::new(
            current.height(),
            current.view() + 1,
            reducer::Generation::new(current.generation().get() + 1),
        );
        let event = reducer::Event::TimeoutElapsed { tag: future };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("future timeout has a producer identity");
        assert_eq!(candidate.0.source_view(), current.view() + 1);
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"future producer beyond durable view"), 51)
            .expect("bind future producer fixture");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("persist future producer fixture")
            .expect("future producer reserves one address");
        assert_eq!(
            adapter.producer_continuations[&reservation.address].status(),
            ProducerContinuationStatus::Reserved
        );
        assert_eq!(adapter.current_tag(), current);
    }

    assert!(matches!(
        SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        ),
        Err(AdapterError::ServicedCandidateStore(reason))
            if reason.contains("originated beyond the replayed durable view")
    ));
}

#[test]
fn strict_view_advance_retains_live_producer_admission_until_owner_release() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let event = reducer::Event::TimeoutElapsed { tag };
    let candidate = adapter
        .serviced_candidate(&event, DeferredPriority::Completion, None, None)
        .expect("timeout has a producer stage");
    let causal_key = Hash::new(b"live producer across strict view advance");
    adapter
        .bind_selected_producer_lifecycle(causal_key.clone(), 1)
        .expect("bind live producer owner");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve live producer")
        .expect("tracked timeout reserves");
    let address = reservation.address;
    adapter.clear_selected_producer_lifecycle();

    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS-normal key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let timeout_signers = vec![0, 1, 2];
    let timeout_preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: timeout_signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let timeout_shares = timeout_signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small fixture signer")].private_key(),
                &timeout_preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate strict-view timeout votes");
    let timeout = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: timeout_signers,
            aggregate_signature: timeout_signature,
        }],
    };
    adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install timeout certificate and advance the view");
    assert_eq!(adapter.current_tag().view(), tag.view() + 1);
    assert_eq!(
        adapter.durable_producer_continuations.get(&address),
        adapter.producer_continuations.get(&address),
        "strict-view reclamation must not split a still-live producer from its durable admission"
    );

    adapter
        .bind_selected_producer_lifecycle(causal_key, 1)
        .expect("rebind exact live retry");
    let retry = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("exact live retry remains admissible")
        .expect("exact live retry retains its reservation");
    assert_eq!(retry.address, address);
    assert_eq!(retry.change, ProducerReservationChange::Unchanged);
    assert!(!adapter.fail_closed);

    drop(retry);
    drop(adapter);
    let (restarted, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after the WAL won before volatile owner cleanup");
    assert!(
        !restarted.producer_continuations.contains_key(&address)
            && !restarted
                .durable_producer_continuations
                .contains_key(&address)
            && !restarted
                .restored_dormant_producer_continuations
                .contains(&address),
        "restart must persistently prune an obsolete old-view Reserved producer"
    );
    drop(restarted);
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after persisted restored-frontier pruning");
    assert!(
        !restarted_again
            .producer_continuations
            .contains_key(&address)
    );
}

#[test]
fn strict_view_advance_reclaims_process_terminal_before_retagged_retry() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let event = reducer::Event::TimeoutElapsed { tag };
    let candidate = adapter
        .serviced_candidate(&event, DeferredPriority::Completion, None, None)
        .expect("timeout has a producer stage");
    let causal_key = Hash::new(b"process terminal before retagged retry");
    adapter
        .bind_selected_producer_lifecycle(causal_key.clone(), 1)
        .expect("bind original producer owner");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve original producer")
        .expect("tracked timeout reserves");
    let handoff = adapter
        .record_serviced_candidate(Some(candidate), false, false, Some(reservation))
        .expect("record original producer service")
        .expect("original service returns a handoff");
    let address = handoff.identity().address();
    adapter
        .acknowledge_producer_handoff(
            handoff,
            ProducerContinuationHandoffEvidence::ConcreteSuccessor,
        )
        .expect("acknowledge volatile successor");
    adapter.clear_selected_producer_lifecycle();
    assert_eq!(
        adapter.producer_continuations[&address].status(),
        ProducerContinuationStatus::Terminal
    );
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&address)
    );
    assert!(adapter.serviced_candidates.contains_key(&candidate.0));

    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };
    let timeout = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA6; 96],
        }],
    };
    adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install timeout certificate and advance the view");
    assert_eq!(adapter.current_tag().view(), tag.view() + 1);
    assert!(!adapter.serviced_candidates.contains_key(&candidate.0));
    assert!(
        !adapter.producer_continuations.contains_key(&address),
        "the strict episode exit must reclaim its process-only terminal"
    );

    let retagged_candidate = (candidate.0, adapter.current_tag().view(), candidate.2);
    adapter
        .bind_selected_producer_lifecycle(causal_key, 1)
        .expect("rebind the exact deferred owner");
    let retry = adapter
        .reserve_selected_producer_continuation(Some(retagged_candidate))
        .expect("retagged exact retry does not collide with a stale terminal")
        .expect("retagged exact retry reserves");
    assert_eq!(retry.change, ProducerReservationChange::Inserted);
    assert!(!adapter.fail_closed);
}

#[test]
fn terminal_producer_tombstone_survives_restart_blocks_aba_and_advances_shared_source() {
    let directory = TempDir::new().expect("temporary directory");
    let causal_key = Hash::new(b"terminal producer parent");
    let address;
    let terminal;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(causal_key.clone(), 41)
            .expect("bind selected source");
        address = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve producer")
            .expect("tracked candidate reserves an address")
            .address;
        adapter
            .terminalize_producer_continuation(Some(address))
            .expect("terminalize after source retirement");
        terminal = adapter.producer_continuations[&address].clone();
        adapter
            .durable_producer_continuations
            .insert(address, terminal.clone());
        let terminal_candidate = terminal.identity().candidate();
        adapter
            .durable_serviced_candidates
            .insert(terminal_candidate, terminal_candidate.source_view());
        adapter
            .serviced_candidate_store
            .persist_with_producer_continuations(
                &adapter.durable_serviced_candidates,
                &adapter.durable_producer_continuations,
                adapter.serviced_candidates_decision_reclaimed,
            )
            .expect("publish terminal high-watermark");
    }

    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restore terminal producer high-watermark");
    assert!(startup.is_empty());
    assert_eq!(restarted.producer_continuations[&address], terminal);
    let restored_high_watermark = restarted
        .restored_producer_continuation_ordinal_high_watermark()
        .expect("restored producer tombstone carries an ordinal");
    assert_eq!(restored_high_watermark, 41);
    let serve_high_watermark = 7;
    assert!(restored_high_watermark > serve_high_watermark);
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            serve_high_watermark,
        );
    lifecycle_ordinals
        .advance_past(restored_high_watermark)
        .expect("fold producer high-watermark into actor source");
    let first_runtime_owner = lifecycle_ordinals
        .reserve_one()
        .expect("mint first post-restart runtime owner");
    let first_serve_owner = lifecycle_ordinals
        .reserve_one()
        .expect("mint first post-restart Serve owner");
    assert_eq!(first_runtime_owner, restored_high_watermark + 1);
    assert!(first_serve_owner > first_runtime_owner);
    assert!(
        restarted
            .serviced_candidate_store
            .reserve_producer_continuation(
                &mut restarted.producer_continuations,
                ProducerContinuationRecord::new(
                    terminal.identity(),
                    ProducerContinuationStatus::Reserved,
                    Vec::new(),
                )
                .expect("construct stale ABA retry"),
            )
            .is_err(),
        "a drained logical stage cannot resurrect through its old identity"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn serviced_candidate_reclaim_failure_fail_stops_then_replay_reclaims() {
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let snapshot_path;
    let stale_snapshot;
    let marker = 0x42;
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader adapter");
        assert!(startup.is_empty());
        durably_retire_unowned_body_event(&mut adapter, marker);
        let pre_decision_timeout = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let pre_decision_producer = adapter
            .serviced_candidate(
                &pre_decision_timeout,
                DeferredPriority::Completion,
                None,
                None,
            )
            .expect("timeout has a producer identity");
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"pre-Decision producer tombstone"), 1)
            .expect("bind pre-Decision producer");
        let pre_decision_reservation = adapter
            .reserve_selected_producer_continuation(Some(pre_decision_producer))
            .expect("reserve pre-Decision producer")
            .expect("tracked timeout reserves");
        let handoff = adapter
            .record_serviced_candidate(
                Some(pre_decision_producer),
                true,
                true,
                Some(pre_decision_reservation),
            )
            .expect("stage paired pre-Decision producer tombstone")
            .expect("pre-Decision producer has a runtime handoff");
        adapter
            .acknowledge_producer_handoff(
                handoff,
                ProducerContinuationHandoffEvidence::DurableTerminal,
            )
            .expect("publish paired pre-Decision producer tombstone");
        adapter.clear_selected_producer_lifecycle();
        assert!(!adapter.producer_continuations.is_empty());
        assert!(!adapter.durable_producer_continuations.is_empty());
        assert!(adapter.serviced_candidate_count_for_test() > 0);
        snapshot_path = adapter
            .serviced_candidate_store_path_for_test()
            .to_path_buf();
        stale_snapshot = std::fs::read(&snapshot_path).expect("retain the pre-Decision snapshot");

        let decided_subject = subject(0x43);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, decided_subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let manifest = proposal.manifest;
        let (_, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x43; 96],
        };

        std::fs::remove_file(&snapshot_path).expect("remove the published snapshot");
        std::fs::create_dir(&snapshot_path).expect("replace the reclaim target with a directory");
        let wal_records_before = adapter.wal.recovered_records().len();
        assert!(matches!(
            adapter.receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    decision
                ),),
            )),
            Err(AdapterError::ServicedCandidateStore(_))
        ));
        assert!(adapter.fail_closed);
        assert!(
            adapter.wal.recovered_records().len() > wal_records_before,
            "the safety WAL advances before adjacent tombstone reclamation"
        );
        assert!(
            adapter.reducer.durable_state().decision().is_some(),
            "the failed adjacent snapshot publication cannot roll back the durable Decision"
        );
    }

    std::fs::remove_dir(&snapshot_path).expect("remove the injected reclaim obstacle");
    std::fs::write(&snapshot_path, &stale_snapshot)
        .expect("restore the last durable pre-Decision snapshot");
    let leader = context.leader(0);
    let (restarted, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context.clone()),
        Some(leader),
        reducer::Generation::new(2),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("replay the durable Decision and reclaim the stale snapshot");
    assert!(restarted.reducer.durable_state().decision().is_some());
    assert!(restarted.serviced_candidates_decision_reclaimed);
    assert_eq!(restarted.serviced_candidate_count_for_test(), 0);
    assert!(restarted.producer_continuations.is_empty());
    assert!(restarted.durable_producer_continuations.is_empty());
    assert_ne!(
        std::fs::read(&snapshot_path).expect("read replay-reclaimed snapshot"),
        stale_snapshot,
        "replay must durably replace the stale pre-Decision snapshot"
    );
    drop(restarted);

    let (mut replayed_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(3),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restore the replay-reclaimed snapshot on a second restart");
    assert!(replayed_again.reducer.durable_state().decision().is_some());
    assert!(replayed_again.serviced_candidates_decision_reclaimed);
    assert_eq!(replayed_again.serviced_candidate_count_for_test(), 0);
    assert!(replayed_again.producer_continuations.is_empty());
    assert!(replayed_again.durable_producer_continuations.is_empty());
    let decision_subject = replayed_again
        .reducer
        .durable_state()
        .decision()
        .expect("replayed Decision exists")
        .subject();
    let completion = reducer::Event::ApplicationCompleted {
        tag: replayed_again.current_tag(),
        subject: decision_subject,
    };
    let completion_candidate = replayed_again
        .serviced_candidate(&completion, DeferredPriority::Completion, None, None)
        .expect("application completion has a service identity");
    replayed_again
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision completion"), 1)
        .expect("bind post-Decision completion");
    let completion_reservation = replayed_again
        .reserve_selected_producer_continuation(Some(completion_candidate))
        .expect("suppress post-Decision producer reservation");
    assert!(completion_reservation.is_none());
    assert!(replayed_again.durable_producer_continuations.is_empty());
    assert!(replayed_again.producer_continuations.is_empty());
    replayed_again.clear_selected_producer_lifecycle();
    let post_replay = unowned_body_event(&replayed_again, marker);
    replayed_again
        .step(post_replay)
        .expect("post-Decision candidate handling remains fail-safe");
    assert_eq!(
        replayed_again.serviced_candidate_count_for_test(),
        0,
        "replay reclamation prevents the old candidate epoch from resurrecting"
    );
}

#[test]
fn serviced_candidate_snapshot_is_bound_to_the_local_validator_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let owner_a_wal = directory.path().join("owner-a.wal");
    let owner_a_snapshot;
    {
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            &owner_a_wal,
            verified_genesis(context.clone()),
            Some(0),
            reducer::Generation::new(1),
            [0xA1; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("open owner-A adapter");
        assert!(startup.is_empty());
        durably_retire_unowned_body_event(&mut adapter, 0xA1);
        owner_a_snapshot = adapter
            .serviced_candidate_store_path_for_test()
            .to_path_buf();
    }

    let owner_b_wal = directory.path().join("owner-b.wal");
    let owner_b_snapshot = directory.path().join("owner-b.wal.serviced-candidates");
    std::fs::copy(&owner_a_snapshot, &owner_b_snapshot)
        .expect("transplant owner-A sidecar onto owner-B path");
    let mut owner_b_fingerprints = fingerprints();
    owner_b_fingerprints.node = Hash::new(b"owner-b node");
    assert!(matches!(
        SumeragiV2Adapter::open_with_aggregator(
            owner_b_wal,
            verified_genesis(context),
            Some(1),
            reducer::Generation::new(1),
            [0xB2; 32],
            owner_b_fingerprints,
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        ),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let signer_subsets = [
        vec![0, 1, 2],
        vec![0, 1, 3],
        vec![0, 2, 3],
        vec![1, 2, 3],
        vec![0, 1, 2, 3],
    ];
    let marker_count = adapter.serviced_candidate_count_for_test();
    let mut qc_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xC1),
            execution_commitment: execution_commitment(0xC1),
            signers: signers.clone(),
            aggregate_signature: vec![0xC0 | marker; 96],
        };
        let carrier = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
        )
        .encode();
        let certificate = adapter
            .registry
            .qc_to_core(&certificate, &adapter.wire_context)
            .expect("convert valid same-reference QC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::QuorumCertificateReceived { tag, certificate },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("QC has a service identity");
        assert_eq!(
            candidate.0.class(),
            ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
            "scheduler priority is excluded from the logical key"
        );
        match qc_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "valid quorum subset and aggregate replacement is not a new QC owner"
            ),
            None => qc_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce QC carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 1,
        "all valid QC carrier variants share one transient identity"
    );

    let mut tc_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: signers.clone(),
                aggregate_signature: vec![0xD0 | marker; 96],
            }],
        };
        let carrier = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate.clone()),
        )
        .encode();
        let certificate = adapter
            .registry
            .tc_to_core(&certificate, &adapter.wire_context)
            .expect("convert valid same-reference TC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::TimeoutCertificateReceived { tag, certificate },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("TC has a service identity");
        match tc_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "valid timeout quorum subset and aggregate replacement is not a new owner"
            ),
            None => tc_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce TC carrier variant");
    }
    assert_ne!(qc_key, tc_key);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 2
    );

    let mut timeout_vote_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let highest_prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xC2),
            execution_commitment: execution_commitment(0xC2),
            signers: signers.clone(),
            aggregate_signature: vec![0xE0 | marker; 96],
        };
        let vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(highest_prepare),
            signer: 0,
            signature: vec![0x70 | marker; 96],
        };
        let carrier = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(
            vote.clone(),
        ))
        .encode();
        let vote = adapter
            .registry
            .timeout_vote_to_core(&vote, &adapter.wire_context)
            .expect("convert TimeoutVote with alternate high-QC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::TimeoutVoteReceived { tag, vote },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("TimeoutVote has a service identity");
        match timeout_vote_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "nested high-QC signer and signature variants are one TimeoutVote owner"
            ),
            None => timeout_vote_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce nested TimeoutVote carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 3
    );

    let proposal_round = wire::ConsensusRound { view: 1, ..round };
    let proposal_subject = subject(0xC3);
    let proposal_payload = [0xC3, 2];
    let manifest = encode_payload(
        &adapter.wire_context,
        proposal_round,
        proposal_subject,
        &proposal_payload,
    )
    .expect("encode proposal payload")
    .manifest()
    .clone();
    let mut proposal_key = None;
    for (variant, signers) in signer_subsets.iter().enumerate() {
        let marker = u8::try_from(variant).expect("small carrier variant");
        let certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: signers.clone(),
                aggregate_signature: vec![0x50 | marker; 96],
            }],
        };
        let proposal = wire::Proposal {
            round: proposal_round,
            proposer: adapter.wire_context.leader(proposal_round.view),
            subject: proposal_subject,
            manifest: manifest.clone(),
            justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                timeout_certificate: certificate,
                highest_prepare_qc: None,
            }),
            signature: vec![0x60 | marker; 96],
        };
        let carrier = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
            proposal.clone(),
        ))
        .encode();
        let proposal = adapter
            .registry
            .proposal_to_core(&proposal, &adapter.wire_context)
            .expect("convert proposal with alternate TC carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::ProposalReceived { tag, proposal },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("proposal has a service identity");
        match proposal_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "nested TC and proposal-signature variants are one proposal owner"
            ),
            None => proposal_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce nested proposal carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 4
    );

    let mut vote_key = None;
    for variant in 0_u8..5 {
        let vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xC4),
            execution_commitment: execution_commitment(0xC4),
            signer: 1,
            signature: vec![0x20 | variant; 96],
        };
        let carrier =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote.clone()))
                .encode();
        let vote = adapter
            .registry
            .vote_to_core(&vote, &adapter.wire_context)
            .expect("convert alternate vote signature carrier");
        let candidate = adapter
            .serviced_candidate(
                &reducer::Event::VoteReceived { tag, vote },
                if variant % 2 == 0 {
                    DeferredPriority::Normal
                } else {
                    DeferredPriority::Progress
                },
                None,
                Some(&carrier),
            )
            .expect("vote has a service identity");
        match vote_key {
            Some(expected) => assert_eq!(
                candidate.0, expected,
                "authenticated signature replacements are one vote owner"
            ),
            None => vote_key = Some(candidate.0),
        }
        adapter
            .record_serviced_candidate(Some(candidate), false, false, None)
            .expect("coalesce vote carrier variant");
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count + 5
    );
}

#[test]
fn serviced_candidate_capacity_exhaustion_never_evicts_an_old_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let geometry = ServicedCandidateCapacityGeometry::new(7, 3);
    let (mut adapter, startup) = open_test_with_capacity_geometry(&directory, geometry)
        .expect("open adapter with non-default production geometry");
    assert!(startup.is_empty());
    let capacity =
        serviced_candidate_capacity_with_geometry(adapter.wire_context.roster.len(), geometry);
    assert_eq!(adapter.serviced_candidate_capacity, capacity);
    assert_ne!(
        capacity,
        serviced_candidate_capacity(adapter.wire_context.roster.len()),
        "the configured runtime/effect geometry must replace the fixture default"
    );
    adapter.serviced_candidates.clear();
    for index in 0..capacity {
        let mut evidence = [0_u8; 32];
        evidence[..8].copy_from_slice(
            &u64::try_from(index)
                .expect("bounded capacity index fits u64")
                .to_le_bytes(),
        );
        let source_view = u64::try_from(index).expect("bounded source view fits u64");
        assert_eq!(
            adapter.serviced_candidates.insert(
                ServicedCandidateKey::new(
                    adapter.wire_context.id(),
                    adapter.wire_context.height,
                    adapter.fingerprints.node.into(),
                    adapter.wire_context.leader(source_view),
                    source_view,
                    None,
                    0,
                    DeferredPriority::Normal.code(),
                    u8::MAX,
                    evidence,
                ),
                adapter.current_tag().view(),
            ),
            None
        );
    }
    let retained = adapter.serviced_candidates.clone();
    let reducer_before = adapter.reducer.clone();
    let overflow = unowned_body_event(&adapter, 0x42);
    assert!(matches!(
        adapter.step(overflow),
        Err(AdapterError::ServicedCandidateStore(reason))
            if reason.contains("capacity")
    ));
    assert!(adapter.fail_closed);
    assert_eq!(
        adapter.serviced_candidates, retained,
        "capacity exhaustion cannot evict a prior tombstone"
    );
    assert_eq!(
        adapter.reducer, reducer_before,
        "capacity must be reserved before the consuming reducer transition"
    );
}

#[test]
fn persistence_macro_step_budgets_have_exact_four_effect_maximum() {
    let expected = [
        (
            PersistenceMacroStepClass::ProposalIntent,
            PersistenceMacroStepBudget::new(1, 1),
        ),
        (
            PersistenceMacroStepClass::PrepareIntent,
            PersistenceMacroStepBudget::new(2, 1),
        ),
        (
            PersistenceMacroStepClass::ObservePrepare,
            PersistenceMacroStepBudget::new(4, 1),
        ),
        (
            PersistenceMacroStepClass::LockAndCommit,
            PersistenceMacroStepBudget::new(3, 1),
        ),
        (
            PersistenceMacroStepClass::TimeoutIntent,
            PersistenceMacroStepBudget::new(1, 1),
        ),
        (
            PersistenceMacroStepClass::InstallTimeout,
            PersistenceMacroStepBudget::new(1, 4),
        ),
        (
            PersistenceMacroStepClass::Decision,
            PersistenceMacroStepBudget::new(2, 2),
        ),
    ];
    assert_eq!(
        PersistenceMacroStepClass::ALL,
        expected.map(|(class, _)| class),
        "the exhaustive WAL class inventory must remain source ordered"
    );
    for (class, budget) in expected {
        assert_eq!(class.budget(), budget);
        assert!(budget.initial_effects >= 1);
        assert!(budget.continuation_effects <= reducer::MAX_EFFECTS_PER_STEP);
        assert!(budget.flattened_effects() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    }
    assert_eq!(
        PersistenceMacroStepClass::ALL
            .into_iter()
            .map(|class| class.budget().flattened_effects())
            .max(),
        Some(MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP)
    );
    assert_eq!(
        PersistenceMacroStepClass::InstallTimeout
            .budget()
            .flattened_effects(),
        MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP,
        "local TC formation is the four-effect persistence witness"
    );
}

#[test]
fn quorum_forming_local_timeout_flattens_to_certificate_only() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = reducer::Round::new(tag.height(), tag.view());
    let context_id = adapter.reducer.context().id();

    for signer_index in [1_u32, 2] {
        let signer = adapter
            .registry
            .validator_id(signer_index)
            .expect("remote timeout signer belongs to the frozen roster");
        let retained = adapter
            .reducer
            .step(reducer::Event::TimeoutVoteReceived {
                tag,
                vote: reducer::SignedTimeoutVote::new(
                    reducer::TimeoutVote::new(context_id, round, signer, None),
                    reducer::OpaqueSignature::new(vec![
                        u8::try_from(signer_index)
                            .expect("small signer index");
                        96
                    ]),
                ),
            })
            .expect("retain the remote timeout share before local signing");
        assert!(retained.effects().is_empty());
    }

    let sign = adapter
        .timeout_elapsed(tag)
        .expect("persist the local timeout intent");
    let sign_tag = match sign.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected timeout signing frontier: {effects:?}"),
    };
    let formed = adapter
        .signature_completed(sign_tag, vec![0xA1; 96])
        .expect("flatten the quorum-forming timeout persistence boundary");
    assert!(matches!(
        formed.effects(),
        [
            AdapterEffect::EnterView {
                tag: entered_tag,
                protected_lock: None,
                ..
            },
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
                ..
            }),
        ] if entered_tag.view() == tag.view() + 1
            && certificate.round.view == tag.view()
            && certificate.groups.iter().any(|group| group.signers.contains(&0))
    ));
    assert_eq!(adapter.current_tag().view(), tag.view() + 1);
    assert!(!adapter.fail_closed);
}

#[test]
fn drive_effects_rejects_oversized_non_persisting_batch() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let effect = reducer::Effect::FetchBody {
        tag,
        round: reducer::Round::new(tag.height(), tag.view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let oversized = vec![effect; MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1];

    assert!(matches!(
        adapter.drive_effects(oversized),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects,
            maximum_initial_effects,
            persist_effects: 0,
            continuation_effects: 0,
            continuation_contains_persist: false,
            ..
        }) if initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1
            && maximum_initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}

#[test]
fn drive_effects_rejects_record_specific_overbudget_before_wal_append() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed { tag })
        .expect("stage the sole TimeoutIntent Persist")
        .into_effects();
    assert!(matches!(
        timeout.as_slice(),
        [reducer::Effect::Persist { .. }]
    ));
    let unrelated = reducer::Effect::FetchBody {
        tag,
        round: reducer::Round::new(tag.height(), tag.view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut overbudget = vec![unrelated];
    overbudget.extend(timeout);

    assert!(matches!(
        adapter.drive_effects(overbudget),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects: 2,
            maximum_initial_effects: 1,
            persist_effects: 1,
            continuation_effects: 0,
            maximum_continuation_effects: 1,
            maximum_flattened_effects: 1,
            continuation_contains_persist: false,
        })
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}

#[test]
fn drive_effects_rejects_multiple_persist_owners_before_wal_append() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed { tag })
        .expect("stage the sole TimeoutIntent Persist")
        .into_effects();
    let persist = timeout.pop().expect("one Persist effect");
    assert!(matches!(&persist, reducer::Effect::Persist { .. }));

    assert!(matches!(
        adapter.drive_effects(vec![persist.clone(), persist]),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            persist_effects: 2,
            continuation_effects: 0,
            continuation_contains_persist: false,
            ..
        })
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}

#[test]
fn post_wal_oversized_continuation_fails_closed_and_replays_exact_record() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let protected_subject = subject(0x6d);
    let prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: protected_subject,
        execution_commitment: execution_commitment(0x6d),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x6d; 96],
    };
    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x6e; 96],
        }],
    };
    let wire_context = adapter.wire_context.clone();
    let timeout = adapter
        .registry
        .tc_to_core(&timeout, &wire_context)
        .expect("convert the lock-promoting timeout certificate");
    let timeout_tag = adapter.current_tag();
    let pending_timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutCertificateReceived {
            tag: timeout_tag,
            certificate: timeout,
        })
        .expect("stage the real InstallTimeout persistence");
    let mut pending_effects = pending_timeout.into_effects();
    let reducer::Effect::Persist { tag, entry } = pending_effects
        .pop()
        .expect("InstallTimeout has one Persist effect")
    else {
        panic!("InstallTimeout must stage persistence");
    };
    assert!(pending_effects.is_empty());

    // Keep the reducer's real lock-promoting continuation, but classify
    // and encode this adversarial boundary call as the smaller
    // TimeoutIntent class. The substitute is itself a valid first WAL
    // record with the exact pending persistence ID, so the continuation
    // guard is reached only after the append succeeds.
    let timeout_round = reducer::Round::new(wire_round.height, wire_round.view);
    let local_validator = adapter
        .reducer
        .local_validator()
        .expect("test adapter is a validator");
    let forged_entry = reducer::WalEntry::new(
        entry.id(),
        reducer::WalRecord::TimeoutIntent(reducer::TimeoutVote::new(
            adapter.reducer.context().id(),
            timeout_round,
            local_validator,
            None,
        )),
    );
    assert!(matches!(
        adapter.drive_effects(vec![reducer::Effect::Persist {
            tag,
            entry: forged_entry,
        }]),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects: 1,
            maximum_initial_effects: 1,
            persist_effects: 1,
            continuation_effects: 2,
            maximum_continuation_effects: 1,
            maximum_flattened_effects: 1,
            continuation_contains_persist: false,
        })
    ));
    assert!(adapter.fail_closed);
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert_eq!(adapter.wal.recovered_records()[0].sequence, 0);
    drop(adapter);

    let (recovered, first_startup) =
        open_test(&directory).expect("replay the one valid timeout intent");
    assert!(recovered.ingress_ready());
    assert!(!recovered.fail_closed);
    assert_eq!(recovered.wal.recovered_records().len(), 1);
    assert_eq!(recovered.reducer.durable_state().last_id().get(), 1);
    assert!(
        recovered
            .reducer
            .durable_state()
            .timeout_intent(timeout_round)
            .is_some()
    );
    assert!(first_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    assert!(matches!(
        first_startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(vote),
            ..
        }] if vote.round == wire_round
            && vote.highest_prepare_qc.is_none()
            && vote.signer == 0
            && vote.signature.is_empty()
    ));
    drop(recovered);

    let (recovered_again, second_startup) =
        open_test(&directory).expect("repeat deterministic timeout-intent replay");
    assert_eq!(second_startup, first_startup);
    assert!(second_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    assert_eq!(recovered_again.wal.recovered_records().len(), 1);
    assert!(recovered_again.ingress_ready());
    assert!(!recovered_again.fail_closed);
}

#[test]
fn open_records_exactly_one_recovery_progress_transition() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    assert!(matches!(
        adapter.last_progress,
        Some((
            generation,
            round,
            wire::SumeragiV2ProgressTransition::RecoveryReplayed
        )) if generation == adapter.current_tag().generation()
            && round == reducer::Round::new(adapter.wire_context.height, 0)
    ));
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::Duplicate)
            .copied()
            .unwrap_or_default(),
        0,
        "opening must step ResumeAfterReplay once, not record a duplicate replay"
    );
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "the replay control trigger cannot consume candidate-tombstone capacity"
    );
    for attempt in 0..3 {
        adapter
            .retransmit_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("retransmit control attempt {attempt}: {error}"));
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "periodic retransmission triggers remain executable without becoming tombstones"
    );
    let status = adapter.status().expect("status after replay");
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
}

#[cfg(feature = "bls")]
#[test]
fn first_recovery_snapshot_tracks_the_durable_locked_body() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let locked_subject = subject(0xCE);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let (_, keys, _) = authenticated_context();
    let mut wire_prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xCE),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut wire_prepare, &keys);
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register durable PrepareQC");
    let round = prepare.round();
    let core_subject = prepare.subject();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare,
            vote: reducer::Vote::new(
                adapter.reducer.context().id(),
                round,
                reducer::Phase::Commit,
                core_subject,
                local_validator,
            ),
        },
    );
    let encoded = adapter
        .registry
        .encode_wal_entry(&lock_entry, &TestAggregator)
        .expect("encode durable lock");
    assert_eq!(
        adapter.wal.append(&encoded).expect("append durable lock"),
        0
    );
    drop(adapter);

    let (mut recovered, startup) = open_test(&directory).expect("recover durable lock");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        }] if vote.phase == wire::GlobalPhase::Commit
            && vote.subject == locked_subject
    ));
    assert_eq!(recovered.active_subject, Some((round, core_subject)));
    let status = recovered.status().expect("first locked recovery snapshot");
    assert_eq!(
        status.liveness.work.candidate,
        wire::SumeragiV2LocalWorkStage::Complete
    );
    assert_eq!(
        status.liveness.work.body_recovery,
        wire::SumeragiV2LocalWorkStage::Queued
    );
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
}

#[test]
fn persistence_is_fsynced_before_sign_is_exposed() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    assert!(adapter.ingress_ready());
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(7);
    let proposal = proposal(&adapter.wire_context, proposer, subject);
    let fetch = adapter
        .receive_verified(proposal)
        .expect("accept proposal")
        .into_effects();
    let (tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let round = manifest.round;
    let store = adapter
        .body_available(tag, manifest)
        .expect("body available")
        .into_effects();
    assert!(matches!(
        store.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    let receipt = durable_body_receipt(&adapter, round, subject);
    let validate = adapter
        .body_stored(tag, round, subject, &receipt)
        .expect("body stored")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let validated = ValidatedBodyReceipt::for_test(receipt.clone());
    let sign = adapter
        .validation_succeeded(tag, round, subject, &validated)
        .expect("valid body")
        .into_effects();
    assert!(matches!(sign.as_slice(), [AdapterEffect::Sign { .. }]));
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
}

fn advance_direct_validation_fixture_to_durable(
    adapter: &mut SumeragiV2Adapter,
    marker: u8,
) -> (
    reducer::EventTag,
    wire::PayloadManifest,
    DurableBodyReceipt,
    ValidatedBodyReceipt,
) {
    let proposer = adapter.status().expect("status").leader;
    let body_subject = subject(marker);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, body_subject))
        .expect("accept direct-validation proposal")
        .into_effects();
    let (tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected direct-validation fetch effects: {effects:?}"),
    };
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare direct BodyAvailable transition")
    else {
        panic!("missing body must prepare one Store successor")
    };
    assert!(matches!(
        available.commit(),
        AdapterEffect::StoreBody {
            tag: effect_tag,
            round,
            subject,
        } if effect_tag == tag && round == manifest.round && subject == manifest.subject
    ));
    let durable = durable_body_receipt(adapter, manifest.round, manifest.subject);
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(tag, manifest.round, manifest.subject, &durable)
        .expect("prepare direct BodyStored transition")
    else {
        panic!("available body must prepare one Validate successor")
    };
    assert!(matches!(
        stored.commit(),
        AdapterEffect::ValidateBody {
            tag: effect_tag,
            round,
            subject,
        } if effect_tag == tag && round == manifest.round && subject == manifest.subject
    ));
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    (tag, manifest, durable, validated)
}
