#[test]
fn fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers() {
    let directory = TempDir::new().expect("temporary real-adapter ordering directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let start = Instant::now();
    runtime
        .arm_live_clocks(start)
        .expect("arm runtime after adapter startup");
    // Service one complete periodic episode before the signer becomes
    // busy. A later tick must mint a new lifecycle ordinal rather than
    // resurrecting this drained episode at its old position.
    let before_timeout = start + runtime.retransmit_interval();
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("service pre-fence retransmission"),
        RuntimeStep::Advanced(_)
    ));
    let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
    runtime
        .enqueue_network(proposal)
        .expect("enqueue authenticated proposal");
    let proposal_effects = match runtime
        .step_and_take_scheduler_ownership_for_test(before_timeout)
        .expect("dispatch authenticated proposal")
    {
        RuntimeStep::Advanced(effects) => effects,
        RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
    };
    let (tag, manifest) = match proposal_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("enqueue reconstructed body");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch reconstructed body"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
    ));
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
        .expect("enqueue durable-body completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch durable-body completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
    ));
    let validation_effects = runtime
        .driver_mut_for_test()
        .settle_ready_validate_succeeded_for_runtime_test(
            tag,
            manifest.round,
            manifest.subject,
            &ValidatedBodyReceipt::for_test(durable),
        );
    runtime
        .retain_external_lifecycle_effect_ownership_for_test(&validation_effects)
        .expect("bind the lifecycle-owned Validate successor for runtime ordering");
    let prepare_effect_ownership = runtime
        .take_effect_ownership(validation_effects.len())
        .expect("Prepare signature request retains its lifecycle owner");
    let (prepare_sign_tag, prepare_signature_preimage) = match validation_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] if vote.phase == wire::GlobalPhase::Prepare
            && vote.round == manifest.round
            && vote.subject == manifest.subject =>
        {
            (*tag, vote.signature_preimage())
        }
        effects => panic!("unexpected validation effects: {effects:?}"),
    };
    assert_eq!(prepare_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![prepare_effect_ownership[0].owner().clone()])
        .expect("publish the pending Prepare signer owner");
    // The second periodic episode is still before the absolute deadline,
    // but it is frozen only at this serialized runner entry. The pending
    // Prepare signer already owns an older lifecycle position, so the new
    // episode waits without entering the adapter or creating fence debt.
    let second_retransmission = before_timeout + runtime.retransmit_interval();
    assert!(second_retransmission < start + runtime.round_timeout());
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(second_retransmission)
            .expect("freeze the pre-deadline second retransmission"),
        RuntimeStep::Idle
    ));
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty(),
        "a younger periodic owner cannot enter the adapter ahead of the signer"
    );
    assert!(
        runtime.retransmit_owner.is_some(),
        "the fresh periodic episode remains frozen at its later lifecycle position"
    );
    let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(
            prepare_sign_tag,
            prepare_signature,
            &prepare_effect_ownership[0],
        )
        .expect("enqueue exact Prepare signature completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the pending Prepare signer owner after completion enqueue");
    assert_eq!(runtime.queued_commands(), 1);
    let prepare_broadcast = runtime
        .step(second_retransmission)
        .expect("owned Prepare completion precedes the younger retransmission");
    let prepare_completion = runtime
        .take_last_scheduler_ownership()
        .expect("Prepare completion retains exact scheduler ownership");
    assert_eq!(prepare_completion.selected, RuntimeSelectedOwnerKind::Fifo);
    assert!(!prepare_completion.fence_completion_bypass);
    assert!(
        prepare_completion
            .fence_predecessor_lifecycle_ordinal
            .is_none()
    );
    assert!(prepare_completion.validate_exact().is_ok());
    let RuntimeStep::Advanced(prepare_broadcasts) = prepare_broadcast else {
        panic!("Prepare completion unexpectedly idled")
    };
    assert!(matches!(
        prepare_broadcasts.as_slice(),
        [AdapterEffect::Broadcast(message)]
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::Vote(vote)
                    if vote.phase == wire::GlobalPhase::Prepare
                        && vote.round == manifest.round
                        && vote.subject == manifest.subject
            )
    ));
    runtime
        .take_effect_ownership(prepare_broadcasts.len())
        .expect("test executor consumes Prepare broadcast ownership");
    assert!(
        runtime.retransmit_owner.is_some(),
        "the younger periodic episode remains frozen until its own turn"
    );
    assert_eq!(runtime.queued_commands(), 0);
    // Once the older completion drains, the retained fresh episode runs
    // and rebroadcasts the newly published Prepare vote.
    let retransmit_retry = runtime
        .step_and_take_scheduler_ownership_for_test(second_retransmission)
        .expect("service younger pre-deadline retransmission episode");
    assert!(matches!(
        retransmit_retry,
        RuntimeStep::Advanced(ref effects)
            if effects.iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::Vote(vote)
                            if vote.phase == wire::GlobalPhase::Prepare
                                && vote.round == manifest.round
                    )
            ))
    ));
    assert_eq!(
        prepare_completion.validate_exact(),
        Ok(()),
        "immutable completion evidence remains valid after the younger owner runs"
    );
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(runtime.retransmit_owner.is_none());
    // Absolute timeout remains one-shot after the pre-deadline episode
    // drains. Its signing lifecycle likewise predates the next periodic
    // episode.
    let deadline = start + runtime.round_timeout();
    let timeout_macro_step = runtime
        .step(deadline)
        .expect("deliver the absolute timeout through the real adapter");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout macro-step retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_macro_step else {
        panic!("absolute timeout unexpectedly idled")
    };
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout signature request retains its lifecycle owner");
    let (timeout_sign_tag, timeout_signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] if vote.round == manifest.round => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    assert_eq!(timeout_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending TimeoutVote signer owner");
    // A fresh retransmission episode becomes due while TimeoutVote signing
    // is active. Its new ordinal follows the signer, so it remains at the
    // runtime boundary instead of entering the adapter as Busy debt.
    let post_timeout_retransmission = deadline + runtime.retransmit_interval();
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
            .expect("freeze post-timeout retransmission behind signing"),
        RuntimeStep::Idle
    ));
    assert!(
        runtime.retransmit_owner.is_some(),
        "post-timeout retransmission retains its fresh runtime owner while blocked"
    );
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    let timeout_signature = Signature::new(keys[0].private_key(), &timeout_signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(
            timeout_sign_tag,
            timeout_signature,
            &timeout_effect_ownership[0],
        )
        .expect("enqueue exact TimeoutVote signature completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the pending TimeoutVote signer owner after completion enqueue");
    let first_timeout_vote = runtime
        .step(post_timeout_retransmission)
        .expect("owned TimeoutVote completion precedes the younger retransmission");
    let timeout_completion = runtime
        .take_last_scheduler_ownership()
        .expect("TimeoutVote completion retains exact scheduler ownership");
    assert_eq!(timeout_completion.selected, RuntimeSelectedOwnerKind::Fifo);
    assert!(!timeout_completion.fence_completion_bypass);
    assert!(
        timeout_completion
            .fence_predecessor_lifecycle_ordinal
            .is_none()
    );
    assert!(timeout_completion.validate_exact().is_ok());
    let RuntimeStep::Advanced(first_timeout_vote_effects) = first_timeout_vote else {
        panic!("TimeoutVote completion unexpectedly idled")
    };
    assert!(first_timeout_vote_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                    if vote.round == manifest.round
            )
    )));
    runtime
        .take_effect_ownership(first_timeout_vote_effects.len())
        .expect("test executor consumes first TimeoutVote ownership");
    assert!(
        runtime.retransmit_owner.is_some(),
        "the younger post-timeout episode remains frozen until its own turn"
    );
    // Treat the first TimeoutVote broadcast as lost. The retained younger
    // periodic episode now owns the next serialized turn and rebroadcasts
    // the published vote.
    let timeout_vote_retry = runtime
        .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
        .expect("rebroadcast a lost first TimeoutVote");
    assert!(matches!(
        timeout_vote_retry,
        RuntimeStep::Advanced(ref effects)
            if effects.iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                            if vote.round == manifest.round
                    )
            ))
    ));
    assert_eq!(runtime.queued_commands(), 0);
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(runtime.retransmit_owner.is_none());
    // A later periodic tick remains armed after the one-shot timeout and
    // continues broadcasting the published TimeoutVote.
    let later_post_timeout_tick = post_timeout_retransmission + runtime.retransmit_interval();
    let later_retry = runtime
        .step(later_post_timeout_tick)
        .expect("service a later post-timeout periodic tick");
    let later_retry_owner = runtime
        .take_last_scheduler_ownership()
        .expect("later periodic tick retains scheduler ownership");
    assert_eq!(
        later_retry_owner.selected,
        RuntimeSelectedOwnerKind::PeriodicTimer
    );
    assert!(later_retry_owner.validate_exact().is_ok());
    let RuntimeStep::Advanced(later_retry_effects) = later_retry else {
        panic!("later post-timeout periodic tick unexpectedly idled")
    };
    assert!(later_retry_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                    if vote.round == manifest.round
            )
    )));
    runtime
        .take_effect_ownership(later_retry_effects.len())
        .expect("test executor consumes later TimeoutVote retry ownership");
    assert_eq!(runtime.queued_commands(), 0);
    assert!(
        runtime
            .driver()
            .all_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
}
#[test]
fn round_timeout_grows_linearly_then_stays_bounded_without_wrapping() {
    let base = Duration::from_secs(10);
    assert_eq!(round_timeout_for_view(base, 0), base);
    assert_eq!(round_timeout_for_view(base, 1), Duration::from_secs(20));
    assert_eq!(round_timeout_for_view(base, 7), Duration::from_secs(80));
    assert_eq!(round_timeout_for_view(base, 9), Duration::from_secs(100));
    assert_eq!(round_timeout_for_view(base, 10), Duration::from_secs(100));
    assert_eq!(round_timeout_for_view(base, 260), Duration::from_secs(100));
    assert_eq!(
        round_timeout_for_view(Duration::new(1, 500_000_000), 1),
        Duration::from_secs(3),
    );
    assert_eq!(
        round_timeout_for_view(Duration::from_secs(1), u64::MAX - 1),
        Duration::from_secs(10)
    );
    assert_eq!(
        round_timeout_for_view(Duration::from_secs(1), u64::MAX),
        Duration::from_secs(10)
    );
    assert_eq!(round_timeout_for_view(Duration::MAX, 1), Duration::MAX);
}
#[test]
fn recovered_nonzero_view_uses_scaled_timeout_from_live_arm() {
    let constructed_at = Instant::now();
    let armed_at = constructed_at + Duration::from_secs(500);
    let recovered = tag(4);
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(recovered),
        constructed_at,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("open recovered runtime");
    runtime
        .arm_live_clocks(armed_at)
        .expect("arm after recovered startup");
    assert_eq!(runtime.round_timeout(), Duration::from_secs(50));
    let _ = runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(49));
    assert!(runtime.driver.timeouts.is_empty());
    let _ = runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(50));
    assert_eq!(runtime.driver.timeouts, vec![recovered]);
}
#[test]
fn recovered_high_view_uses_bounded_timeout_from_live_arm() {
    let constructed_at = Instant::now();
    let armed_at = constructed_at + Duration::from_secs(500);
    let recovered = tag(260);
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(recovered),
        constructed_at,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("open recovered high-view runtime");
    runtime
        .arm_live_clocks(armed_at)
        .expect("arm after recovered high-view startup");
    assert_eq!(runtime.round_timeout(), Duration::from_secs(100));
    let _ = runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(99));
    assert!(runtime.driver.timeouts.is_empty());
    let _ = runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(100));
    assert_eq!(runtime.driver.timeouts, vec![recovered]);
}
#[test]
fn class_aware_ingress_is_bounded_and_reserves_progress_and_completion_slots() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(5, 2, 1),
    );
    assert_eq!(runtime.remaining_completion_capacity(), 4);
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .unwrap();
    assert_eq!(runtime.remaining_completion_capacity(), 3);
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(99)
        ),
        Err(EnqueueError::ReservedCapacity)
    );
    for value in [2, 3] {
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(value),
        )
        .expect("each configured progress slot remains reserved");
    }
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Completion,
        FakeCommand::record(4),
    )
    .expect("reserved completion slot");
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert_eq!(runtime.queued_commands(), 4);
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(5)
        ),
        Err(EnqueueError::Full)
    );
    for offset in 0..4 {
        let _ = runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_millis(offset));
    }
    assert_eq!(
        runtime.driver.delivered,
        vec![(initial, 4), (initial, 2), (initial, 1), (initial, 3)],
        "the persistent Completion/Progress/Normal cursor bounds service debt while preserving reserved capacity"
    );
}
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
    assert!(evidence.clocks_armed);
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
        candidate.causal_origin.root_lifecycle_ordinal(),
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
    candidate.local_proposal_worker_completed_before_deadline = true;
    candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
    mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
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
fn prepared_completion_capacity_relief_dispatches_only_the_frozen_completion() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(4, 1, 1),
    );
    for (class, value) in [
        (CommandClass::Normal, 1),
        (CommandClass::Completion, 2),
        (CommandClass::Progress, 3),
    ] {
        enqueue_fake(&mut runtime, owner_tag, class, FakeCommand::record(value))
            .expect("fill one ordinary runtime position per service class");
    }
    runtime.ingress.next_class = CommandClass::Normal;
    for queued in &mut runtime.ingress.commands {
        queued.eligible_skips = match queued.command.record {
            Some(1) => 2,
            Some(2) => 7,
            Some(3) => 3,
            _ => unreachable!("fixture contains only record commands"),
        };
    }
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    let cursor_before = runtime.ingress.next_class;
    let schedule_before = runtime.schedule;
    let blocked_completion_lifecycle_ordinal = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| queued.command.record == Some(2))
        .and_then(|queued| queued.lifecycle_ordinal)
        .expect("queued Completion exposes its lifecycle ordinal");
    let prepared = runtime
        .prepare_completion_capacity_relief(blocked_completion_lifecycle_ordinal)
        .expect("freeze exact full-FIFO Completion relief")
        .expect("a physical Completion owner can release capacity");
    assert_eq!(
        prepared.blocked_completion_lifecycle_ordinal,
        blocked_completion_lifecycle_ordinal
    );
    assert!(prepared.validate_identity());
    assert_eq!(runtime.driver.delivered, Vec::new());
    assert_eq!(runtime.queued_commands(), 3);

    let supplied_cut = start + Duration::from_secs(30);
    assert!(matches!(
        runtime.step_prepared_completion_capacity_relief(supplied_cut, prepared),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 2)]);
    assert!(runtime.driver.timeouts.is_empty());
    assert!(runtime.driver.retransmits.is_empty());
    assert_eq!(runtime.ingress.next_class, cursor_before);
    assert_eq!(runtime.schedule, schedule_before);
    assert_eq!(runtime.remaining_completion_capacity(), 1);
    assert_eq!(
        runtime
            .ingress
            .commands
            .iter()
            .map(|queued| (queued.command.record, queued.eligible_skips))
            .collect::<Vec<_>>(),
        vec![(Some(1), 2), (Some(3), 3)],
        "capacity relief neither selects nor charges Normal/Progress work"
    );
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("capacity relief publishes exact scheduler ownership");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::CompletionCapacityRelief
    );
    assert_eq!(evidence.queue_before.service_cursor, SERVICE_CLASS_NORMAL);
    assert_eq!(evidence.queue_after.service_cursor, SERVICE_CLASS_NORMAL);
    assert_eq!(evidence.fifo_owed_before, evidence.fifo_owed_after);
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &evidence.candidate else {
        panic!("capacity relief must retain one exact FIFO candidate")
    };
    assert_eq!(candidate.class, SERVICE_CLASS_COMPLETION);
    assert_eq!(
        candidate.causal_origin.root_lifecycle_ordinal(),
        Some(blocked_completion_lifecycle_ordinal)
    );
    assert_eq!(
        candidate.identity,
        FakeCommand::record(2)
            .exact_runtime_command_identity()
            .digest()
    );
    assert_eq!(candidate.eligible_skips_before, 7);
    assert_eq!(candidate.eligible_skips_after, 7);
    assert_eq!(
        candidate.selection_seal.kind,
        RuntimeQueueSelectionKind::CompletionCapacityRelief
    );
    assert_eq!(
        candidate.selection_seal.ordinary_remaining_capacity_before,
        Some(0)
    );
    assert_eq!(evidence.validate_exact(), Ok(()));
    runtime
        .take_effect_ownership(1)
        .expect("consume the capacity-relief effect owner");
}

#[test]
fn prepared_completion_capacity_relief_rejects_nonfull_younger_and_retyped_owners() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(4, 1, 1),
    );
    for (class, value) in [(CommandClass::Normal, 1), (CommandClass::Completion, 2)] {
        enqueue_fake(&mut runtime, owner_tag, class, FakeCommand::record(value))
            .expect("ordinary owner fits before the lane is full");
    }
    let completion_ordinal = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| queued.class == CommandClass::Completion)
        .and_then(|queued| queued.lifecycle_ordinal)
        .expect("Completion owner has an ordinal");
    assert!(matches!(
        runtime.prepare_completion_capacity_relief(completion_ordinal),
        Ok(None)
    ));
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Progress,
        FakeCommand::record(3),
    )
    .expect("fill the last ordinary runtime position");
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert!(matches!(
        runtime.prepare_completion_capacity_relief(
            completion_ordinal
                .checked_sub(1)
                .expect("fixture Completion ordinal has a predecessor")
        ),
        Ok(None)
    ));

    let mut rebound = runtime
        .prepare_completion_capacity_relief(completion_ordinal)
        .expect("prepare a bound-integrity probe")
        .expect("the exact Completion is eligible");
    rebound.blocked_completion_lifecycle_ordinal = completion_ordinal
        .checked_add(1)
        .expect("bounded fixture ordinal has a successor");
    assert!(
        !rebound.validate_identity(),
        "the blocked Completion bound is part of the move-only token projection"
    );

    let mut prepared = runtime
        .prepare_completion_capacity_relief(completion_ordinal)
        .expect("prepare full-FIFO Completion relief")
        .expect("the exact Completion is not younger than the blocked owner");
    let (progress_position, progress) = runtime
        .ingress
        .commands
        .iter()
        .enumerate()
        .find(|(_, queued)| queued.class == CommandClass::Progress)
        .expect("fixture retains one Progress owner");
    prepared.selected_position =
        u64::try_from(progress_position).expect("bounded test position fits u64");
    prepared.selected_owner = progress
        .cached_queue_occurrence_owner(&runtime.ingress.selection_source_identity)
        .cloned()
        .expect("Progress owner retains its queue capability");
    prepared.selected_lifecycle_ordinal = progress
        .lifecycle_ordinal
        .expect("Progress owner has a lifecycle ordinal");
    prepared.blocked_completion_lifecycle_ordinal = prepared.selected_lifecycle_ordinal;
    prepared.projection_hash = prepared_completion_capacity_relief_projection_hash(&prepared);
    assert!(matches!(
        runtime.step_prepared_completion_capacity_relief(start, prepared),
        Err(RuntimeError::FailClosed)
    ));
    assert!(runtime.fail_closed);
    assert_eq!(runtime.driver.delivered, Vec::new());
    assert_eq!(runtime.queued_commands(), 3);
}

#[test]
fn retryable_completion_capacity_relief_preserves_cursor_debt_and_exact_owner() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    assert!(driver.retry_once.insert(2));
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(4, 1, 1));
    for (class, value) in [
        (CommandClass::Normal, 1),
        (CommandClass::Completion, 2),
        (CommandClass::Progress, 3),
    ] {
        enqueue_fake(&mut runtime, owner_tag, class, FakeCommand::record(value))
            .expect("fill one ordinary runtime position per service class");
    }
    runtime.ingress.next_class = CommandClass::Progress;
    for queued in &mut runtime.ingress.commands {
        queued.eligible_skips = u64::from(queued.command.record.expect("record command")) + 4;
    }
    let queue_before = runtime.ingress.ownership_snapshot();
    let schedule_before = runtime.schedule;
    let blocked_completion_lifecycle_ordinal = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| queued.class == CommandClass::Completion)
        .and_then(|queued| queued.lifecycle_ordinal)
        .expect("Completion owner has a lifecycle ordinal");
    let prepared = runtime
        .prepare_completion_capacity_relief(blocked_completion_lifecycle_ordinal)
        .expect("prepare retryable capacity relief")
        .expect("full queue has one runnable Completion owner");
    assert!(matches!(
        runtime.step_prepared_completion_capacity_relief(start, prepared),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.ingress.ownership_snapshot(), queue_before);
    assert_eq!(runtime.schedule, schedule_before);
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert_eq!(runtime.driver.delivered, Vec::new());
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("retry publishes typed retained ownership");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::CompletionCapacityReliefRetryRetained
    );
    assert_eq!(evidence.queue_before, evidence.queue_after);
    assert_eq!(evidence.fifo_owed_before, evidence.fifo_owed_after);
    assert_eq!(evidence.validate_exact(), Ok(()));
    runtime
        .take_effect_ownership(0)
        .expect("retry emits no effect ownership");
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
fn scheduler_owner_carrier_covers_live_and_typed_deferred_branches() {
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
fn restored_high_watermark_exhaustion_fails_without_erasing_the_source() {
    let source = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 1);
    assert!(source.advance_past(u128::MAX).is_err());
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("inspect source after rejected restored high-watermark"),
        Some(u128::MAX),
        "a rejected restored high-watermark must not turn exhaustion into an empty source"
    );
    let already_exhausted = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX);
    assert!(already_exhausted.advance_past(0).is_err());
    assert_eq!(
        already_exhausted
            .next_ordinal_for_test()
            .expect("inspect an already exhausted restored source"),
        None
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
fn causal_lifecycle_key_ignores_only_process_generation() {
    let first_tag = EventTag::new(9, 4, Generation::new(1));
    let replay_tag = EventTag::new(9, 4, Generation::new(7));
    let different_view = EventTag::new(9, 5, Generation::new(7));
    let command = FakeCommand::record(0xA5);
    let first =
        RuntimeCandidateCausalOrigin::mint(first_tag, CommandClass::Progress, &command, None);
    let replay =
        RuntimeCandidateCausalOrigin::mint(replay_tag, CommandClass::Progress, &command, None);
    let other_view =
        RuntimeCandidateCausalOrigin::mint(different_view, CommandClass::Progress, &command, None);
    assert!(first.same_lifecycle(&replay));
    assert_eq!(first.lifecycle_key, replay.lifecycle_key);
    assert_ne!(
        first.projection_hash, replay.projection_hash,
        "the full diagnostic carrier still records process generation"
    );
    assert!(!first.same_lifecycle(&other_view));
    assert_ne!(first.lifecycle_key, other_view.lifecycle_key);
}
#[test]
fn aggregate_certificate_causal_roots_ignore_signer_carrier_replacement() {
    let (context, keys) = authenticated_runtime_context();
    let owner_tag = tag(0);
    let source_a = PeerId::new(keys[0].public_key().clone());
    let source_b = PeerId::new(keys[1].public_key().clone());
    let tagged_origin = |message: wire::ConsensusMessageV2, source: PeerId| {
        let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &message,
            fair_runtime_ownership(&message, source.clone(), source),
        )
        .expect("fair ingress yields exact runtime ownership");
        let authenticated = AuthenticatedConsensusMessage::for_test(message);
        assert_eq!(
            authenticated.exact_runtime_command_identity(),
            AdapterCommand::Authenticated(authenticated.clone()).exact_runtime_command_identity(),
            "the authenticated token and adapter wrapper share one exact identity"
        );
        TaggedCommand::with_ingress_ownership(
            owner_tag,
            CommandClass::Progress,
            authenticated,
            Instant::now(),
            ownership,
        )
        .causal_origin
    };
    let qc_a = signed_runtime_quorum_certificate(&context, &keys, 0xD1);
    let mut qc_b = qc_a.clone();
    qc_b.signers.rotate_left(1);
    qc_b.aggregate_signature = vec![0xB2; qc_b.aggregate_signature.len()];
    let qc_origin_a = tagged_origin(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_a)),
        source_a.clone(),
    );
    let qc_origin_b = tagged_origin(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_b)),
        source_b.clone(),
    );
    assert!(qc_origin_a.same_lifecycle(&qc_origin_b));
    let tc_a = signed_runtime_timeout_certificate(&context, &keys);
    let mut tc_b = tc_a.clone();
    tc_b.groups[0].signers.rotate_left(1);
    tc_b.groups[0].aggregate_signature = vec![0xC3; tc_b.groups[0].aggregate_signature.len()];
    let tc_message_a = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_a.clone()),
    );
    let tc_message_b =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_b));
    let exact_tc_a = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
        tc_message_a.clone(),
    ))
    .exact_runtime_command_identity()
    .digest();
    let exact_tc_b = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
        tc_message_b.clone(),
    ))
    .exact_runtime_command_identity()
    .digest();
    assert_ne!(
        exact_tc_a, exact_tc_b,
        "deep command identity still distinguishes replaceable certificate carriers"
    );
    let tc_origin_a = tagged_origin(tc_message_a, source_a);
    let tc_origin_b = tagged_origin(tc_message_b, source_b.clone());
    assert!(tc_origin_a.same_lifecycle(&tc_origin_b));
    let mut other_round = tc_a;
    other_round.round.view = other_round.round.view.saturating_add(1);
    let other_round_origin = tagged_origin(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
            other_round,
        )),
        source_b,
    );
    assert!(
        !tc_origin_a.same_lifecycle(&other_round_origin),
        "transition-relevant certified round cannot collide with carrier normalization"
    );
}
