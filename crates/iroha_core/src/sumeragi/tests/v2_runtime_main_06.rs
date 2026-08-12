#[test]
fn decision_retires_stale_local_completion_for_durable_recovery() {
    let directory = TempDir::new().expect("temporary stale-decision directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let stale_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xD7);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    let commitment = validated.execution_commitment();
    stage_completion_for_queue_test(
        &mut runtime,
        stale_tag,
        AdapterCommand::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable,
            validated_receipt: validated,
        },
    );

    runtime.round_tag = EventTag::new(
        stale_tag.height(),
        stale_tag.view().saturating_add(1),
        Generation::new(stale_tag.generation().get().saturating_add(1)),
    );
    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
            .expect("retire stale exact completion after certified view change"),
        DecisionProposalRetirement::new(None, 1)
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.fail_closed);
    runtime
        .enqueue_body_available(runtime.round_tag(), manifest)
        .expect("durable reconstruction can claim the current reducer tag");
}

#[test]
fn progress_cursor_decision_preserves_outer_ingress_completion_until_apply() {
    const PHASE_INVENTORY: [&str; 2] = ["decided_local_proposal_ready", "application_completed"];

    let directory = TempDir::new().expect("temporary Decision-race directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xD9);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    let commitment = validated.execution_commitment();
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xD9; 96],
    };
    runtime
        .ingress
        .enqueue_authenticated(
            owner_tag,
            CommandClass::Progress,
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
            )),
        )
        .expect("enqueue the older CommitQC progress item");
    // The completion is admitted second. The class cursor may select
    // between siblings of one lifecycle, but it cannot move this later
    // local callback ahead of the already-admitted Decision.
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable.clone(),
            validated_receipt: validated.clone(),
        },
    );
    runtime
        .enqueue_local_proposal(
            owner_tag,
            manifest.clone(),
            durable.clone(),
            validated.clone(),
        )
        .expect("an exact trusted retry coalesces with its existing owner");
    assert_eq!(runtime.queued_commands(), 2);
    runtime.ingress.next_class = CommandClass::Progress;
    let now = Instant::now();
    runtime.arm_live_clocks(now).expect("arm runtime clocks");

    let RuntimeStep::Advanced(decision_effects) = runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("Progress cursor installs Decision")
    else {
        panic!("queued CommitQC must advance the reducer")
    };
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody {
            subject,
            certificate: Some(certificate),
            ..
        }] if *subject == manifest.subject && certificate == &decision
    ));
    assert_eq!(runtime.queued_commands(), 1);

    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
            .expect("Decision cleanup preserves the exact completion"),
        DecisionProposalRetirement::new(Some(owner_tag), 0)
    );
    let RuntimeStep::Advanced(completion_effects) = runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("fair completion service reaches the reducer")
    else {
        panic!("retained completion must advance the reducer")
    };
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply {
            subject,
            certificate,
            ..
        }] if *subject == manifest.subject && certificate == &decision
    ));
    assert!(!completion_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::FetchBody { .. } | AdapterEffect::StoreBody { .. }
    )));
    assert_eq!(runtime.queued_commands(), 0);

    let mut suppressed_phases = Vec::new();
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_local_proposal(owner_tag, manifest.clone(), durable, validated)
        .expect("the decided validated body suppresses a drained local completion retry");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("decided_local_proposal_ready");

    runtime
        .enqueue_application_completed(owner_tag, manifest.subject)
        .expect("enqueue exact Apply acknowledgement");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch exact Apply acknowledgement"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    for _ in 0..3 {
        runtime
            .enqueue_application_completed(owner_tag, manifest.subject)
            .expect("an applied-height acknowledgement retry is a monotone stutter");
    }
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("application_completed");
    assert_eq!(suppressed_phases, PHASE_INVENTORY);
}

#[test]
fn decision_cleanup_preserves_unique_busy_deferred_completion() {
    let directory = TempDir::new().expect("temporary Busy-deferred Decision directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xDA);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &manifest,
            DeferredBodyPipelineStageForTest::LocalProposalReady,
        )
        .expect("stage exact Busy-deferred completion");

    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
            .expect("retain exact Busy-deferred completion"),
        DecisionProposalRetirement::new(Some(owner_tag), 0)
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(
        runtime
            .driver
            .deferred_decided_local_proposal_counts(
                owner_tag,
                manifest.round,
                manifest.subject,
                commitment,
            )
            .retainable(),
        1
    );
}

#[test]
fn decision_commitment_mismatch_fails_closed_before_retirement() {
    let directory = TempDir::new().expect("temporary mismatched-decision directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xD8);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"decision mismatch parent state"),
        Hash::new(b"decision mismatch post state"),
        Hash::new(b"decision mismatch ordinary writes"),
        1,
        Hash::new(b"decision mismatch executed block"),
    );
    assert_ne!(validated.execution_commitment(), conflicting_commitment);
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable,
            validated_receipt: validated,
        },
    );

    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(
                manifest.round,
                manifest.subject,
                conflicting_commitment,
            )
            .expect_err("Decision commitment drift must fail closed"),
        "Sumeragi v2 decided local proposal evidence conflicts with the durable Decision"
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.queued_commands(),
        1,
        "conflict preflight must preserve the original evidence for diagnosis"
    );
    assert!(matches!(
        runtime.ingress.commands.front().map(|queued| &queued.command),
        Some(AdapterCommand::LocalProposalReady {
            manifest: queued,
            ..
        }) if queued == &manifest
    ));
}

#[test]
fn unbound_direct_prepare_and_commit_votes_are_recoverable_after_validation() {
    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        let directory = TempDir::new().expect("temporary unbound-vote directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let manifest = runtime_manifest(&context, 0xD7);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable);
        let signed_vote = signed_runtime_vote(
            &keys,
            manifest.round,
            phase,
            manifest.subject,
            validated.execution_commitment(),
        );

        let far_future_round = wire::ConsensusRound {
            view: u64::MAX,
            ..manifest.round
        };
        let signed_far_future = signed_runtime_vote(
            &keys,
            far_future_round,
            phase,
            manifest.subject,
            validated.execution_commitment(),
        );
        assert!(
            runtime.can_admit_network_message(&signed_far_future),
            "a structurally valid far-future {phase:?} vote must drain without certified local view authority"
        );
        assert!(matches!(
            runtime.enqueue_network(signed_far_future),
            Err(NetworkIngressError::Authentication(
                AdapterError::MissingExecutionCommitment
            ))
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            !runtime.fail_closed,
            "rejecting a far-future unbound {phase:?} vote must not poison the runtime"
        );

        let mut malformed_future = signed_vote.clone();
        let wire::ConsensusMessageV2Payload::Vote(malformed_vote) = &mut malformed_future.payload
        else {
            unreachable!("fixture is a direct vote");
        };
        malformed_vote.round.view = u64::MAX;
        malformed_vote.proposal_round.view = u64::MAX;
        malformed_vote.signature.clear();
        assert!(
            runtime.can_admit_network_message(&malformed_future),
            "a structurally invalid far-future {phase:?} vote must drain for normal rejection"
        );
        assert!(matches!(
            runtime.enqueue_network(malformed_future),
            Err(NetworkIngressError::Authentication(_))
        ));
        assert_eq!(runtime.queued_commands(), 0);

        assert!(
            !runtime.can_admit_network_message(&signed_vote),
            "an early {phase:?} vote must remain fair-ingress owned until its proposal is validated"
        );
        // The mutating seam still rejects a caller that bypasses the
        // non-mutating fair-ingress gate.
        assert!(matches!(
            runtime.enqueue_network(signed_vote.clone()),
            Err(NetworkIngressError::Authentication(
                AdapterError::MissingExecutionCommitment
            ))
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            !runtime.fail_closed,
            "recoverable {phase:?} authentication rejection must not poison the runtime"
        );

        let proposer = context.leader(manifest.round.view);
        let mut proposal = wire::Proposal {
            round: manifest.round,
            proposer,
            subject: manifest.subject,
            manifest: manifest.clone(),
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        runtime
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal),
            ))
            .expect("matching proposal establishes a pending body pipeline");
        assert_eq!(runtime.queued_commands(), 1);
        assert!(
            !runtime.can_admit_network_message(&signed_vote),
            "the {phase:?} vote remains a recoverable fair-ingress prerequisite while validation is pending"
        );
        runtime
            .arm_live_clocks(Instant::now())
            .expect("arm fixture clocks before dispatch");
        runtime
            .step_and_take_scheduler_ownership_for_test(Instant::now())
            .expect("dispatch matching proposal");
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            !runtime.can_admit_network_message(&signed_vote),
            "the registered manifest keeps the {phase:?} vote deferred while validation is pending"
        );
        assert!(!runtime.fail_closed);

        let reducer_tag_before_binding = runtime.driver.current_tag();
        let reducer_body_before_binding = runtime
            .driver
            .body_state_for_test(manifest.round, manifest.subject);
        runtime
            .bind_validated_body(&manifest, &validated)
            .expect("live validation establishes canonical commitment authority");
        assert_eq!(
            runtime.driver.current_tag(),
            reducer_tag_before_binding,
            "wire-authority binding cannot retag the reducer"
        );
        assert_eq!(
            runtime
                .driver
                .body_state_for_test(manifest.round, manifest.subject),
            reducer_body_before_binding,
            "wire-authority binding cannot revive a reducer consumer"
        );
        assert!(
            runtime.can_admit_network_message(&signed_vote),
            "the retained fair-ingress {phase:?} vote becomes drainable after validation"
        );

        let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting early vote parent state"),
            Hash::new(b"conflicting early vote post state"),
            Hash::new(b"conflicting early vote ordinary writes"),
            1,
            Hash::new(b"conflicting early vote executed block"),
        );
        assert_ne!(
            conflicting_commitment,
            validated.execution_commitment(),
            "the conflict fixture must differ from canonical validation"
        );
        let conflicting_vote = signed_runtime_vote(
            &keys,
            manifest.round,
            phase,
            manifest.subject,
            conflicting_commitment,
        );
        assert!(
            runtime.can_admit_network_message(&conflicting_vote),
            "a conflicting bound {phase:?} vote must drain for authenticated rejection"
        );
        assert!(matches!(
            runtime.enqueue_network(conflicting_vote),
            Err(NetworkIngressError::Authentication(
                AdapterError::ConflictingExecutionCommitment
            ))
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            !runtime.fail_closed,
            "conflicting {phase:?} vote rejection must not poison the runtime"
        );

        runtime
            .enqueue_network(signed_vote)
            .expect("the same signed canonical vote becomes admissible after validation");
        assert_eq!(runtime.queued_commands(), 1);
        assert!(!runtime.fail_closed);

        let stale_directory = TempDir::new().expect("temporary stale-vote directory");
        let (mut stale_runtime, stale_context, stale_keys) =
            authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
        let stale_manifest = runtime_manifest(&stale_context, 0xD9);
        let stale_durable = DurableBodyReceipt::for_test(
            stale_context.id(),
            stale_manifest.round,
            stale_manifest.subject,
            HashOf::new(&stale_manifest),
        );
        let stale_validated = ValidatedBodyReceipt::for_test(stale_durable);
        let stale_message = signed_runtime_vote(
            &stale_keys,
            stale_manifest.round,
            phase,
            stale_manifest.subject,
            stale_validated.execution_commitment(),
        );
        assert!(
            !stale_runtime.can_admit_network_message(&stale_message),
            "an unbound {phase:?} vote is retained while its view remains active"
        );
        let initial = stale_runtime.round_tag();
        let next = EventTag::new(
            initial.height(),
            initial.view() + 1,
            Generation::new(initial.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut stale_runtime, initial, next, &stale_manifest);
        assert!(
            stale_runtime.can_admit_network_message(&stale_message),
            "view change releases an unmatched stale {phase:?} vote for bounded rejection"
        );
    }
}

#[test]
fn exact_authenticated_network_retransmission_obeys_runtime_boundaries() {
    let directory = TempDir::new().expect("temporary runtime ingress directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(5, 1, 1));
    let original = signed_runtime_proposal(&context, &keys, 1);
    let second = signed_runtime_proposal(&context, &keys, 2);
    let third = signed_runtime_proposal(&context, &keys, 3);
    let transport = match &original.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadManifest(proposal.manifest.clone()),
        ),
        _ => unreachable!("fixture is a proposal"),
    };

    let owner_tag = runtime
        .enqueue_network(original.clone())
        .expect("first authenticated proposal owns one normal slot");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime
            .enqueue_network(original.clone())
            .expect("exact duplicate coalesces below the normal boundary"),
        owner_tag
    );
    assert_eq!(runtime.queued_commands(), 1);

    let mut invalid = third.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut invalid.payload else {
        unreachable!("fixture is a proposal")
    };
    proposal.signature[0] ^= 0x80;
    assert!(matches!(
        runtime.enqueue_network(invalid),
        Err(NetworkIngressError::Authentication(_))
    ));
    assert_eq!(runtime.queued_commands(), 1);

    runtime
        .enqueue_network(second.clone())
        .expect("non-identical authenticated proposal uses ordinary capacity");
    assert_eq!(runtime.queued_commands(), 2);
    assert_eq!(
        runtime
            .enqueue_network(original.clone())
            .expect("exact duplicate coalesces at reserved capacity"),
        owner_tag
    );
    assert!(matches!(
        runtime.enqueue_network(third.clone()),
        Err(NetworkIngressError::Backpressure(
            EnqueueError::ReservedCapacity
        ))
    ));

    let cursor_before = runtime.ingress.next_class;
    let tags_before = runtime
        .ingress
        .commands
        .iter()
        .map(|queued| queued.tag)
        .collect::<Vec<_>>();
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![4]),
    );
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![5]),
    );
    assert_eq!(runtime.queued_commands(), 4);
    assert!(runtime.can_admit_network_message(&original));
    assert!(!runtime.can_admit_network_message(&third));
    assert_eq!(
        runtime
            .enqueue_network(original.clone())
            .expect("exact authenticated duplicate coalesces at full ordinary capacity"),
        owner_tag
    );
    assert_eq!(runtime.queued_commands(), 4);
    assert_eq!(runtime.ingress.next_class, cursor_before);
    assert_eq!(
        runtime
            .ingress
            .commands
            .iter()
            .take(tags_before.len())
            .map(|queued| queued.tag)
            .collect::<Vec<_>>(),
        tags_before
    );
    assert!(matches!(
        runtime.enqueue_network(third),
        Err(NetworkIngressError::Backpressure(EnqueueError::Full))
    ));

    runtime.fail_closed = true;
    assert!(matches!(
        runtime.enqueue_network(original.clone()),
        Err(NetworkIngressError::FailClosed)
    ));
    assert!(matches!(
        runtime.enqueue_network(transport.clone()),
        Err(NetworkIngressError::FailClosed)
    ));
    runtime.fail_closed = false;
    assert!(matches!(
        runtime.enqueue_network(transport),
        Err(NetworkIngressError::TransportPayload)
    ));
}

#[test]
fn certified_commit_uses_physical_slot_reserved_from_completions() {
    let directory = TempDir::new().expect("temporary certified-capacity directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();

    for signature in [vec![3], vec![4], vec![5]] {
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(signature),
        );
    }
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(runtime.remaining_completion_capacity(), 0);

    let commit = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
        signed_runtime_quorum_certificate(&context, &keys, 0xE0),
    ));
    assert!(
        runtime.can_admit_network_message(&commit),
        "the authenticated CommitQC owns the one slot hidden from completion producers"
    );
    runtime
        .enqueue_network(commit)
        .expect("the CommitQC consumes its reserved physical slot");
    assert_eq!(runtime.queued_commands(), 4);
    assert!(matches!(
        runtime.ingress.check_capacity(CommandClass::Completion),
        Err(EnqueueError::Full)
    ));
    assert!(!runtime.fail_closed);
}

#[test]
fn certified_commit_arriving_first_preserves_every_ordinary_reserve() {
    let directory = TempDir::new().expect("temporary certified-order directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let owner_tag = runtime.round_tag();

    let commit = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
        signed_runtime_quorum_certificate(&context, &keys, 0xD0),
    ));
    runtime
        .enqueue_network(commit)
        .expect("the early CommitQC is charged to the certified slot");
    assert_eq!(
        runtime.remaining_completion_capacity(),
        7,
        "charging the CommitQC to its own slot leaves every ordinary position free"
    );

    for marker in 0xD1..=0xD5 {
        let prepare =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate_for_phase(
                    &context,
                    &keys,
                    marker,
                    wire::GlobalPhase::Prepare,
                ),
            ));
        runtime
            .enqueue_network(prepare)
            .expect("ordinary Progress capacity is independent of certificate arrival order");
    }
    assert_eq!(runtime.remaining_completion_capacity(), 2);

    let manifest = runtime_manifest(&context, 0xD6);
    let body_reservation = runtime
        .ingress
        .reserve_canonical_body_available(owner_tag, manifest)
        .expect("BodyAvailable can reserve the first completion slot after an early CommitQC");
    assert_eq!(runtime.queued_commands(), 6);
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    runtime
        .ingress
        .commit_canonical_body_available(body_reservation)
        .expect("the reserved BodyAvailable materializes without another capacity charge");
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![4]),
    );
    assert_eq!(runtime.queued_commands(), 8);
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert!(matches!(
        runtime.ingress.check_capacity(CommandClass::Completion),
        Err(EnqueueError::Full)
    ));
    assert!(!runtime.fail_closed);
}

#[test]
fn prepare_qc_cannot_spend_the_certified_physical_credit() {
    let directory = TempDir::new().expect("temporary certified-classifier directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let certificate = |marker, phase| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase(&context, &keys, marker, phase),
        ))
    };

    for marker in [0xB0, 0xB1] {
        runtime
            .enqueue_network(certificate(marker, wire::GlobalPhase::Prepare))
            .expect("the two ordinary Progress positions accept PrepareQCs");
    }
    assert!(matches!(
        runtime.enqueue_network(certificate(0xB2, wire::GlobalPhase::Prepare)),
        Err(NetworkIngressError::Backpressure(
            EnqueueError::ReservedCapacity
        ))
    ));

    runtime
        .enqueue_network(certificate(0xB3, wire::GlobalPhase::Commit))
        .expect("only the CommitQC receives the certified physical credit");
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    assert!(!runtime.fail_closed);
}

#[test]
fn distinct_certificates_share_exactly_one_physical_credit() {
    let directory = TempDir::new().expect("temporary certified-credit directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let commit = |marker| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, marker),
        ))
    };

    for marker in 0xC0..=0xC2 {
        runtime
            .enqueue_network(commit(marker))
            .expect("one certified root uses the extra slot and the others use Progress");
    }
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    assert!(
        !runtime.can_admit_network_message(&commit(0xC3)),
        "a fourth certificate cannot receive a second physical credit"
    );
    assert!(matches!(
        runtime.enqueue_network(commit(0xC3)),
        Err(NetworkIngressError::Backpressure(
            EnqueueError::ReservedCapacity
        ))
    ));

    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![0xC4]),
    );
    assert_eq!(runtime.queued_commands(), 4);
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert!(!runtime.fail_closed);
}

#[test]
fn retiring_the_sole_certificate_does_not_fake_completion_headroom() {
    let directory = TempDir::new().expect("temporary certified-retirement directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let certificate = |marker, phase| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase(&context, &keys, marker, phase),
        ))
    };

    runtime
        .enqueue_network(certificate(0xA0, wire::GlobalPhase::Commit))
        .expect("the CommitQC owns the single certified credit");
    for marker in [0xA1, 0xA2] {
        runtime
            .enqueue_network(certificate(marker, wire::GlobalPhase::Prepare))
            .expect("ordinary Progress fills its exact class allocation");
    }
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![0xA3]),
    );
    assert_eq!(runtime.queued_commands(), 4);
    assert_eq!(runtime.remaining_completion_capacity(), 0);

    let (retired, _) = runtime
        .ingress
        .pop_pacemaker_progress_with_ownership(
            |_| true,
            |command| command.is_certified_fence_escape(),
        )
        .expect("the certified priority seam remains exact")
        .expect("the retained CommitQC is selectable");
    assert!(retired.command.is_certified_fence_escape());
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(
        runtime.remaining_completion_capacity(),
        0,
        "retiring the sole certificate removes its credit as well as its physical owner"
    );
    assert!(matches!(
        runtime.ingress.check_capacity(CommandClass::Completion),
        Err(EnqueueError::Full)
    ));

    runtime
        .ingress
        .pop_next()
        .expect("one ordinary FIFO service turn opens completion admission");
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    assert!(
        runtime
            .ingress
            .check_capacity(CommandClass::Completion)
            .is_ok()
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn unpublished_body_replacement_cannot_overbook_the_certified_slot() {
    let directory = TempDir::new().expect("temporary body-replacement directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let proposal = signed_runtime_proposal(&context, &keys, 0x94);
    let mut canonical = match &proposal.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal.manifest.clone(),
        _ => unreachable!("runtime proposal fixture has Proposal payload"),
    };
    canonical.chunk_hashes = vec![Hash::new(b"canonical replacement chunk"); 2];
    canonical.chunk_root = Hash::new(b"canonical replacement root");

    runtime
        .enqueue_network(proposal)
        .expect("the conflicting proposal occupies Normal capacity");
    runtime
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate_for_phase(
                    &context,
                    &keys,
                    0x95,
                    wire::GlobalPhase::Prepare,
                ),
            ),
        ))
        .expect("ordinary Progress occupies its class allocation");
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![0x96]),
    );
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(runtime.remaining_completion_capacity(), 0);

    let reservation = runtime
        .ingress
        .reserve_canonical_body_available(owner_tag, canonical)
        .expect("the unpublished body atomically replaces its conflicting proposal");
    assert_eq!(
        runtime.queued_commands(),
        2,
        "the conflicting proposal must retire before the reservation becomes live"
    );
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    runtime
        .ingress
        .abort_canonical_body_available(reservation.clone());

    runtime
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ),
        ))
        .expect("the retained unpublished body cannot exclude the certified escape");
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(
        runtime
            .ingress
            .occupied_with_dormant_reservations()
            .expect("bounded live ownership remains countable"),
        4
    );

    runtime
        .ingress
        .commit_canonical_body_available(reservation)
        .expect("the exact token materializes without changing total ownership");
    assert_eq!(runtime.queued_commands(), 4);
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    let snapshot = runtime.ingress.ownership_snapshot();
    assert!(snapshot.validate_identity());
    for (fifo_position, owner) in snapshot.occurrence_owners.iter().enumerate() {
        assert_eq!(
            snapshot.occurrence_index.get(&owner.admission_ordinal),
            Some(&fifo_position),
            "a reserved earlier admission materialized at the FIFO tail without corrupting its exact position"
        );
    }
    assert!(!runtime.fail_closed);
}

#[test]
fn pacemaker_retry_marks_excludes_and_reconciles_exact_fifo_occurrence() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    driver.retry_once.insert(0xE2);
    driver.signature_fence_active = true;
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Progress,
        FakeCommand::record(0xE2),
    )
    .expect("admit one unblocked retryable Progress root");
    bind_fake_local_deferred_target_for_test(&mut runtime, b"pacemaker-retry-target");

    let first = runtime
        .dispatch_one_pacemaker_progress(start)
        .expect("retryable pacemaker dispatch remains exact")
        .expect("the unmarked occurrence owns one bounded turn");
    assert!(matches!(first, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("retry retains exact scheduler evidence");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::PacemakerProgressRetryRetained
    );
    assert!(evidence.fence_retry_marker_required);
    assert!(evidence.fence_retry_blocked_fifo_before.is_empty());
    let [retained_marker] = evidence.fence_retry_blocked_fifo_after.as_slice() else {
        panic!("retry installs exactly one physical occurrence marker")
    };
    let retained_marker = retained_marker.clone();
    assert_eq!(evidence.validate_exact(), Ok(()));

    let mut missing_requirement = evidence.clone();
    missing_requirement.fence_retry_marker_required = false;
    missing_requirement.projection_hash = runtime_scheduler_projection_hash(&missing_requirement);
    assert_eq!(
        missing_requirement.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "a coherently rehashed retry cannot omit its required marker transition"
    );

    assert!(
        runtime
            .dispatch_one_pacemaker_progress(start)
            .expect("marked pacemaker selection remains valid")
            .is_none(),
        "the same retryable occurrence cannot spin on the next turn"
    );
    assert!(runtime.last_scheduler_ownership().is_none());

    runtime
        .reconcile_fence_retry_blocked_fifo_owners()
        .expect("a duplicate certified transition preserves the same fence owner");
    assert_eq!(
        runtime.fence_retry_blocked_fifo_owners,
        vec![retained_marker.clone()]
    );

    runtime.driver.signature_fence_identity += 1;
    runtime
        .reconcile_fence_retry_blocked_fifo_owners()
        .expect("a successor signer retires the prior fence's retry exclusions");
    assert!(runtime.fence_retry_blocked_fifo_owners.is_empty());
    assert!(runtime.fence_retry_signature_fence_identity.is_none());

    runtime
        .retain_fence_retry_blocked_fifo_owner(retained_marker)
        .expect("the still-queued occurrence can bind to the successor fence");
    runtime.ingress.commands.pop_front();
    runtime
        .reconcile_fence_retry_blocked_fifo_owners()
        .expect("an independently retired exact occurrence prunes its marker");
    assert!(runtime.fence_retry_blocked_fifo_owners.is_empty());
    assert!(!runtime.fail_closed);
}

#[test]
fn fence_predecessor_retry_gets_one_bounded_dependency_turn() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    driver.signature_fence_active = true;
    assert!(driver.retry_once.insert(0xD1));
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(0xD1),
    )
    .expect("admit one pre-target retryable predecessor");

    let target_ordinal =
        bind_fake_local_deferred_target_for_test(&mut runtime, b"retryable-fence-target");
    assert_eq!(
        runtime
            .physically_eligible_deferred_admission_ordinals()
            .expect("the target remains physically eligible behind its predecessor"),
        BTreeSet::from([target_ordinal])
    );
    assert!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("ordinary deferred eligibility remains logically ordered")
            .is_empty(),
        "serviceable deferred work may not overtake its older FIFO predecessor"
    );

    let first = runtime
        .dispatch_one_fence_dependency(start, None)
        .expect("the retryable predecessor dependency remains exact")
        .expect("the oldest predecessor owns one bounded turn");
    assert!(matches!(first, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("retry retains exact fence-dependency evidence");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::FencePredecessorRetryRetained
    );
    assert!(evidence.fence_retry_marker_required);
    assert!(evidence.fence_retry_blocked_fifo_before.is_empty());
    assert_eq!(evidence.fence_retry_blocked_fifo_after.len(), 1);
    assert_eq!(evidence.queue_before, evidence.queue_after);
    assert_eq!(evidence.validate_exact(), Ok(()));
    assert_eq!(runtime.queued_commands(), 1);

    assert!(
        runtime
            .dispatch_one_fence_dependency(start, None)
            .expect("the marked dependency set remains valid")
            .is_none(),
        "the same retryable predecessor cannot spin ahead of its fence completion"
    );
    assert!(runtime.last_scheduler_ownership().is_none());
    assert!(!runtime.fail_closed);
}

#[test]
fn stale_certified_escape_preserves_same_fence_retry_exclusion() {
    let directory = TempDir::new().expect("temporary stale-certified directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(6, 2, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before installing two certified views");

    for certificate_view in [0_u64, 1] {
        runtime
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    signed_runtime_timeout_certificate_for_view(&context, &keys, certificate_view),
                ),
            ))
            .expect("admit the next exact view certificate");
        let advanced = runtime
            .try_step_pacemaker_escape(now)
            .expect("certified view installation remains exact")
            .expect("the next TC owns one pacemaker turn");
        let RuntimeStep::Advanced(effects) = advanced else {
            panic!("certified view installation unexpectedly idled")
        };
        assert!(matches!(
            effects.as_slice(),
            [AdapterEffect::EnterView { tag, .. }]
                if tag.view() == certificate_view + 1
        ));
        runtime
            .take_last_scheduler_ownership()
            .expect("view installation retains exact scheduler evidence");
        runtime
            .take_effect_ownership(effects.len())
            .expect("consume the installed view's effect ownership");
    }

    let signer_tag = runtime.round_tag();
    assert_eq!(signer_tag.view(), 2);
    let timeout = runtime
        .driver
        .timeout_elapsed(signer_tag)
        .expect("open the view-two local TimeoutVote signer");
    assert!(matches!(
        timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    assert!(runtime.driver.signature_fence_is_active());

    let prepare = |marker| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase_at_view(
                &context,
                &keys,
                marker,
                wire::GlobalPhase::Prepare,
                2,
            ),
        ))
    };
    runtime
        .enqueue_network(prepare(0xE7))
        .expect("admit the deferred PrepareQC target");
    let deferred = runtime
        .try_step_pacemaker_escape(now)
        .expect("PrepareQC Busy handoff remains exact")
        .expect("the first PrepareQC owns one pacemaker turn");
    assert!(matches!(deferred, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    runtime
        .take_last_scheduler_ownership()
        .expect("Busy PrepareQC retains exact scheduler evidence");
    assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
    assert!(!runtime.driver.deferred_work_is_serviceable());

    let highest_prepare = signed_runtime_quorum_certificate_for_phase_at_view(
        &context,
        &keys,
        0xE9,
        wire::GlobalPhase::Prepare,
        0,
    );
    let stale_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let signers = vec![0, 1, 2];
    let stale_preimage = wire::TimeoutVote {
        round: stale_round,
        highest_prepare_qc: Some(highest_prepare.clone()),
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let stale_shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                &stale_preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let stale_share_refs = stale_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let stale = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
        wire::TimeoutCertificate {
            round: stale_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(highest_prepare),
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                    &stale_share_refs,
                )
                .expect("aggregate the stale certified fixture"),
            }],
        },
    ));
    let marked_prepare = prepare(0xE8);
    let marked_source = context.roster[1].validator.clone();
    let stale_source = context.roster[2].validator.clone();
    let (_leader_wire_directory, _leader_wire_ingress, ownerships) =
        preowned_leader_wire_ownerships(
            &context,
            &[
                (marked_prepare.clone(), marked_source),
                (stale.clone(), stale_source),
            ],
            runtime.ingress.lifecycle_ordinals.clone(),
        );
    let [marked_ownership, stale_ownership]: [FairV2IngressOwnershipEvidence; 2] = ownerships
        .try_into()
        .expect("fixture creates one pre-cut marker and one post-cut certified owner");
    let target_cut = runtime
        .deferred_lifecycle_ownership
        .values()
        .next()
        .expect("the PrepareQC target retains its frozen physical cut")
        .physical_cut;
    assert!(
        u128::from(
            marked_ownership
                .physical_admission_ordinal()
                .expect("marked PrepareQC owns a physical occurrence")
        ) < target_cut
    );
    assert!(
        u128::from(
            stale_ownership
                .physical_admission_ordinal()
                .expect("stale TC owns a physical occurrence")
        ) >= target_cut,
        "the stale certified command must exercise the pacemaker path, not the pre-cut dependency path"
    );

    runtime
        .enqueue_network_with_ingress_ownership(marked_prepare, marked_ownership)
        .expect("admit one exact blocked FIFO occurrence");
    let marker = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| {
            queued
                .cached_queue_occurrence_owner(&runtime.ingress.selection_source_identity)
                .cloned()
        })
        .expect("blocked PrepareQC retains its exact occurrence owner");
    runtime
        .retain_fence_retry_blocked_fifo_owner(marker.clone())
        .expect("bind the exact retry exclusion to the active signer");
    let marker_before = runtime.fence_retry_blocked_fifo_owners.clone();
    runtime
        .enqueue_network_with_ingress_ownership(stale, stale_ownership)
        .expect("admit a valid but stale certified escape");

    let escaped = runtime
        .try_step_pacemaker_escape(now)
        .expect("stale certified scheduling remains exact")
        .expect("the authenticated stale TC owns one pacemaker turn");
    assert!(matches!(escaped, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("stale TC retains exact scheduler evidence");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &evidence.candidate else {
        panic!("stale TC owns one exact authenticated FIFO occurrence")
    };
    assert_eq!(
        candidate.selection_seal.kind,
        RuntimeQueueSelectionKind::PacemakerCertifiedProgress
    );
    assert_eq!(evidence.fence_retry_blocked_fifo_before, marker_before);
    assert_eq!(evidence.fence_retry_blocked_fifo_after, marker_before);
    assert_eq!(evidence.validate_exact(), Ok(()));
    assert!(runtime.driver.signature_fence_is_active());
    runtime
        .reconcile_fence_retry_blocked_fifo_owners()
        .expect("the unchanged view-two signer preserves its retry exclusion");
    assert_eq!(runtime.fence_retry_blocked_fifo_owners, marker_before);
    assert!(!runtime.fail_closed);
}

#[test]
fn certified_tc_crosses_full_fence_blocked_prepare_prefix() {
    let directory = TempDir::new().expect("temporary certified-prefix directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(4, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before opening the signing fence");
    let owner_tag = runtime.round_tag();
    let timeout = runtime
        .driver
        .timeout_elapsed(owner_tag)
        .expect("open one local TimeoutVote signing fence");
    assert!(matches!(
        timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));

    let prepare = |marker| {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase(
                &context,
                &keys,
                marker,
                wire::GlobalPhase::Prepare,
            ),
        ))
    };
    runtime
        .enqueue_network(prepare(0xE1))
        .expect("admit the first PrepareQC");
    let first = runtime
        .try_step_pacemaker_escape(now)
        .expect("first PrepareQC scheduling is valid")
        .expect("first PrepareQC owns a pacemaker turn");
    assert!(matches!(first, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    let first_owner = runtime
        .take_last_scheduler_ownership()
        .expect("first PrepareQC retains scheduler ownership");
    assert_eq!(
        first_owner.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
    assert!(!runtime.driver().deferred_work_is_serviceable());

    runtime
        .enqueue_network(prepare(0xE2))
        .expect("admit the second PrepareQC");
    assert!(
        runtime
            .try_step_pacemaker_escape(now)
            .expect("blocked PrepareQC classification is valid")
            .is_none(),
        "pacemaker escape cannot repeatedly redispatch a fence-blocked PrepareQC"
    );
    assert!(runtime.last_scheduler_ownership().is_none());
    assert_eq!(runtime.queued_commands(), 1);

    runtime
        .enqueue_network(prepare(0xE3))
        .expect("fill the second ordinary Progress slot");
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::SignatureCompleted(vec![0xE4]),
    );
    assert_eq!(runtime.queued_commands(), 3);
    assert_eq!(runtime.remaining_completion_capacity(), 0);

    let tc = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
        signed_runtime_timeout_certificate(&context, &keys),
    ));
    assert!(
        runtime.can_admit_network_message(&tc),
        "the certified escape slot remains available after every ordinary slot fills"
    );
    runtime
        .enqueue_network(tc)
        .expect("the TC consumes the reserved certified slot");
    assert_eq!(runtime.queued_commands(), 4);

    let certified = runtime
        .try_step_pacemaker_escape(now)
        .expect("certified selection remains valid")
        .expect("the later TC bypasses the older retry owner");
    let RuntimeStep::Advanced(effects) = certified else {
        panic!("certified TC unexpectedly idled")
    };
    assert!(matches!(
        effects.as_slice(),
        [AdapterEffect::EnterView { tag, .. }] if tag.view() == owner_tag.view() + 1
    ));
    let certified_owner = runtime
        .take_last_scheduler_ownership()
        .expect("TC retains exact certified scheduler ownership");
    assert_eq!(
        certified_owner.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &certified_owner.candidate else {
        panic!("TC must own one exact queued candidate")
    };
    assert_eq!(
        candidate.selection_seal.kind,
        RuntimeQueueSelectionKind::PacemakerCertifiedProgress
    );
    assert!(certified_owner.validate_exact().is_ok());
    runtime
        .take_effect_ownership(effects.len())
        .expect("the executor consumes the TC EnterView ownership");
    assert!(runtime.driver().deferred_work_is_serviceable());
    let retired = runtime
        .try_step_pacemaker_escape(now)
        .expect("the now-unblocked retained PrepareQC remains schedulable")
        .expect("the retained PrepareQC receives its terminal service turn");
    assert!(matches!(retired, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    runtime
        .take_last_scheduler_ownership()
        .expect("retired PrepareQC preserves its exact scheduler owner");
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(!runtime.fail_closed);
}

#[test]
fn exact_authenticated_timeout_certificate_coalesces_then_applies_through_signer() {
    let directory = TempDir::new().expect("temporary multi-source TC directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(4, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before authenticated ingress");
    let round_tag = runtime.round_tag();
    let timeout_effects = runtime
        .driver
        .timeout_elapsed(round_tag)
        .expect("install a local signing fence")
        .into_effects();
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));

    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
            signed_runtime_timeout_certificate(&context, &keys),
        ));
    let first_source = PeerId::new(keys[1].public_key().clone());
    let second_source = PeerId::new(keys[2].public_key().clone());
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(
                message.clone(),
                fair_network_ownership(&message, first_source),
            )
            .expect("the first authenticated TC carrier owns the runtime command"),
        round_tag
    );
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(
                message.clone(),
                fair_network_ownership(&message, second_source),
            )
            .expect("the same TC from another source coalesces"),
        round_tag
    );
    assert_eq!(
        runtime.queued_commands(),
        1,
        "one exact aggregate TC must retain every bounded source carrier"
    );
    let retained = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("the coalesced TC retains exact ingress ownership");
    assert!(retained.validate_exact());
    assert_eq!(retained.direct.len(), 2);

    let effects = match runtime.step(now) {
        Ok(RuntimeStep::Advanced(effects)) => effects,
        other => panic!("authenticated TC did not apply immediately: {other:?}"),
    };
    assert!(matches!(
        effects.as_slice(),
        [AdapterEffect::EnterView { tag, .. }] if tag.view() == round_tag.view() + 1
    ));
    let selected = runtime
        .take_last_scheduler_ownership()
        .expect("the applied TC dispatch retains its exact runtime owner");
    assert!(selected.validate_exact().is_ok());
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &selected.candidate else {
        panic!("the applied TC must retain its exact queued owner")
    };
    assert!(
        candidate
            .ingress_ownership
            .as_ref()
            .is_some_and(|ownership| { ownership.validate_exact() && ownership.direct.len() == 2 })
    );
    runtime
        .take_effect_ownership(effects.len())
        .expect("the executor consumes the TC EnterView owner");
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(!runtime.fail_closed);
}

#[test]
fn admitted_progress_cannot_be_starved_by_older_normal_churn() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(7, 2, 1),
    );
    for value in 0..3 {
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(value),
        )
        .unwrap();
    }
    for value in 100..140 {
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value)
            ),
            Err(EnqueueError::ReservedCapacity)
        );
    }
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Progress,
        FakeCommand::record(200),
    )
    .expect("CommitQC/progress reserve remains available");

    let initial_queue = runtime.queue_snapshot(start);
    assert_eq!(initial_queue.normal.depth, 3);
    assert_eq!(initial_queue.progress.depth, 1);

    runtime
        .step_and_take_scheduler_ownership_for_test(start)
        .expect("bounded class service selects admitted Progress");
    assert_eq!(runtime.driver.delivered.last(), Some(&(initial, 200)));

    for (expected, replacement) in [(0, 3), (1, 4), (2, 5)] {
        runtime
            .step_and_take_scheduler_ownership_for_test(start)
            .expect("normal work resumes after the bounded Progress turn");
        assert_eq!(runtime.driver.delivered.last(), Some(&(initial, expected)));
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(replacement),
        )
        .expect("later normal churn may refill only the vacated normal slot");
    }
    assert_eq!(
        runtime.driver.delivered,
        vec![(initial, 200), (initial, 0), (initial, 1), (initial, 2)]
    );
    let queue = runtime.queue_snapshot(start);
    assert_eq!(queue.normal.depth, 3);
    assert_eq!(queue.normal.capacity, 3);
    assert_eq!(queue.normal.max_service_debt, 0);
    assert_eq!(queue.progress.depth, 0);
    assert_eq!(queue.completion.depth, 0);
}

include!("v2_runtime_periodic_fairness.rs");

#[test]
fn periodic_delay_is_bounded_and_absolute_timeout_has_priority() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(5, 1, 1),
    );
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(7),
    )
    .unwrap();

    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
        .expect("periodic retransmission gets one prompt bounded turn");
    assert!(runtime.driver.delivered.is_empty());
    assert_eq!(runtime.driver.retransmits, vec![initial]);
    assert!(runtime.driver.timeouts.is_empty());

    runtime
        .step(start + Duration::from_secs(2))
        .expect("FIFO debt runs immediately after the periodic turn");
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("admitted FIFO publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
    assert_eq!(runtime.driver.retransmits, vec![initial]);
    assert!(runtime.driver.timeouts.is_empty());
    runtime
        .take_effect_ownership(1)
        .expect("consume the FIFO effect owner before the timeout turn");

    runtime
        .step(start + Duration::from_secs(10))
        .expect("absolute timeout preempts every replenished periodic owner");
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("absolute timeout publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Timeout
    );
    assert_eq!(runtime.driver.timeouts, vec![initial]);
    assert_eq!(
        runtime.driver.retransmits,
        vec![initial],
        "the absolute deadline cannot replenish a periodic owner ahead of timeout"
    );
}

#[test]
fn due_timeout_becomes_older_than_replenished_exact_serve_tickets() {
    let start = Instant::now();
    let initial = tag(0);
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(5, 1, 1),
        Vec::new(),
        lifecycle_ordinals.clone(),
    )
    .expect("construct runtime with the shared Serve source")
    .0;
    runtime
        .arm_live_clocks(start)
        .expect("arm shared-source runtime");

    let first_barrier = lifecycle_ordinals
        .reserve_one()
        .expect("reserve first exact Serve occurrence");
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(start + Duration::from_secs(10), first_barrier,)
            .expect("first barrier freezes the due timeout"),
        "a clock first frozen behind this ticket cannot overtake it"
    );

    let second_barrier = lifecycle_ordinals
        .reserve_one()
        .expect("reserve a distinct retransmission occurrence");
    assert!(
        runtime
            .older_lifecycle_predates_exact_serve(start + Duration::from_secs(10), second_barrier,)
            .expect("replenished barrier validates against the same source"),
        "the frozen timeout must predate every later exact ticket"
    );
    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
        .expect("one bounded predecessor episode dispatches the timeout");
    assert_eq!(runtime.driver.timeouts, vec![initial]);
}

#[test]
fn restored_serve_high_watermark_precedes_startup_runtime_owner() {
    let start = Instant::now();
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(41);
    let (mut runtime, startup) = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
        FakeDriver::new(tag(0)),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(5, 1, 1),
        vec![FakeEffect::other()],
        lifecycle_ordinals.clone(),
    )
    .expect("construct restarted runtime after durable Serve waiter");
    let ownership = runtime
        .take_effect_ownership(startup.len())
        .expect("startup owner retains exact lifecycle sidecar");
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0].owner().lifecycle_ordinal(), 42);
    assert_eq!(
        lifecycle_ordinals
            .reserve_one()
            .expect("later exact Serve ticket follows startup recovery"),
        43
    );
}

#[test]
fn full_runtime_churn_cannot_cross_an_exact_serve_ordinal() {
    let start = Instant::now();
    let initial = tag(0);
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(6, 1, 1),
        Vec::new(),
        lifecycle_ordinals.clone(),
    )
    .expect("construct runtime with shared admission order")
    .0;
    runtime
        .arm_live_clocks(start)
        .expect("arm shared-source runtime");
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .expect("admit the frozen predecessor");
    let barrier = lifecycle_ordinals
        .reserve_one()
        .expect("reserve exact Serve position");
    for value in 2..=3 {
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(value),
        )
        .expect("fill only the later normal prefix");
    }

    assert!(
        runtime
            .older_lifecycle_predates_exact_serve(start, barrier)
            .expect("compare the full runtime prefix")
    );
    runtime
        .step_and_take_scheduler_ownership_for_test(start)
        .expect("one bounded predecessor transition runs");
    assert_eq!(runtime.driver.delivered, vec![(initial, 1)]);
    assert_eq!(runtime.queued_commands(), 2);
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(start, barrier)
            .expect("later churn remains behind the exact ticket")
    );
}

#[test]
fn network_admission_uses_exact_normal_and_progress_reservations() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(4, 1, 1),
    );
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"runtime-test-context",
        ))),
        height: 7,
        view: 3,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime-test-block")),
        payload_hash: Hash::new(b"runtime-test-payload"),
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"runtime parent state"),
        Hash::new(b"runtime post state"),
        Hash::new(b"runtime ordinary writes"),
        1,
        Hash::new(b"runtime executed block wire"),
    );
    let vote = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer: 0,
        signature: vec![1],
    });
    let locked_commit_vote = match &vote {
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            let mut vote = vote.clone();
            vote.phase = wire::GlobalPhase::Commit;
            wire::ConsensusMessageV2Payload::Vote(vote)
        }
        _ => unreachable!("fixture is a vote"),
    };
    runtime.driver.protected_commit = Some((round, subject, execution_commitment));
    let mismatched_commit_vote = match &locked_commit_vote {
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            let mut vote = vote.clone();
            vote.subject.payload_hash = Hash::new(b"mismatched runtime commit vote");
            wire::ConsensusMessageV2Payload::Vote(vote)
        }
        _ => unreachable!("fixture is a vote"),
    };
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let mut prepare_certificate = certificate.clone();
    prepare_certificate.phase = wire::GlobalPhase::Prepare;
    let prepare_qc = wire::ConsensusMessageV2Payload::QuorumCertificate(prepare_certificate);
    let commit_qc = wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone());
    let timeout_vote = wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: vec![1],
    });
    let timeout_certificate =
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire::TimeoutCertificate {
            round,
            groups: Vec::new(),
        });
    let commit_response = wire::ConsensusMessageV2Payload::CommitCertificateResponse(
        wire::CommitCertificateResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime commit request")),
            certificate,
            responder: PeerId::new(KeyPair::random().public_key().clone()),
            signature: vec![1],
        },
    );
    assert_eq!(network_command_class(&vote), Some(CommandClass::Normal));
    assert_eq!(
        network_command_class(&commit_qc),
        Some(CommandClass::Progress)
    );
    assert_eq!(
        network_command_class(&timeout_vote),
        Some(CommandClass::Progress),
        "authenticated TimeoutVote traffic owns the protected progress prefix"
    );
    assert_eq!(network_command_class(&commit_response), None);
    assert_eq!(
        network_admission_class(&commit_response),
        Some(CommandClass::Progress)
    );
    assert!(runtime.can_admit_network_payload(&vote));
    assert!(runtime.can_admit_network_payload(&prepare_qc));
    assert!(runtime.can_admit_network_payload(&commit_qc));
    assert!(runtime.can_admit_network_payload(&timeout_vote));
    assert!(runtime.can_admit_network_payload(&timeout_certificate));
    assert!(runtime.can_admit_network_payload(&commit_response));

    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .expect("fill the normal prefix while preserving every reserved class");
    assert!(!runtime.can_admit_network_payload(&vote));
    assert!(
        !runtime.can_admit_network_payload(&mismatched_commit_vote),
        "a merely Commit-shaped vote must stop at pre-authentication backpressure"
    );
    assert!(
        runtime.can_admit_network_payload(&locked_commit_vote),
        "the exact locked Commit vote can reach authentication through the progress reserve"
    );
    assert!(
        runtime.can_admit_network_payload(&commit_qc),
        "CommitQC can use the reserved progress slot"
    );
    assert!(
        runtime.can_admit_network_payload(&timeout_vote),
        "TimeoutVote can use the reserved progress slot"
    );
    assert!(runtime.can_admit_network_payload(&commit_response));

    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Progress,
        FakeCommand::record(3),
    )
    .expect("fill the progress prefix");
    assert!(!runtime.can_admit_network_payload(&vote));
    assert!(!runtime.can_admit_network_payload(&mismatched_commit_vote));
    assert!(!runtime.can_admit_network_payload(&locked_commit_vote));
    assert!(
        !runtime.can_admit_network_payload(&prepare_qc),
        "PrepareQC cannot spend the final physical certified-fence slot"
    );
    assert!(
        runtime.can_admit_network_payload(&commit_qc),
        "CommitQC owns the final physical certified-fence slot"
    );
    assert!(!runtime.can_admit_network_payload(&timeout_vote));
    assert!(
        runtime.can_admit_network_payload(&timeout_certificate),
        "TC owns the final physical certified-fence slot"
    );
    assert!(
        runtime.can_admit_network_payload(&commit_response),
        "a CommitQC recovery response owns the final physical certified-fence slot"
    );

    let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 2,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1,
            max_chunk_count: 2,
        },
        chunk_hashes: vec![Hash::new([0_u8]); 2],
        chunk_root: Hash::new(b"runtime transport root"),
    });
    assert!(runtime.can_admit_network_payload(&transport));
}

#[test]
fn stale_completion_retains_tag_and_precedes_a_later_due_retransmit() {
    let start = Instant::now();
    let current = tag(4);
    let stale = tag(2);
    let mut runtime = runtime(
        FakeDriver::new(current),
        start,
        RuntimeQueueConfig::new(5, 1, 1),
    );
    enqueue_fake(
        &mut runtime,
        stale,
        CommandClass::Completion,
        FakeCommand::record(9),
    )
    .unwrap();
    runtime
        .step(start + Duration::from_secs(2))
        .expect("the older admitted completion owns the first turn");
    assert_eq!(runtime.driver.delivered, vec![(stale, 9)]);
    assert!(runtime.driver.retransmits.is_empty());
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("the completion publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    runtime
        .take_effect_ownership(1)
        .expect("consume the completion effect owner before the next turn");

    // The retransmit lifecycle was frozen when it first became due, so it
    // owns the next turn after the older completion drains.
    runtime
        .step(start + Duration::from_secs(4))
        .expect("the frozen retransmit owns the next turn");
    assert_eq!(runtime.driver.retransmits, vec![current]);
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("the retransmit publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::PeriodicTimer
    );
}

#[test]
fn only_enter_view_effect_restarts_both_clocks() {
    let start = Instant::now();
    let initial = tag(0);
    let next = tag(1);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );

    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .unwrap();
    let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(1));
    assert_eq!(runtime.round_tag(), initial);

    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Progress,
        FakeCommand::enter_view(next),
    )
    .unwrap();
    // The first periodic prompt cannot overtake ready Progress. EnterView
    // therefore owns this turn and resets both clocks before either fires.
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9)),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(runtime.round_tag(), next);
    assert!(runtime.driver.retransmits.is_empty());
    runtime
        .reconcile_active_view_producer(next, false)
        .expect("the nonleader test peer retires the positional view producer");
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9)),
        Ok(RuntimeStep::Idle)
    ));
    assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
    assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(22));

    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10)),
        Ok(RuntimeStep::Idle)
    ));
    let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(11));
    assert_eq!(runtime.driver.retransmits, vec![next]);
    let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(19));
    assert!(runtime.driver.timeouts.is_empty());
    let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(29));
    assert_eq!(runtime.driver.timeouts, vec![next]);
}

#[test]
fn startup_enter_view_effect_restarts_clocks_and_is_returned_unchanged() {
    let start = Instant::now();
    let initial = tag(0);
    let next = tag(1);
    let (mut runtime, effects) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        vec![FakeEffect::enter_view(next), FakeEffect::other()],
    )
    .unwrap();
    assert_eq!(runtime.round_tag(), next);
    assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
    assert_eq!(
        effects,
        vec![FakeEffect::enter_view(next), FakeEffect::other()]
    );
    runtime
        .take_effect_ownership(effects.len())
        .expect("the startup executor consumes both returned effect owners");
    assert!(matches!(
        runtime.step(start + Duration::from_secs(100)),
        Err(RuntimeError::ClocksNotArmed)
    ));
    runtime
        .reconcile_active_view_producer(next, false)
        .expect("the nonleader startup peer retires the positional producer");
    runtime
        .arm_live_clocks(start + Duration::from_secs(100))
        .expect("arm after startup effects are dispatched");
    assert_eq!(
        runtime.arm_live_clocks(start + Duration::from_secs(101)),
        Err(RuntimeClockError::AlreadyArmed)
    );
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(119)),
        Ok(RuntimeStep::Advanced(_)) | Ok(RuntimeStep::Idle)
    ));
    assert!(runtime.driver.timeouts.is_empty());
    let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(120));
    assert_eq!(runtime.driver.timeouts, vec![next]);
}

#[test]
fn interrupted_tip_recovery_drains_ingress_without_arming_live_timers() {
    let start = Instant::now();
    let initial = tag(0);
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("open unarmed recovery runtime");
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Completion,
        FakeCommand::record(7),
    )
    .expect("queue local recovery completion");

    assert!(matches!(
        runtime.step_recovery_and_take_scheduler_ownership_for_test(
            start + Duration::from_secs(1_000)
        ),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
    assert!(runtime.driver.timeouts.is_empty());
    assert!(runtime.driver.retransmits.is_empty());
    assert!(matches!(
        runtime.step_recovery_and_take_scheduler_ownership_for_test(
            start + Duration::from_secs(2_000)
        ),
        Ok(RuntimeStep::Idle)
    ));
}

#[test]
fn interrupted_tip_recovery_is_rejected_after_live_clock_arm() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );

    assert!(matches!(
        runtime.step_recovery(start),
        Err(RuntimeError::RecoveryAfterClocksArmed)
    ));
}

#[test]
fn adapter_failure_closes_runtime_permanently() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(5, 1, 1),
    );
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Completion,
        FakeCommand::fail(),
    )
    .unwrap();
    assert!(matches!(
        runtime.step(start),
        Err(RuntimeError::Driver(FakeError))
    ));
    assert_eq!(
        runtime.fail_closed_reason.as_deref(),
        Some("runtime driver rejected a serialized transition: fake driver failure")
    );
    assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
    assert_eq!(
        runtime.fail_closed_reason.as_deref(),
        Some("runtime driver rejected a serialized transition: fake driver failure"),
        "the generic closed guard cannot replace the driver root cause"
    );
}

#[test]
fn invalid_configuration_is_rejected() {
    let start = Instant::now();
    let initial = tag(0);
    let result = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::ZERO,
        RuntimeQueueConfig::new(4, 1, 1),
        Vec::<FakeEffect>::new(),
    );
    assert!(matches!(
        result,
        Err(RuntimeConfigError::InvalidRoundTimeout)
    ));

    let invalid_queue = RuntimeQueueConfig::new(3, 1, 1).validate();
    assert_eq!(
        invalid_queue,
        Err(RuntimeConfigError::InvalidQueueAllocation)
    );
}

#[test]
fn queue_configuration_excludes_one_certified_credit_from_ordinary_limits() {
    let config = RuntimeQueueConfig::new(8, 2, 2)
        .validate()
        .expect("C=8, P=2, K=2 leaves a distinct certified credit");

    assert_eq!(config.normal_limit(), 3);
    assert_eq!(config.progress_limit(), 5);
    assert_eq!(config.ordinary_total_limit(), 7);
    assert_eq!(
        config.normal_limit() + config.progress_reserve + config.completion_reserve + 1,
        config.capacity
    );
}
