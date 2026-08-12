#[test]
fn scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    let lifecycle_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve one shared causal lifecycle");
    let root_command = FakeCommand::record(1);
    let mut causal_origin =
        RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &root_command, None);
    assert!(causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal));
    let causal_command = |class, command| {
        TaggedCommand::with_causal_origin(
            owner_tag,
            class,
            command,
            start,
            causal_origin.clone(),
            lifecycle_ordinal,
        )
        .expect("construct an exact causal sibling")
    };
    runtime
        .ingress
        .enqueue(causal_command(CommandClass::Normal, root_command))
        .expect("normal causal owner fits");
    runtime
        .ingress
        .enqueue(causal_command(
            CommandClass::Progress,
            FakeCommand::record(9),
        ))
        .expect("progress causal owner fits");

    assert!(matches!(runtime.step(start), Ok(RuntimeStep::Advanced(_))));
    let evidence = runtime
        .last_scheduler_ownership()
        .expect("FIFO dispatch retains exact scheduler ownership")
        .clone();
    assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Fifo);
    assert_eq!(evidence.round_tag, owner_tag);
    assert_eq!(evidence.queue_before.len, 2);
    assert_eq!(evidence.queue_after.len, 1);
    assert_eq!(
        evidence.queue_before.service_cursor,
        SERVICE_CLASS_COMPLETION
    );
    assert_eq!(evidence.queue_after.service_cursor, SERVICE_CLASS_NORMAL);
    assert_eq!(evidence.queue_before.max_service_debt, 0);
    assert_eq!(evidence.queue_after.max_service_debt, 1);
    assert!(evidence.live_mode);
    assert!(!evidence.timeout_due);
    assert!(!evidence.periodic_timer_due);
    assert!(evidence.fifo_ready);
    assert!(!evidence.completion_ready);
    assert!(evidence.progress_ready);
    assert!(evidence.normal_ready);
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &evidence.candidate else {
        panic!("FIFO dispatch must carry one exact command candidate");
    };
    assert_eq!(
        candidate.identity,
        FakeCommand::record(9)
            .exact_runtime_command_identity()
            .digest()
    );
    assert_eq!(candidate.kind, RuntimeCommandKind::Test);
    assert_eq!(candidate.class, SERVICE_CLASS_PROGRESS);
    assert_eq!(candidate.tag, owner_tag);
    assert_eq!(candidate.admission_ordinal, 3);
    assert_eq!(candidate.lifecycle_ordinal, lifecycle_ordinal);
    assert_eq!(
        candidate.causal_origin.root_lifecycle_ordinal,
        Some(lifecycle_ordinal)
    );
    assert_eq!(candidate.fifo_position, 1);
    assert_eq!(candidate.eligible_skips_before, 0);
    assert_eq!(candidate.eligible_skips_after, 0);
    assert_eq!(evidence.validate_exact(), Ok(()));

    let rejected = |mutated: RuntimeSchedulerOwnershipEvidence| {
        assert_eq!(
            mutated.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection)
        );
    };

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.identity.canonical_hash = iroha_crypto::Hash::new([0xFF]);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.identity = FakeCommand::record(42)
        .exact_runtime_command_identity()
        .digest();
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.kind = RuntimeCommandKind::Authenticated;
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.class = SERVICE_CLASS_NORMAL;
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.tag = tag(99);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.tag = tag(99);
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.admission_ordinal = 0;
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.admission_ordinal = 0;
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.lifecycle_ordinal = candidate
        .lifecycle_ordinal
        .checked_add(1)
        .expect("small test lifecycle rank has a successor");
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    let replacement_origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        candidate.tag,
        CommandClass::Progress,
        RuntimeFreshRootKind::StartupRecovery,
        b"coherently-rehashed-causal-root",
    );
    candidate.causal_origin =
        RuntimeLifecycleOwner::new(replacement_origin, candidate.lifecycle_ordinal)
            .expect("replacement causal root retains the same logical ordinal")
            .causal_origin()
            .clone();
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.admission_ordinal = candidate
        .lifecycle_ordinal
        .checked_sub(1)
        .expect("fresh FIFO lifecycle rank has a nonzero predecessor");
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.fifo_position = 0;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.queue_after.service_cursor = SERVICE_CLASS_COMPLETION;
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.queue_after.max_service_debt = evidence.queue_before.max_service_debt.saturating_add(2);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.queue_before.service_cursor = SERVICE_CLASS_NONE;
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.timeout_due = true;
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.progress_ready = false;
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence.clone();
    mutated.fifo_owed_after = true;
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);

    let mut mutated = evidence;
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
        unreachable!();
    };
    candidate.eligible_skips_before = 1;
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
    rejected(mutated);
}

#[test]
fn scheduler_minimum_uses_cached_admission_but_dispatch_revalidates_ingress() {
    let directory = TempDir::new().expect("temporary cached-admission directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0xA6),
        ));
    let source = PeerId::new(keys[0].public_key().clone());

    runtime
        .enqueue_network_with_ingress_ownership(
            message.clone(),
            fair_network_ownership(&message, source),
        )
        .expect("deeply validated authenticated command enters the runtime FIFO");
    let lifecycle_ordinal = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.lifecycle_ordinal)
        .expect("published command owns a lifecycle ordinal");

    // Model corruption after publication to prove the two validation
    // boundaries are distinct. A rank scan consumes only the immutable
    // cached admission certificate; dispatch still validates the full
    // ingress carrier and therefore must fail closed before removal.
    let queued = runtime
        .ingress
        .commands
        .front_mut()
        .expect("published command remains queued");
    queued
        .ingress_ownership
        .as_mut()
        .expect("authenticated command retains ingress ownership")
        .projection_hash = Hash::new(b"invalid retained ingress projection");
    assert!(!queued.validate_admission_identity());
    assert_eq!(
        runtime.ingress.oldest_lifecycle_ordinal(),
        Ok(Some(lifecycle_ordinal)),
        "scheduler rank scans must not repeat deep envelope validation"
    );
    assert!(matches!(
        runtime.ingress.pop_next_with_ownership(),
        Err(EnqueueError::FailClosed)
    ));
    assert_eq!(
        runtime.ingress.commands.len(),
        1,
        "dispatch rejects corrupted ingress before consuming the cached owner"
    );
}

#[test]
fn scheduler_queue_seal_rejects_valid_same_wire_ingress_carrier_substitution() {
    let directory = TempDir::new().expect("temporary scheduler-ingress-seal directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before authenticated scheduler selection");
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0xA7),
        ));
    let original_source = PeerId::new(keys[0].public_key().clone());
    let replacement_source = PeerId::new(keys[1].public_key().clone());
    let replacement_ingress = RuntimeIngressOwnershipEvidence::from_fair_ingress(
        &message,
        fair_network_ownership(&message, replacement_source),
    )
    .expect("independent same-wire carrier has exact runtime ownership");
    assert!(replacement_ingress.validate_frozen_physical());

    runtime
        .enqueue_network_with_ingress_ownership(
            message.clone(),
            fair_network_ownership(&message, original_source),
        )
        .expect("original authenticated carrier enters the runtime FIFO");
    assert!(matches!(runtime.step(now), Ok(RuntimeStep::Advanced(_))));
    let evidence = runtime
        .last_scheduler_ownership()
        .expect("authenticated FIFO selection retains exact scheduler ownership")
        .clone();
    assert_eq!(evidence.validate_exact(), Ok(()));
    let RuntimeSelectedCandidateOwnership::Exact(original) = &evidence.candidate else {
        panic!("authenticated FIFO dispatch must retain one exact candidate")
    };
    let original_ingress = original
        .ingress_ownership
        .as_ref()
        .expect("authenticated candidate retains its full ingress carrier");
    assert_ne!(
        replacement_ingress.projection_hash, original_ingress.projection_hash,
        "independent sources have distinct complete ownership projections"
    );
    assert_eq!(
        runtime_ingress_causal_origin_projection_hash(&replacement_ingress),
        runtime_ingress_causal_origin_projection_hash(original_ingress),
        "equal aggregate certificates retain one route-neutral logical identity"
    );
    assert_eq!(
        replacement_ingress.earliest_physical_carrier(),
        original_ingress.earliest_physical_carrier(),
        "the independent test queues deliberately assign the same valid physical shape"
    );
    assert_eq!(
        replacement_ingress.earliest_lifecycle_ordinal(),
        original_ingress.earliest_lifecycle_ordinal(),
        "the replacement is rank-compatible before the private selection check"
    );

    let mut substituted = evidence;
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut substituted.candidate else {
        unreachable!();
    };
    candidate.ingress_ownership = Some(replacement_ingress);
    assert!(runtime_fifo_candidate_ingress_is_exact(candidate));
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    substituted.projection_hash = runtime_scheduler_projection_hash(&substituted);
    assert_eq!(
        substituted.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "the queue-private seal rejects a valid same-wire full-carrier substitution after every public projection is recomputed"
    );
}

#[test]
fn full_lane_retryable_backpressure_preserves_owner_across_class_fairness() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    assert!(driver.retry_once.insert(2));
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(4, 1, 1));
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .expect("oldest retryable owner fits");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(2),
    )
    .expect("later completion owner fits");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Progress,
        FakeCommand::record(3),
    )
    .expect("later progress owner fills the lane");
    assert_eq!(runtime.ingress.remaining_capacity(), 0);
    let original = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| queued.command.record == Some(2))
        .expect("selected Completion owner is present")
        .clone();

    assert!(matches!(
        runtime.step(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    let evidence = runtime
        .last_scheduler_ownership()
        .expect("retry turn retains typed scheduler ownership")
        .clone();
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::FifoRetryRetained
    );
    assert_eq!(evidence.queue_before.len, 3);
    assert_eq!(evidence.queue_after.len, 3);
    assert_eq!(evidence.validate_exact(), Ok(()));
    let restored = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| queued.command.record == Some(2))
        .expect("retry restores the original Completion owner");
    assert_eq!(restored.tag, original.tag);
    assert_eq!(restored.class, original.class);
    assert_eq!(restored.identity, original.identity);
    assert_eq!(restored.admission_ordinal, original.admission_ordinal);
    assert_eq!(restored.lifecycle_ordinal, original.lifecycle_ordinal);
    assert_eq!(restored.causal_origin, original.causal_origin);
    assert_eq!(runtime.driver.delivered, Vec::new());

    let mut weakened = evidence.clone();
    weakened.selected = RuntimeSelectedOwnerKind::Fifo;
    weakened.projection_hash = runtime_scheduler_projection_hash(&weakened);
    assert_eq!(
        weakened.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "an equal-length retry cannot be relabelled as completed FIFO service"
    );
    assert!(runtime.take_last_scheduler_ownership().is_some());
    assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 3)]);
    assert_eq!(runtime.ingress.len(), 2);
    runtime
        .step_and_take_scheduler_ownership_for_test(start)
        .expect("Normal class receives its bounded turn");
    runtime
        .step_and_take_scheduler_ownership_for_test(start)
        .expect("restored Completion owner is retried without replacement");
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 3), (owner_tag, 1), (owner_tag, 2)]
    );
    assert_eq!(runtime.ingress.len(), 0);
}

#[test]
fn retry_unadmitted_predecessor_gets_one_bounded_serve_attempt() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    assert!(driver.retry_once.insert(7));
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(4, 1, 1));
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(7),
    )
    .expect("older predecessor fits");
    let serve_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("external Serve ticket shares the actor ordinal source");

    let first_witness = runtime
        .exact_serve_predecessor_episode_witness(start, serve_ordinal, None)
        .expect("older runnable predecessor is visible")
        .expect("older prefix issues one runtime witness");
    assert_eq!(first_witness.serve_lifecycle_ordinal(), serve_ordinal);
    assert_eq!(first_witness.episode(), 1);
    let retained_response_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("retained response target shares the actor ordinal source");
    assert!(
        runtime
            .older_lifecycle_predates_retained_response(start, retained_response_ordinal)
            .expect("alternate retained-response target sees the same older owner")
    );
    assert_eq!(
        runtime
            .exact_serve_predecessor_episode_witness(start, serve_ordinal, None)
            .expect("alternate target cannot reset selected-Serve witness state"),
        Some(first_witness),
        "selected Serve retains one monotone witness across the legacy target probe"
    );
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.queued_commands(), 1);
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(start, serve_ordinal)
            .expect("retryable pressure cannot become a Serve barrier")
    );
    assert_eq!(runtime.exact_serve_target_ordinal, Some(serve_ordinal));
    assert!(runtime.exact_serve_predecessor_retry_attempted);
    assert_eq!(
        runtime.retained_response_predecessor_target_ordinal,
        Some(retained_response_ordinal)
    );
    assert!(runtime.retained_response_predecessor_retry_attempted);
    assert!(runtime.exact_serve_predecessor_physically_present);
    assert_eq!(runtime.exact_serve_predecessor_episode, 1);
    assert_eq!(runtime.exact_serve_predecessor_witness, Some(first_witness));
    assert!(
        runtime
            .exact_serve_predecessor_episode_witness(start, serve_ordinal, None)
            .expect("the restored predecessor remains a suppressed physical owner")
            .is_none(),
        "retry polling cannot mint a second predecessor episode"
    );
    assert_eq!(runtime.exact_serve_predecessor_episode, 1);
    assert!(
        !runtime
            .older_lifecycle_predates_retained_response(start, retained_response_ordinal)
            .expect("alternate target also suppresses the one attempted owner"),
        "one retry attempt is shared across both exact target comparisons"
    );

    runtime
        .step_and_take_scheduler_ownership_for_test(start)
        .expect("the restored owner remains available after Serve settlement");
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 7)]);
    assert!(
        runtime
            .exact_serve_predecessor_episode_witness(start, serve_ordinal, None)
            .expect("settled retry clears its physical-presence latch")
            .is_none()
    );
    assert!(!runtime.exact_serve_predecessor_retry_attempted);
    assert!(!runtime.exact_serve_predecessor_physically_present);
    assert!(
        !runtime
            .older_lifecycle_predates_retained_response(start, retained_response_ordinal)
            .expect("settled owner clears alternate-target retry suppression")
    );
    assert!(!runtime.retained_response_predecessor_retry_attempted);

    let completed_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("completed service owner shares the actor ordinal source");
    let completed_evidence = ExactServePredecessorCompletionEvidence::try_new(completed_ordinal)
        .expect("completed service evidence is nonzero and exact");
    let completed_target = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("new Serve target follows the completed service owner");
    assert!(
        runtime
            .exact_serve_predecessor_episode_witness(start, completed_target, None)
            .expect("passive ownership alone remains absent")
            .is_none()
    );
    let completed_witness = runtime
        .exact_serve_predecessor_episode_witness(start, completed_target, Some(completed_evidence))
        .expect("completion-qualified owner is accepted")
        .expect("completion-qualified owner opens one predecessor episode");
    assert_eq!(
        completed_witness.predecessor_lifecycle_ordinal(),
        completed_ordinal
    );
    assert_eq!(completed_witness.episode(), 1);
    assert_eq!(
        runtime
            .exact_serve_predecessor_episode_witness(
                start,
                completed_target,
                Some(completed_evidence),
            )
            .expect("repeated completion evidence remains stable"),
        Some(completed_witness)
    );
    assert!(
        runtime
            .exact_serve_predecessor_episode_witness(start, completed_target, None)
            .expect("consumed completion evidence closes its episode")
            .is_none()
    );
}

#[test]
fn typed_pacemaker_escape_selects_only_progress_root() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(4, 1, 1),
    );
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .expect("ordinary owner fits");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Progress,
        FakeCommand::record(2),
    )
    .expect("Progress owner fits");

    assert!(matches!(
        runtime.try_step_pacemaker_escape(start),
        Ok(Some(RuntimeStep::Advanced(ref effects))) if effects.len() == 1
    ));
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("typed Progress turn publishes exact scheduler ownership");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert_eq!(evidence.validate_exact(), Ok(()));
    runtime
        .take_effect_ownership(1)
        .expect("consume the Progress effect owner");
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 2)]);
    assert_eq!(runtime.queued_commands(), 1);

    assert!(matches!(runtime.try_step_pacemaker_escape(start), Ok(None)));
    assert_eq!(runtime.queued_commands(), 1);
    runtime
        .step_and_take_scheduler_ownership_for_test(start)
        .expect("ordinary owner remains exact for the normal scheduler");
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 2), (owner_tag, 1)]
    );
}

#[test]
fn missing_nonempty_effect_ownership_latches_runtime_fail_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(4, 1, 1),
    );

    assert_eq!(
        runtime.take_effect_ownership(1),
        Err("Sumeragi v2 effect batch omitted its lifecycle ownership".to_owned()),
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        ),
        Err(EnqueueError::FailClosed),
        "missing runtime ownership permanently closes later ingress",
    );
}

#[test]
fn retryable_backpressure_restores_the_exact_recovery_fifo_owner_once() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    assert!(driver.retry_once.insert(7));
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        driver,
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(4, 1, 1),
        Vec::new(),
    )
    .expect("construct unarmed recovery runtime");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(7),
    )
    .expect("recovery owner fits");
    let original_owner = runtime
        .ingress
        .commands
        .front()
        .expect("recovery owner is present")
        .lifecycle_owner()
        .expect("recovery owner is exact");

    assert!(matches!(
        runtime.step_recovery(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    let evidence = runtime
        .last_scheduler_ownership()
        .expect("retrying recovery retains scheduler ownership");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained
    );
    assert_eq!(evidence.queue_before.len, evidence.queue_after.len);
    assert_eq!(evidence.validate_exact(), Ok(()));
    assert_eq!(
        runtime
            .ingress
            .commands
            .front()
            .expect("recovery retry remains physically admitted")
            .lifecycle_owner()
            .expect("restored recovery owner is exact"),
        original_owner
    );
    assert!(runtime.take_last_scheduler_ownership().is_some());
    assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

    assert!(matches!(
        runtime.step_recovery_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 7)]);
    assert_eq!(runtime.queued_commands(), 0);
}

#[test]
fn adapter_command_identity_is_derived_from_exact_immutable_payload() {
    let owner_tag = tag(4);
    let command = AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x33]);
    let expected = command.exact_runtime_command_identity();
    let shared = expected.clone();
    assert!(Arc::ptr_eq(
        &expected.canonical_bytes,
        &shared.canonical_bytes
    ));
    assert_ne!(
        expected,
        AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x34]).exact_runtime_command_identity()
    );

    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));
    ingress
        .enqueue(TaggedCommand::new(
            owner_tag,
            CommandClass::Completion,
            command,
            Instant::now(),
        ))
        .expect("exact adapter command fits completion capacity");
    let (_, candidate) = ingress
        .pop_next_with_ownership()
        .expect("adapter command retains its admission ordinal")
        .expect("adapter command owns the selected FIFO occurrence");
    assert_eq!(candidate.identity, expected.digest());
    assert_eq!(candidate.kind, RuntimeCommandKind::SignatureCompleted);
    assert_eq!(candidate.class, SERVICE_CLASS_COMPLETION);
    assert_eq!(candidate.tag, owner_tag);
    assert_eq!(candidate.admission_ordinal, 1);
    assert_eq!(candidate.fifo_position, 0);
}

#[test]
fn scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches() {
    let start = Instant::now();
    let owner_tag = tag(0);

    let mut idle = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    assert!(matches!(idle.step(start), Ok(RuntimeStep::Idle)));
    assert_eq!(
        idle.last_scheduler_ownership()
            .map(|evidence| evidence.selected),
        Some(RuntimeSelectedOwnerKind::Idle)
    );
    let mut nonempty_debt_on_empty_queue = idle
        .last_scheduler_ownership()
        .expect("idle branch retains its empty queue projection")
        .clone();
    nonempty_debt_on_empty_queue.queue_before.max_service_debt = 1;
    nonempty_debt_on_empty_queue.projection_hash =
        runtime_scheduler_projection_hash(&nonempty_debt_on_empty_queue);
    assert_eq!(
        nonempty_debt_on_empty_queue.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "a coherently rehashed empty queue cannot claim service debt"
    );
    assert!(idle.take_last_scheduler_ownership().is_some());

    assert!(matches!(
        idle.step(start + Duration::from_secs(2)),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(
        idle.last_scheduler_ownership()
            .map(|evidence| evidence.selected),
        Some(RuntimeSelectedOwnerKind::PeriodicTimer)
    );
    assert!(idle.take_last_scheduler_ownership().is_some());
    assert!(matches!(
        idle.step(start + Duration::from_secs(10)),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(
        idle.last_scheduler_ownership()
            .map(|evidence| evidence.selected),
        Some(RuntimeSelectedOwnerKind::Timeout)
    );

    let (mut recovery, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(owner_tag),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(6, 2, 1),
        Vec::new(),
    )
    .expect("construct unarmed recovery runtime");
    enqueue_fake(
        &mut recovery,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(7),
    )
    .expect("recovery FIFO owner fits");
    assert!(matches!(
        recovery.step_recovery(start),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(
        recovery
            .last_scheduler_ownership()
            .map(|evidence| evidence.selected),
        Some(RuntimeSelectedOwnerKind::RecoveryFifo)
    );
    assert_eq!(
        recovery
            .last_scheduler_ownership()
            .expect("recovery FIFO retains evidence")
            .validate_exact(),
        Ok(())
    );
    assert!(
        !recovery
            .last_scheduler_ownership()
            .expect("recovery FIFO retains evidence")
            .live_mode
    );
    assert!(recovery.take_last_scheduler_ownership().is_some());
    recovery
        .take_effect_ownership(1)
        .expect("the recovery executor consumes the delivered effect owner");
    assert!(matches!(
        recovery.step_recovery(start),
        Ok(RuntimeStep::Idle)
    ));
    assert_eq!(
        recovery
            .last_scheduler_ownership()
            .map(|evidence| evidence.selected),
        Some(RuntimeSelectedOwnerKind::RecoveryIdle)
    );
    assert_eq!(
        recovery
            .last_scheduler_ownership()
            .expect("recovery idle retains evidence")
            .validate_exact(),
        Ok(())
    );

    let mut deferred_driver = FakeDriver::new(owner_tag);
    deferred_driver
        .deferred_effects
        .push_back(vec![FakeEffect::other()]);
    let mut deferred = runtime(deferred_driver, start, RuntimeQueueConfig::new(6, 2, 1));
    assert!(matches!(deferred.step(start), Ok(RuntimeStep::Advanced(_))));
    let evidence = deferred
        .last_scheduler_ownership()
        .expect("deferred dispatch retains its typed occurrence");
    assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Deferred);
    assert_eq!(evidence.validate_exact(), Ok(()));
    assert!(matches!(
        &evidence.candidate,
        RuntimeSelectedCandidateOwnership::ExactDeferred(candidate)
            if candidate.service.admission_ordinal == 0
                && candidate.lifecycle_ownership.owner.lifecycle_ordinal() == 1
                && candidate.service.validate_exact()
                && candidate.ingress_ownership.is_none()
    ));

    let mut unavailable_driver = FakeDriver::new(owner_tag);
    unavailable_driver.deferred_identity_unavailable = true;
    unavailable_driver
        .deferred_effects
        .push_back(vec![FakeEffect::other()]);
    let mut unavailable = runtime(unavailable_driver, start, RuntimeQueueConfig::new(6, 2, 1));
    assert!(matches!(
        unavailable.step(start),
        Err(RuntimeError::FailClosed)
    ));
    assert!(unavailable.last_scheduler_ownership().is_none());
}

#[test]
fn runtime_rejects_replayed_foreign_and_mutated_deferred_tokens() {
    let start = Instant::now();
    let owner_tag = tag(0);

    let mut replay_driver = FakeDriver::new(owner_tag);
    replay_driver
        .deferred_effects
        .push_back(vec![FakeEffect::other()]);
    replay_driver
        .deferred_effects
        .push_back(vec![FakeEffect::other()]);
    let replayed = DeferredServiceEvidence::completion_for_test(
        &replay_driver.deferred_admission_ordinals,
        owner_tag,
        2,
        DeferredPriority::Completion,
    );
    assert!(replayed.claim_adapter_service_for_test());
    replay_driver
        .deferred_evidence_overrides
        .push_back(replayed.clone());
    replay_driver
        .deferred_evidence_overrides
        .push_back(replayed);
    let mut replay = runtime(replay_driver, start, RuntimeQueueConfig::new(6, 2, 1));
    assert!(matches!(replay.step(start), Ok(RuntimeStep::Advanced(_))));
    assert!(replay.take_last_scheduler_ownership().is_some());
    assert!(matches!(replay.step(start), Err(RuntimeError::FailClosed)));

    let mut foreign_driver = FakeDriver::new(owner_tag);
    foreign_driver
        .deferred_effects
        .push_back(vec![FakeEffect::other()]);
    let foreign_source = DeferredAdmissionOrdinalSource::new(0);
    let foreign_evidence = DeferredServiceEvidence::completion_for_test(
        &foreign_source,
        owner_tag,
        1,
        DeferredPriority::Completion,
    );
    assert!(foreign_evidence.claim_adapter_service_for_test());
    foreign_driver
        .deferred_evidence_overrides
        .push_back(foreign_evidence);
    let mut foreign = runtime(foreign_driver, start, RuntimeQueueConfig::new(6, 2, 1));
    assert!(matches!(foreign.step(start), Err(RuntimeError::FailClosed)));

    let mut mutated_driver = FakeDriver::new(owner_tag);
    mutated_driver
        .deferred_effects
        .push_back(vec![FakeEffect::other()]);
    let mut mutated = DeferredServiceEvidence::completion_for_test(
        &mutated_driver.deferred_admission_ordinals,
        owner_tag,
        1,
        DeferredPriority::Completion,
    );
    assert!(mutated.claim_adapter_service_for_test());
    mutated.protected_progress = true;
    mutated_driver
        .deferred_evidence_overrides
        .push_back(mutated);
    let mut mutated = runtime(mutated_driver, start, RuntimeQueueConfig::new(6, 2, 1));
    assert!(matches!(mutated.step(start), Err(RuntimeError::FailClosed)));
}

#[test]
fn runtime_rejects_driver_selection_outside_eligible_deferred_owner_set() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    driver.deferred_effects.push_back(vec![FakeEffect::other()]);
    let ineligible = DeferredServiceEvidence::completion_for_test(
        &driver.deferred_admission_ordinals,
        owner_tag,
        1,
        DeferredPriority::Completion,
    );
    assert_eq!(ineligible.admission_ordinal, 0);
    assert!(ineligible.claim_adapter_service_for_test());
    driver.deferred_evidence_overrides.push_back(ineligible);
    driver.deferred_active_ordinals.insert(1);

    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
    let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        owner_tag,
        CommandClass::Progress,
        RuntimeFreshRootKind::StartupRecovery,
        b"eligible-deferred-owner",
    );
    let owner = RuntimeLifecycleOwner::new(origin, 1)
        .expect("test target owns the global minimum lifecycle rank");
    let ownership = deferred_lifecycle_ownership_for_test(
        owner,
        1,
        RuntimeDispatchIngress::LocalOrCausal,
        None,
        runtime.ingress_physical_cut,
    )
    .expect("test target retains an exact runtime wrapper");
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(1, ownership)
            .is_none()
    );
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("the active target has one exact eligible owner"),
        BTreeSet::from([1])
    );

    assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
    assert_eq!(
        runtime.fail_closed_reason.as_deref(),
        Some("deferred driver selected an ineligible admission owner")
    );
}

#[test]
fn runtime_rejects_two_deferred_occurrences_for_one_logical_lifecycle() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    driver.deferred_effects.push_back(vec![FakeEffect::other()]);
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
    let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        owner_tag,
        CommandClass::Progress,
        RuntimeFreshRootKind::StartupRecovery,
        b"duplicate-deferred-logical-owner",
    );
    let owner = RuntimeLifecycleOwner::new(origin, 1)
        .expect("duplicate fixture owns one exact logical lifecycle");
    let physical_cut = runtime.ingress_physical_cut;
    let (first, second) = {
        let source = runtime.driver.deferred_admission_ordinal_source();
        let make = || {
            let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                source,
                owner.causal_origin().lifecycle_key.clone(),
                owner.lifecycle_ordinal(),
                false,
                None,
                physical_cut,
            );
            let ordinal = runtime_seal.admission_ordinal();
            let ownership = RuntimeDeferredLifecycleOwnership::new(
                owner.clone(),
                ordinal,
                RuntimeDispatchIngress::LocalOrCausal,
                None,
                physical_cut,
                runtime_seal,
            )
            .expect("each duplicate wrapper is independently well formed");
            (ordinal, ownership)
        };
        (make(), make())
    };
    for (ordinal, ownership) in [first, second] {
        runtime.driver.deferred_active_ordinals.insert(ordinal);
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(ordinal, ownership)
                .is_none()
        );
    }

    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
    assert_eq!(runtime.driver.deferred_dispatches, 0);
    assert_eq!(
        runtime.fail_closed_reason.as_deref(),
        Some("deferred physical-cut lifecycle ownership was invalid")
    );
}

#[test]
fn scheduler_owner_must_be_taken_before_a_later_step_can_enter() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut blocked_runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );

    assert!(matches!(blocked_runtime.step(start), Ok(RuntimeStep::Idle)));
    let first_projection_hash = blocked_runtime
        .last_scheduler_ownership()
        .expect("first idle selection retains a carrier")
        .projection_hash;

    let periodic_at = start + blocked_runtime.retransmit_interval();
    assert!(matches!(
        blocked_runtime.step(periodic_at),
        Err(RuntimeError::FailClosed)
    ));
    assert_eq!(
        blocked_runtime.fail_closed_reason.as_deref(),
        Some("live scheduling began with an unconsumed scheduler owner")
    );
    blocked_runtime.latch_fail_closed("a later generic failure");
    assert_eq!(
        blocked_runtime.fail_closed_reason.as_deref(),
        Some("live scheduling began with an unconsumed scheduler owner"),
        "fail-closed diagnostics retain the first invariant violation"
    );
    let retained = blocked_runtime
        .last_scheduler_ownership()
        .expect("failed re-entry preserves the first unconsumed carrier");
    assert_eq!(retained.selected, RuntimeSelectedOwnerKind::Idle);
    assert_eq!(retained.projection_hash, first_projection_hash);

    let mut runtime = self::runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));

    let taken = runtime
        .take_last_scheduler_ownership()
        .expect("effect boundary takes the exact first occurrence");
    assert_eq!(taken.selected, RuntimeSelectedOwnerKind::Idle);
    assert_eq!(taken.validate_exact(), Ok(()));
    assert!(runtime.last_scheduler_ownership().is_none());

    assert!(matches!(
        runtime.step(periodic_at),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .map(|evidence| evidence.selected),
        Some(RuntimeSelectedOwnerKind::PeriodicTimer)
    );
    assert!(runtime.last_scheduler_ownership().is_none());
}

#[test]
fn checked_admission_reservation_rejection_preserves_and_reuses_the_owner() {
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(40);
    let rejected: Result<(), EnqueueError> =
        source.with_checked_reservation(1, |first, successor| {
            assert_eq!(first, 41);
            assert_eq!(successor, 42);
            Err(EnqueueError::FailClosed)
        });
    assert_eq!(rejected, Err(EnqueueError::FailClosed));
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after rejected checked reservation"),
        Some(41),
        "a rejected checked admission cannot burn its prospective owner"
    );

    let admitted = source
        .with_checked_reservation(1, |first, successor| Ok((first, successor)))
        .expect("retry commits the same prospective owner");
    assert_eq!(admitted, (41, 42));
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after committed retry"),
        Some(42)
    );
}

#[test]
fn checked_ingress_rejection_preserves_dormant_owner_until_exact_retry() {
    let owner_tag = tag(0);
    let lifecycle_key = Hash::new(b"checked rejection dormant owner");
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let mut ingress =
        BoundedIngress::with_lifecycle_ordinals(RuntimeQueueConfig::new(4, 1, 1), source.clone());
    let dormant = RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 9);
    ingress
        .install_dormant_local_fifo_reservations(vec![dormant.clone()])
        .expect("install one exact restart-dormant owner");
    let mirror_before = ingress.next_admission_ordinal;

    let rejected: Result<(), EnqueueError> =
        ingress.with_checked_admission_ordinal_range(1, |checked_ingress, first, successor| {
            assert_eq!((first, successor), (2, 3));
            assert!(
                checked_ingress
                    .dormant_local_fifo_reservations
                    .contains(&dormant)
            );
            Err(EnqueueError::FailClosed)
        });
    assert_eq!(rejected, Err(EnqueueError::FailClosed));
    assert_eq!(ingress.next_admission_ordinal, mirror_before);
    assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
    assert!(ingress.commands.is_empty());
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after rejected dormant replacement"),
        Some(2)
    );

    ingress
        .enqueue(restored_fake_command(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(1),
            lifecycle_key,
            1,
            9,
        ))
        .expect("exact retry reuses and commits the rejected prospective ordinal");
    assert!(ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(ingress.commands.len(), 1);
    assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
    assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
    assert_eq!(ingress.next_admission_ordinal, Some(3));
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after exact dormant retry"),
        Some(3)
    );
}

#[test]
fn checked_admission_reservation_exhaustion_never_enters_commit() {
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 1);
    let commit_called = std::cell::Cell::new(false);
    for _ in 0..2 {
        let result: Result<(), EnqueueError> = source.with_checked_reservation(1, |_, _| {
            commit_called.set(true);
            Ok(())
        });
        assert_eq!(result, Err(EnqueueError::FailClosed));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect exhausted checked source"),
            Some(u128::MAX),
            "exhaustion and retry must preserve the last prospective value"
        );
    }
    assert!(!commit_called.get());
}

#[test]
fn admission_ordinal_exhaustion_fails_runtime_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    runtime.ingress.lifecycle_ordinals =
        RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 2);
    runtime.ingress.next_admission_ordinal = Some(u128::MAX - 1);
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .expect("the last ordinal with a representable successor is valid");
    assert_eq!(
        runtime.ingress.commands[0].admission_ordinal,
        Some(u128::MAX - 1)
    );
    let next_before_rejection = runtime.ingress.next_admission_ordinal;
    let source_before_rejection = runtime
        .ingress
        .lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect source before exhausted FIFO admission");
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(2),
        ),
        Err(EnqueueError::FailClosed)
    );
    assert!(runtime.fail_closed);
    assert_eq!(runtime.ingress.commands.len(), 1);
    assert_eq!(
        runtime.ingress.next_admission_ordinal,
        next_before_rejection
    );
    assert_eq!(
        runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after exhausted FIFO admission"),
        source_before_rejection,
        "failed FIFO admission cannot advance either ordinal representation"
    );
}

#[test]
fn selected_owner_without_a_runtime_minted_ordinal_fails_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    runtime.ingress.commands.push_back(TaggedCommand::new(
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
        start,
    ));

    assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
    assert!(runtime.fail_closed);
    assert!(runtime.last_scheduler_ownership().is_none());
}

#[test]
fn corrupt_cached_identity_and_rebound_origin_are_rejected_before_service() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
    let mut corrupt = TaggedCommand::new(
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
        admitted_at,
    );
    corrupt.identity.canonical_hash = iroha_crypto::Hash::new(b"corrupt cached identity");
    assert_eq!(ingress.enqueue(corrupt), Err(EnqueueError::FailClosed));
    assert!(ingress.commands.is_empty());

    let root = FakeCommand::record(2);
    let mut origin =
        RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &root, None);
    assert!(origin.bind_lifecycle_ordinal(7));
    assert!(matches!(
        TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(3),
            admitted_at,
            origin,
            8,
        ),
        Err(EnqueueError::FailClosed)
    ));
}

#[test]
fn lifecycle_owner_constructor_rejects_a_conflicting_prebound_ordinal() {
    let owner_tag = tag(0);
    let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        owner_tag,
        CommandClass::Progress,
        RuntimeFreshRootKind::HistoricalLockedRetransmit,
        b"prebound-owner",
    );
    assert!(origin.bind_lifecycle_ordinal(7));
    assert!(matches!(
        RuntimeLifecycleOwner::new(origin.clone(), 8),
        Err(EnqueueError::FailClosed)
    ));
    let exact = RuntimeLifecycleOwner::new(origin, 7)
        .expect("the already-bound exact ordinal remains admissible");
    assert!(exact.validate_exact());
    assert_eq!(exact.lifecycle_ordinal(), 7);
}

#[test]
fn runtime_physical_cut_is_monotone_and_regression_fails_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    assert_eq!(runtime.ingress_physical_cut, 1);
    runtime
        .set_ingress_physical_cut(4)
        .expect("receiver high-watermark advances");
    runtime
        .set_ingress_physical_cut(4)
        .expect("publishing the same high-watermark is idempotent");
    assert_eq!(runtime.ingress_physical_cut, 4);
    assert!(runtime.set_ingress_physical_cut(3).is_err());
    assert!(runtime.fail_closed);
    assert_eq!(runtime.ingress_physical_cut, 4);
}

#[test]
fn deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences() {
    let directory = TempDir::new().expect("temporary physical-cut runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0x5A);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
        unreachable!("signed runtime proposal fixture carries Proposal")
    };
    let semantic_origin = context.roster
        [usize::try_from(proposal.proposer).expect("small fixture proposer")]
    .validator
    .clone();
    let (_owner_directory, _owner_ingress, mut ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), semantic_origin)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let pre_cut_fair = ownerships
        .pop()
        .expect("one productive leader-wire ownership carrier");
    let predecessor_ordinal = pre_cut_fair
        .runtime_lifecycle_ordinal()
        .expect("leader-wire carrier has an immutable logical ordinal");
    let target_cut = pre_cut_fair
        .runtime_physical_cut()
        .expect("checked dequeue freezes the target predecessor cut");
    assert!(
        u128::from(
            pre_cut_fair
                .physical_admission_ordinal()
                .expect("leader-wire carrier has a physical occurrence")
        ) < target_cut
    );

    let target_owner = runtime
        .mint_fresh_lifecycle_owner(
            runtime.round_tag(),
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"already-admitted deferred continuation",
        )
        .expect("mint target lifecycle after the leader-wire predecessor");
    assert!(predecessor_ordinal < target_owner.lifecycle_ordinal());
    let target = deferred_lifecycle_ownership_for_test(
        target_owner.clone(),
        7,
        RuntimeDispatchIngress::LocalOrCausal,
        None,
        target_cut,
    )
    .expect("freeze the target physical cut exactly once");
    assert!(matches!(
        deferred_lifecycle_ownership_for_test(
            target_owner.clone(),
            7,
            RuntimeDispatchIngress::LocalOrCausal,
            Some(u64::try_from(target_cut).expect("small target cut")),
            target_cut,
        ),
        Err(EnqueueError::FailClosed)
    ));
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(7, target.clone())
            .is_none()
    );
    let foreign_source = DeferredAdmissionOrdinalSource::new(7);
    let mut foreign_target = target.clone();
    foreign_target.runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
        &foreign_source,
        foreign_target.owner.causal_origin().lifecycle_key.clone(),
        foreign_target.owner.lifecycle_ordinal(),
        false,
        None,
        foreign_target.physical_cut,
    );
    assert!(
        foreign_target.validate_exact(),
        "the foreign capability can be internally self-consistent"
    );
    assert!(
        !foreign_target.validate_active_against_ingress(
            None,
            runtime.driver.deferred_admission_ordinal_source(),
        ),
        "a same-number capability minted by another source cannot own this runtime"
    );

    let make_command = |runtime: &SerializedV2Runtime<SumeragiV2Adapter>,
                        fair: FairV2IngressOwnershipEvidence| {
        let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, fair)
            .expect("project exact leader-wire ownership into runtime");
        let authenticated = runtime
            .driver
            .authenticate(message.clone())
            .expect("authenticate the exact leader-wire proposal");
        TaggedCommand::with_ingress_ownership(
            runtime.round_tag(),
            CommandClass::Normal,
            AdapterCommand::Authenticated(authenticated),
            Instant::now(),
            ownership,
        )
    };

    let pre_cut_command = make_command(&runtime, pre_cut_fair.clone());
    runtime
        .ingress
        .enqueue(pre_cut_command)
        .expect("enqueue the real pre-cut predecessor");
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("pre-cut minimum is exact"),
        Some(predecessor_ordinal),
        "a physical predecessor with an older logical identity still blocks"
    );

    runtime.ingress.commands.clear();
    let mut post_cut_fair = pre_cut_fair;
    let post_cut_ordinal = u64::try_from(target_cut).expect("small receiver-local physical cut");
    post_cut_fair.first.physical_admission_ordinal = post_cut_ordinal;
    post_cut_fair.latest.physical_admission_ordinal = post_cut_ordinal;
    post_cut_fair.runtime_physical_cut = target_cut.checked_add(1);
    assert!(
        post_cut_fair.validate_exact(),
        "the replay retains its exact logical identity at a fresh physical occurrence"
    );
    let periodic_replay_fair = post_cut_fair.clone();
    let post_cut_command = make_command(&runtime, post_cut_fair);
    runtime
        .ingress
        .enqueue(post_cut_command)
        .expect("enqueue the exact post-cut replay");
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("post-cut minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "a post-cut replay cannot resurrect its obsolete logical queue position"
    );

    let replay_owner = runtime
        .ingress
        .commands
        .front()
        .expect("post-cut replay remains physically queued")
        .lifecycle_owner()
        .expect("post-cut replay retains its old logical owner");
    let replay_ingress = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.clone())
        .expect("post-cut replay retains its exact ingress carrier");
    runtime.ingress.commands.clear();
    let causal_completion = TaggedCommand::with_causal_origin(
        runtime.round_tag(),
        CommandClass::Completion,
        AdapterCommand::ApplicationCompleted(proposal.subject),
        Instant::now(),
        replay_owner.causal_origin().clone(),
        replay_owner.lifecycle_ordinal(),
    )
    .expect("construct a local completion inheriting the replay root");
    runtime
        .ingress
        .enqueue(causal_completion)
        .expect("enqueue the post-cut causal completion");
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("post-cut causal FIFO minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "dropping the current envelope cannot drop the causal root's physical position"
    );
    runtime.ingress.commands.clear();
    runtime.pending_effect_ownership = Some(vec![RuntimeEffectOwnership::inherited(
        replay_owner.clone(),
    )]);
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("post-cut effect minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "post-cut effect and external work cannot reclaim the root's old logical rank"
    );
    runtime.pending_effect_ownership = None;
    let replay = deferred_lifecycle_ownership_for_test(
        replay_owner.clone(),
        8,
        RuntimeDispatchIngress::DirectAuthenticated,
        Some(post_cut_ordinal),
        target_cut
            .checked_add(1)
            .expect("small target cut has a successor"),
    )
    .expect("post-cut replay can cross into a distinct Busy-deferred owner");
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(8, replay)
            .is_none()
    );
    assert!(
        runtime
            .deferred_ingress_ownership
            .insert(8, replay_ingress)
            .is_none()
    );
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("deferred post-cut minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "crossing Busy cannot turn the post-cut replay into a predecessor"
    );
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("pairwise deferred cut relation is exact"),
        BTreeSet::from([7]),
        "the earlier target remains the sole runner-eligible continuation"
    );

    // Retire the earlier deferred target, leaving only the replay whose
    // physical occurrence began at that target's old cut. Its inherited
    // logical ordinal is older than the timeout which is frozen next, but
    // the new physical occurrence is not: the timeout cut must win.
    assert!(runtime.deferred_lifecycle_ownership.remove(&7).is_some());
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("the replay is otherwise the logical minimum"),
        BTreeSet::from([8])
    );
    runtime
        .set_ingress_physical_cut(target_cut)
        .expect("publish the timeout's receiver-local cut");
    let clock_start = Instant::now();
    runtime
        .arm_live_clocks(clock_start)
        .expect("arm timeout for the post-cut replay regression");
    let timeout_owner = runtime
        .frozen_timeout_owner_for_test(clock_start + runtime.base_round_timeout)
        .expect("freeze one exact timeout owner");
    assert!(replay_owner.lifecycle_ordinal() < timeout_owner.lifecycle_ordinal());
    assert_eq!(runtime.timeout_owner_physical_cut, Some(target_cut));
    assert!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("the timeout cut rejects obsolete logical resurrection")
            .is_empty(),
        "a post-cut replay cannot overtake the already-admitted timeout"
    );
    let frozen_timeout_owner = runtime
        .timeout_owner
        .clone()
        .expect("the timeout owner remains frozen until transfer");
    runtime.timeout_owner_physical_cut = None;
    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    runtime.timeout_owner_physical_cut = Some(target_cut);
    runtime.timeout_owner = None;
    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    runtime.timeout_owner = Some(frozen_timeout_owner);
    runtime
        .set_ingress_physical_cut(
            target_cut
                .checked_add(1)
                .expect("small timeout cut has a successor"),
        )
        .expect("later ingress advances only the live high-watermark");
    assert_eq!(
        runtime.timeout_owner_physical_cut,
        Some(target_cut),
        "later ingress cannot refresh the frozen timeout cut"
    );
    let arbitration = runtime
        .scheduler_arbitration_inputs(clock_start + runtime.base_round_timeout)
        .expect("the frozen timeout compares against its original physical cut");
    assert!(
        arbitration.timeout_due,
        "post-cut deferred replay cannot suppress an already-admitted timeout"
    );
    runtime.timeout_owner = None;
    runtime.timeout_owner_physical_cut = None;

    // Retire the old physical occurrence, freeze a periodic owner at the
    // advanced receiver cut, then admit another physical replay which
    // retains the same obsolete logical lifecycle. The periodic selector
    // must compare only with its immutable pre-cut prefix.
    assert!(runtime.deferred_lifecycle_ownership.remove(&8).is_some());
    assert!(runtime.deferred_ingress_ownership.remove(&8).is_some());
    runtime.timeout_emitted = true;
    runtime.retransmit_started_at = clock_start;
    let periodic_due_at = clock_start + runtime.retransmit_interval;
    runtime
        .freeze_due_clock_owners(periodic_due_at)
        .expect("freeze one exact periodic lifecycle and physical cut");
    let frozen_periodic_owner = runtime
        .retransmit_owner
        .clone()
        .expect("the due periodic episode owns one lifecycle position");
    let periodic_cut = runtime
        .retransmit_owner_physical_cut
        .expect("the due periodic episode freezes receiver ingress");
    assert_eq!(periodic_cut, runtime.ingress_physical_cut);

    let mut later_replay_fair = periodic_replay_fair;
    let later_physical_ordinal = u64::try_from(periodic_cut).expect("small periodic cut fits u64");
    later_replay_fair.first.physical_admission_ordinal = later_physical_ordinal;
    later_replay_fair.latest.physical_admission_ordinal = later_physical_ordinal;
    later_replay_fair.runtime_physical_cut = periodic_cut.checked_add(1);
    assert!(later_replay_fair.validate_exact());
    let later_replay_command = make_command(&runtime, later_replay_fair.clone());
    let later_replay_owner = later_replay_command
        .lifecycle_owner()
        .expect("later replay retains its old logical lifecycle");
    assert!(
        later_replay_owner.lifecycle_ordinal() < frozen_periodic_owner.lifecycle_ordinal(),
        "the regression requires physically later but logically older replay"
    );
    runtime
        .set_ingress_physical_cut(
            periodic_cut
                .checked_add(1)
                .expect("small periodic cut has a successor"),
        )
        .expect("publish the later physical admission without refreshing the clock cut");
    let mut pre_runtime_replay = later_replay_fair.clone();
    pre_runtime_replay.runtime_physical_cut = None;
    pre_runtime_replay.leader_wire_runtime_receipt = None;
    assert!(pre_runtime_replay.validate_exact());
    assert!(
        !runtime.can_admit_network_message_with_ingress_ownership(&message, &pre_runtime_replay,),
        "checked dequeue must retain a post-cut productive replay behind the periodic owner",
    );
    assert!(!runtime.fail_closed);
    let queue_len_before_replay = runtime.ingress.commands.len();
    assert_eq!(
        runtime.enqueue_after_clock_reservation(later_replay_command),
        Err(EnqueueError::Full),
        "the physically later replay remains on its existing ingress carrier"
    );
    assert_eq!(
        runtime.ingress.commands.len(),
        queue_len_before_replay,
        "backpressure cannot publish a FIFO position ahead of the periodic owner"
    );
    assert_eq!(runtime.retransmit_owner_physical_cut, Some(periodic_cut));
    let arbitration = runtime
        .scheduler_arbitration_inputs(periodic_due_at)
        .expect("periodic arbitration uses the frozen physical prefix");
    assert!(
        arbitration.periodic_timer_due,
        "post-cut replay cannot suppress an already-admitted periodic episode"
    );
    let (selected, _) = ScheduleState { fifo_owed: true }.select(
        arbitration.timeout_due,
        arbitration.periodic_timer_due,
        arbitration.fifo_ready,
    );
    assert_eq!(
        selected,
        ScheduledWork::PeriodicTimer,
        "a later replay cannot inherit stale FIFO debt ahead of the frozen target"
    );

    runtime.retransmit_owner_physical_cut = None;
    assert!(matches!(
        runtime.scheduler_arbitration_inputs(periodic_due_at),
        Err(EnqueueError::FailClosed)
    ));
    runtime.retransmit_owner_physical_cut = Some(periodic_cut);
    runtime.retransmit_owner = None;
    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    runtime.retransmit_owner = Some(frozen_periodic_owner);
    runtime.retransmit_owner = None;
    runtime.retransmit_owner_physical_cut = None;
    assert!(
        runtime.can_admit_network_message_with_ingress_ownership(&message, &pre_runtime_replay,),
        "the retained productive replay becomes admissible after clock transfer",
    );
    let later_replay_command = make_command(&runtime, later_replay_fair);
    runtime
        .enqueue_after_clock_reservation(later_replay_command)
        .expect("the same retained replay becomes admissible after target transfer");

    // Pairwise target-relative precedence can form a cycle even though
    // every source/cut pair is individually exact: B logically precedes
    // A, C logically precedes B, and A physically precedes C.  The global
    // selector must first exclude C as post-A-cut, then choose B by
    // logical rank.  Retiring each selected owner yields B, A, C without
    // a lasso or an empty eligible set.
    runtime.ingress.commands.clear();
    runtime.deferred_ingress_ownership.clear();
    runtime.deferred_lifecycle_ownership.clear();
    let (a, b, c) = {
        let source = runtime.driver.deferred_admission_ordinal_source();
        let make_owner = |semantic_identity: &[u8],
                          source_physical_ordinal: Option<u64>,
                          physical_cut: u128,
                          lifecycle_ordinal: u128| {
            let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                runtime.round_tag(),
                CommandClass::Progress,
                RuntimeFreshRootKind::StartupRecovery,
                semantic_identity,
            );
            if let Some(source_physical_ordinal) = source_physical_ordinal {
                origin.root_ingress_identity = Some(Hash::new(semantic_identity));
                origin.root_ingress_physical_ownership = Some(RuntimeIngressPhysicalOwnership {
                    source_ordinal: source_physical_ordinal,
                    physical_cut,
                });
                origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
            }
            let owner = RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                .expect("cycle fixture owns an exact logical lifecycle");
            let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                source,
                owner.causal_origin().lifecycle_key.clone(),
                owner.lifecycle_ordinal(),
                false,
                source_physical_ordinal,
                physical_cut,
            );
            let admission_ordinal = runtime_seal.admission_ordinal();
            let ownership = RuntimeDeferredLifecycleOwnership::new(
                owner,
                admission_ordinal,
                RuntimeDispatchIngress::LocalOrCausal,
                source_physical_ordinal,
                physical_cut,
                runtime_seal,
            )
            .expect("cycle fixture retains an exact source-bound runtime seal");
            assert!(ownership.validate_active_against_ingress(None, source));
            (admission_ordinal, ownership)
        };
        (
            make_owner(b"cycle-a", None, 5, 3),
            make_owner(b"cycle-b", Some(4), 9, 2),
            make_owner(b"cycle-c", Some(8), 12, 1),
        )
    };
    for (ordinal, ownership) in [a.clone(), b.clone(), c.clone()] {
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(ordinal, ownership)
                .is_none()
        );
    }
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("two-stage selector breaks the physical/logical cycle"),
        BTreeSet::from([b.0])
    );
    assert!(runtime.deferred_lifecycle_ownership.remove(&b.0).is_some());
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("A becomes eligible after B retires"),
        BTreeSet::from([a.0])
    );
    assert!(runtime.deferred_lifecycle_ownership.remove(&a.0).is_some());
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("C becomes eligible only after its physical predecessor retires"),
        BTreeSet::from([c.0])
    );
}

#[test]
fn passive_external_owner_cannot_fence_fifo_or_absolute_timeout() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );
    let older = runtime
        .mint_fresh_lifecycle_owner(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"older external exact request",
        )
        .expect("mint the older externally retained lifecycle");
    runtime
        .configure_external_lifecycle_owner_capacity(4)
        .expect("install the independent asynchronous bound");
    runtime
        .set_external_lifecycle_owners(vec![older.clone()])
        .expect("publish the older external owner");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(9),
    )
    .expect("enqueue later unrelated work");

    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 9)]);
    assert_eq!(runtime.queued_commands(), 0);

    let due = start + Duration::from_secs(10);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert!(runtime.timeout_owner.is_none());
    assert!(
        runtime.retransmit_owner.is_none(),
        "an absolute timeout suppresses replenishing the periodic owner during its turn"
    );
    assert_eq!(runtime.driver.timeouts, vec![owner_tag]);
    assert!(runtime.driver.retransmits.is_empty());

    let older_effect = RuntimeEffectOwnership::fresh(
        older.clone(),
        RuntimeFreshRootKind::HistoricalLockedRetransmit,
    );
    runtime
        .enqueue_with_lifecycle_owner(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(1),
            &older_effect,
        )
        .expect("enqueue the exact older completion");
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.driver.retransmits, vec![owner_tag]);
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 9), (owner_tag, 1)]
    );
    assert_eq!(runtime.queued_commands(), 0);

    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("the asynchronous owner retires after its exact completion handoff");
    assert!(runtime.retransmit_owner.is_none());
}

#[test]
fn external_owner_bound_uses_effect_capacity_not_small_ingress_capacity() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );
    let pending_bound = 1_024usize;
    runtime
        .configure_external_lifecycle_owner_capacity(pending_bound)
        .expect("configure the executor's independent pending-work bound");
    let exact_capacity = pending_bound + 2 * MAX_EFFECTS_PER_STEP;
    let owners = (0..exact_capacity)
        .map(|ordinal| {
            let ordinal = u128::try_from(ordinal).expect("small test owner ordinal");
            let semantic = ordinal.to_le_bytes();
            RuntimeLifecycleOwner::new(
                RuntimeCandidateCausalOrigin::mint_fresh_root(
                    owner_tag,
                    CommandClass::Progress,
                    RuntimeFreshRootKind::HistoricalLockedRetransmit,
                    &semantic,
                ),
                ordinal,
            )
            .expect("synthetic external owner binds its first ordinal")
        })
        .collect::<Vec<_>>();
    runtime
        .set_external_lifecycle_owners(owners)
        .expect("pending owners plus two retained batches fit despite ingress capacity 8");
    assert_eq!(runtime.external_lifecycle_owners.len(), exact_capacity);
    assert!(!runtime.fail_closed);
}

#[test]
fn restart_and_periodic_historical_retries_reuse_one_lifecycle_owner() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let historical = FakeEffect::historical(0xA5);
    let (mut runtime, startup) = SerializedV2Runtime::with_driver(
        FakeDriver::new(owner_tag),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        vec![historical],
    )
    .expect("construct deterministic restart ownership");
    assert_eq!(startup, vec![historical]);
    let startup_owner = runtime
        .take_effect_ownership(1)
        .expect("consume startup ownership")
        .pop()
        .expect("one startup owner");
    assert_eq!(
        startup_owner.causality(),
        RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
    );
    runtime
        .arm_live_clocks(start)
        .expect("startup dispatch completes before clocks arm");
    runtime.driver.timer_effects.push_back(vec![historical]);
    runtime.driver.timer_effects.push_back(vec![historical]);

    let mut retry_owners = Vec::new();
    for elapsed in [2, 4] {
        let RuntimeStep::Advanced(effects) = runtime
            .step(start + Duration::from_secs(elapsed))
            .expect("periodic historical retry dispatches")
        else {
            panic!("periodic historical retry must advance");
        };
        assert_eq!(effects, vec![historical]);
        runtime
            .take_last_scheduler_ownership()
            .expect("periodic retry publishes scheduler ownership");
        retry_owners.push(
            runtime
                .take_effect_ownership(1)
                .expect("consume retry ownership")
                .pop()
                .expect("one retry owner"),
        );
    }
    assert!(retry_owners.iter().all(|ownership| {
        ownership.causality()
            == RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::HistoricalLockedRetransmit)
            && ownership.owner() == startup_owner.owner()
    }));
    let cache_after_owned_retries = runtime.dormant_fresh_lifecycle_owners.len();
    assert_ne!(cache_after_owned_retries, 0);
    for elapsed in [6, 8] {
        let RuntimeStep::Advanced(effects) = runtime
            .step(start + Duration::from_secs(elapsed))
            .expect("drained historical lifecycle still services its periodic clock")
        else {
            panic!("the periodic clock must advance even after exact work drains")
        };
        assert!(
            effects.is_empty(),
            "a drained exact historical request cannot recreate physical work"
        );
        runtime
            .take_last_scheduler_ownership()
            .expect("proofless periodic stutter retains scheduler ownership");
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime.dormant_fresh_lifecycle_owners.len(),
            cache_after_owned_retries,
            "fresh periodic episodes replace one bounded cache slot rather than growing it"
        );
    }
    assert_eq!(runtime.driver.retransmits, vec![owner_tag; 4]);

    let next_tag = tag(1);
    runtime
        .observe_effects_with_test_ownership(
            start + Duration::from_secs(9),
            &[FakeEffect::enter_view(next_tag)],
        )
        .expect("test EnterView retains positional producer ownership");
    assert!(
        runtime.dormant_fresh_lifecycle_owners.is_empty(),
        "certified view transition purges every prior-view dormant alias"
    );
}

#[test]
fn dormant_fresh_owner_cache_is_derived_bounded_and_purged_by_view() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let queue = RuntimeQueueConfig::new(8, 2, 2);
    let exact_capacity = queue.capacity + MAX_EFFECTS_PER_STEP;
    let mut runtime = runtime(FakeDriver::new(owner_tag), start, queue);
    let mut last_ordinal = None;
    for identity in 0..exact_capacity {
        let identity = u128::try_from(identity)
            .expect("small dormant-cache fixture")
            .to_le_bytes();
        let owner = runtime
            .mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                &identity,
            )
            .expect("derived dormant-cache capacity admits every configured owner");
        last_ordinal = Some(owner.lifecycle_ordinal());
    }
    assert_eq!(runtime.dormant_fresh_lifecycle_owners.len(), exact_capacity);
    assert_eq!(
        runtime.mint_fresh_lifecycle_owner(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"one owner beyond the derived bound",
        ),
        Err(EnqueueError::Full)
    );

    let next_tag = tag(1);
    runtime
        .observe_effects_with_test_ownership(start, &[FakeEffect::enter_view(next_tag)])
        .expect("test EnterView retains positional producer ownership");
    assert!(runtime.dormant_fresh_lifecycle_owners.is_empty());
    let successor = runtime
        .mint_fresh_lifecycle_owner(
            next_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"successor-view exact request",
        )
        .expect("view reclamation reopens the same derived cache geometry");
    assert!(
        successor.lifecycle_ordinal() > last_ordinal.expect("cache was filled"),
        "cache reclamation cannot reuse an old admission ordinal"
    );
}

#[test]
fn causal_successors_inherit_root_and_lifecycle_ordinal() {
    let admitted_at = Instant::now();
    let root_tag = tag(0);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 2, 2));
    ingress
        .enqueue(TaggedCommand::new(
            root_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            admitted_at,
        ))
        .expect("root candidate is admitted");
    let (root, root_owner) = ingress
        .pop_next_with_ownership()
        .expect("root selection is exact")
        .expect("root candidate is ready");
    assert_eq!(root.lifecycle_ordinal, Some(root_owner.lifecycle_ordinal));

    let successor_tag = EventTag::new(
        root_tag.height(),
        root_tag.view() + 1,
        Generation::new(root_tag.generation().get() + 1),
    );
    for value in [2, 3, 4] {
        ingress
            .enqueue(
                TaggedCommand::with_causal_origin(
                    successor_tag,
                    CommandClass::Completion,
                    FakeCommand::record(value),
                    admitted_at,
                    root_owner.causal_origin.clone(),
                    root_owner.lifecycle_ordinal,
                )
                .expect("causal owner is internally consistent"),
            )
            .expect("causal child is admitted with a unique physical owner");
    }

    let physical_ordinals = ingress
        .commands
        .iter()
        .map(|candidate| {
            assert_eq!(
                candidate.causal_origin, root_owner.causal_origin,
                "evidence/view rewriting cannot replace the first-admission root"
            );
            assert_eq!(
                candidate.lifecycle_ordinal,
                Some(root_owner.lifecycle_ordinal),
                "every child inherits one logical lifecycle ordinal"
            );
            candidate
                .admission_ordinal
                .expect("every physical child has its own FIFO ordinal")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(physical_ordinals.len(), 3);

    let unrelated = TaggedCommand::new(
        successor_tag,
        CommandClass::Completion,
        FakeCommand::record(2),
        admitted_at,
    );
    assert!(
        !unrelated
            .causal_origin
            .same_lifecycle(&root_owner.causal_origin),
        "a physically similar command with a different causal root cannot coalesce"
    );
}

#[test]
fn equal_lifecycle_fence_siblings_follow_exact_physical_rank() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"fence-sibling-block")),
        payload_hash: Hash::new(b"fence-sibling-payload"),
    };
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 2, 0));
    let lifecycle_ordinal = ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve one shared fence lifecycle");
    let predecessor = AdapterCommand::ApplicationCompleted(subject);
    let mut causal_origin =
        RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &predecessor, None);
    assert!(causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal));
    let sibling = |class, command| {
        TaggedCommand::with_causal_origin(
            owner_tag,
            class,
            command,
            admitted_at,
            causal_origin.clone(),
            lifecycle_ordinal,
        )
        .expect("construct one exact same-lifecycle fence sibling")
    };
    ingress
        .enqueue(sibling(CommandClass::Normal, predecessor))
        .expect("enqueue the physical predecessor");
    ingress
        .enqueue(sibling(
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(vec![0xA5]),
        ))
        .expect("enqueue the later causal completion");

    let (first, first_owner, first_is_completion) = ingress
        .pop_fence_dependency_with_ownership(
            lifecycle_ordinal,
            u128::MAX,
            |queued| matches!(queued.command, AdapterCommand::SignatureCompleted(_)),
            |_| true,
        )
        .expect("the exact dependency rank is valid")
        .expect("one equal-lifecycle owner is ready");
    assert!(matches!(
        first.command,
        AdapterCommand::ApplicationCompleted(_)
    ));
    assert!(!first_is_completion);
    assert_eq!(first_owner.fifo_position, 0);

    let (second, second_owner, second_is_completion) = ingress
        .pop_fence_dependency_with_ownership(
            lifecycle_ordinal,
            u128::MAX,
            |queued| matches!(queued.command, AdapterCommand::SignatureCompleted(_)),
            |_| true,
        )
        .expect("the remaining completion rank is valid")
        .expect("the equal-lifecycle completion is ready after its predecessor");
    assert!(matches!(
        second.command,
        AdapterCommand::SignatureCompleted(_)
    ));
    assert!(second_is_completion);
    assert_eq!(second_owner.fifo_position, 0);
    assert!(ingress.commands.is_empty());
}

#[test]
fn preassigned_batch_lifecycles_require_shared_mint_and_exact_root() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let unminted_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let mut unminted_ingress = BoundedIngress::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(4, 1, 1),
        unminted_source.clone(),
    );
    let unminted_command = FakeCommand::record(1);
    let mut unminted_origin = RuntimeCandidateCausalOrigin::mint(
        owner_tag,
        CommandClass::Completion,
        &unminted_command,
        None,
    );
    assert!(unminted_origin.bind_lifecycle_ordinal(1));
    let unminted = TaggedCommand::with_causal_origin(
        owner_tag,
        CommandClass::Completion,
        unminted_command,
        admitted_at,
        unminted_origin,
        1,
    )
    .expect("construct internally exact but unminted lifecycle");
    assert_eq!(
        unminted_ingress.enqueue_completion_batch(vec![unminted]),
        Err(EnqueueError::FailClosed)
    );
    assert!(unminted_ingress.commands.is_empty());
    assert_eq!(
        unminted_source
            .next_ordinal_for_test()
            .expect("unminted batch rejection preserves the source"),
        Some(1)
    );

    let collision_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let mut collision_ingress = BoundedIngress::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(4, 1, 1),
        collision_source.clone(),
    );
    collision_ingress
        .enqueue(TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(2),
            admitted_at,
        ))
        .expect("mint one exact lifecycle root");
    let (_, root_owner) = collision_ingress
        .pop_next_with_ownership()
        .expect("select the minted root exactly")
        .expect("root is ready");
    let sibling = TaggedCommand::with_causal_origin(
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(3),
        admitted_at,
        root_owner.causal_origin.clone(),
        root_owner.lifecycle_ordinal,
    )
    .expect("construct one legitimate causal sibling");
    let conflicting_command = FakeCommand::record(4);
    let mut conflicting_origin = RuntimeCandidateCausalOrigin::mint(
        owner_tag,
        CommandClass::Completion,
        &conflicting_command,
        None,
    );
    assert!(conflicting_origin.bind_lifecycle_ordinal(root_owner.lifecycle_ordinal));
    let conflicting = TaggedCommand::with_causal_origin(
        owner_tag,
        CommandClass::Completion,
        conflicting_command,
        admitted_at,
        conflicting_origin,
        root_owner.lifecycle_ordinal,
    )
    .expect("construct a distinct root at the colliding ordinal");
    let next_before_collision = collision_source
        .next_ordinal_for_test()
        .expect("inspect source before batch collision");
    assert_eq!(
        collision_ingress.enqueue_completion_batch(vec![sibling, conflicting]),
        Err(EnqueueError::FailClosed)
    );
    assert!(
        collision_ingress.commands.is_empty(),
        "batch collision must reject atomically"
    );
    assert_eq!(
        collision_source
            .next_ordinal_for_test()
            .expect("batch collision preserves the source"),
        next_before_collision,
        "collision validation must run before reserving physical positions"
    );
}

#[test]
fn restart_dormant_local_fifo_reservation_survives_full_class_churn() {
    let started_at = Instant::now();
    let owner_tag = tag(0);
    let lifecycle_key = Hash::new(b"restart dormant Local FIFO lifecycle");
    let mut driver = FakeDriver::new(owner_tag);
    driver.dormant_local_fifo_reservations = vec![RuntimeDormantLocalFifoReservation::completion(
        lifecycle_key,
        1,
        8,
    )];
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
        driver,
        started_at,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(7, 1, 2),
        Vec::new(),
        lifecycle_ordinals,
    )
    .expect("restart installs exact latent FIFO ownership")
    .0;
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime without advancing its latent owner");
    assert_eq!(
        runtime.remaining_completion_capacity(),
        5,
        "the dormant Local stage consumes one physical completion slot"
    );
    let later_serve = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("mint a later exact Serve ticket");
    assert_eq!(
        runtime.minimum_active_lifecycle_ordinal(),
        Ok(Some(1)),
        "the complete active inventory retains restart-dormant lifecycle debt"
    );
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(started_at, later_serve)
            .expect("inspect passive dormant ownership at the Serve cut"),
        "passive dormant debt cannot open an executable Serve predecessor episode"
    );

    for value in [1, 2, 3] {
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(value),
        )
        .expect("ordinary churn fills only the remaining normal prefix");
    }
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(4),
        ),
        Err(EnqueueError::ReservedCapacity),
        "normal churn cannot acquire the dormant target's slot"
    );
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Progress,
        FakeCommand::record(5),
    )
    .expect("progress fills its existing prefix");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(6),
    )
    .expect("a trusted completion fills the last unreserved position");
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert!(
        runtime.driver.delivered.is_empty(),
        "the full-capacity cut is retained before exact replacement"
    );

    runtime.driver.admission_preflight_override =
        Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
            causal_lifecycle_key: lifecycle_key,
            admission_ordinal: 1,
            producer_stage: 8,
        });
    let next_before_replay = runtime.ingress.next_admission_ordinal;
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(9),
    )
    .expect("exact retry atomically replaces its latent slot at full capacity");
    assert!(runtime.ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(runtime.queued_commands(), 6);
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert_eq!(
        runtime.minimum_active_lifecycle_ordinal(),
        Ok(Some(1)),
        "the restored FIFO owner retains the pre-restart lifecycle age"
    );

    let next_after_replay = runtime.ingress.next_admission_ordinal;
    assert_ne!(
        next_after_replay, next_before_replay,
        "the first physical replay receives one fresh FIFO position"
    );
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(9),
    )
    .expect("duplicate exact retry coalesces with the one physical owner");
    assert_eq!(runtime.queued_commands(), 6);
    assert_eq!(
        runtime.ingress.next_admission_ordinal, next_after_replay,
        "coalescing cannot mint another physical admission ordinal"
    );

    let RuntimeStep::Advanced(effects) = runtime
        .step(started_at)
        .expect("the exact replacement becomes the global ready owner")
    else {
        panic!("the exact replacement must dispatch before younger queued work");
    };
    assert_eq!(effects, vec![FakeEffect::other()]);
    let selected = runtime
        .take_last_scheduler_ownership()
        .expect("the replacement dispatch retains exact FIFO ownership");
    runtime
        .take_effect_ownership(effects.len())
        .expect("the executor consumes the restored target's effect owner");
    assert_eq!(selected.selected, RuntimeSelectedOwnerKind::Fifo);
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 9)],
        "the restored target dispatches before every younger physical command"
    );
    assert_eq!(runtime.queued_commands(), 5);

    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(9),
        ),
        Err(EnqueueError::FailClosed),
        "ReuseDormant after latent-slot removal cannot recreate the drained stage"
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.queued_commands(),
        5,
        "rejected resurrection cannot install another physical owner"
    );
}
