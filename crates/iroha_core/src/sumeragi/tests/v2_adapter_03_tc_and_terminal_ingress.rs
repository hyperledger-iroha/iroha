#[test]
#[allow(clippy::too_many_lines)]
fn tc_reset_readmits_exact_locked_commit_once_per_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let locked_subject = subject(0xD6);
    let locked_execution_commitment = execution_commitment(0xD6);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let wire_prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xA6; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the durable PrepareQC");
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
                core_context.id(),
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
        .expect("encode the durable lock");
    assert_eq!(
        adapter
            .wal
            .append(&encoded)
            .expect("append the durable lock"),
        0
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(1),
        [lock_entry],
    )
    .expect("recover the durable locked Commit intent");
    let replay_tag = adapter.reducer.current_tag();
    let replay = adapter
        .reducer
        .step(reducer::Event::ResumeAfterReplay { tag: replay_tag })
        .expect("resume the durable Commit intent");
    assert!(matches!(
        replay.effects(),
        [reducer::Effect::Sign {
            message: reducer::SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == reducer::Phase::Commit
    ));
    adapter
        .signature_completed(replay_tag, vec![0xB6])
        .expect("restore the local locked CommitVote");

    let locked_vote = |signer, marker| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer,
            signature: vec![marker],
        }))
    };
    let first = AuthenticatedConsensusMessage::for_test(locked_vote(1, 0xB7));
    assert!(adapter.authenticated_ingress_is_progress(&first));
    assert_eq!(
        adapter
            .receive_authenticated(first)
            .expect("deliver the original locked vote")
            .disposition(),
        reducer::StepDisposition::Applied
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("suppress a same-generation duplicate")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );

    let tag_before_tc = adapter.current_tag();
    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(wire_prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC6; 96],
        }],
    };
    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install the timeout certificate through the adapter");
    assert_eq!(installed.disposition(), reducer::StepDisposition::Applied);
    let installed_effects = installed.into_effects();
    let commit_sign_tag = installed_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.round == wire_round && vote.phase == wire::GlobalPhase::Commit => Some(*tag),
            _ => None,
        })
        .expect("TC installation must reconstruct the exact local locked Commit vote");
    assert!(adapter.current_tag().strictly_advances(tag_before_tc));
    assert_eq!(
        adapter.current_tag().generation(),
        reducer::Generation::INITIAL
    );
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 0);
    assert_eq!(
        adapter.active_subject,
        Some((round, core_subject)),
        "EnterView must restore the durable locked subject"
    );

    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("re-admit the exact locked vote after the pool reset")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "the reset generation must own exactly one deferred remote vote"
    );
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("coalesce a duplicate behind the reset-generation owner")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    adapter
        .signature_completed(commit_sign_tag, vec![0xB8])
        .expect("complete the reconstructed local vote");
    adapter
        .drain_deferred()
        .expect("drain remote ownership in its own macro-step");
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
                1, 0xB7,
            )))
            .expect("suppress a second delivery in the reset generation")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );

    let liveness = adapter.status().expect("build liveness snapshot");
    liveness
        .validate()
        .expect("adapter liveness snapshot is structurally valid");
    assert_eq!(
        liveness.liveness.generation,
        adapter.current_tag().generation().get()
    );
    assert!(liveness.liveness.outbound_intents.iter().any(|intent| {
        intent.kind == wire::SumeragiV2OutboundIntentKind::CommitVote
            && intent.round == wire_round
            && intent.subject == Some(locked_subject)
            && intent.execution_commitment == Some(locked_execution_commitment)
            && intent.stage == wire::SumeragiV2OutboundIntentStage::Sent
    }));
    assert!(liveness.liveness.commit_quorums.iter().any(|quorum| {
        quorum.round == wire_round
            && quorum.subject == locked_subject
            && quorum.execution_commitment == locked_execution_commitment
            && quorum.signer_count == 2
    }));
    assert_eq!(liveness.liveness.queues.len(), 4);
    assert!(liveness.liveness.ignore_counts.iter().any(|entry| {
        entry.reason == wire::SumeragiV2IgnoreReason::Duplicate && entry.count >= 2
    }));

    let conflict =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: subject(0xD7),
            execution_commitment: execution_commitment(0xD7),
            signer: 1,
            signature: vec![0xD7],
        }));
    assert!(
        !adapter.authenticated_ingress_is_progress(&AuthenticatedConsensusMessage::for_test(
            conflict.clone()
        ))
    );
    let evidence = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(conflict.clone()))
        .expect("report the conflicting locked-round vote");
    assert!(matches!(
        evidence.effects(),
        [AdapterEffect::ReportEquivocation {
            evidence
        }]
            if matches!(evidence.as_ref(), wire::SumeragiV2Equivocation::PhaseVote { .. })
    ));
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(conflict))
            .expect("cap repeated equivocation evidence")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );

    let penultimate = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
            2, 0xB8,
        )))
        .expect("deliver another locked vote");
    assert_eq!(penultimate.disposition(), reducer::StepDisposition::Applied);
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote(
            3, 0xB9,
        )))
        .expect("rebuild the old-round Commit quorum through the adapter");
    assert!(
        penultimate
            .effects()
            .iter()
            .chain(decided.effects())
            .any(|effect| matches!(
                effect,
                AdapterEffect::Apply { subject, certificate, .. }
                    if *subject == locked_subject
                        && certificate.round == wire_round
                        && certificate.phase == wire::GlobalPhase::Commit
            ) || matches!(
                effect,
                AdapterEffect::FetchBody {
                    subject,
                    certificate: Some(certificate),
                    ..
                } if *subject == locked_subject
                    && certificate.round == wire_round
                    && certificate.phase == wire::GlobalPhase::Commit
            ))
    );
    assert!(adapter.reducer.durable_state().decision().is_some());
    assert!(
        adapter.reducer.durable_state().locked().is_some(),
        "the retained Prepare lock makes this a post-decision admission regression"
    );

    // The local signer's vote was reconstructed from WAL rather than
    // admitted through network ingress, so this is a fresh semantic key
    // arriving only after Decision became durable.
    let decided_vote = locked_vote(0, 0xBA);
    let authenticated = AuthenticatedConsensusMessage::for_test(decided_vote.clone());
    assert!(!adapter.wire_ingress_may_use_progress(&decided_vote.payload));
    assert!(!adapter.authenticated_ingress_is_progress(&authenticated));

    let decided_key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: 0,
    };
    let protected_before = adapter
        .deferred_progress_inputs
        .iter()
        .filter(|input| input.protected_progress)
        .count();
    assert_eq!(
        adapter
            .receive_authenticated(authenticated)
            .expect("terminally ignore a new vote after Decision")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::AlreadyDecided)
    );
    for _ in 0..3 {
        // Model successive pool generations retaining the same semantic
        // delivery. Once Decision is durable, the old locked vote must be
        // height-long duplicate history rather than a per-generation retry.
        adapter
            .ingress_deliveries
            .get_mut(&decided_key)
            .expect("terminal AlreadyDecided delivery remains recorded")
            .generation = reducer::Generation::new(1);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    decided_vote.clone(),
                ))
                .expect("suppress the decided vote through the full ingress path")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
            "a durable Decision closes generation-scoped locked-vote retries"
        );
    }
    assert_eq!(
        adapter
            .deferred_progress_inputs
            .iter()
            .filter(|input| input.protected_progress)
            .count(),
        protected_before,
        "decided votes cannot consume protected deferred ownership"
    );
}

#[test]
fn terminal_ignored_ingress_is_recorded_before_duplicate_coalescing() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xBD),
        execution_commitment: execution_commitment(0xBD),
        signer: 1,
        signature: vec![0xBD],
    };
    let payload = wire::ConsensusMessageV2Payload::Vote(vote.clone());
    let (early, admission) = adapter
        .admit_authenticated_payload(&payload)
        .expect("admit a fresh authenticated vote");
    assert!(early.is_none());
    let admission = admission.expect("fresh vote owns semantic admission");
    let context = adapter.wire_context.clone();
    let core_vote = adapter
        .registry
        .vote_to_core(&vote, &context)
        .expect("convert the admitted vote");
    let stale_tag = reducer::EventTag::new(round.height, round.view, reducer::Generation::new(0));

    let ignored = adapter
        .step_authenticated_ingress(
            reducer::Event::VoteReceived {
                tag: stale_tag,
                vote: core_vote,
            },
            Some(admission),
        )
        .expect("terminally ignore the stale-generation delivery");
    assert_eq!(
        ignored.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::StaleGeneration)
    );

    let (duplicate, admission) = adapter
        .admit_authenticated_payload(&payload)
        .expect("coalesce the exact terminal retransmission");
    assert_eq!(
        duplicate
            .expect("terminal delivery is duplicate history")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert!(admission.is_none());
}
