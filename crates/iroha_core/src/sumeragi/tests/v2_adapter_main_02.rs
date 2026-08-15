fn assert_restored_stage_seven_retirement_does_not_resurrect(
    marker: u8,
    reserve_completion: bool,
    materialize_before_retirement: bool,
    inject_persistence_failure: bool,
) {
    let directory = TempDir::new().expect("temporary stage-7 retirement directory");
    let StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    } = persist_stage_seven_crash_cut(&directory, marker);
    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the stage-7 retirement crash cut");
    assert!(startup.is_empty());
    assert!(
        restarted
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );
    let restarted_tag = restarted.current_tag();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: body_subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([marker, 3]),
            Hash::new([marker, 4]),
            Hash::new([marker, 5]),
            1,
            Hash::new([marker, 6]),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    certificate
        .validate(&wire_context)
        .expect("certified retirement reconstruction is structurally valid");
    let reconstructed_fetch = AdapterEffect::FetchBody {
        tag: restarted_tag,
        round,
        subject: body_subject,
        manifest: (marker != 0xBD).then_some(manifest.clone()),
        certified_sources: wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            logical_ordinal,
        );
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            vec![reconstructed_fetch.clone()],
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals,
        )
        .expect("construct runtime for restored stage-7 retirement");
    assert_eq!(startup, vec![reconstructed_fetch.clone()]);
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the stage-7 retirement runtime");
    let mut ownership = runtime
        .take_effect_ownership(1)
        .expect("take reconstructed retirement fetch ownership");
    let fetch_ownership = ownership.pop().expect("one reconstructed fetch owner");
    assert!(ownership.is_empty());
    assert_ne!(
        fetch_ownership.owner().causal_origin().lifecycle_key,
        logical_key
    );
    let capacity_before = runtime.remaining_completion_capacity();
    if !reserve_completion {
        assert!(
            runtime
                .retire_restored_body_fetch_parent(&reconstructed_fetch, &fetch_ownership)
                .expect("persist terminal restored fetch-parent retirement")
        );
        assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
        assert!(
            !runtime
                .driver()
                .producer_continuations
                .contains_key(&restored_address)
                && !runtime
                    .driver()
                    .durable_producer_continuations
                    .contains_key(&restored_address)
                && !runtime
                    .driver()
                    .restored_dormant_producer_continuations
                    .contains(&restored_address),
            "terminal fetch cancellation must remove its dormant stage-7 parent"
        );
        drop(runtime.into_driver());
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
        .expect("reopen after terminal restored fetch cancellation");
        assert!(restarted_again.producer_continuations.is_empty());
        return;
    }
    let reservation = runtime
        .reserve_body_available_with_owner(restarted_tag, manifest, &fetch_ownership)
        .expect("reserve the restored completion before terminal retirement");
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before - 1);
    assert!(
        !inject_persistence_failure || !materialize_before_retirement,
        "the persistence-failure seam targets the unpublished token"
    );
    let sabotaged_snapshot = inject_persistence_failure.then(|| {
        let path = runtime
            .driver()
            .serviced_candidate_store_path_for_test()
            .to_path_buf();
        let bytes = std::fs::read(&path).expect("read the stage-7 producer snapshot");
        std::fs::remove_file(&path).expect("remove the published producer snapshot");
        std::fs::create_dir(&path).expect("replace producer snapshot with a directory");
        (path, bytes)
    });
    let retired = if materialize_before_retirement {
        runtime
            .commit_body_available(reservation)
            .expect("materialize restored completion before pipeline retirement");
        runtime
            .retire_body_pipeline_completions(restarted_tag, round, body_subject)
            .map(|retired| retired.body_available())
    } else {
        runtime.retire_unpublished_body_available(restarted_tag, round, body_subject)
    };
    if let Some((path, bytes)) = sabotaged_snapshot {
        assert!(
            retired.is_err(),
            "a failed durable release cannot publish volatile token retirement"
        );
        assert_eq!(
            runtime.remaining_completion_capacity(),
            capacity_before - 1,
            "failed persistence retains the exact unpublished physical owner"
        );
        assert!(runtime.driver().fail_closed);
        assert_eq!(
            runtime
                .driver()
                .producer_continuations
                .get(&restored_address),
            runtime
                .driver()
                .durable_producer_continuations
                .get(&restored_address),
            "failed persistence restores both in-memory producer aliases"
        );
        assert!(
            runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address)
        );
        std::fs::remove_dir(&path).expect("remove sabotaged producer directory");
        std::fs::write(&path, bytes).expect("restore the pre-retirement producer snapshot");
        drop(runtime.into_driver());
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
        .expect("reopen the retained stage-7 producer after failed retirement");
        assert!(
            restarted_again
                .restored_dormant_producer_continuations
                .contains(&restored_address),
            "failed retirement must reopen the old owner instead of losing it"
        );
        return;
    }
    assert!(retired.expect("persist and retire the restored body completion"));
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
    assert!(
        !runtime
            .driver()
            .producer_continuations
            .contains_key(&restored_address)
            && !runtime
                .driver()
                .durable_producer_continuations
                .contains_key(&restored_address)
            && !runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address),
        "terminal runtime retirement must persistently release the restored producer"
    );
    drop(runtime.into_driver());
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
    .expect("reopen after terminal stage-7 retirement");
    assert!(
        restarted_again.producer_continuations.is_empty()
            && restarted_again.durable_producer_continuations.is_empty()
            && restarted_again
                .restored_dormant_producer_continuations
                .is_empty(),
        "a terminally retired stage-7 producer cannot resurrect on restart"
    );
}
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
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic Decision signer")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let decision_preimage = wire::Vote {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let decision_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &decision_preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let decision_share_refs = decision_shares
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &decision_share_refs,
            )
            .expect("aggregate durable Decision CommitQC"),
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
