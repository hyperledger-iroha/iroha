#[test]
fn forged_body_receipt_cannot_cross_the_prepare_durability_boundary() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(31);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, proposed_subject))
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
    adapter
        .body_available(tag, manifest)
        .expect("body available");
    let correct = durable_body_receipt(&adapter, round, proposed_subject);
    let forged = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        round,
        subject(32),
        correct.manifest_hash(),
    );
    assert!(matches!(
        adapter.body_stored(tag, round, proposed_subject, &forged),
        Err(AdapterError::DurableBodyMismatch)
    ));
    assert!(matches!(
        adapter
            .body_stored(tag, round, proposed_subject, &correct)
            .expect("the real durable receipt remains usable")
            .effects(),
        [AdapterEffect::ValidateBody { .. }]
    ));
}

#[test]
fn local_proposal_and_prepare_are_each_persisted_before_signing() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let subject = subject(8);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let (durable, validated) =
        validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
    let proposal_tag = adapter.current_tag();
    let sign = adapter
        .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
        .expect("submit local proposal")
        .into_effects();
    let tag = match sign.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ] => {
            assert!(proposal.signature.is_empty());
            *tag
        }
        effects => panic!("unexpected local proposal effects: {effects:?}"),
    };
    assert_eq!(adapter.wal.recovered_records().len(), 1);

    let effects = adapter
        .signature_completed(tag, vec![0xD1; 96])
        .expect("sign local proposal")
        .into_effects();
    assert!(matches!(
        effects.as_slice(),
        [
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            }),
            AdapterEffect::Sign {
                request: SignRequest::Vote(_),
                ..
            }
        ]
    ));
    assert_eq!(adapter.wal.recovered_records().len(), 2);
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
}

#[test]
fn local_proposal_commitment_conflict_is_transactional() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let proposed_subject = subject(0x7b);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let conflicting = execution_commitment(0x7c);
    assert_ne!(conflicting, validated.execution_commitment());
    adapter
        .registry
        .register_execution_commitment(round, core_subject, conflicting)
        .expect("pre-bind a conflicting authenticated commitment");

    let subjects_before = adapter.registry.subjects.clone();
    let manifests_before = adapter.registry.manifests.clone();
    let commitments_before = adapter.registry.execution_commitments.clone();
    let active_before = adapter.active_subject;
    let reducer_before = adapter.reducer.clone();
    let wal_len_before = adapter.wal.recovered_records().len();

    assert!(matches!(
        adapter.local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated,),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert_eq!(adapter.registry.subjects, subjects_before);
    assert_eq!(adapter.registry.manifests, manifests_before);
    assert_eq!(adapter.registry.execution_commitments, commitments_before);
    assert_eq!(adapter.active_subject, active_before);
    assert_eq!(adapter.reducer, reducer_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_len_before);
}

#[test]
fn post_decision_selected_lifecycles_cannot_reopen_the_reclaimed_owner_epoch() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());

    let decided_subject = subject(0x7c);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7c; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact durable Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    let reclaimed_snapshot = std::fs::read(adapter.serviced_candidate_store_path_for_test())
        .expect("read reclaimed owner snapshot");

    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision validated body"), 1)
        .expect("bind post-Decision validation lifecycle");
    let applied = adapter
        .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
        .expect("service selected post-Decision validation without a producer owner");
    adapter.clear_selected_producer_lifecycle();
    let apply_tag = match applied.effects() {
        [
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            },
        ] if *subject == decided_subject && certificate == &decision => *tag,
        effects => panic!("unexpected exact Decision application effects: {effects:?}"),
    };
    assert!(applied.producer_handoff().is_none());

    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision application"), 2)
        .expect("bind post-Decision application lifecycle");
    let completed = adapter
        .application_completed(apply_tag, decided_subject)
        .expect("service selected post-Decision application completion");
    adapter.clear_selected_producer_lifecycle();
    assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    assert!(completed.producer_handoff().is_none());

    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    assert!(adapter.restored_dormant_producer_continuations.is_empty());
    assert!(adapter.deferred_producer_continuations.is_empty());
    assert!(adapter.pending_producer_handoffs.is_empty());
    assert_eq!(
        std::fs::read(adapter.serviced_candidate_store_path_for_test())
            .expect("reread reclaimed owner snapshot"),
        reclaimed_snapshot,
        "post-Decision service cannot republish or mutate the reclaimed owner epoch"
    );
}

#[test]
fn exact_local_completion_after_decision_reports_body_validated_progress() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let predecision_a = unowned_body_event(&adapter, 0x79);
    adapter
        .step(predecision_a)
        .expect("service pre-Decision candidate A");
    let predecision_b = unowned_body_event(&adapter, 0x7A);
    adapter
        .step(predecision_b)
        .expect("service pre-Decision candidate B");
    assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
    assert_eq!(adapter.durable_serviced_candidates.len(), 2);
    let decided_subject = subject(0x7d);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7d; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact durable Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "durable Decision reclaims the complete candidate-service epoch, including its triggering occurrence"
    );

    let applied = adapter
        .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
        .expect("transfer trusted local validation to the Decision");
    let apply_tag = match applied.effects() {
        [
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            },
        ] if *subject == decided_subject && certificate == &decision => *tag,
        effects => panic!("unexpected exact Decision application effects: {effects:?}"),
    };
    assert!(matches!(
        adapter.status().expect("liveness snapshot").liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            round,
            transition: wire::SumeragiV2ProgressTransition::BodyValidated,
            ..
        }) if round == decision.round
    ));
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "post-Decision application progress cannot resurrect candidate tombstones"
    );
    let completed = adapter
        .application_completed(apply_tag, decided_subject)
        .expect("retire the exact Decision application lifecycle");
    assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    let expected_retransmit = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
    ));
    for attempt in 0..3 {
        let retransmit = adapter
            .retransmit_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("post-drain retransmission {attempt}: {error}"));
        assert_eq!(
            retransmit.effects(),
            std::slice::from_ref(&expected_retransmit),
            "a drained exact Decision may retransmit only its exact durable CommitQC control"
        );
    }
    assert!(adapter.deferred_completions.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "monotone applied state, not a recycled dormant ordinal or tombstone, suppresses resurrection"
    );
}

#[test]
fn busy_local_completion_during_decision_wal_reaches_apply_once() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7e);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7e; 96],
    };
    let context = adapter.wire_context.clone();
    let certificate = adapter
        .registry
        .qc_to_core(&decision, &context)
        .expect("convert exact Decision certificate");
    let decision_tag = adapter.current_tag();
    let pending_decision = adapter
        .reducer
        .step(reducer::Event::QuorumCertificateReceived {
            tag: decision_tag,
            certificate,
        })
        .expect("stage Decision WAL persistence");
    assert!(matches!(
        pending_decision.effects(),
        [reducer::Effect::Persist { .. }]
    ));

    let busy = adapter
        .local_proposal_ready(decision_tag, manifest, &durable, &validated)
        .expect("Busy boundary retains the trusted local completion");
    assert_eq!(
        busy.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(adapter.deferred_completions.len(), 1);

    let decision_effects = adapter
        .drive_effects(pending_decision.into_effects())
        .expect("fsync and acknowledge the Decision WAL record");
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody {
            subject,
            certificate: Some(certificate),
            ..
        }] if *subject == decided_subject && certificate == &decision
    ));
    let completion_effects = adapter
        .drain_deferred()
        .expect("fairly service the Busy-deferred completion");
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply {
            subject,
            certificate,
            ..
        }] if *subject == decided_subject && certificate == &decision
    ));
    assert!(adapter.deferred_completions.is_empty());
    assert!(
        adapter
            .drain_deferred()
            .expect("completion cannot be applied twice")
            .is_empty()
    );
}

#[test]
fn busy_deferred_input_blocks_terminal_readiness_until_serviced() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7f);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7f; 96],
    };
    let context = adapter.wire_context.clone();
    let certificate = adapter
        .registry
        .qc_to_core(&decision, &context)
        .expect("convert exact Decision certificate");
    let decision_tag = adapter.current_tag();
    let pending_decision = adapter
        .reducer
        .step(reducer::Event::QuorumCertificateReceived {
            tag: decision_tag,
            certificate,
        })
        .expect("stage Decision WAL persistence");
    assert!(matches!(
        pending_decision.effects(),
        [reducer::Effect::Persist { .. }]
    ));

    let busy_completion = adapter
        .local_proposal_ready(decision_tag, manifest.clone(), &durable, &validated)
        .expect("retain the trusted completion across the Busy fence");
    assert_eq!(
        busy_completion.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let terminal_vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signer: 3,
        signature: vec![0x80; 96],
    };
    let busy_vote = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(terminal_vote)),
        ))
        .expect("retain authenticated ingress across the Busy fence");
    assert_eq!(
        busy_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_completions.len(), 1);
    assert_eq!(adapter.deferred_inputs.len(), 1);

    let decision_effects = adapter
        .drive_effects(pending_decision.into_effects())
        .expect("fsync and acknowledge the Decision WAL record");
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody { subject, .. }] if *subject == decided_subject
    ));
    let completion_effects = adapter
        .drain_deferred()
        .expect("service the retained completion first");
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply { subject, .. }] if *subject == decided_subject
    ));
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(adapter.deferred_inputs.len(), 1);

    let applied = adapter
        .application_completed(decision_tag, decided_subject)
        .expect("acknowledge exact decision application");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(applied.effects().is_empty());
    assert!(adapter.reducer.ready_to_finish());
    assert!(adapter.deferred_work_is_serviceable());
    assert!(
        !adapter.ready_to_finish(),
        "adapter-owned Busy debt must block terminal height rollover"
    );

    assert!(
        adapter
            .drain_deferred()
            .expect("retire the authenticated terminal vote")
            .is_empty()
    );
    assert!(adapter.deferred_inputs.is_empty());
    assert!(adapter.ready_to_finish());
}

#[test]
fn saturated_normal_lane_retains_exact_local_proposal_completion() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());

    let proposed_subject = subject(0x81);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
        manifest: manifest.clone(),
        durable_receipt: durable.clone(),
        validated_receipt: validated.clone(),
    };
    let proposal_tag = adapter.current_tag();
    let sign = adapter
        .local_proposal_ready(proposal_tag, manifest.clone(), &durable, &validated)
        .expect("persist the local proposal before signing")
        .into_effects();
    let sign_tag = match sign.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(_),
            },
        ] => *tag,
        effects => panic!("unexpected local proposal effects: {effects:?}"),
    };

    let deferred_vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0x82),
        execution_commitment: execution_commitment(0x82),
        signer: 0,
        signature: vec![0x82; 96],
    };
    let busy = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(deferred_vote),
        ))
        .expect("defer normal ingress behind the proposal signature");
    assert_eq!(
        busy.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let filler = adapter
        .deferred_inputs
        .front()
        .expect("normal ingress owns one deferred slot")
        .clone();
    assert_eq!(filler.priority, DeferredPriority::Normal);
    let mut saturated_inputs = VecDeque::from([filler.clone()]);
    for _ in 1..MAX_DEFERRED_INPUTS {
        let admission_capability = adapter
            .deferred_admission_ordinals
            .mint(filler.admission_capability.origin)
            .expect("each saturated fixture owns a distinct adapter admission");
        let mut distinct_filler = filler.clone();
        distinct_filler.admission_ordinal = admission_capability.ordinal;
        distinct_filler.admission_capability = admission_capability;
        saturated_inputs.push_back(distinct_filler);
    }
    adapter.deferred_inputs = saturated_inputs;

    let first_retry = adapter
        .local_proposal_ready(proposal_tag, manifest.clone(), &durable, &validated)
        .expect("trusted local completion bypasses saturated normal ingress");
    assert_eq!(
        first_retry.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (1, 1),
        "the full manifest and both receipts have exactly one completion owner"
    );
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::LocalProposalReady { .. },
            priority: DeferredPriority::Completion,
            ..
        })
    ));
    let first_completion_ordinal = adapter
        .deferred_completions
        .front()
        .expect("first completion retains an exact owner")
        .admission_ordinal;
    let next_ordinal_before_duplicate = adapter.deferred_admission_ordinals.next_for_test();

    let exact_retry = adapter
        .local_proposal_ready(proposal_tag, manifest, &durable, &validated)
        .expect("an exact retry coalesces with its existing owner");
    assert_eq!(
        exact_retry.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (1, 1),
        "an exact retry cannot duplicate completion ownership"
    );
    assert_eq!(
        adapter
            .deferred_completions
            .front()
            .expect("duplicate retains the original owner")
            .admission_ordinal,
        first_completion_ordinal,
        "an exact duplicate must not mint or reset its admission ordinal"
    );
    assert_eq!(
        adapter.deferred_admission_ordinals.next_for_test(),
        next_ordinal_before_duplicate,
        "duplicate coalescing must not consume an actor ordinal"
    );

    let completed = adapter
        .signature_completed(sign_tag, vec![0x81; 96])
        .expect("signature completion drains the retained proposal retry")
        .into_effects();
    assert!(completed.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(_),
            ..
        })
    )));
    let prepare_sign_tag = completed
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.phase == wire::GlobalPhase::Prepare && vote.subject == proposed_subject => {
                Some(*tag)
            }
            _ => None,
        })
        .expect("proposal completion opens its serialized Prepare signature");
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (1, 1),
        "the retry remains owned while the causally next signature is outstanding"
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);

    let prepare_completed = adapter
        .signature_completed(prepare_sign_tag, vec![0x82; 96])
        .expect("Prepare signature releases all deferred reducer work")
        .into_effects();
    assert!(prepare_completed.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
            && vote.subject == proposed_subject
    )));
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (1, 1),
        "signature completion cannot concatenate a second reducer macro-step"
    );
    assert!(adapter.deferred_work_is_serviceable());
    assert!(
        adapter
            .drain_deferred()
            .expect("service the retained local completion in its own turn")
            .is_empty()
    );
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (0, 0),
        "one explicit deferred turn retires the sole completion owner"
    );
    assert!(adapter.deferred_inputs.len() <= MAX_DEFERRED_INPUTS);
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);
}

#[test]
fn replay_resigns_a_durable_proposal_before_prepare() {
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, _) = open_test_as_leader(&directory).expect("open leader");
        let subject = subject(10);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
            .expect("persist proposal intent");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
    }

    let (adapter, startup) = open_test_as_leader(&directory).expect("replay leader");
    assert!(adapter.ingress_ready());
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
}

#[test]
fn proposal_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let proposal_signature = vec![0xD1; 96];
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposed_subject = subject(0xA8);
        let proposal = proposal(
            &adapter.wire_context,
            adapter.wire_context.leader(0),
            proposed_subject,
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal fixture")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let sign = adapter
            .local_proposal_ready(
                adapter.current_tag(),
                proposal.manifest,
                &durable,
                &validated,
            )
            .expect("persist proposal intent before signing");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(_),
                },
            ] => *tag,
            effects => panic!("unexpected proposal sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, proposal_signature.clone())
            .expect("complete proposal signature before simulated control loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            })
        )));
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            } if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            retained,
            "a Signed callback is not a durable candidate tombstone"
        );
        // Drop both returned controls: the WAL contains ProposalIntent and
        // PrepareIntent, while neither broadcast reached transport.
    }

    let context = context();
    let leader = context.leader(0);
    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(2),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover proposal and Prepare intents");
    let proposal_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(_),
            },
        ] => *tag,
        effects => panic!("unexpected recovered proposal frontier: {effects:?}"),
    };
    let retained = recovered.serviced_candidate_count_for_test();
    let replayed = recovered
        .signature_completed(proposal_tag, proposal_signature)
        .expect("new generation accepts the replay-issued proposal callback");
    assert!(replayed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(_),
            ..
        })
    )));
    let prepare_tag = replayed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.phase == wire::GlobalPhase::Prepare => Some(*tag),
            _ => None,
        })
        .expect("recovered proposal releases its durable Prepare signature");
    assert_eq!(recovered.serviced_candidate_count_for_test(), retained);
    let prepare_signature = vec![0xD2; 96];
    let prepared = recovered
        .signature_completed(prepare_tag, prepare_signature.clone())
        .expect("complete replayed Prepare signature");
    assert!(prepared.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
    )));
    assert_eq!(
        recovered
            .signature_completed(prepare_tag, prepare_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}

#[test]
fn vote_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let vote_signature = vec![0xE1; 96];
    let prepared_subject = subject(0xA9);
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let proposer = adapter.status().expect("status").leader;
        let fetch = adapter
            .receive_verified(proposal(&adapter.wire_context, proposer, prepared_subject))
            .expect("accept remote proposal");
        let (tag, manifest) = match fetch.effects() {
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
        adapter
            .body_available(tag, manifest)
            .expect("make remote body available");
        let receipt = durable_body_receipt(&adapter, round, prepared_subject);
        adapter
            .body_stored(tag, round, prepared_subject, &receipt)
            .expect("acknowledge durable body");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let sign = adapter
            .validation_succeeded(tag, round, prepared_subject, &validated)
            .expect("persist Prepare intent");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                },
            ] if vote.phase == wire::GlobalPhase::Prepare => *tag,
            effects => panic!("unexpected Prepare sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, vote_signature.clone())
            .expect("complete Prepare signature before simulated transport loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote),
                ..
            }) if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
    }

    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover durable Prepare intent");
    let sign_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] if vote.phase == wire::GlobalPhase::Prepare && vote.subject == prepared_subject => *tag,
        effects => panic!("unexpected recovered Prepare frontier: {effects:?}"),
    };
    let validation_authority = recovered
        .recovered_validation_authority(&startup)
        .expect("WAL replay mints the exact bounded validation frontier");
    assert_eq!(validation_authority.len(), 1);
    assert!(validation_authority.authorizes(
        wire::ConsensusRound {
            context_id: context().id(),
            height: context().height,
            view: 0,
        },
        prepared_subject,
    ));
    let signed = recovered
        .signature_completed(sign_tag, vote_signature.clone())
        .expect("new generation accepts the replay-issued Prepare callback");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
            && vote.subject == prepared_subject
    )));
    assert_eq!(
        recovered
            .signature_completed(sign_tag, vote_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}

#[test]
fn recovered_validation_authority_uses_locked_proposal_round() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let proposal_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let certificate_round = wire::ConsensusRound {
        view: 2,
        ..proposal_round
    };
    let timeout = |view, marker| wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            view,
            ..proposal_round
        },
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        }],
    };
    let timeout_zero = adapter
        .registry
        .tc_to_core(&timeout(0, 0xA8), &adapter.wire_context)
        .expect("register the view-zero timeout certificate");
    let timeout_one = adapter
        .registry
        .tc_to_core(&timeout(1, 0xA9), &adapter.wire_context)
        .expect("register the view-one timeout certificate");
    let locked_subject = subject(0xAA);
    let wire_prepare = wire::QuorumCertificate {
        round: certificate_round,
        proposal_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xAA),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAA; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the carried durable PrepareQC");
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(3),
        reducer::WalRecord::LockAndCommit {
            vote: reducer::Vote::new_with_proposal_round(
                core_context.id(),
                prepare.round(),
                prepare.proposal_round(),
                reducer::Phase::Commit,
                prepare.subject(),
                local_validator,
            ),
            prepare,
        },
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(2),
        [
            reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::InstallTimeout(timeout_zero),
            ),
            reducer::WalEntry::new(
                reducer::PersistenceId::new(2),
                reducer::WalRecord::InstallTimeout(timeout_one),
            ),
            lock_entry,
        ],
    )
    .expect("recover the carried durable lock");

    let authority = adapter
        .recovered_validation_authority(&[])
        .expect("mint the recovered lock frontier");
    assert_eq!(authority.len(), 1);
    assert!(authority.authorizes(proposal_round, locked_subject));
    assert!(!authority.authorizes(certificate_round, locked_subject));
}

#[test]
fn timeout_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let timeout_signature = vec![0xF1; 96];
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let sign = adapter
            .timeout_elapsed(adapter.current_tag())
            .expect("persist Timeout intent");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                },
            ] => *tag,
            effects => panic!("unexpected Timeout sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, timeout_signature.clone())
            .expect("complete Timeout signature before simulated transport loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        )));
        assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
    }

    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover durable Timeout intent");
    let sign_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected recovered Timeout frontier: {effects:?}"),
    };
    let signed = recovered
        .signature_completed(sign_tag, timeout_signature.clone())
        .expect("new generation accepts the replay-issued Timeout callback");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
            ..
        })
    )));
    assert_eq!(
        recovered
            .signature_completed(sign_tag, timeout_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}

#[test]
fn locally_signed_timeout_quorum_leads_with_enter_view_and_subsumes_vote() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };

    for signer in [1, 2] {
        let retained = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                    round,
                    highest_prepare_qc: None,
                    signer,
                    signature: vec![signer as u8; 96],
                }),
            ))
            .expect("retain a remote TimeoutVote before the local timeout");
        assert_eq!(retained.disposition(), reducer::StepDisposition::Applied);
        assert!(retained.effects().is_empty());
    }

    let timeout = adapter
        .timeout_elapsed(tag)
        .expect("persist the local timeout intent");
    let sign_tag = match timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected local TimeoutVote effects: {effects:?}"),
    };
    let entered = adapter
        .signature_completed(sign_tag, vec![0xF2; 96])
        .expect("the local signature completes the retained timeout quorum")
        .into_effects();

    assert!(
        matches!(
            entered.as_slice(),
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
            ] if entered_tag.view() == round.view + 1
                && certificate.round == round
                && certificate.groups.iter().any(|group| group.signers.contains(&0))
        ),
        "the advancing WAL continuation must lead and its durable TC must subsume the old-view vote: {entered:?}"
    );
}

#[test]
fn locally_signed_timeout_without_quorum_broadcasts_only_the_vote() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };

    let timeout = adapter
        .timeout_elapsed(tag)
        .expect("persist the local timeout intent");
    let sign_tag = match timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected local TimeoutVote effects: {effects:?}"),
    };
    let signed = adapter
        .signature_completed(sign_tag, vec![0xF3; 96])
        .expect("complete the non-quorum local TimeoutVote")
        .into_effects();

    assert!(
        matches!(
            signed.as_slice(),
            [AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(vote),
                ..
            })] if vote.round == round && vote.signer == 0
        ),
        "a non-quorum local timeout must emit only its vote broadcast: {signed:?}"
    );
}

#[test]
fn deferred_adapter_replay_with_startup_effects_publishes_no_status() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposal = proposal(
            &adapter.wire_context,
            adapter.wire_context.leader(0),
            subject(10),
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
            .expect("persist proposal intent");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
    }

    crate::sumeragi::status::clear_v2_status();
    let context = context();
    let leader = context.leader(0);
    let (mut adapter, startup) = SumeragiV2Adapter::open_deferred_status(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(1),
        [0x22; 32],
        fingerprints(),
        deferred_admission_ordinals(),
    )
    .expect("replay leader without publishing status");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "nonempty startup work must not publish the prepared successor"
    );
    let prepared = adapter
        .successor_activation_status()
        .expect("prepare reducer-owned activation snapshot");
    assert_eq!(prepared.height, 1);
    assert!(matches!(
        prepared.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            ..
        })
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "snapshot construction must remain separate from publication"
    );
    crate::sumeragi::status::clear_v2_status();
}

include!("v2_adapter_01_replay_and_registry.rs");
#[test]
#[allow(clippy::too_many_lines)]
fn capacity_bypass_records_follow_current_lock_and_timeout_view() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let install_lock = |adapter: &mut SumeragiV2Adapter, marker: u8| {
        let locked_subject = subject(marker);
        let locked_execution_commitment = execution_commitment(marker);
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
            aggregate_signature: vec![marker; 96],
        };
        let core_context = adapter.reducer.context().clone();
        let prepare = adapter
            .registry
            .qc_to_core(&wire_prepare, &adapter.wire_context)
            .expect("register lock certificate");
        let local_validator = adapter
            .registry
            .validator_id(0)
            .expect("local fixture validator");
        let vote = reducer::Vote::new(
            core_context.id(),
            prepare.round(),
            reducer::Phase::Commit,
            prepare.subject(),
            local_validator,
        );
        adapter.reducer = reducer::Reducer::recover(
            core_context,
            Some(local_validator),
            reducer::Generation::new(u64::from(marker)),
            [reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::LockAndCommit { prepare, vote },
            )],
        )
        .expect("recover durable lock fixture");
        (wire_round, locked_subject, locked_execution_commitment)
    };
    let admit_locked_roster =
        |adapter: &mut SumeragiV2Adapter,
         wire_round: wire::ConsensusRound,
         locked_subject: wire::BlockSubject,
         locked_execution_commitment: wire::ExecutionCommitment| {
            let roster_len = adapter.wire_context.roster.len();
            for signer in 0..roster_len {
                let signer = u32::try_from(signer).expect("fixture signer index fits u32");
                let payload = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                    round: wire_round,
                    proposal_round: wire_round,
                    phase: wire::GlobalPhase::Commit,
                    subject: locked_subject,
                    execution_commitment: locked_execution_commitment,
                    signer,
                    signature: vec![u8::try_from(signer).expect("small fixture signer")],
                });
                let (outcome, admission) = adapter
                    .admit_authenticated_payload(&payload)
                    .expect("exact lock bypasses ordinary capacity");
                assert!(outcome.is_none());
                let admission = admission.expect("lock vote owns a capacity-bypass record");
                assert!(
                    adapter
                        .ingress_equivocations
                        .get(&admission.key)
                        .expect("inserted lock admission")
                        .capacity_bypass
                );
                adapter.record_ingress_delivery(admission);
            }
        };
    let admit_timeout_roster = |adapter: &mut SumeragiV2Adapter,
                                wire_round: wire::ConsensusRound| {
        let roster_len = adapter.wire_context.roster.len();
        for signer in 0..roster_len {
            let signer = u32::try_from(signer).expect("fixture signer index fits u32");
            let payload = wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                round: wire_round,
                highest_prepare_qc: None,
                signer,
                signature: vec![0xE0 ^ u8::try_from(signer).expect("small fixture signer")],
            });
            let (outcome, admission) = adapter
                .admit_authenticated_payload(&payload)
                .expect("retained TimeoutVote bypasses ordinary capacity");
            assert!(outcome.is_none());
            let admission = admission.expect("TimeoutVote owns a capacity-bypass record");
            assert!(
                adapter
                    .ingress_equivocations
                    .get(&admission.key)
                    .expect("inserted TimeoutVote admission")
                    .capacity_bypass
            );
            adapter.record_ingress_delivery(admission);
        }
    };

    let first_lock = install_lock(&mut adapter, 0xDB);
    let ordinary_round = first_lock.0;
    let ingress_context = adapter.wire_context.clone();
    for index in 0..MAX_INGRESS_SEMANTIC_KEYS {
        let proposer = u32::try_from(index).expect("semantic table bound fits u32");
        adapter.ingress_equivocations.insert(
            IngressSemanticKey::Proposal {
                round: ordinary_round,
                proposer,
            },
            IngressEquivocationRecord {
                fingerprint: IngressFingerprint::Proposal(Hash::new(index.to_le_bytes())),
                artifact: synthetic_ingress_proposal(
                    &ingress_context,
                    ordinary_round,
                    proposer,
                    index,
                ),
                equivocation_reported: false,
                capacity_bypass: false,
                admitted_at: Instant::now(),
            },
        );
    }
    admit_locked_roster(&mut adapter, first_lock.0, first_lock.1, first_lock.2);
    let roster_len = adapter.wire_context.roster.len();
    admit_timeout_roster(&mut adapter, first_lock.0);
    let adjacent_timeout_round = wire::ConsensusRound {
        view: first_lock.0.view + reducer::FUTURE_TIMEOUT_VOTE_LOOKAHEAD,
        ..first_lock.0
    };
    admit_timeout_roster(&mut adapter, adjacent_timeout_round);
    assert_eq!(
        adapter.ingress_equivocations.len(),
        semantic_ingress_capacity(roster_len),
        "ordinary, exact-lock, and bounded TimeoutVote owners realize the complete live semantic bound"
    );
    let ingress = adapter
        .adapter_queue_statuses()
        .into_iter()
        .find(|queue| queue.queue == wire::SumeragiV2QueueKind::Ingress)
        .expect("ingress queue status");
    assert_eq!(
        usize::try_from(ingress.depth).unwrap(),
        semantic_ingress_capacity(roster_len)
    );
    assert_eq!(
        usize::try_from(ingress.capacity).unwrap(),
        semantic_ingress_capacity(roster_len)
    );
    assert_eq!(
        adapter
            .ingress_equivocations
            .values()
            .filter(|record| record.capacity_bypass)
            .count(),
        roster_len * 3
    );
    let same_view_equivocations = adapter.ingress_equivocations.clone();
    let same_view_deliveries = adapter.ingress_deliveries.clone();
    adapter.prune_ingress_records();
    assert_eq!(adapter.ingress_equivocations, same_view_equivocations);
    assert_eq!(adapter.ingress_deliveries, same_view_deliveries);

    // The following lock-replacement half isolates durable-lock retention;
    // view-advance retirement for these TimeoutVote owners is exercised by
    // `full_normal_deferred_lane_cannot_drop_absolute_timeout`.
    adapter
        .ingress_equivocations
        .retain(|key, _| !matches!(key, IngressSemanticKey::TimeoutVote { .. }));
    adapter
        .ingress_deliveries
        .retain(|key, _| !matches!(key, IngressSemanticKey::TimeoutVote { .. }));
    assert_eq!(
        adapter.ingress_equivocations.len(),
        MAX_INGRESS_SEMANTIC_KEYS + roster_len
    );

    let second_lock = install_lock(&mut adapter, 0xDC);
    adapter.prune_ingress_records();
    assert_eq!(
        adapter.ingress_equivocations.len(),
        MAX_INGRESS_SEMANTIC_KEYS
    );
    assert!(
        adapter
            .ingress_equivocations
            .values()
            .all(|record| !record.capacity_bypass)
    );
    assert!(adapter.ingress_deliveries.is_empty());

    admit_locked_roster(&mut adapter, second_lock.0, second_lock.1, second_lock.2);
    assert_eq!(
        adapter.ingress_equivocations.len(),
        MAX_INGRESS_SEMANTIC_KEYS + roster_len,
        "capacity-bypass records from successive locks cannot accumulate"
    );
    assert_eq!(
        adapter
            .ingress_equivocations
            .values()
            .filter(|record| record.capacity_bypass)
            .count(),
        roster_len
    );
    let ingress = adapter
        .adapter_queue_statuses()
        .into_iter()
        .find(|queue| queue.queue == wire::SumeragiV2QueueKind::Ingress)
        .expect("ingress queue status after lock advance");
    assert!(ingress.depth <= ingress.capacity);
}

include!("v2_adapter_02_view_and_lock_progress.rs");
#[test]
fn prelock_current_commit_is_readmitted_with_priority_neutral_service_identity() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let locked_subject = subject(0xBE);
    let locked_execution_commitment = execution_commitment(0xBE);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let remote_commit =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer: 1,
            signature: vec![0xBE],
        }));
    let authenticated = AuthenticatedConsensusMessage::for_test(remote_commit.clone());
    assert!(!adapter.authenticated_ingress_is_progress(&authenticated));
    let generation = adapter.current_tag().generation();
    let serviced_before = adapter.serviced_candidate_count_for_test();

    let premature = adapter
        .receive_authenticated(authenticated)
        .expect("deliver the current Commit before its Prepare lock is durable");
    assert_eq!(
        premature.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
    );
    let key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: 1,
    };
    let delivered = adapter
        .ingress_deliveries
        .get(&key)
        .expect("the pre-lock reducer delivery is recorded");
    assert_eq!(delivered.generation, generation);
    assert!(!delivered.locked_commit_progress);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        serviced_before,
        "authenticated policy discards cannot allocate candidate markers"
    );
    assert!(adapter.durable_serviced_candidates.is_empty());
    let wire::ConsensusMessageV2Payload::Vote(normal_vote) = &remote_commit.payload else {
        unreachable!("fixture is a Commit vote")
    };
    let normal_vote = adapter
        .registry
        .vote_to_core(normal_vote, &adapter.wire_context)
        .expect("project the marker-free pre-lock occurrence");
    let normal_candidate = adapter
        .serviced_candidate(
            &reducer::Event::VoteReceived {
                tag: adapter.current_tag(),
                vote: normal_vote,
            },
            DeferredPriority::Normal,
            None,
            None,
        )
        .expect("pre-lock occurrence still has an exact rank identity")
        .0;
    adapter.ingress_deliveries.remove(&key);
    adapter.ingress_equivocations.remove(&key);
    let marker_count = adapter.serviced_candidate_count_for_test();
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                remote_commit.clone(),
            ))
            .expect("marker-free policy discard remains reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
    );
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        marker_count,
        "same-class policy replay cannot consume a tombstone"
    );

    let prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAE; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare,
            )),
        ))
        .expect("observe the current PrepareQC");
    assert!(observed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *round == wire_round
            && *subject == locked_subject
            && certificate.phase == wire::GlobalPhase::Prepare
    )));

    let locked_payload = [0xBE, 2];
    let manifest = encode_payload(
        &adapter.wire_context,
        wire_round,
        locked_subject,
        &locked_payload,
    )
    .expect("encode the certified body payload")
    .manifest()
    .clone();
    let (durable, _) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let validated = ValidatedBodyReceipt::for_test_with_commitment(
        durable.clone(),
        locked_execution_commitment,
    );
    assert!(matches!(
        adapter
            .body_available(adapter.current_tag(), manifest.clone())
            .expect("make the certified body available")
            .effects(),
        [AdapterEffect::StoreBody { .. }]
    ));
    assert!(matches!(
        adapter
            .body_stored(adapter.current_tag(), wire_round, locked_subject, &durable,)
            .expect("acknowledge durable body storage")
            .effects(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let locked = adapter
        .validation_succeeded(
            adapter.current_tag(),
            wire_round,
            locked_subject,
            &validated,
        )
        .expect("persist the exact current LockAndCommit record");
    let commit_sign_tag = locked
        .effects()
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.round == wire_round
                && vote.phase == wire::GlobalPhase::Commit
                && vote.subject == locked_subject =>
            {
                Some(*tag)
            }
            _ => None,
        })
        .expect("durable lock acknowledgement authorizes the local Commit signature");
    assert_eq!(adapter.current_tag().generation(), generation);
    assert!(
        adapter.authenticated_ingress_is_progress(&AuthenticatedConsensusMessage::for_test(
            remote_commit.clone(),
        ))
    );

    let readmitted = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            remote_commit.clone(),
        ))
        .expect("re-admit the exact vote in the new lock consumer epoch");
    assert_eq!(
        readmitted.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    assert!(adapter.deferred_progress_inputs[0].protected_progress);
    let progress_input = &adapter.deferred_progress_inputs[0];
    let progress_candidate = adapter
        .serviced_candidate(
            &progress_input.event,
            progress_input.priority,
            progress_input.completion_evidence.as_ref(),
            progress_input.authenticated_wire_identity.as_deref(),
        )
        .expect("exact-lock Commit has a route-neutral service identity")
        .0;
    assert_eq!(
        progress_candidate.class(),
        ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS
    );
    assert_eq!(
        normal_candidate, progress_candidate,
        "the same authenticated Commit occurrence must coalesce across Normal/Progress routing"
    );
    let delivered = adapter
        .ingress_deliveries
        .get(&key)
        .expect("the exact-lock consumer owns the re-admitted vote");
    assert_eq!(delivered.generation, generation);
    assert!(delivered.locked_commit_progress);
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                remote_commit.clone(),
            ))
            .expect("coalesce behind exact deferred ownership")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );

    adapter
        .signature_completed(commit_sign_tag, vec![0xBF])
        .expect("self-admit the local Commit");
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    adapter
        .drain_deferred()
        .expect("give the deferred remote owner its serialized runtime turn");
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert!(
        adapter.serviced_candidates.contains_key(&normal_candidate),
        "the priority-neutral applied Commit remains coalesced for this process generation"
    );
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(
        adapter
            .status()
            .expect("post-lock status")
            .liveness
            .commit_quorums
            .iter()
            .any(|quorum| quorum.round == wire_round
                && quorum.subject == locked_subject
                && quorum.execution_commitment == locked_execution_commitment
                && quorum.signer_count == 2)
    );
    adapter.ingress_deliveries.remove(&key);
    adapter.ingress_equivocations.remove(&key);
    let marker_count = adapter.serviced_candidate_count_for_test();
    assert_eq!(
        adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(remote_commit))
            .expect("monotone Commit-vote state suppresses replay after ingress reset")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.serviced_candidate_count_for_test(), marker_count);
}

include!("v2_adapter_03_tc_and_terminal_ingress.rs");
#[test]
fn deferred_zero_ordinal_is_exact_single_use_and_never_reminted() {
    let source = DeferredAdmissionOrdinalSource::new(0);
    let tag = reducer::EventTag::new(1, 0, reducer::Generation::new(1));
    let first =
        DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Completion);
    let second =
        DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Completion);

    assert_eq!(first.admission_ordinal, 0);
    assert_eq!(second.admission_ordinal, 1);
    assert!(first.validate_exact());
    assert!(first.belongs_to(&source));
    assert!(first.claim_adapter_service_for_test());
    assert!(!first.claim_adapter_service_for_test());
    assert!(first.claim_runtime_handoff_once());
    assert!(!first.claim_runtime_handoff_once());
    assert!(first.service_handoff_is_complete());
    assert!(second.claim_adapter_service_for_test());
    assert!(second.claim_runtime_handoff_once());
}

#[test]
fn deferred_projection_distinguishes_authenticated_proposal_origins() {
    let context_id = reducer::ContextId::repeat(0xA1);
    let finality_round = reducer::Round::new(7, 4);
    let origin_a = reducer::Round::new(7, 1);
    let origin_b = reducer::Round::new(7, 2);
    let subject = reducer::Subject::repeat(0xA2);
    let signer = reducer::ValidatorId::repeat(0xA3);
    let tag = reducer::EventTag::new(7, 4, reducer::Generation::new(1));
    let signature = reducer::OpaqueSignature::new(vec![0xA4]);
    let project = |event: reducer::Event| {
        let mut projection = Vec::new();
        append_deferred_projection_event(&mut projection, &event);
        projection
    };

    let signed_vote = |proposal_round| {
        reducer::SignedVote::new(
            reducer::Vote::new_with_proposal_round(
                context_id,
                finality_round,
                proposal_round,
                reducer::Phase::Commit,
                subject,
                signer,
            ),
            signature.clone(),
        )
    };
    assert_ne!(
        project(reducer::Event::VoteReceived {
            tag,
            vote: signed_vote(origin_a),
        }),
        project(reducer::Event::VoteReceived {
            tag,
            vote: signed_vote(origin_b),
        })
    );

    let certificate = |proposal_round| {
        reducer::QuorumCertificate::new(
            reducer::CertificateRef::new_with_proposal_round(
                context_id,
                finality_round,
                proposal_round,
                reducer::Phase::Commit,
                subject,
            ),
            vec![reducer::SignatureShare::new(signer, signature.clone())],
        )
    };
    assert_ne!(
        project(reducer::Event::QuorumCertificateReceived {
            tag,
            certificate: certificate(origin_a),
        }),
        project(reducer::Event::QuorumCertificateReceived {
            tag,
            certificate: certificate(origin_b),
        })
    );

    let proposal = |proposal_round| {
        reducer::SignedProposal::new(
            reducer::Proposal::new(
                context_id,
                reducer::Round::new(7, 0),
                signer,
                reducer::PayloadManifest::new(
                    subject,
                    reducer::Digest::repeat(0xA5),
                    reducer::Digest::repeat(0xA6),
                    1,
                    1,
                ),
                reducer::ProposalJustification::ParentCommit(Some(
                    reducer::CertificateRef::new_with_proposal_round(
                        context_id,
                        finality_round,
                        proposal_round,
                        reducer::Phase::Commit,
                        subject,
                    ),
                )),
            ),
            signature.clone(),
        )
    };
    assert_ne!(
        project(reducer::Event::ProposalReceived {
            tag,
            proposal: proposal(origin_a),
        }),
        project(reducer::Event::ProposalReceived {
            tag,
            proposal: proposal(origin_b),
        })
    );
}

#[test]
fn deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    adapter.deferred_admission_ordinals = DeferredAdmissionOrdinalSource::new(u128::MAX - 1);
    let context = adapter.wire_context.clone();
    let proposer = context.leader(0);
    let first = proposal(&context, proposer, subject(0xD1));
    let second = proposal(&context, proposer, subject(0xD2));
    let wire::ConsensusMessageV2Payload::Proposal(first) = first.payload else {
        unreachable!("proposal fixture")
    };
    let wire::ConsensusMessageV2Payload::Proposal(second) = second.payload else {
        unreachable!("proposal fixture")
    };
    let tag = adapter.current_tag();

    adapter
        .defer_body_available_for_test(tag, &first.manifest)
        .expect("last safely advanceable ordinal is admitted");
    assert_eq!(
        adapter
            .deferred_completions
            .front()
            .expect("first owner remains queued")
            .admission_ordinal,
        u128::MAX - 1
    );
    assert!(matches!(
        adapter.defer_body_available_for_test(tag, &second.manifest),
        Err(AdapterError::DeferredAdmissionOrdinalExhausted)
    ));
    assert!(adapter.fail_closed);
    assert_eq!(adapter.deferred_completions.len(), 1);
    assert_eq!(
        adapter.deferred_admission_ordinals.next_for_test(),
        u128::MAX,
        "exhaustion cannot wrap the actor source to a stale ordinal"
    );
}

#[test]
fn deferred_actor_source_never_aliases_across_adapter_instances() {
    let first_directory = TempDir::new().expect("first temporary directory");
    let second_directory = TempDir::new().expect("second temporary directory");
    let source = DeferredAdmissionOrdinalSource::new(0);
    let open = |directory: &TempDir, source: DeferredAdmissionOrdinalSource| {
        SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("shared-ordinal-safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(1),
            [0xD3; 32],
            fingerprints(),
            Box::new(TestAggregator),
            source,
        )
    };
    let (mut first, first_startup) =
        open(&first_directory, source.clone()).expect("open first adapter instance");
    let (mut second, second_startup) =
        open(&second_directory, source.clone()).expect("open second adapter instance");
    assert!(first_startup.is_empty());
    assert!(second_startup.is_empty());
    let first_context = first.wire_context.clone();
    let second_context = second.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(first_proposal) =
        proposal(&first_context, first_context.leader(0), subject(0xD4)).payload
    else {
        unreachable!("proposal fixture")
    };
    let wire::ConsensusMessageV2Payload::Proposal(second_proposal) =
        proposal(&second_context, second_context.leader(0), subject(0xD5)).payload
    else {
        unreachable!("proposal fixture")
    };
    let first_tag = first.current_tag();
    let second_tag = second.current_tag();
    first
        .defer_body_available_for_test(first_tag, &first_proposal.manifest)
        .expect("first adapter instance admits owner zero");
    second
        .defer_body_available_for_test(second_tag, &second_proposal.manifest)
        .expect("second adapter instance advances the same actor source");

    let first_owner = first
        .pop_deferred_next()
        .expect("first adapter instance rank remains valid")
        .expect("first adapter instance returns exact owner")
        .evidence;
    let second_owner = second
        .pop_deferred_next()
        .expect("second adapter instance rank remains valid")
        .expect("second adapter instance returns exact owner")
        .evidence;
    assert_eq!(first_owner.admission_ordinal, 0);
    assert_eq!(second_owner.admission_ordinal, 1);
    assert_ne!(
        first_owner.admission_ordinal,
        second_owner.admission_ordinal
    );
    assert!(first_owner.belongs_to(&source));
    assert!(second_owner.belongs_to(&source));
    assert!(first_owner.validate_exact());
    assert!(second_owner.validate_exact());
}

#[test]
fn deferred_occurrence_capability_binds_direct_authenticated_provenance() {
    let directory = TempDir::new().expect("temporary occurrence-capability directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) =
        proposal(&context, context.leader(0), subject(0xD9)).payload
    else {
        unreachable!("proposal fixture")
    };
    let tag = adapter.current_tag();
    adapter
        .defer_authenticated_proposal_for_test(tag, &proposal)
        .expect("stage one authenticated Busy occurrence");
    let ordinals = adapter
        .all_deferred_admission_ordinals()
        .into_iter()
        .collect::<Vec<_>>();
    let [ordinal] = ordinals.as_slice() else {
        panic!("one exact Busy occurrence is retained")
    };
    let evidence = adapter
        .deferred_occurrence_ownership(*ordinal)
        .expect("snapshot the unclaimed adapter capability");
    assert!(evidence.validate_exact());
    assert_eq!(evidence.admission_ordinal(), *ordinal);
    assert!(evidence.is_authenticated_ingress());

    let mut reclassified = evidence;
    reclassified.authenticated_ingress = false;
    reclassified.projection_hash = deferred_occurrence_ownership_projection_hash(&reclassified);
    assert!(
        !reclassified.validate_exact(),
        "rehashing cannot detach provenance from the private admission capability"
    );
}

#[test]
fn deferred_service_evidence_rejects_every_owner_and_rank_mutation() {
    let source = DeferredAdmissionOrdinalSource::new(0);
    let foreign = DeferredAdmissionOrdinalSource::new(0);
    let tag = reducer::EventTag::new(7, 2, reducer::Generation::new(3));
    let evidence =
        DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Normal);
    assert!(evidence.validate_exact());
    assert!(evidence.belongs_to(&source));
    assert!(!evidence.belongs_to(&foreign));

    let rejected = |mutated: DeferredServiceEvidence| {
        assert!(!mutated.validate_exact());
    };

    let mut mutated = evidence.clone();
    mutated.admission_ordinal = 1;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.priority = DeferredPriority::Progress;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.event_kind = DeferredEventKind::RetransmitElapsed;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.original_tag = reducer::EventTag::new(7, 3, reducer::Generation::new(3));
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.original_event = reducer::Event::RetransmitElapsed { tag };
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.protected_progress = true;
    mutated.projection_hash = deferred_service_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.eligible_skips_after = 1;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.service_cursor_after = DeferredPriority::Normal;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.service_cursor_before = DeferredPriority::Completion;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.queue_lengths_after.completion = 1;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.total_len_before = 2;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.retag = DeferredRetagRelation::AuthenticatedIngress { from: tag, to: tag };
    rejected(mutated);

    let mut mutated = evidence;
    mutated.projection_hash = Hash::new(b"wrong deferred projection");
    rejected(mutated);
}

#[test]
fn deferred_authenticated_retry_retains_exact_original_and_effective_tags() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) =
        proposal(&context, context.leader(0), subject(0xD6)).payload
    else {
        unreachable!("proposal fixture")
    };
    let authenticated_wire_identity = Arc::<[u8]>::from(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal.clone()))
            .encode(),
    );
    let proposal = adapter
        .registry
        .proposal_to_core(&proposal, &context)
        .expect("convert authenticated proposal");
    let effective_tag = adapter.current_tag();
    let original_tag = reducer::EventTag::new(
        effective_tag.height(),
        effective_tag.view().saturating_add(1),
        effective_tag.generation(),
    );
    let admission_capability = adapter
        .mint_deferred_admission_ordinal(true)
        .expect("mint exact deferred owner");
    adapter.deferred_inputs.push_back(DeferredInput {
        admission_ordinal: admission_capability.ordinal,
        admission_capability,
        event: reducer::Event::ProposalReceived {
            tag: original_tag,
            proposal,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Normal,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(authenticated_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    adapter.next_deferred_priority = DeferredPriority::Normal;

    let selection = adapter
        .pop_deferred_next()
        .expect("authenticated retry rank remains valid")
        .expect("select exact authenticated retry");
    assert!(selection.evidence.validate_exact());
    assert_eq!(selection.evidence.original_tag, original_tag);
    assert_eq!(selection.evidence.effective_tag, effective_tag);
    assert_eq!(
        selection.evidence.retag,
        DeferredRetagRelation::AuthenticatedIngress {
            from: original_tag,
            to: effective_tag,
        }
    );
    assert!(
        selection
            .evidence
            .matches_effective_event(&selection.input.event)
    );
    assert!(adapter.deferred_authenticated_event_matches_wire(&selection.evidence));
}

#[test]
fn authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let wire::ConsensusMessageV2Payload::Proposal(first) =
        proposal(&context, context.leader(0), subject(0xD7)).payload
    else {
        unreachable!("proposal fixture")
    };
    let wire::ConsensusMessageV2Payload::Proposal(other) =
        proposal(&context, context.leader(0), subject(0xD8)).payload
    else {
        unreachable!("proposal fixture")
    };
    let first = adapter
        .registry
        .proposal_to_core(&first, &context)
        .expect("convert retained authenticated proposal");
    let event = reducer::Event::ProposalReceived {
        tag: adapter.current_tag(),
        proposal: first,
    };

    assert!(matches!(
        adapter.enqueue_deferred(
            event.clone(),
            true,
            DeferredPriority::Normal,
            None,
            None,
            None,
        ),
        Err(AdapterError::RuntimeIngressOwnershipViolation)
    ));
    assert!(adapter.deferred_inputs.is_empty());

    let admission_capability = adapter
        .mint_deferred_admission_ordinal(true)
        .expect("mint exact deferred owner");
    adapter.deferred_inputs.push_back(DeferredInput {
        admission_ordinal: admission_capability.ordinal,
        admission_capability,
        event,
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Normal,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::Proposal(other),
        )),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    adapter.next_deferred_priority = DeferredPriority::Normal;
    let durable_before = adapter.reducer.durable_state().clone();

    assert!(matches!(
        adapter.drain_deferred_with_evidence(),
        Err(AdapterError::DeferredServiceOwnershipViolation)
    ));
    assert_eq!(adapter.reducer.durable_state(), &durable_before);
    assert!(adapter.fail_closed);
}

#[test]
fn deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let current = adapter.current_tag();
    let stale = reducer::EventTag::new(
        current.height().saturating_add(1),
        current.view(),
        current.generation(),
    );
    let capability = adapter
        .mint_deferred_admission_ordinal(false)
        .expect("mint exact adapter capability");
    let input = |tag| DeferredInput {
        admission_ordinal: capability.ordinal,
        admission_capability: capability.clone(),
        event: reducer::Event::TimeoutElapsed { tag },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority: DeferredPriority::Completion,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter.deferred_completions.push_back(input(stale));

    let (_, first) = adapter
        .drain_deferred_with_evidence()
        .expect("first exact capability crosses the adapter")
        .expect("first deferred owner is serviceable");
    assert!(first.adapter_service_is_claimed());
    assert!(!first.service_handoff_is_complete());
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::WrongHeight)
            .copied(),
        Some(1)
    );
    adapter
        .deferred_completions
        .push_back(input(reducer::EventTag::new(
            stale.height().saturating_add(1),
            stale.view(),
            stale.generation(),
        )));
    assert!(matches!(
        adapter.drain_deferred_with_evidence(),
        Err(AdapterError::DeferredServiceOwnershipViolation)
    ));
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::WrongHeight)
            .copied(),
        Some(1),
        "the replay is rejected before a second reducer transition"
    );

    let foreign_directory = TempDir::new().expect("foreign temporary directory");
    let (mut foreign_adapter, foreign_startup) =
        open_test(&foreign_directory).expect("open foreign adapter");
    assert!(foreign_startup.is_empty());
    let foreign_source = DeferredAdmissionOrdinalSource::new(0);
    let foreign_capability = foreign_source
        .mint(DeferredAdmissionOrigin::LocalOrCausal)
        .expect("mint foreign capability");
    let foreign_tag = reducer::EventTag::new(
        foreign_adapter.current_tag().height().saturating_add(1),
        foreign_adapter.current_tag().view(),
        foreign_adapter.current_tag().generation(),
    );
    foreign_adapter
        .deferred_completions
        .push_back(DeferredInput {
            admission_ordinal: foreign_capability.ordinal,
            admission_capability: foreign_capability,
            event: reducer::Event::TimeoutElapsed { tag: foreign_tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
    assert!(matches!(
        foreign_adapter.drain_deferred_with_evidence(),
        Err(AdapterError::DeferredServiceOwnershipViolation)
    ));
    assert!(foreign_adapter.ignore_counts.is_empty());
}
