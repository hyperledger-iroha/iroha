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
