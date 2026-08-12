#[test]
fn deferred_service_debt_counts_only_oldest_skipped_classes() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut next_ordinal = 1_u128;
    let mut input = |priority: DeferredPriority| {
        let admission_ordinal = next_ordinal;
        next_ordinal = next_ordinal
            .checked_add(1)
            .expect("small deferred fixture ordinal remains representable");
        DeferredInput {
            admission_ordinal,
            admission_capability: DeferredAdmissionCapability::for_test(admission_ordinal),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        }
    };
    adapter
        .deferred_completions
        .push_back(input(DeferredPriority::Completion));
    adapter
        .deferred_completions
        .push_back(input(DeferredPriority::Completion));
    adapter
        .deferred_progress_inputs
        .push_back(input(DeferredPriority::Progress));
    adapter
        .deferred_progress_inputs
        .push_back(input(DeferredPriority::Progress));
    adapter
        .deferred_inputs
        .push_back(input(DeferredPriority::Normal));
    adapter
        .deferred_inputs
        .push_back(input(DeferredPriority::Normal));
    adapter.next_deferred_priority = DeferredPriority::Completion;

    let selected = adapter
        .pop_deferred_next()
        .expect("deferred service debt remains representable")
        .expect("completion receives its turn");
    assert_eq!(selected.evidence.priority, DeferredPriority::Completion);
    assert!(selected.evidence.validate_exact());
    assert_eq!(adapter.deferred_completions[0].eligible_skips, 0);
    assert_eq!(adapter.deferred_progress_inputs[0].eligible_skips, 1);
    assert_eq!(adapter.deferred_progress_inputs[1].eligible_skips, 0);
    assert_eq!(adapter.deferred_inputs[0].eligible_skips, 1);
    assert_eq!(adapter.deferred_inputs[1].eligible_skips, 0);
}

#[test]
fn deferred_selector_services_only_the_runtime_lifecycle_minimum_set() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let input = |ordinal, priority| DeferredInput {
        admission_ordinal: ordinal,
        admission_capability: DeferredAdmissionCapability::for_test(ordinal),
        event: reducer::Event::TimeoutElapsed { tag },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter
        .deferred_completions
        .push_back(input(10, DeferredPriority::Completion));
    adapter
        .deferred_inputs
        .push_back(input(11, DeferredPriority::Normal));
    adapter
        .deferred_inputs
        .push_back(input(1, DeferredPriority::Normal));
    adapter.next_deferred_priority = DeferredPriority::Completion;

    let selection = adapter
        .pop_deferred_next_eligible(&BTreeSet::from([1]))
        .expect("lifecycle-filtered deferred selection remains exact")
        .expect("the runtime-minimal deferred owner is present");
    assert_eq!(selection.evidence.admission_ordinal, 1);
    assert_eq!(selection.evidence.priority, DeferredPriority::Normal);
    assert_eq!(
        selection.evidence.queue_lengths_before,
        DeferredQueueLengths {
            completion: 1,
            progress: 0,
            normal: 2,
        }
    );
    assert_eq!(
        selection.evidence.queue_lengths_after,
        DeferredQueueLengths {
            completion: 1,
            progress: 0,
            normal: 1,
        }
    );
    assert_eq!(
        selection.evidence.eligible_queue_lengths_before,
        DeferredQueueLengths {
            completion: 0,
            progress: 0,
            normal: 1,
        }
    );
    assert!(selection.evidence.validate_exact());
    assert!(
        selection
            .evidence
            .matches_eligible_admission_ordinals(&BTreeSet::from([1]))
    );
    assert!(
        !selection
            .evidence
            .matches_eligible_admission_ordinals(&BTreeSet::from([1, 10])),
        "the adapter seal binds the runtime's complete target-relative set"
    );

    let rejected = |mut evidence: DeferredServiceEvidence| {
        evidence.projection_hash = deferred_service_projection_hash(&evidence);
        assert!(
            !evidence.validate_exact(),
            "coherently rehashed eligible-selector weakening must fail"
        );
    };
    let mut wrong_cursor_class = selection.evidence.clone();
    wrong_cursor_class.eligible_queue_lengths_before.completion = 1;
    rejected(wrong_cursor_class);
    let mut missing_selected_owner = selection.evidence.clone();
    missing_selected_owner.eligible_queue_lengths_before.normal = 0;
    rejected(missing_selected_owner);
    let mut exceeds_total_class = selection.evidence.clone();
    exceeds_total_class.eligible_queue_lengths_before.progress = 1;
    rejected(exceeds_total_class);

    assert_eq!(adapter.deferred_completions[0].admission_ordinal, 10);
    assert_eq!(adapter.deferred_inputs[0].admission_ordinal, 11);
    assert_eq!(adapter.deferred_completions[0].eligible_skips, 0);
    assert_eq!(adapter.deferred_inputs[0].eligible_skips, 0);
    assert!(selection.evidence.claim_adapter_service_for_test());
    assert!(
        !selection.evidence.claim_adapter_service_for_test(),
        "the exact queue-selection capability crosses the adapter seam once"
    );
}

#[test]
fn deferred_service_debt_overflow_is_typed_and_fail_closed() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let input = |ordinal, priority, eligible_skips| DeferredInput {
        admission_ordinal: ordinal,
        admission_capability: DeferredAdmissionCapability::for_test(ordinal),
        event: reducer::Event::TimeoutElapsed { tag },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips,
    };
    adapter
        .deferred_completions
        .push_back(input(1, DeferredPriority::Completion, 0));
    adapter
        .deferred_progress_inputs
        .push_back(input(2, DeferredPriority::Progress, u64::MAX));
    adapter.next_deferred_priority = DeferredPriority::Completion;

    assert!(matches!(
        adapter.pop_deferred_next(),
        Err(AdapterError::DeferredServiceDebtOverflow)
    ));
    assert!(adapter.fail_closed);
}

#[test]
fn deferred_service_cursor_cycles_nonempty_classes() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut next_ordinal = 1_u128;
    let mut input = |priority: DeferredPriority| {
        let admission_ordinal = next_ordinal;
        next_ordinal = next_ordinal
            .checked_add(1)
            .expect("small deferred fixture ordinal remains representable");
        DeferredInput {
            admission_ordinal,
            admission_capability: DeferredAdmissionCapability::for_test(admission_ordinal),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        }
    };
    for priority in [
        DeferredPriority::Completion,
        DeferredPriority::Progress,
        DeferredPriority::Normal,
    ] {
        let queue = match priority {
            DeferredPriority::Completion => &mut adapter.deferred_completions,
            DeferredPriority::Progress => &mut adapter.deferred_progress_inputs,
            DeferredPriority::Normal => &mut adapter.deferred_inputs,
        };
        queue.push_back(input(priority));
        queue.push_back(input(priority));
    }
    adapter.next_deferred_priority = DeferredPriority::Completion;

    let selected = (0..6)
        .map(|_| {
            let selection = adapter
                .pop_deferred_next()
                .expect("deferred service debt remains representable")
                .expect("every nonempty class receives both turns");
            assert!(selection.evidence.validate_exact());
            selection.evidence.priority
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected,
        vec![
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
        ]
    );
    assert!(
        adapter
            .pop_deferred_next()
            .expect("empty rank remains valid")
            .is_none()
    );
}

#[test]
fn deferred_dispatch_decreases_rank_by_exactly_one_macro_step_per_turn() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let current = adapter.current_tag();
    let stale = reducer::EventTag::new(
        current.height().saturating_add(1),
        current.view(),
        current.generation(),
    );
    let input = |priority: DeferredPriority| DeferredInput {
        admission_ordinal: priority.code().into(),
        admission_capability: DeferredAdmissionCapability::for_test(priority.code().into()),
        event: reducer::Event::TimeoutElapsed { tag: stale },
        completion_evidence: None,
        retag_authenticated_ingress: false,
        priority,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: None,
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter
        .deferred_completions
        .push_back(input(DeferredPriority::Completion));
    adapter
        .deferred_progress_inputs
        .push_back(input(DeferredPriority::Progress));
    adapter
        .deferred_inputs
        .push_back(input(DeferredPriority::Normal));
    adapter.next_deferred_priority = DeferredPriority::Completion;

    for (turn, expected_lengths) in [
        (DeferredPriority::Completion, [0, 1, 1]),
        (DeferredPriority::Progress, [0, 0, 1]),
        (DeferredPriority::Normal, [0, 0, 0]),
    ] {
        assert!(adapter.deferred_work_is_serviceable());
        let before = adapter.deferred_completions.len()
            + adapter.deferred_progress_inputs.len()
            + adapter.deferred_inputs.len();
        assert!(
            adapter
                .drain_deferred()
                .expect("service one stale deferred transition")
                .is_empty()
        );
        let after = adapter.deferred_completions.len()
            + adapter.deferred_progress_inputs.len()
            + adapter.deferred_inputs.len();
        assert_eq!(before - after, 1, "{turn:?} owns exactly one turn");
        assert_eq!(
            [
                adapter.deferred_completions.len(),
                adapter.deferred_progress_inputs.len(),
                adapter.deferred_inputs.len(),
            ],
            expected_lengths,
            "the round-robin cursor selected {turn:?}"
        );
    }
    assert!(!adapter.deferred_work_is_serviceable());
}

#[test]
fn deferred_service_contract_violation_is_terminal() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    assert!(matches!(
        adapter.fail_deferred_service_contract(),
        AdapterError::DeferredServiceContractViolation
    ));
    assert!(adapter.fail_closed);
    assert!(matches!(
        adapter.drain_deferred(),
        Err(AdapterError::FailClosed)
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn unowned_busy_prepare_certificate_rolls_back_staged_registry_and_active_subject() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let timeout_sign = adapter
        .timeout_elapsed(tag)
        .expect("start a local timeout signature fence");
    assert!(matches!(
        timeout_sign.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));

    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let qc = |phase, marker| wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase,
        subject: subject(marker),
        execution_commitment: execution_commitment(marker),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    let deferred_prepare = qc(wire::GlobalPhase::Prepare, 0xE0);
    let deferred_prepare_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::QuorumCertificate(deferred_prepare.clone()),
    );
    let deferred_prepare = adapter
        .registry
        .qc_to_core(&deferred_prepare, &adapter.wire_context)
        .expect("convert PrepareQC lane fixture");
    adapter.deferred_progress_inputs.push_back(DeferredInput {
        admission_ordinal: 1,
        admission_capability: DeferredAdmissionCapability::for_authenticated_test(1),
        event: reducer::Event::QuorumCertificateReceived {
            tag,
            certificate: deferred_prepare,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Progress,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(deferred_prepare_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });

    let registry_before = adapter.registry.clone();
    let active_subject_before = adapter.active_subject;
    let deferred_before = adapter.deferred_progress_inputs.clone();
    let outcome = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(qc(
                wire::GlobalPhase::Prepare,
                0xE3,
            )),
        ))
        .expect("apply PrepareQC-class backpressure");
    assert_eq!(
        outcome.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_progress_inputs, deferred_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.active_subject, active_subject_before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn unowned_busy_exact_locked_vote_rolls_back_and_remains_retryable() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    let locked_subject = subject(0xE6);
    let locked_execution_commitment = execution_commitment(0xE6);
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
        aggregate_signature: vec![0xE6; 96],
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

    let roster_len = adapter.wire_context.roster.len();
    let mut fillers = VecDeque::with_capacity(roster_len);
    for signer in 0..roster_len {
        let signer = u32::try_from(signer).expect("fixture signer fits u32");
        let wire_filler_vote = wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer,
            signature: vec![0xE7 ^ u8::try_from(signer).expect("fixture signer fits u8")],
        };
        let filler_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::Vote(wire_filler_vote.clone()),
        );
        let filler_vote = adapter
            .registry
            .vote_to_core(&wire_filler_vote, &adapter.wire_context)
            .expect("convert locked-vote capacity fixture");
        fillers.push_back(DeferredInput {
            admission_ordinal: u128::from(signer).saturating_add(1),
            admission_capability: DeferredAdmissionCapability::for_authenticated_test(
                u128::from(signer).saturating_add(1),
            ),
            event: reducer::Event::VoteReceived {
                tag: replay_tag,
                vote: filler_vote,
            },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Progress,
            protected_progress: true,
            admission: None,
            authenticated_wire_identity: Some(filler_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
    }
    adapter.deferred_progress_inputs = fillers;
    let retried_signer = u32::try_from(
        roster_len
            .checked_sub(1)
            .expect("fixture roster is non-empty"),
    )
    .expect("fixture signer fits u32");

    let locked_vote =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer: retried_signer,
            signature: vec![0xE8],
        }));
    let key = IngressSemanticKey::Vote {
        round: wire_round,
        phase: wire::GlobalPhase::Commit,
        signer: retried_signer,
    };
    let registry_before = adapter.registry.clone();
    let active_subject_before = adapter.active_subject;
    let deferred_before = adapter.deferred_progress_inputs.clone();
    let backpressured = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote.clone()))
        .expect("apply locked-vote-class backpressure");
    assert_eq!(
        backpressured.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        backpressured.requires_runtime_retry(),
        "a full lane retains no adapter owner and must re-expose the exact runtime command"
    );
    assert_eq!(adapter.deferred_progress_inputs, deferred_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.active_subject, active_subject_before);
    assert!(adapter.ingress_equivocations.contains_key(&key));
    assert!(
        !adapter.ingress_deliveries.contains_key(&key),
        "admission without locked-vote queue ownership must remain retryable"
    );

    adapter.deferred_progress_inputs.pop_back();
    let retried = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote))
        .expect("retry after locked-vote ownership becomes available");
    assert_eq!(
        retried.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(!retried.requires_runtime_retry());
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        adapter.wire_context.roster.len()
    );
    assert!(adapter.ingress_deliveries.contains_key(&key));
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::VoteReceived { .. },
            admission: Some(_),
            protected_progress: true,
            ..
        })
    ));
}

#[test]
fn deferred_progress_capacity_matches_partition_geometry() {
    assert_eq!(deferred_progress_capacity(0), 3);
    assert_eq!(deferred_progress_capacity(1), 5);
    assert_eq!(deferred_progress_capacity(4), 11);
    assert_eq!(
        deferred_progress_capacity(wire::MAX_VALIDATORS_PER_HEIGHT),
        MAX_DEFERRED_PROGRESS_INPUTS
    );
    assert_eq!(
        deferred_progress_capacity(wire::MAX_VALIDATORS_PER_HEIGHT.saturating_add(1)),
        MAX_DEFERRED_PROGRESS_INPUTS,
        "invalid oversized rosters cannot expand the static adapter bound"
    );
    assert_eq!(semantic_ingress_capacity(0), MAX_INGRESS_SEMANTIC_KEYS);
    assert_eq!(semantic_ingress_capacity(4), MAX_INGRESS_SEMANTIC_KEYS + 12);
    assert_eq!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE, 11);
    assert_eq!(
        BTreeSet::from(ServicedCandidateStage::ALL.map(|stage| stage as u8)).len(),
        SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        "the closed adapter-event projection has eleven distinct classes"
    );
    assert_eq!(
        serviced_candidate_capacity(4),
        (MAX_INGRESS_SEMANTIC_KEYS
            + 12
            + MAX_DEFERRED_INPUTS * 2
            + 11
            + MAX_DEFERRED_INPUTS * 4
            + MAX_DEFERRED_INPUTS
            + CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY
            + 1)
            * SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        "serviced identities cover active causal/effect/clock owners as well as service queues"
    );
    for roster_len in [0, 1, 4, wire::MAX_VALIDATORS_PER_HEIGHT] {
        assert_eq!(
            serviced_candidate_capacity(roster_len),
            candidate_lifecycle_capacity(roster_len, DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,)
                .saturating_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE),
            "the bound is the complete reviewed lifecycle geometry times the exact stage \
                 carrier for roster size {roster_len}"
        );
    }
    let configured = ServicedCandidateCapacityGeometry::new(4_096, 777);
    assert_eq!(
        candidate_lifecycle_capacity(4, configured),
        semantic_ingress_capacity(4)
            + MAX_DEFERRED_INPUTS * 2
            + deferred_progress_capacity(4)
            + 4_096 * 4
            + 777
            + CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY
            + 1,
        "runtime and effect ownership are derived from the supplied production configuration"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn deferred_progress_partition_owns_every_vote_and_certificate_class() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let roster_len = adapter.wire_context.roster.len();
    let tag = adapter.current_tag();
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };

    for signer in 0..roster_len {
        let signer = u32::try_from(signer).expect("fixture signer fits u32");
        let marker = u8::try_from(signer).expect("fixture signer fits u8") | 0xA0;
        let locked_subject = subject(marker);
        let locked_commitment = execution_commitment(marker);
        let wire_vote = wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_commitment,
            signer,
            signature: vec![marker],
        };
        let vote_wire_identity =
            authenticated_wire_identity(wire::ConsensusMessageV2Payload::Vote(wire_vote.clone()));
        let vote = adapter
            .registry
            .vote_to_core(&wire_vote, &adapter.wire_context)
            .expect("convert locked Commit capacity fixture");
        let admission = IngressAdmission {
            key: IngressSemanticKey::Vote {
                round: wire_round,
                phase: wire::GlobalPhase::Commit,
                signer,
            },
            fingerprint: IngressFingerprint::Vote(wire_round, locked_subject, locked_commitment),
            generation: tag.generation(),
            inserted_equivocation: false,
            locked_commit_progress: true,
        };
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::VoteReceived { tag, vote },
                    true,
                    DeferredPriority::Progress,
                    Some(admission),
                    None,
                    Some(vote_wire_identity),
                )
                .expect("admit one locked Commit owner per frozen validator")
                .is_some()
        );

        let wire_timeout = wire::TimeoutVote {
            round: wire_round,
            highest_prepare_qc: None,
            signer,
            signature: vec![marker ^ 0x0F],
        };
        let timeout_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire_timeout.clone()),
        );
        let timeout = adapter
            .registry
            .timeout_vote_to_core(&wire_timeout, &adapter.wire_context)
            .expect("convert TimeoutVote capacity fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::TimeoutVoteReceived { tag, vote: timeout },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(timeout_wire_identity),
                )
                .expect("admit one TimeoutVote owner per frozen validator")
                .is_some()
        );
        if signer == 0 {
            let retained = adapter.deferred_progress_inputs.clone();
            let wire_distinct_same_signer = wire::TimeoutVote {
                round: wire::ConsensusRound {
                    view: wire_round.view + 1,
                    ..wire_round
                },
                highest_prepare_qc: None,
                signer,
                signature: vec![marker ^ 0xF0],
            };
            let distinct_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire_distinct_same_signer.clone()),
            );
            let distinct_same_signer = adapter
                .registry
                .timeout_vote_to_core(&wire_distinct_same_signer, &adapter.wire_context)
                .expect("convert distinct same-signer TimeoutVote fixture");
            let distinct_same_signer = reducer::Event::TimeoutVoteReceived {
                tag,
                vote: distinct_same_signer,
            };
            assert!(
                adapter
                    .enqueue_deferred(
                        distinct_same_signer.clone(),
                        true,
                        DeferredPriority::Progress,
                        None,
                        None,
                        Some(Arc::clone(&distinct_wire_identity)),
                    )
                    .expect("same signer cannot consume a second TimeoutVote slot")
                    .is_none(),
                "TimeoutVote ownership must be signer-injective before the class is full"
            );
            assert_eq!(
                adapter.deferred_progress_inputs, retained,
                "later same-signer traffic must not displace admitted progress"
            );
            let core_signer = adapter
                .registry
                .validator_id(signer)
                .expect("fixture signer belongs to the frozen roster");
            let owned_index = adapter
                .deferred_progress_inputs
                .iter()
                .position(|queued| {
                    deferred_progress_owner(queued)
                        == Some(DeferredProgressOwner::TimeoutVote(core_signer))
                })
                .expect("original same-signer TimeoutVote owns one slot");
            adapter.deferred_progress_inputs.remove(owned_index);
            assert!(
                adapter
                    .enqueue_deferred(
                        distinct_same_signer,
                        true,
                        DeferredPriority::Progress,
                        None,
                        None,
                        Some(distinct_wire_identity),
                    )
                    .expect("same signer retries after its prior owner is serviced")
                    .is_some()
            );
        }
    }

    for (phase, marker) in [
        (wire::GlobalPhase::Prepare, 0xB0),
        (wire::GlobalPhase::Commit, 0xB1),
    ] {
        let certificate = wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase,
            subject: subject(marker),
            execution_commitment: execution_commitment(marker),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        };
        let certificate_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
        );
        let certificate = adapter
            .registry
            .qc_to_core(&certificate, &adapter.wire_context)
            .expect("convert QC capacity fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::QuorumCertificateReceived { tag, certificate },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(certificate_wire_identity),
                )
                .expect("admit the independent QC class owner")
                .is_some()
        );
    }
    let timeout_certificate = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xB2; 96],
        }],
    };
    let timeout_certificate_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate.clone()),
    );
    let timeout_certificate = adapter
        .registry
        .tc_to_core(&timeout_certificate, &adapter.wire_context)
        .expect("convert TC capacity fixture");
    assert!(
        adapter
            .enqueue_deferred(
                reducer::Event::TimeoutCertificateReceived {
                    tag,
                    certificate: timeout_certificate,
                },
                true,
                DeferredPriority::Progress,
                None,
                None,
                Some(timeout_certificate_wire_identity),
            )
            .expect("admit the independent TC class owner")
            .is_some()
    );

    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        deferred_progress_capacity(roster_len)
    );
    for (class, expected) in [
        (DeferredProgressClass::LockedCommitVote, roster_len),
        (DeferredProgressClass::TimeoutVote, roster_len),
        (DeferredProgressClass::PrepareCertificate, 1),
        (DeferredProgressClass::CommitCertificate, 1),
        (DeferredProgressClass::TimeoutCertificate, 1),
    ] {
        assert_eq!(
            adapter
                .deferred_progress_inputs
                .iter()
                .filter(|input| deferred_progress_class(input) == Some(class))
                .count(),
            expected,
            "each protected Progress class owns its exact partition"
        );
    }

    let retained = adapter.deferred_progress_inputs.clone();
    let later_round = wire::ConsensusRound {
        view: 1,
        ..wire_round
    };
    let overflow = wire::TimeoutVote {
        round: later_round,
        highest_prepare_qc: None,
        signer: 0,
        signature: vec![0xBF],
    };
    let overflow_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutVote(overflow.clone()),
    );
    let overflow = adapter
        .registry
        .timeout_vote_to_core(&overflow, &adapter.wire_context)
        .expect("convert distinct TimeoutVote overflow fixture");
    assert!(
        adapter
            .enqueue_deferred(
                reducer::Event::TimeoutVoteReceived {
                    tag,
                    vote: overflow,
                },
                true,
                DeferredPriority::Progress,
                None,
                None,
                Some(overflow_wire_identity),
            )
            .expect("a full TimeoutVote partition rejects without displacement")
            .is_none()
    );
    assert_eq!(adapter.deferred_progress_inputs, retained);
}

#[test]
fn protected_locked_vote_uses_reserved_capacity_without_evicting_certificate_ownership() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let wire_timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xCA; 96],
        }],
    };
    let timeout_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire_timeout.clone()),
    );
    let timeout = adapter
        .registry
        .tc_to_core(&wire_timeout, &adapter.wire_context)
        .expect("convert certificate lane fixture");
    let tag = adapter.current_tag();
    let certificate_input = DeferredInput {
        admission_ordinal: 1,
        admission_capability: DeferredAdmissionCapability::for_authenticated_test(1),
        event: reducer::Event::TimeoutCertificateReceived {
            tag,
            certificate: timeout,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Progress,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(timeout_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    };
    adapter
        .deferred_progress_inputs
        .push_back(certificate_input.clone());
    assert!(
        adapter
            .deferred_progress_inputs
            .iter()
            .all(|input| progress_rank(&input.event) > 0)
    );
    let admitted_before = adapter.deferred_progress_inputs.clone();
    let wire_overflow_certificate = wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            view: wire_round.view + 1,
            ..wire_round
        },
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xCB; 96],
        }],
    };
    let overflow_certificate_wire_identity = authenticated_wire_identity(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire_overflow_certificate.clone()),
    );
    let overflow_certificate = adapter
        .registry
        .tc_to_core(&wire_overflow_certificate, &adapter.wire_context)
        .expect("convert distinct certificate overflow fixture");
    assert!(
        adapter
            .enqueue_deferred(
                reducer::Event::TimeoutCertificateReceived {
                    tag,
                    certificate: overflow_certificate,
                },
                true,
                DeferredPriority::Progress,
                None,
                None,
                Some(overflow_certificate_wire_identity),
            )
            .expect("ordinary certificate overflow is rejected before admission")
            .is_none()
    );
    assert_eq!(
        adapter.deferred_progress_inputs, admitted_before,
        "equal-rank traffic must never replace already admitted certificate ownership"
    );

    let locked_subject = subject(0xDA);
    let locked_execution_commitment = execution_commitment(0xDA);
    let wire_vote = wire::Vote {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Commit,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signer: 1,
        signature: vec![0xDA],
    };
    let vote_wire_identity =
        authenticated_wire_identity(wire::ConsensusMessageV2Payload::Vote(wire_vote.clone()));
    let vote = adapter
        .registry
        .vote_to_core(&wire_vote, &adapter.wire_context)
        .expect("convert protected locked vote fixture");
    let admission = IngressAdmission {
        key: IngressSemanticKey::Vote {
            round: wire_round,
            phase: wire::GlobalPhase::Commit,
            signer: 1,
        },
        fingerprint: IngressFingerprint::Vote(
            wire_round,
            locked_subject,
            locked_execution_commitment,
        ),
        generation: tag.generation(),
        inserted_equivocation: false,
        locked_commit_progress: true,
    };
    let protected_event = reducer::Event::VoteReceived { tag, vote };
    assert_eq!(progress_rank(&protected_event), 0);

    assert!(
        adapter
            .enqueue_deferred(
                protected_event,
                true,
                DeferredPriority::Progress,
                Some(admission),
                None,
                Some(vote_wire_identity),
            )
            .expect("protected ownership uses its reserved locked-vote capacity")
            .is_some()
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 2);
    assert_eq!(
        adapter
            .deferred_progress_inputs
            .iter()
            .filter(|input| input.protected_progress)
            .count(),
        1
    );
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::VoteReceived { .. },
            admission: Some(_),
            protected_progress: true,
            ..
        })
    ));
}

fn saturate_ordinary_semantic_history(
    adapter: &mut SumeragiV2Adapter,
    round: wire::ConsensusRound,
) {
    let ingress_context = adapter.wire_context.clone();
    for index in 0..MAX_INGRESS_SEMANTIC_KEYS {
        if adapter.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS {
            break;
        }
        let proposer = u32::MAX
            .checked_sub(u32::try_from(index).expect("semantic index fits u32"))
            .expect("fixture proposer remains in range");
        adapter.ingress_equivocations.insert(
            IngressSemanticKey::Proposal { round, proposer },
            IngressEquivocationRecord {
                fingerprint: IngressFingerprint::Proposal(Hash::new(index.to_le_bytes())),
                artifact: synthetic_ingress_proposal(&ingress_context, round, proposer, index),
                equivocation_reported: false,
                capacity_bypass: false,
                admitted_at: Instant::now(),
            },
        );
    }
    assert_eq!(
        adapter.ingress_equivocations.len(),
        MAX_INGRESS_SEMANTIC_KEYS
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let current_tag = adapter.current_tag();
    let current_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: current_tag.view(),
    };

    let local_timeout = adapter
        .timeout_elapsed(current_tag)
        .expect("start the local TimeoutVote signature fence");
    assert!(matches!(
        local_timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        },]
    ));

    let timeout_certificate = wire::TimeoutCertificate {
        round: current_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC1; 96],
        }],
    };
    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate),
        ))
        .expect("authenticated TC bypasses the hung local signature");
    assert_eq!(installed.disposition(), reducer::StepDisposition::Applied);
    assert!(matches!(
        installed.effects(),
        [AdapterEffect::EnterView {
            tag,
            protected_lock: None,
            ..
        }] if tag.view() == current_round.view + 1
    ));
    assert_eq!(adapter.current_tag().view(), current_round.view + 1);
    assert!(adapter.deferred_progress_inputs.is_empty());

    let adjacent_round = wire::ConsensusRound {
        view: current_round
            .view
            .saturating_add(reducer::FUTURE_TIMEOUT_VOTE_LOOKAHEAD),
        ..current_round
    };
    let adjacent_vote = wire::TimeoutVote {
        round: adjacent_round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xC3],
    };
    let adjacent_key = IngressSemanticKey::TimeoutVote {
        round: adjacent_round,
        signer: 1,
    };
    let applied = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(adjacent_vote.clone()),
        ))
        .expect("apply the adjacent vote after its view becomes current");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(adapter.ingress_deliveries.contains_key(&adjacent_key));

    let duplicate = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(adjacent_vote),
        ))
        .expect("coalesce the delivered adjacent TimeoutVote");
    assert_eq!(
        duplicate.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn busy_deferred_source_identity_coalesces_across_consumer_view_change() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let first_tag = adapter.current_tag();
    let first_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: first_tag.view(),
    };
    saturate_ordinary_semantic_history(&mut adapter, first_round);

    let first_timeout = adapter
        .timeout_elapsed(first_tag)
        .expect("start the first local TimeoutVote signature fence");
    let _first_sign_tag = match first_timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected first timeout effects: {effects:?}"),
    };

    let old_timeout = wire::TimeoutVote {
        round: first_round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xD8],
    };
    let old_key = IngressSemanticKey::TimeoutVote {
        round: first_round,
        signer: 1,
    };
    let deferred_old = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(old_timeout.clone()),
        ))
        .expect("defer the old-view TimeoutVote behind the signature fence");
    assert_eq!(
        deferred_old.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        adapter
            .ingress_equivocations
            .get(&old_key)
            .is_some_and(|record| record.capacity_bypass)
    );
    assert!(adapter.ingress_deliveries.contains_key(&old_key));
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);
    let old_input = adapter
        .deferred_progress_inputs
        .back()
        .expect("the old-view TimeoutVote owns the later Busy slot");
    let original_candidate = adapter
        .serviced_candidate(
            &old_input.event,
            old_input.priority,
            old_input.completion_evidence.as_ref(),
            old_input.authenticated_wire_identity.as_deref(),
        )
        .expect("authenticated TimeoutVote has a service identity");
    assert_eq!(original_candidate.1, first_round.view);
    assert_eq!(original_candidate.0.source_view(), first_round.view);
    assert_eq!(
        original_candidate.0.leader(),
        adapter.wire_context.leader(first_round.view)
    );

    let duplicate_old = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(old_timeout),
        ))
        .expect("coalesce the exact deferred TimeoutVote");
    assert_eq!(
        duplicate_old.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_progress_inputs.len(), 1);

    let timeout_certificate = wire::TimeoutCertificate {
        round: first_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD7; 96],
        }],
    };
    let enter_view = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate),
        ))
        .expect("authenticated TC bypasses the first signature fence");
    assert_eq!(enter_view.disposition(), reducer::StepDisposition::Applied);
    assert!(enter_view.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::EnterView { tag, .. } if tag.view() == 1
    )));
    assert_eq!(adapter.current_tag().view(), 1);
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "EnterView must leave the later old-view TimeoutVote owned until service"
    );
    let old_owner = adapter
        .registry
        .validator_id(1)
        .expect("fixture TimeoutVote signer belongs to the frozen roster");
    assert!(matches!(
        adapter.deferred_progress_inputs.front(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutVoteReceived { vote, .. },
            ..
        }) if vote.vote().round().view() == 0
            && vote.vote().signer() == old_owner
    ));
    let old_input = adapter
        .deferred_progress_inputs
        .front()
        .expect("the old-view TimeoutVote remains owned");
    let retagged_event = old_input
        .event
        .clone()
        .retag_authenticated_ingress(adapter.current_tag());
    let retagged_candidate = adapter
        .serviced_candidate(
            &retagged_event,
            old_input.priority,
            old_input.completion_evidence.as_ref(),
            old_input.authenticated_wire_identity.as_deref(),
        )
        .expect("retagged TimeoutVote retains a service identity");
    assert_eq!(retagged_candidate.0, original_candidate.0);
    assert_eq!(retagged_candidate.0.source_view(), first_round.view);
    assert_eq!(retagged_candidate.1, adapter.current_tag().view());
    assert_ne!(
        retagged_candidate.0.leader(),
        adapter.wire_context.leader(retagged_candidate.1),
        "logical leader ownership derives from source view, not the consumer episode"
    );
    assert_ne!(
        original_candidate.1, retagged_candidate.1,
        "the consumer episode advanced while semantic source identity stayed fixed"
    );
    assert!(
        !adapter.ingress_equivocations.contains_key(&old_key)
            && !adapter.ingress_deliveries.contains_key(&old_key),
        "a capacity-bypass TimeoutVote record must retire when its view is no longer current"
    );

    let second_tag = adapter.current_tag();
    let second_timeout = adapter
        .timeout_elapsed(second_tag)
        .expect("start the current-view TimeoutVote signature fence");
    let second_sign_tag = match second_timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected second timeout effects: {effects:?}"),
    };
    let second_round = wire::ConsensusRound {
        view: second_tag.view(),
        ..first_round
    };
    let current_timeout = wire::TimeoutVote {
        round: second_round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xDA],
    };
    let current_key = IngressSemanticKey::TimeoutVote {
        round: second_round,
        signer: 1,
    };
    let registry_before = adapter.registry.clone();
    let active_subject_before = adapter.active_subject;
    let deferred_before = adapter.deferred_progress_inputs.clone();
    for attempt in 0..2 {
        let blocked = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout.clone()),
            ))
            .expect("same-owner TimeoutVote remains retryable before service");
        assert_eq!(
            blocked.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy),
            "pre-service attempt {attempt} must not be poisoned as a duplicate"
        );
        assert_eq!(adapter.deferred_progress_inputs, deferred_before);
        assert_registry_eq(&adapter.registry, &registry_before);
        assert_eq!(adapter.active_subject, active_subject_before);
        assert!(
            adapter
                .ingress_equivocations
                .get(&current_key)
                .is_some_and(|record| record.capacity_bypass)
        );
        assert!(!adapter.ingress_deliveries.contains_key(&current_key));
    }

    adapter
        .signature_completed(second_sign_tag, vec![0xDB; 96])
        .expect("complete the current-view signature");
    assert!(
        adapter
            .drain_deferred()
            .expect("service the old owner in its own macro-step")
            .is_empty()
    );
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert_eq!(
        adapter.serviced_candidates.get(&original_candidate.0),
        None,
        "retagged authenticated policy discard remains marker-free"
    );
    let retained_count = adapter.serviced_candidate_count_for_test();
    adapter
        .record_serviced_candidate(Some(retagged_candidate), false, false, None)
        .expect("an exact same-episode source occurrence coalesces");
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        retained_count + 1,
        "a transient same-source projection remains owned until strict episode exit"
    );

    let applied = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout.clone()),
        ))
        .expect("retry the current-view TimeoutVote after service");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(adapter.ingress_deliveries.contains_key(&current_key));
    let duplicate = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout),
        ))
        .expect("coalesce the delivered current-view TimeoutVote");
    assert_eq!(
        duplicate.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
}

#[test]
fn full_normal_deferred_lane_cannot_drop_absolute_timeout() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());

    // Leave the reducer waiting for a Prepare signature, then model a
    // saturated untrusted deferred lane. The absolute timeout is delivered
    // while that signature fence is active, exactly where it used to be
    // classified as normal traffic and silently discarded.
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(0xD2);
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
    let receipt = durable_body_receipt(&adapter, round, proposed_subject);
    adapter
        .body_stored(tag, round, proposed_subject, &receipt)
        .expect("body stored");
    let validated = ValidatedBodyReceipt::for_test(receipt);
    let sign = adapter
        .validation_succeeded(tag, round, proposed_subject, &validated)
        .expect("body valid")
        .into_effects();
    let sign_tag = match sign.as_slice() {
        [AdapterEffect::Sign { tag, .. }] => *tag,
        effects => panic!("unexpected validation effects: {effects:?}"),
    };

    let normal_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xD3),
        execution_commitment: execution_commitment(0xD3),
        signer: 1,
        signature: vec![0xD3],
    };
    let deferred_vote = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(normal_vote.clone()),
        ))
        .expect("defer normal authenticated vote");
    assert_eq!(
        deferred_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let filler = adapter
        .deferred_inputs
        .front()
        .expect("normal vote is queued")
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

    let mut backpressured_vote = normal_vote;
    backpressured_vote.signer = 2;
    backpressured_vote.signature = vec![0xD4];
    let backpressured_key = IngressSemanticKey::Vote {
        round,
        phase: wire::GlobalPhase::Prepare,
        signer: 2,
    };
    let backpressured = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(backpressured_vote.clone()),
        ))
        .expect("apply normal-lane backpressure");
    assert_eq!(
        backpressured.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        adapter
            .ingress_equivocations
            .contains_key(&backpressured_key)
    );
    assert!(
        !adapter.ingress_deliveries.contains_key(&backpressured_key),
        "admission without queue ownership must remain retryable"
    );

    adapter.deferred_inputs.pop_back();
    let retried = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(backpressured_vote),
        ))
        .expect("retry after reserved ownership becomes available");
    assert_eq!(
        retried.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(adapter.ingress_deliveries.contains_key(&backpressured_key));
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);

    // Saturate the ordinary semantic table as well. TimeoutVote owns an
    // independent signer-bounded semantic slot, so it must still reach the
    // protected Busy-deferred partition instead of being rejected before
    // the reducer boundary.
    saturate_ordinary_semantic_history(&mut adapter, round);

    let timeout_vote = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xD5],
    };
    let timeout_key = IngressSemanticKey::TimeoutVote { round, signer: 1 };
    let deferred_timeout_vote = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote),
        ))
        .expect("defer TimeoutVote through its protected class");
    assert_eq!(
        deferred_timeout_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert!(
        adapter
            .ingress_equivocations
            .get(&timeout_key)
            .is_some_and(|record| record.capacity_bypass),
        "current-view TimeoutVote must bypass saturated ordinary semantic capacity"
    );
    assert!(adapter.ingress_deliveries.contains_key(&timeout_key));
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutVoteReceived { .. },
            priority: DeferredPriority::Progress,
            protected_progress: false,
            ..
        })
    ));
    assert_eq!(
        deferred_progress_class(
            adapter
                .deferred_progress_inputs
                .back()
                .expect("deferred TimeoutVote owns the progress lane")
        ),
        Some(DeferredProgressClass::TimeoutVote)
    );

    let timeout = adapter
        .timeout_elapsed(sign_tag)
        .expect("defer trusted absolute timeout");
    assert_eq!(
        timeout.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutElapsed { .. },
            priority: DeferredPriority::Completion,
            ..
        })
    ));

    let completed = adapter
        .signature_completed(sign_tag, vec![0xD2; 96])
        .expect("complete outstanding Prepare signature")
        .into_effects();
    assert!(completed.iter().all(|effect| !matches!(
        effect,
        AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }
    )));
    let timeout_effects = adapter
        .drain_deferred()
        .expect("service the absolute timeout as one deferred macro-step");
    let timeout_sign_tag = timeout_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            } => Some(*tag),
            _ => None,
        })
        .expect("absolute timeout starts the durable local TimeoutVote signature");
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "the remote TimeoutVote remains owned while the local TimeoutVote signature fences the reducer"
    );

    adapter
        .signature_completed(timeout_sign_tag, vec![0xD6; 96])
        .expect("complete the local TimeoutVote signature");
    adapter
        .drain_deferred()
        .expect("service protected progress in its own macro-step");
    assert!(adapter.deferred_progress_inputs.is_empty());
}

#[test]
fn failed_ingress_conversion_rolls_back_registry_and_admission() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(0xE0);
    let valid = proposal(&adapter.wire_context, proposer, proposed_subject);
    let mut malformed = valid.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut malformed.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    proposal.justification = wire::ProposalJustification::Timeout(wire::TimeoutJustification {
        timeout_certificate: wire::TimeoutCertificate {
            round: proposal.round,
            groups: Vec::new(),
        },
        highest_prepare_qc: None,
    });

    let subject_count = adapter.registry.subjects.len();
    let manifest_count = adapter.registry.manifests.len();
    assert!(adapter.receive_verified(malformed).is_err());
    assert_eq!(adapter.registry.subjects.len(), subject_count);
    assert_eq!(adapter.registry.manifests.len(), manifest_count);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());
    assert!(adapter.active_subject.is_none());

    // The failed conversion did not poison the semantic key; the valid
    // proposal for the same leader and round is still admitted.
    assert!(matches!(
        adapter
            .receive_verified(valid)
            .expect("valid retry")
            .effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
}

#[cfg(feature = "bls")]
#[test]
fn authentication_rejects_valid_commitment_conflicts_without_mutating_adapter() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified = VerifiedHeightContext::genesis(context.clone(), pops).expect("verified context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("commitment-auth-safety.wal"),
        verified,
        None,
        reducer::Generation::new(1),
        [0x83; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());

    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let locally_validated_subject = subject(0x87);
    let locally_validated_payload = [0x87, 2];
    let locally_validated_manifest = encode_payload(
        &context,
        round,
        locally_validated_subject,
        &locally_validated_payload,
    )
    .expect("encode locally validated payload")
    .manifest()
    .clone();
    let (_, locally_validated_receipt) =
        validated_receipts_for_manifest(&context, &locally_validated_manifest);
    let locally_validated_commitment = locally_validated_receipt.execution_commitment();
    let wrong_unbound_commitment = execution_commitment(0x87);
    assert_ne!(wrong_unbound_commitment, locally_validated_commitment);
    let signed_vote = |execution_commitment| {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: locally_validated_subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(vote.signer).expect("small signer")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        vote
    };
    let wrong_unbound_vote = signed_vote(wrong_unbound_commitment);
    let canonical_unbound_vote = signed_vote(locally_validated_commitment);
    let registry_before_unbound_votes = adapter.registry.clone();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote.clone()),
        )),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote.clone()),
        )),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    assert_registry_eq(&adapter.registry, &registry_before_unbound_votes);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());
    assert!(adapter.deferred_completions.is_empty());
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert!(adapter.deferred_inputs.is_empty());
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    adapter
        .recover_validated_body(&locally_validated_manifest, &locally_validated_receipt)
        .expect("local deterministic validation establishes canonical commitment authority");
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote),
        ))
        .expect("the same signed canonical vote is admissible after local validation");
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    let bound_subject = subject(0x83);
    let canonical_commitment = execution_commitment(0x83);
    let conflicting_commitment = execution_commitment(0x84);
    let core_subject = adapter
        .registry
        .register_subject(bound_subject)
        .expect("register canonical subject");
    adapter
        .registry
        .register_execution_commitment(
            reducer::Round::new(round.height, round.view),
            core_subject,
            canonical_commitment,
        )
        .expect("bind canonical validated execution result");
    let retained_registry = adapter.registry.clone();
    let retained_equivocations = adapter.ingress_equivocations.clone();
    let retained_deliveries = adapter.ingress_deliveries.clone();
    let retained_queue_lengths = (
        adapter.deferred_completions.len(),
        adapter.deferred_progress_inputs.len(),
        adapter.deferred_inputs.len(),
    );

    let mut conflicting_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: conflicting_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    conflicting_vote.signature = Signature::new(
        keys[usize::try_from(conflicting_vote.signer).expect("small signer")].private_key(),
        &conflicting_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(conflicting_vote.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let mut conflicting_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: conflicting_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut conflicting_qc, &keys);
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(conflicting_qc.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let later_round = wire::ConsensusRound { view: 1, ..round };
    let mut cross_round_conflicting_vote = wire::Vote {
        round: later_round,
        proposal_round: later_round,
        signature: Vec::new(),
        ..conflicting_vote
    };
    cross_round_conflicting_vote.signature = Signature::new(
        keys[usize::try_from(cross_round_conflicting_vote.signer).expect("small signer index")]
            .private_key(),
        &cross_round_conflicting_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let cross_round_conflicting_payload =
        wire::ConsensusMessageV2Payload::Vote(cross_round_conflicting_vote.clone());
    assert_eq!(
        adapter.wire_ingress_missing_execution_commitment(&cross_round_conflicting_payload),
        None,
        "a same-subject cross-round conflict must drain instead of retaining fair-ingress ownership"
    );
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            cross_round_conflicting_payload,
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let mut cross_round_canonical_vote = wire::Vote {
        execution_commitment: canonical_commitment,
        signature: Vec::new(),
        ..cross_round_conflicting_vote
    };
    cross_round_canonical_vote.signature = Signature::new(
        keys[usize::try_from(cross_round_canonical_vote.signer).expect("small signer index")]
            .private_key(),
        &cross_round_canonical_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let cross_round_canonical_payload =
        wire::ConsensusMessageV2Payload::Vote(cross_round_canonical_vote);
    assert_eq!(
        adapter.wire_ingress_missing_execution_commitment(&cross_round_canonical_payload),
        Some((later_round, bound_subject)),
        "the same commitment on another round remains unbound until exact-round validation"
    );
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(cross_round_canonical_payload,)),
        Err(AdapterError::MissingExecutionCommitment)
    ));

    let mut cross_round_conflict = wire::QuorumCertificate {
        round: later_round,
        proposal_round: later_round,
        ..conflicting_qc.clone()
    };
    authenticate_qc(&mut cross_round_conflict, &keys);
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_conflict),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let mut cross_round_canonical = wire::QuorumCertificate {
        round: later_round,
        proposal_round: later_round,
        execution_commitment: canonical_commitment,
        ..conflicting_qc.clone()
    };
    authenticate_qc(&mut cross_round_canonical, &keys);
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_canonical),
        ))
        .expect("an unchanged re-proposal authenticates the same deterministic execution");

    let timeout_round = wire::ConsensusRound { view: 1, ..round };
    let timeout_preimage = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: Some(conflicting_qc.clone()),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let timeout_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &timeout_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate valid timeout signatures");
    let mut conflicting_timeout_vote = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: Some(conflicting_qc.clone()),
        signer: 0,
        signature: Vec::new(),
    };
    conflicting_timeout_vote.signature = Signature::new(
        keys[usize::try_from(conflicting_timeout_vote.signer).expect("small signer")].private_key(),
        &conflicting_timeout_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(conflicting_timeout_vote),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let conflicting_tc = wire::TimeoutCertificate {
        round: timeout_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(conflicting_qc.clone()),
            signers: vec![0, 1, 2],
            aggregate_signature: timeout_signature,
        }],
    };
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(conflicting_tc.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let proposal_round = wire::ConsensusRound { view: 2, ..round };
    let proposal_subject = bound_subject;
    let proposal_body = vec![0x83, 2];
    let proposal_manifest =
        encode_payload(&context, proposal_round, proposal_subject, &proposal_body)
            .expect("encode later-view proposal payload")
            .manifest()
            .clone();
    let proposer = context.leader(proposal_round.view);
    let mut conflicting_proposal = wire::Proposal {
        round: proposal_round,
        proposer,
        subject: proposal_subject,
        manifest: proposal_manifest,
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: conflicting_tc,
            highest_prepare_qc: Some(conflicting_qc.clone()),
        }),
        signature: Vec::new(),
    };
    conflicting_proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &conflicting_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let conflicting_proposal_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal),
    );
    // Exercise the read-only embedded-certificate compatibility walk
    // directly, then confirm ordinary ingress rejects the same
    // structurally valid proposal for its conflicting deterministic
    // execution result.
    let authenticated_conflicting_proposal =
        AuthenticatedConsensusMessage::for_test(conflicting_proposal_message.clone());
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(
            &authenticated_conflicting_proposal,
        ),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert!(matches!(
        adapter.authenticate(conflicting_proposal_message),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    let unbound_subject = subject(0x85);
    let mut unbound_qc_a = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: unbound_subject,
        execution_commitment: execution_commitment(0x85),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut unbound_qc_a, &keys);
    let mut unbound_qc_b = wire::QuorumCertificate {
        execution_commitment: execution_commitment(0x86),
        ..unbound_qc_a.clone()
    };
    authenticate_qc(&mut unbound_qc_b, &keys);
    let timeout_group = |highest_prepare_qc: wire::QuorumCertificate,
                         signers: Vec<wire::ValidatorIndex>| {
        let preimage = wire::TimeoutVote {
            round: timeout_round,
            highest_prepare_qc: Some(highest_prepare_qc.clone()),
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(highest_prepare_qc),
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate valid disjoint timeout group"),
        }
    };
    let mut conflicting_groups = vec![
        timeout_group(unbound_qc_a, vec![0, 1]),
        timeout_group(unbound_qc_b, vec![2, 3]),
    ];
    conflicting_groups.sort_by_key(|group| {
        group
            .highest_prepare_qc
            .as_ref()
            .map(wire::QuorumCertificate::as_ref)
    });
    let within_envelope_conflict = wire::TimeoutCertificate {
        round: timeout_round,
        groups: conflicting_groups,
    };
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(within_envelope_conflict),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert!(
        !adapter
            .registry
            .execution_commitments
            .keys()
            .any(|(_, registered_subject)| *registered_subject
                == reducer::Subject::new(Hash::new(unbound_subject.encode()).into())),
        "within-envelope checking cannot bind either attacker commitment"
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    // Transport adapters authenticate their outer request/response
    // identities separately. The same read-only compatibility walk still
    // covers every embedded certificate before a transport payload is
    // unwrapped into reducer ingress.
    let certified_request = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
            round,
            subject: bound_subject,
            certificate: conflicting_qc.clone(),
            requester: context.roster[0].validator.clone(),
            signature: vec![0x83; 96],
        }),
    ));
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(&certified_request),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let commit_response = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"commitment-conflict-request",
                )),
                certificate: wire::QuorumCertificate {
                    phase: wire::GlobalPhase::Commit,
                    ..conflicting_qc
                },
                responder: context.roster[1].validator.clone(),
                signature: vec![0x84; 96],
            },
        ),
    ));
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(&commit_response),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));

    assert_registry_eq(&adapter.registry, &retained_registry);
    assert_eq!(adapter.ingress_equivocations, retained_equivocations);
    assert_eq!(adapter.ingress_deliveries, retained_deliveries);
    assert_eq!(
        (
            adapter.deferred_completions.len(),
            adapter.deferred_progress_inputs.len(),
            adapter.deferred_inputs.len(),
        ),
        retained_queue_lengths
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);

    let mut canonical_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: canonical_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    canonical_vote.signature = Signature::new(
        keys[usize::try_from(canonical_vote.signer).expect("small signer")].private_key(),
        &canonical_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_vote),
        ))
        .expect("the exact canonical commitment remains authentically admissible");
    assert!(adapter.ingress_ready());
}

#[cfg(feature = "bls")]
#[test]
fn authenticated_ingress_verifies_individual_and_aggregate_bls() {
    let (context, keys, pops) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(12);
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
        .payload()
        .to_vec();
    verify_authenticated_message(
        &context,
        None,
        &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
        &pops,
    )
    .expect("verify individual vote");

    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&refs)
            .expect("aggregate BLS votes"),
    };
    verify_authenticated_message(
        &context,
        None,
        &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            certificate,
        )),
        &pops,
    )
    .expect("verify aggregate QC");
}

#[cfg(feature = "bls")]
#[test]
fn timeout_vote_installs_embedded_qc_before_forming_tc() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified_context =
        VerifiedHeightContext::genesis(context.clone(), pops).expect("verify context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("timeout-safety.wal"),
        verified_context,
        None,
        reducer::Generation::new(1),
        [0x33; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());

    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(13);
    let prepare_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(13),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let prepare_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &prepare_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(13),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
            .expect("aggregate PrepareQC"),
    };
    let protected_payload = [13, 2];
    let manifest = encode_payload(&context, round, subject, &protected_payload)
        .expect("encode protected-body payload")
        .manifest()
        .clone();
    let core_manifest = adapter
        .registry
        .manifest_to_core(&manifest, &context)
        .expect("register protected-body manifest");
    let core_round = reducer::Round::new(round.height, round.view);
    let core_subject = core_manifest.subject();
    let original_tag = adapter.current_tag();

    let mut all_effects = Vec::new();
    for signer in 0_u32..3 {
        if signer == 2 {
            adapter.deferred_completions.push_back(DeferredInput {
                admission_ordinal: 1,
                admission_capability: DeferredAdmissionCapability::for_test(1),
                event: reducer::Event::BodyAvailable {
                    tag: original_tag,
                    round: core_round,
                    subject: core_subject,
                },
                completion_evidence: Some(BodyPipelineCompletionEvidence::BodyAvailable {
                    manifest: manifest.clone(),
                }),
                retag_authenticated_ingress: false,
                priority: DeferredPriority::Completion,
                protected_progress: false,
                admission: None,
                authenticated_wire_identity: None,
                admitted_at: Instant::now(),
                eligible_skips: 0,
            });
        }
        let mut timeout = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer,
            signature: Vec::new(),
        };
        timeout.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer")].private_key(),
            &timeout.signature_preimage(),
        )
        .payload()
        .to_vec();
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(timeout),
            ))
            .expect("authenticate self-contained timeout vote");
        all_effects.push(
            adapter
                .receive_authenticated(authenticated)
                .expect("ingest timeout vote")
                .into_effects(),
        );
    }
    let final_effects = all_effects.pop().expect("three timeout outcomes");

    assert_eq!(adapter.reducer.durable_state().current_view(), 1);
    assert!(adapter.reducer.durable_state().highest_prepare().is_some());
    assert!(final_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
            ..
        })
    )));
    assert!(
        final_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::EnterView { .. }))
    );
    assert!(
        !final_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::StoreBody { .. })),
        "old-generation BodyAvailable must not cross EnterView before executor rebinding"
    );
    let (rebound_tag, protected_lock) = final_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                protected_lock,
                ..
            } => Some((*tag, protected_lock.as_ref())),
            _ => None,
        })
        .expect("view installation effect");
    assert_eq!(protected_lock, Some(&prepare));
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::BodyAvailable { tag, round, subject },
            ..
        }) if *tag == original_tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(
        adapter.rebind_deferred_body_available(original_tag, rebound_tag, &manifest),
        1
    );
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::BodyAvailable { tag, .. },
            ..
        }) if *tag == rebound_tag
    ));
    assert_eq!(
        adapter
            .retire_deferred_body_available(rebound_tag, &manifest)
            .expect("persist rebound completion retirement"),
        1
    );
    assert!(adapter.deferred_completions.is_empty());
}
