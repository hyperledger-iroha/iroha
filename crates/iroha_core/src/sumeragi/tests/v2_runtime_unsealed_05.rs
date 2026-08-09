
#[test]
fn applied_local_proposal_handoff_suppresses_retry_before_ordinal_allocation() {
    const PHASE_INVENTORY: [&str; 1] = ["local_proposal_ready"];

    let directory = TempDir::new().expect("temporary local-proposal phase directory");
    let (fixture_context, _) = authenticated_runtime_context();
    let leader = fixture_context.leader(0);
    let (mut runtime, context, _keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(leader),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for local proposal dispatch");
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x9C);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    runtime
        .enqueue_local_proposal(tag, manifest.clone(), durable.clone(), validated.clone())
        .expect("enqueue exact local proposal completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("persist the exact proposal intent"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::Sign { .. }])
    ));

    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_local_proposal(tag, manifest, durable, validated)
        .expect("the durable proposal intent suppresses its exact callback retry");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert_eq!(["local_proposal_ready"], PHASE_INVENTORY);
}

#[test]
fn drained_internal_ignore_uses_exact_durable_tombstone_before_readmission() {
    const PHASE_INVENTORY: [&str; 2] = ["terminal_ignore", "restart_tombstone"];

    let directory = TempDir::new().expect("temporary runtime tombstone directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x9D);
    let ordinal_before_first = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("the first ownerless completion reaches its terminal reducer discard");
    assert_eq!(runtime.queued_commands(), 1);
    assert_ne!(runtime.ingress.next_admission_ordinal, ordinal_before_first);
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(Instant::now())
            .expect("drain the first ownerless completion"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));

    let next_ordinal = runtime.ingress.next_admission_ordinal;
    for _ in 0..3 {
        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("the exact terminal lifecycle coalesces in-process");
    }
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    let mut suppressed_phases = vec!["terminal_ignore"];
    drop(runtime);

    let (mut restarted, restarted_context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    assert_eq!(restarted_context.id(), context.id());
    let restarted_tag = restarted.round_tag();
    let next_ordinal = restarted.ingress.next_admission_ordinal;
    for _ in 0..3 {
        restarted
            .enqueue_body_available(restarted_tag, manifest.clone())
            .expect("the exact terminal lifecycle coalesces after restart");
    }
    assert_eq!(restarted.queued_commands(), 0);
    assert_eq!(restarted.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("restart_tombstone");
    assert_eq!(suppressed_phases, PHASE_INVENTORY);
}

#[test]
fn stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal() {
    let stale_directory = TempDir::new().expect("temporary stale internal-callback directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
    let current = runtime.round_tag();
    let stale = EventTag::new(
        current.height(),
        current.view(),
        Generation::new(current.generation().get().saturating_sub(1)),
    );
    let manifest = runtime_manifest(&context, 0x9E);
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_body_available(stale, manifest.clone())
        .expect("valid stale internal callback is discarded before admission");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    drop(runtime);

    let (mut restarted, restarted_context, _keys) =
        authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
    assert_eq!(restarted_context.id(), context.id());
    let next_ordinal = restarted.ingress.next_admission_ordinal;
    restarted
        .enqueue_body_available(restarted.round_tag(), manifest)
        .expect("stale discard did not create a current-incarnation tombstone");
    assert_eq!(restarted.queued_commands(), 1);
    assert_ne!(restarted.ingress.next_admission_ordinal, next_ordinal);

    let malformed_directory =
        TempDir::new().expect("temporary malformed internal-callback directory");
    let (mut malformed_runtime, malformed_context, _keys) =
        authenticated_network_runtime(&malformed_directory, RuntimeQueueConfig::new(8, 1, 1));
    let mut malformed_manifest = runtime_manifest(&malformed_context, 0x9F);
    let mut foreign_context = malformed_context.clone();
    foreign_context.chain_id = "foreign-runtime-preflight".into();
    malformed_manifest.round.context_id = foreign_context.id();
    let next_ordinal = malformed_runtime.ingress.next_admission_ordinal;
    assert_eq!(
        malformed_runtime.enqueue_body_available(malformed_runtime.round_tag(), malformed_manifest),
        Err(EnqueueError::FailClosed)
    );
    assert_eq!(malformed_runtime.queued_commands(), 0);
    assert_eq!(
        malformed_runtime.ingress.next_admission_ordinal,
        next_ordinal
    );
    assert!(malformed_runtime.fail_closed);
}

#[test]
fn body_pipeline_retirement_spans_ingress_and_busy_deferred_owners_and_rejects_duplicates() {
    let directory = TempDir::new().expect("temporary body-pipeline retirement directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = runtime.round_tag();
    let receipts = |manifest: &wire::PayloadManifest| {
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        (durable, validated)
    };
    let three_stages = RetiredBodyPipelineCompletions {
        body_available: 0,
        body_stored: 1,
        validation: 1,
        local_proposal: 1,
    };
    let validation_only = RetiredBodyPipelineCompletions {
        body_available: 0,
        body_stored: 0,
        validation: 1,
        local_proposal: 0,
    };
    let body_available_only = RetiredBodyPipelineCompletions {
        body_available: 1,
        body_stored: 0,
        validation: 0,
        local_proposal: 0,
    };

    let dormant_manifest = runtime_manifest(&context, 0xA0);
    let dormant_lifecycle_key = Hash::new(b"bulk-retired dormant body lifecycle");
    let dormant_lifecycle_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("mint the restart-restored body lifecycle");
    let dormant_command = AdapterCommand::BodyAvailable {
        manifest: dormant_manifest.clone(),
    };
    let dormant_owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
        owner_tag,
        CommandClass::Completion,
        &dormant_command,
        None,
        dormant_lifecycle_key,
        dormant_lifecycle_ordinal,
    )
    .expect("restore the exact dormant body owner");
    let dormant_statement = RuntimeCandidateSemanticStatement::new(
        dormant_manifest.round,
        dormant_manifest.round,
        Some(dormant_manifest.subject),
        None,
        None,
    );
    let dormant = RuntimeDormantLocalFifoReservation::completion(
        dormant_lifecycle_key,
        dormant_lifecycle_ordinal,
        8,
    );
    let capacity_before_dormant = runtime.remaining_completion_capacity();
    runtime
        .ingress
        .install_dormant_local_fifo_reservations(vec![dormant])
        .expect("install one dormant body-pipeline slot");
    let dormant_reservation = runtime
        .ingress
        .reserve_canonical_body_available_internal(
            owner_tag,
            dormant_manifest.clone(),
            Some(&dormant_owner),
            Some(dormant_statement),
            Some(8),
        )
        .expect("reserve an unpublished token backed by the dormant slot");
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before_dormant - 1,
        "the token aliases rather than duplicates its dormant capacity charge",
    );
    assert_eq!(
        runtime.ingress.body_pipeline_completion_counts(
            owner_tag,
            dormant_manifest.round,
            dormant_manifest.subject,
        ),
        body_available_only,
        "the unpublished reservation is exactly one BodyAvailable owner",
    );
    let dormant_mismatch = runtime_manifest(&context, 0xAF);
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                dormant_mismatch.round,
                dormant_mismatch.subject,
            )
            .expect("mismatched bulk retirement is an atomic no-op"),
        RetiredBodyPipelineCompletions::default(),
    );
    assert_eq!(
        runtime.ingress.reserved_body_available.as_ref(),
        Some(&dormant_reservation),
    );
    assert!(
        runtime
            .ingress
            .dormant_local_fifo_reservations
            .contains(&dormant)
    );
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before_dormant - 1,
        "mismatched bulk retirement preserves the aliased capacity charge",
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                dormant_manifest.round,
                dormant_manifest.subject,
            )
            .expect("retire the unpublished dormant-backed body token"),
        body_available_only,
    );
    assert!(runtime.ingress.reserved_body_available.is_none());
    assert!(
        !runtime
            .ingress
            .dormant_local_fifo_reservations
            .contains(&dormant)
    );
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before_dormant,
        "bulk retirement releases the token and its one aliased capacity owner",
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                dormant_manifest.round,
                dormant_manifest.subject,
            )
            .expect("a repeated exact retirement cannot recreate the drained stage"),
        RetiredBodyPipelineCompletions::default(),
    );
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before_dormant,
        "repeated retirement cannot reacquire or release capacity",
    );

    let ingress_manifest = runtime_manifest(&context, 0xA1);
    let (durable, validated) = receipts(&ingress_manifest);
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::BodyStored {
            round: ingress_manifest.round,
            subject: ingress_manifest.subject,
            receipt: durable.clone(),
        },
    );
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::ValidationSucceeded {
            round: ingress_manifest.round,
            subject: ingress_manifest.subject,
            receipt: validated.clone(),
        },
    );
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: ingress_manifest.clone(),
            durable_receipt: durable,
            validated_receipt: validated,
        },
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
            )
            .expect("retire ingress body pipeline"),
        three_stages
    );

    let ingress_failure_manifest = runtime_manifest(&context, 0xA2);
    runtime
        .enqueue_validation_failed(
            owner_tag,
            ingress_failure_manifest.round,
            ingress_failure_manifest.subject,
        )
        .expect("enqueue ingress validation-failure owner");
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                ingress_failure_manifest.round,
                ingress_failure_manifest.subject,
            )
            .expect("retire ingress validation failure"),
        validation_only
    );

    let deferred_manifest = runtime_manifest(&context, 0xB1);
    for stage in [
        DeferredBodyPipelineStageForTest::BodyStored,
        DeferredBodyPipelineStageForTest::ValidationSucceeded,
        DeferredBodyPipelineStageForTest::LocalProposalReady,
    ] {
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(owner_tag, &deferred_manifest, stage)
            .expect("stage Busy-deferred body completion");
    }
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_manifest.round,
                deferred_manifest.subject,
            )
            .expect("retire Busy-deferred body pipeline"),
        three_stages
    );

    let deferred_failure_manifest = runtime_manifest(&context, 0xB2);
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &deferred_failure_manifest,
            DeferredBodyPipelineStageForTest::ValidationFailed,
        )
        .expect("stage Busy-deferred validation failure");
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_failure_manifest.round,
                deferred_failure_manifest.subject,
            )
            .expect("retire Busy-deferred validation failure"),
        validation_only
    );

    let duplicate_body_stored = runtime_manifest(&context, 0xC1);
    let (durable, _) = receipts(&duplicate_body_stored);
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::BodyStored {
            round: duplicate_body_stored.round,
            subject: duplicate_body_stored.subject,
            receipt: durable,
        },
    );
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &duplicate_body_stored,
            DeferredBodyPipelineStageForTest::BodyStored,
        )
        .expect("stage duplicate deferred BodyStored owner");
    let stored_only = RetiredBodyPipelineCompletions {
        body_available: 0,
        body_stored: 1,
        validation: 0,
        local_proposal: 0,
    };
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.body_pipeline_completion_counts(
            owner_tag,
            duplicate_body_stored.round,
            duplicate_body_stored.subject,
        ),
        stored_only
    );
    assert_eq!(
        runtime.driver.deferred_body_pipeline_completion_counts(
            owner_tag,
            duplicate_body_stored.round,
            duplicate_body_stored.subject,
        ),
        stored_only
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            )
            .expect_err("duplicate BodyStored ownership must fail"),
        "Sumeragi v2 body pipeline has duplicate exact serialized completion stages"
    );
    assert!(runtime.fail_closed);
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.body_pipeline_completion_counts(
            owner_tag,
            duplicate_body_stored.round,
            duplicate_body_stored.subject,
        ),
        stored_only,
        "preflight must retain the ingress owner"
    );
    assert_eq!(
        runtime.driver.deferred_body_pipeline_completion_counts(
            owner_tag,
            duplicate_body_stored.round,
            duplicate_body_stored.subject,
        ),
        stored_only,
        "preflight must retain the Busy-deferred owner"
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            )
            .expect_err("fail-closed runtime must reject a second pipeline retirement"),
        "Sumeragi v2 runtime is fail-closed"
    );
    assert_eq!(
        runtime.enqueue_application_completed(owner_tag, duplicate_body_stored.subject,),
        Err(EnqueueError::FailClosed)
    );
    assert!(matches!(
        runtime.step(Instant::now()),
        Err(RuntimeError::FailClosed)
    ));

    let duplicate_directory =
        TempDir::new().expect("temporary duplicate dormant-body retirement directory");
    let (mut duplicate_runtime, duplicate_context, _keys) =
        authenticated_network_runtime(&duplicate_directory, RuntimeQueueConfig::new(4, 1, 1));
    let duplicate_tag = duplicate_runtime.round_tag();
    let duplicate_manifest = runtime_manifest(&duplicate_context, 0xD1);
    let duplicate_lifecycle_key = Hash::new(b"duplicate bulk-retired dormant body lifecycle");
    let duplicate_lifecycle_ordinal = duplicate_runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("mint the duplicate fixture's dormant lifecycle");
    let duplicate_command = AdapterCommand::BodyAvailable {
        manifest: duplicate_manifest.clone(),
    };
    let duplicate_owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
        duplicate_tag,
        CommandClass::Completion,
        &duplicate_command,
        None,
        duplicate_lifecycle_key,
        duplicate_lifecycle_ordinal,
    )
    .expect("restore the duplicate fixture's dormant body owner");
    let duplicate_statement = RuntimeCandidateSemanticStatement::new(
        duplicate_manifest.round,
        duplicate_manifest.round,
        Some(duplicate_manifest.subject),
        None,
        None,
    );
    let duplicate_dormant = RuntimeDormantLocalFifoReservation::completion(
        duplicate_lifecycle_key,
        duplicate_lifecycle_ordinal,
        8,
    );
    duplicate_runtime
        .ingress
        .install_dormant_local_fifo_reservations(vec![duplicate_dormant])
        .expect("install duplicate fixture dormant ownership");
    let duplicate_reservation = duplicate_runtime
        .ingress
        .reserve_canonical_body_available_internal(
            duplicate_tag,
            duplicate_manifest.clone(),
            Some(&duplicate_owner),
            Some(duplicate_statement),
            Some(8),
        )
        .expect("reserve duplicate fixture unpublished ownership");
    stage_completion_for_queue_test(&mut duplicate_runtime, duplicate_tag, duplicate_command);
    let duplicate_capacity_before_rejection = duplicate_runtime.remaining_completion_capacity();
    assert_eq!(
        duplicate_runtime.ingress.body_pipeline_completion_counts(
            duplicate_tag,
            duplicate_manifest.round,
            duplicate_manifest.subject,
        ),
        RetiredBodyPipelineCompletions {
            body_available: 2,
            body_stored: 0,
            validation: 0,
            local_proposal: 0,
        },
    );
    let duplicate_mismatch = runtime_manifest(&duplicate_context, 0xDF);
    assert_eq!(
        duplicate_runtime
            .retire_body_pipeline_completions(
                duplicate_tag,
                duplicate_mismatch.round,
                duplicate_mismatch.subject,
            )
            .expect("mismatched duplicate retirement is an atomic no-op"),
        RetiredBodyPipelineCompletions::default(),
    );
    assert_eq!(
        duplicate_runtime
            .retire_body_pipeline_completions(
                duplicate_tag,
                duplicate_manifest.round,
                duplicate_manifest.subject,
            )
            .expect_err("duplicate unpublished and queued owners must fail closed"),
        "Sumeragi v2 body pipeline has duplicate exact serialized completion stages",
    );
    assert!(duplicate_runtime.fail_closed);
    assert_eq!(
        duplicate_runtime.ingress.reserved_body_available.as_ref(),
        Some(&duplicate_reservation),
        "duplicate preflight cannot consume the unpublished token",
    );
    assert!(
        duplicate_runtime
            .ingress
            .dormant_local_fifo_reservations
            .contains(&duplicate_dormant)
    );
    assert_eq!(duplicate_runtime.queued_commands(), 1);
    assert_eq!(
        duplicate_runtime.remaining_completion_capacity(),
        duplicate_capacity_before_rejection,
        "duplicate preflight must preserve the complete capacity charge",
    );
}

#[test]
fn pre_dequeue_probe_validates_unfrozen_leader_wire_identity() {
    let directory = TempDir::new().expect("temporary pre-dequeue probe directory");
    let (runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(3, 1, 1));
    let fixture = leader_wire_proposal_fixture(
        &directory,
        &context,
        &keys,
        0xC0,
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let projected = RuntimeIngressOwnershipEvidence::from_fair_ingress(
        &fixture.message,
        fixture.ownership.clone(),
    )
    .expect("checked dequeue publishes exact runtime ownership");
    assert!(projected.validate_frozen_physical());
    assert!(
        projected
            .leader_wire_runtime_receipt()
            .is_ok_and(|receipt| receipt.is_some())
    );
}

#[test]
fn decision_retirement_releases_queued_leader_wire_runtime_owner() {
    let directory = TempDir::new().expect("temporary leader-wire Decision directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let fixture = leader_wire_proposal_fixture(
        &directory,
        &context,
        &keys,
        0xC1,
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &fixture.message.payload else {
        unreachable!("leader-wire fixture carries Proposal")
    };
    runtime
        .enqueue_network_with_ingress_ownership(fixture.message.clone(), fixture.ownership.clone())
        .expect("enqueue proposal with durable leader-wire runtime ownership");
    let ordinal = fixture.receipt.owner().admission_ordinal();
    assert_eq!(
        runtime.leader_wire_runtime_receipts.get(&ordinal),
        Some(&fixture.receipt)
    );

    let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"leader-wire Decision state root"),
        Hash::new(b"leader-wire Decision event root"),
        Hash::new(b"leader-wire Decision reject root"),
        1,
        Hash::new(b"leader-wire Decision fee root"),
    );
    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(proposal.round, proposal.subject, commitment,)
            .expect("Decision retires queued proposal ownership"),
        DecisionProposalRetirement::default()
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.leader_wire_runtime_receipts.contains_key(&ordinal));
    let terminals = runtime.take_leader_wire_runtime_terminals();
    let [LeaderWireRuntimeTerminal::Volatile(receipt)] = terminals.as_slice() else {
        panic!("Decision retirement must emit one volatile leader-wire terminal")
    };
    assert_volatile_leader_wire_release(&fixture, receipt);
    assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime after consuming Decision terminal");
    assert!(matches!(runtime.step(now), Ok(RuntimeStep::Idle)));
    assert!(!runtime.fail_closed);
}

#[test]
fn lock_retirement_releases_busy_deferred_leader_wire_runtime_owner() {
    let directory = TempDir::new().expect("temporary leader-wire lock directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let fixture = leader_wire_proposal_fixture(
        &directory,
        &context,
        &keys,
        0xC2,
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let (proposal, _deferred_ordinal) =
        bind_authenticated_deferred_proposal_for_test(&mut runtime, &fixture);
    let ordinal = fixture.receipt.owner().admission_ordinal();
    assert_eq!(
        runtime.leader_wire_runtime_receipts.get(&ordinal),
        Some(&fixture.receipt)
    );

    let locked_subject = runtime_manifest(&context, 0xC3).subject;
    assert_ne!(locked_subject, proposal.subject);
    assert_eq!(
        runtime
            .retire_unsafe_proposals_for_lock(proposal.round, locked_subject)
            .expect("lock retires unsafe Busy-deferred proposal"),
        1
    );
    assert!(
        runtime
            .driver
            .authenticated_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(!runtime.leader_wire_runtime_receipts.contains_key(&ordinal));
    let terminals = runtime.take_leader_wire_runtime_terminals();
    let [LeaderWireRuntimeTerminal::Volatile(receipt)] = terminals.as_slice() else {
        panic!("lock retirement must emit one volatile leader-wire terminal")
    };
    assert_volatile_leader_wire_release(&fixture, receipt);
    assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime after consuming lock terminal");
    assert!(matches!(runtime.step(now), Ok(RuntimeStep::Idle)));
    assert!(!runtime.fail_closed);

    // A BodyAvailable continuation can own an older causal lifecycle than
    // a proposal which crossed into Busy while the shared reducer fence
    // was closed. Once the fence opens, servicing that completion removes
    // the conflicting Busy proposal inside the adapter dispatch. The
    // runtime must terminalize the removed proposal's durable leader-wire
    // receipt after classifying the selected completion owner.
    let dispatch_directory =
        TempDir::new().expect("temporary dispatch-side leader-wire retirement directory");
    let (mut dispatch_runtime, dispatch_context, dispatch_keys) =
        authenticated_network_runtime(&dispatch_directory, RuntimeQueueConfig::new(8, 1, 1));
    let dispatch_tag = dispatch_runtime.round_tag();
    let body_parent = dispatch_runtime
        .mint_fresh_lifecycle_owner(
            dispatch_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"older-body-available-continuation",
        )
        .expect("reserve the older body continuation lifecycle");
    let body_ownership =
        RuntimeEffectOwnership::fresh(body_parent, RuntimeFreshRootKind::StartupRecovery);
    let dispatch_fixture = leader_wire_proposal_fixture(
        &dispatch_directory,
        &dispatch_context,
        &dispatch_keys,
        0xCB,
        dispatch_runtime.ingress.lifecycle_ordinals.clone(),
    );
    let (busy_proposal, busy_ordinal) =
        bind_authenticated_deferred_proposal_for_test(&mut dispatch_runtime, &dispatch_fixture);
    assert!(
        body_ownership.owner().lifecycle_ordinal()
            < dispatch_fixture.receipt.owner().admission_ordinal(),
        "the reconstructed body retains the frozen predecessor lifecycle"
    );
    let canonical_body = b"canonical body superseding Busy proposal".to_vec();
    let canonical_manifest = wire::PayloadManifest::derive(
        &dispatch_context,
        busy_proposal.round,
        busy_proposal.subject,
        u64::try_from(canonical_body.len()).expect("small canonical body length fits u64"),
        &[canonical_body],
    )
    .expect("derive a structurally valid conflicting canonical manifest");
    assert_ne!(canonical_manifest, busy_proposal.manifest);
    let reservation = dispatch_runtime
        .reserve_body_available_with_owner(dispatch_tag, canonical_manifest, &body_ownership)
        .expect("reserve the older causal BodyAvailable owner");
    dispatch_runtime
        .commit_body_available(reservation)
        .expect("publish the exact BodyAvailable completion");
    assert_eq!(dispatch_runtime.queued_commands(), 1);
    assert!(
        dispatch_runtime
            .eligible_deferred_admission_ordinals()
            .expect("compare the two exact lifecycle owners")
            .is_empty(),
        "the later Busy proposal cannot overtake the older body continuation"
    );
    assert!(
        dispatch_runtime
            .deferred_lifecycle_ownership
            .contains_key(&busy_ordinal)
    );

    let dispatch_now = Instant::now();
    dispatch_runtime
        .arm_live_clocks(dispatch_now)
        .expect("arm runtime for dispatch-side retirement");
    let body_step = dispatch_runtime
        .step(dispatch_now)
        .expect("the older BodyAvailable owner receives the FIFO turn");
    let body_scheduling = dispatch_runtime
        .take_last_scheduler_ownership()
        .expect("BodyAvailable dispatch retains exact scheduler ownership");
    assert_eq!(body_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
    let RuntimeStep::Advanced(body_effects) = body_step else {
        panic!("BodyAvailable dispatch unexpectedly idled")
    };
    dispatch_runtime
        .take_effect_ownership(body_effects.len())
        .expect("consume BodyAvailable effect ownership");
    assert_eq!(dispatch_runtime.queued_commands(), 0);
    assert!(
        dispatch_runtime
            .driver
            .authenticated_deferred_admission_ordinals()
            .is_empty()
    );
    assert!(dispatch_runtime.deferred_ingress_ownership.is_empty());
    assert!(dispatch_runtime.deferred_lifecycle_ownership.is_empty());
    let dispatch_receipt_ordinal = dispatch_fixture.receipt.owner().admission_ordinal();
    assert!(
        !dispatch_runtime
            .leader_wire_runtime_receipts
            .contains_key(&dispatch_receipt_ordinal)
    );
    let dispatch_terminals = dispatch_runtime.take_leader_wire_runtime_terminals();
    let [LeaderWireRuntimeTerminal::Volatile(receipt)] = dispatch_terminals.as_slice() else {
        panic!("BodyAvailable cleanup must retire the orphaned Busy proposal receipt")
    };
    assert_volatile_leader_wire_release(&dispatch_fixture, receipt);
    assert!(!dispatch_runtime.fail_closed);

    // Materializing the same older completion can prune a conflicting
    // proposal which is still in FIFO rather than Busy. Its durable
    // receipt is allowed to remain Runtime only while the exact finite
    // BodyAvailable predecessor is physically queued; servicing that
    // predecessor must publish the volatile terminal in the same turn.
    let queued_directory =
        TempDir::new().expect("temporary queued leader-wire retirement directory");
    let (mut queued_runtime, queued_context, queued_keys) =
        authenticated_network_runtime(&queued_directory, RuntimeQueueConfig::new(8, 1, 1));
    let queued_tag = queued_runtime.round_tag();
    let queued_body_parent = queued_runtime
        .mint_fresh_lifecycle_owner(
            queued_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"older-queued-body-available-continuation",
        )
        .expect("reserve the older queued body lifecycle");
    let queued_body_ownership =
        RuntimeEffectOwnership::fresh(queued_body_parent, RuntimeFreshRootKind::StartupRecovery);
    let queued_fixture = leader_wire_proposal_fixture(
        &queued_directory,
        &queued_context,
        &queued_keys,
        0xCC,
        queued_runtime.ingress.lifecycle_ordinals.clone(),
    );
    let wire::ConsensusMessageV2Payload::Proposal(queued_proposal) =
        &queued_fixture.message.payload
    else {
        unreachable!("queued leader-wire fixture carries Proposal")
    };
    queued_runtime
        .enqueue_network_with_ingress_ownership(
            queued_fixture.message.clone(),
            queued_fixture.ownership.clone(),
        )
        .expect("enqueue the conflicting leader-wire proposal");
    let queued_receipt_ordinal = queued_fixture.receipt.owner().admission_ordinal();
    assert!(
        queued_body_ownership.owner().lifecycle_ordinal() < queued_receipt_ordinal,
        "the body completion retains the older causal lifecycle"
    );
    let queued_canonical_body = b"canonical body superseding queued proposal".to_vec();
    let queued_canonical_manifest = wire::PayloadManifest::derive(
        &queued_context,
        queued_proposal.round,
        queued_proposal.subject,
        u64::try_from(queued_canonical_body.len())
            .expect("small queued canonical body length fits u64"),
        &[queued_canonical_body],
    )
    .expect("derive a conflicting canonical manifest for the queued proposal");
    assert_ne!(queued_canonical_manifest, queued_proposal.manifest);
    let queued_reservation = queued_runtime
        .reserve_body_available_with_owner(
            queued_tag,
            queued_canonical_manifest,
            &queued_body_ownership,
        )
        .expect("reserve the queued-prune BodyAvailable owner");
    queued_runtime
        .commit_body_available(queued_reservation)
        .expect("atomically replace the conflicting FIFO proposal");
    assert_eq!(queued_runtime.queued_commands(), 1);
    assert!(
        queued_runtime
            .ingress
            .commands
            .iter()
            .all(|queued| matches!(&queued.command, AdapterCommand::BodyAvailable { .. }))
    );
    assert_eq!(
        queued_runtime
            .leader_wire_runtime_receipts
            .get(&queued_receipt_ordinal),
        Some(&queued_fixture.receipt),
        "the finite queued completion temporarily owns retirement of the pruned receipt"
    );
    assert!(queued_runtime.pending_leader_wire_terminals.is_empty());

    let queued_now = Instant::now();
    queued_runtime
        .arm_live_clocks(queued_now)
        .expect("arm runtime for queued-prune retirement");
    let queued_body_step = queued_runtime
        .step(queued_now)
        .expect("service the exact queued BodyAvailable predecessor");
    let queued_scheduling = queued_runtime
        .take_last_scheduler_ownership()
        .expect("queued BodyAvailable dispatch retains scheduler ownership");
    assert_eq!(queued_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
    let RuntimeStep::Advanced(queued_body_effects) = queued_body_step else {
        panic!("queued BodyAvailable dispatch unexpectedly idled")
    };
    queued_runtime
        .take_effect_ownership(queued_body_effects.len())
        .expect("consume queued BodyAvailable effect ownership");
    assert_eq!(queued_runtime.queued_commands(), 0);
    assert!(
        !queued_runtime
            .leader_wire_runtime_receipts
            .contains_key(&queued_receipt_ordinal)
    );
    let queued_terminals = queued_runtime.take_leader_wire_runtime_terminals();
    let [LeaderWireRuntimeTerminal::Volatile(receipt)] = queued_terminals.as_slice() else {
        panic!("queued proposal pruning must emit one volatile leader-wire terminal")
    };
    assert_volatile_leader_wire_release(&queued_fixture, receipt);
    assert!(!queued_runtime.fail_closed);
}

#[test]
fn production_authenticated_preflight_is_never_semantic_only_coalesce() {
    let directory = TempDir::new().expect("temporary authenticated-preflight directory");
    let (runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let message = signed_runtime_proposal(&context, &keys, 0xC4);
    let authenticated = runtime
        .driver
        .authenticate(message)
        .expect("authenticate the production Proposal command");
    let command = AdapterCommand::Authenticated(authenticated);

    assert_eq!(
        runtime
            .driver
            .preflight_runtime_command_admission(runtime.round_tag(), &command),
        RuntimeCommandAdmissionPreflight::Admit
    );
}

#[test]
fn semantic_only_authenticated_coalesce_fails_before_receipt_registration() {
    let directory = TempDir::new().expect("temporary coalesce-defense directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let existing = signed_runtime_proposal(&context, &keys, 0xC5);
    runtime
        .enqueue_network(existing)
        .expect("retain an existing authenticated semantic owner");
    let queued_before = runtime.queued_commands();

    let candidate = leader_wire_proposal_fixture(
        &directory,
        &context,
        &keys,
        0xC6,
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let candidate_ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
        &candidate.message,
        candidate.ownership.clone(),
    )
    .expect("project the fresh leader-wire runtime receipt");
    assert!(
        candidate_ownership
            .leader_wire_runtime_receipt()
            .expect("inspect exact candidate receipt")
            .is_some()
    );
    assert!(runtime.leader_wire_runtime_receipts.is_empty());

    assert!(matches!(
            runtime.reject_authenticated_preflight_coalescence(
                RuntimeCommandAdmissionPreflight::Coalesce,
            ),
            Err(NetworkIngressError::FailClosed)
        ));
    assert_eq!(
        runtime.queued_commands(),
        queued_before,
        "defensive rejection must not delete the existing semantic owner"
    );
    assert!(
        runtime.leader_wire_runtime_receipts.is_empty(),
        "semantic-only coalescence cannot register an ownerless runtime receipt"
    );
    assert!(runtime.pending_leader_wire_terminals.is_empty());
    assert!(runtime.fail_closed);
}

#[test]
fn decision_retires_proposal_owners_but_preserves_body_and_application_completions() {
    let directory = TempDir::new().expect("temporary decision-retirement directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(12, 1, 1));
    let owner_tag = runtime.round_tag();
    let receipts = |manifest: &wire::PayloadManifest| {
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        (durable, validated)
    };

    let decision_manifest = runtime_manifest(&context, 0xD0);
    let (decision_durable, decision_validated) = receipts(&decision_manifest);
    let decision_commitment = decision_validated.execution_commitment();
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0xD1))
        .expect("enqueue authenticated proposal at decided height");
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: decision_manifest.clone(),
            durable_receipt: decision_durable.clone(),
            validated_receipt: decision_validated,
        },
    );
    let other_local_manifest = runtime_manifest(&context, 0xD2);
    let (other_durable, other_validated) = receipts(&other_local_manifest);
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: other_local_manifest.clone(),
            durable_receipt: other_durable,
            validated_receipt: other_validated,
        },
    );
    runtime
        .enqueue_body_available(owner_tag, decision_manifest.clone())
        .expect("enqueue body-recovery completion");
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::BodyStored {
            round: decision_manifest.round,
            subject: decision_manifest.subject,
            receipt: decision_durable,
        },
    );
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::ApplicationCompleted(decision_manifest.subject),
    );

    let deferred_proposal = match signed_runtime_proposal(&context, &keys, 0xD3).payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal,
        _ => unreachable!("fixture is a proposal"),
    };
    runtime
        .driver
        .defer_authenticated_proposal_for_test(owner_tag, &deferred_proposal)
        .expect("stage Busy-deferred authenticated proposal");
    let deferred_local_manifest = runtime_manifest(&context, 0xD4);
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &deferred_local_manifest,
            DeferredBodyPipelineStageForTest::LocalProposalReady,
        )
        .expect("stage Busy-deferred LocalProposalReady");
    let deferred_body_manifest = runtime_manifest(&context, 0xD5);
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &deferred_body_manifest,
            DeferredBodyPipelineStageForTest::BodyStored,
        )
        .expect("stage Busy-deferred body-store completion");
    assert_eq!(
        runtime
            .driver
            .status()
            .expect("status before decision retirement")
            .liveness
            .work
            .candidate,
        wire::SumeragiV2LocalWorkStage::Complete
    );

    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(
                decision_manifest.round,
                decision_manifest.subject,
                decision_commitment,
            )
            .expect("retire proposal work after decision"),
        DecisionProposalRetirement::new(Some(owner_tag), 0),
        "the exact current-tag LocalProposalReady owner must remain queued"
    );
    assert_eq!(runtime.queued_commands(), 4);
    assert!(runtime.ingress.commands.iter().all(|queued| !matches!(
        &queued.command,
        AdapterCommand::Authenticated(authenticated)
            if matches!(
                authenticated.payload(),
                wire::ConsensusMessageV2Payload::Proposal(_)
            )
    )));
    assert!(runtime.ingress.commands.iter().any(|queued| matches!(
        &queued.command,
        AdapterCommand::LocalProposalReady { manifest, .. }
            if manifest == &decision_manifest
    )));
    assert!(
        runtime
            .ingress
            .commands
            .iter()
            .any(|queued| matches!(&queued.command, AdapterCommand::BodyAvailable { .. }))
    );
    assert!(
        runtime
            .ingress
            .commands
            .iter()
            .any(|queued| matches!(&queued.command, AdapterCommand::BodyStored { .. }))
    );
    assert!(
        runtime
            .ingress
            .commands
            .iter()
            .any(|queued| matches!(&queued.command, AdapterCommand::ApplicationCompleted(_)))
    );
    assert_eq!(
        runtime
            .driver
            .status()
            .expect("status after decision retirement")
            .liveness
            .work
            .candidate,
        wire::SumeragiV2LocalWorkStage::Idle,
        "decision retirement clears stale active proposal state"
    );
    let deferred_local_commitment = receipts(&deferred_local_manifest).1.execution_commitment();
    assert_eq!(
        runtime
            .ingress
            .decided_local_proposal_counts(
                owner_tag,
                deferred_local_manifest.round,
                deferred_local_manifest.subject,
                deferred_local_commitment,
            )
            .merge(runtime.driver.deferred_decided_local_proposal_counts(
                owner_tag,
                deferred_local_manifest.round,
                deferred_local_manifest.subject,
                deferred_local_commitment,
            )),
        DecisionLocalProposalCounts::default(),
        "all nonmatching local proposal completions were retired"
    );

    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                decision_manifest.round,
                decision_manifest.subject,
            )
            .expect("body recovery remains queued after decision"),
        RetiredBodyPipelineCompletions {
            body_available: 1,
            body_stored: 1,
            validation: 0,
            local_proposal: 1,
        }
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_body_manifest.round,
                deferred_body_manifest.subject,
            )
            .expect("Busy-deferred body store remains queued after decision"),
        RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 0,
            local_proposal: 0,
        }
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.ingress.commands.front().map(|queued| &queued.command),
        Some(AdapterCommand::ApplicationCompleted(subject))
            if *subject == decision_manifest.subject
    ));

    let duplicate_manifest = runtime_manifest(&context, 0xD6);
    let (duplicate_durable, duplicate_validated) = receipts(&duplicate_manifest);
    let duplicate_commitment = duplicate_validated.execution_commitment();
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: duplicate_manifest.clone(),
            durable_receipt: duplicate_durable,
            validated_receipt: duplicate_validated,
        },
    );
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &duplicate_manifest,
            DeferredBodyPipelineStageForTest::LocalProposalReady,
        )
        .expect("stage duplicate exact local completion in Busy-deferred lane");
    assert_eq!(runtime.queued_commands(), 2);
    assert_eq!(
        runtime
            .ingress
            .decided_local_proposal_counts(
                owner_tag,
                duplicate_manifest.round,
                duplicate_manifest.subject,
                duplicate_commitment,
            )
            .retainable(),
        1,
    );
    assert_eq!(
        runtime
            .driver
            .deferred_decided_local_proposal_counts(
                owner_tag,
                duplicate_manifest.round,
                duplicate_manifest.subject,
                duplicate_commitment,
            )
            .retainable(),
        1,
    );
    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(
                duplicate_manifest.round,
                duplicate_manifest.subject,
                duplicate_commitment,
            )
            .expect_err("duplicate exact local completion ownership must fail"),
        "Sumeragi v2 decided local proposal completion has duplicate serialized owners"
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.queued_commands(),
        2,
        "preflight must retain the application and ingress proposal owners"
    );
    assert_eq!(
        runtime
            .ingress
            .decided_local_proposal_counts(
                owner_tag,
                duplicate_manifest.round,
                duplicate_manifest.subject,
                duplicate_commitment,
            )
            .retainable(),
        1,
    );
    assert_eq!(
        runtime
            .driver
            .deferred_decided_local_proposal_counts(
                owner_tag,
                duplicate_manifest.round,
                duplicate_manifest.subject,
                duplicate_commitment,
            )
            .retainable(),
        1,
        "preflight must retain the Busy-deferred proposal owner"
    );
    assert_eq!(
        runtime
            .retire_proposal_work_after_decision(
                duplicate_manifest.round,
                duplicate_manifest.subject,
                duplicate_commitment,
            )
            .expect_err("fail-closed runtime must reject a second proposal retirement"),
        "Sumeragi v2 runtime is fail-closed"
    );
    assert_eq!(
        runtime.enqueue_signature(owner_tag, vec![0xD6]),
        Err(EnqueueError::FailClosed)
    );
    assert!(matches!(
        runtime.step(Instant::now()),
        Err(RuntimeError::FailClosed)
    ));
}

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
    assert_eq!(runtime.queued_commands(), 1);
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
        .expect("enqueue the CommitQC progress item");
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
