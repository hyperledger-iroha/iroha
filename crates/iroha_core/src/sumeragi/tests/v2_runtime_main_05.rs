#[test]
fn conflicting_body_pipeline_evidence_fails_closed_before_body_available_pruning() {
    let body_directory = TempDir::new().expect("temporary body evidence directory");
    let (mut body_runtime, context, keys) =
        authenticated_network_runtime(&body_directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = body_runtime.round_tag();
    let proposal = signed_runtime_proposal(&context, &keys, 0x95);
    let manifest = match &proposal.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal.manifest.clone(),
        _ => unreachable!("fixture is a proposal"),
    };
    body_runtime
        .enqueue_network(proposal)
        .expect("enqueue the exact authenticated proposal");
    body_runtime
        .enqueue_body_available(owner_tag, manifest.clone())
        .expect("enqueue the first canonical body completion");
    assert_eq!(body_runtime.queued_commands(), 2);

    let mut conflicting_manifest = manifest.clone();
    conflicting_manifest.chunk_hashes[0] = Hash::new(b"conflicting completion chunk");
    conflicting_manifest.chunk_root = Hash::new(b"conflicting completion root");
    assert_eq!(
        body_runtime.enqueue_body_available(owner_tag, conflicting_manifest),
        Err(EnqueueError::DuplicateCompletionOwnership)
    );
    assert!(body_runtime.fail_closed);
    assert_eq!(
        body_runtime.queued_commands(),
        2,
        "ownership must fail before a conflicting completion prunes the exact proposal"
    );
    assert!(body_runtime.ingress.commands.iter().any(|queued| matches!(
        &queued.command,
        AdapterCommand::Authenticated(authenticated)
            if matches!(
                authenticated.payload(),
                wire::ConsensusMessageV2Payload::Proposal(proposal)
                    if proposal.manifest == manifest
            )
    )));
    assert_eq!(
        body_runtime.enqueue_body_available(owner_tag, manifest),
        Err(EnqueueError::FailClosed)
    );

    let stored_directory = TempDir::new().expect("temporary durable evidence directory");
    let (mut stored_runtime, context, _keys) =
        authenticated_network_runtime(&stored_directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = stored_runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x96);
    let exact_receipt = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let mut other_manifest = manifest.clone();
    other_manifest.chunk_hashes[0] = Hash::new(b"different durable receipt chunk");
    other_manifest.chunk_root = Hash::new(b"different durable receipt root");
    let conflicting_receipt = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&other_manifest),
    );
    stage_completion_for_queue_test(
        &mut stored_runtime,
        owner_tag,
        AdapterCommand::BodyStored {
            round: manifest.round,
            subject: manifest.subject,
            receipt: exact_receipt,
        },
    );
    assert_eq!(
        stored_runtime.enqueue_body_stored(
            owner_tag,
            manifest.round,
            manifest.subject,
            conflicting_receipt,
        ),
        Err(EnqueueError::DuplicateCompletionOwnership)
    );
    assert!(stored_runtime.fail_closed);

    let validation_directory = TempDir::new().expect("temporary validation polarity directory");
    let (mut validation_runtime, context, _keys) =
        authenticated_network_runtime(&validation_directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = validation_runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x97);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    stage_completion_for_queue_test(
        &mut validation_runtime,
        owner_tag,
        AdapterCommand::ValidationSucceeded {
            round: manifest.round,
            subject: manifest.subject,
            receipt: ValidatedBodyReceipt::for_test(durable),
        },
    );
    assert_eq!(
        validation_runtime.enqueue_validation_failed(owner_tag, manifest.round, manifest.subject,),
        Err(EnqueueError::DuplicateCompletionOwnership),
        "opposite validation polarity is conflicting evidence"
    );
    assert!(validation_runtime.fail_closed);

    let deferred_failure_directory =
        TempDir::new().expect("temporary deferred validation-failure directory");
    let (mut deferred_failure_runtime, context, _keys) = authenticated_network_runtime(
        &deferred_failure_directory,
        RuntimeQueueConfig::new(8, 1, 1),
    );
    let owner_tag = deferred_failure_runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x9B);
    deferred_failure_runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &manifest,
            DeferredBodyPipelineStageForTest::ValidationFailed,
        )
        .expect("stage Busy-deferred validation failure");
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    assert_eq!(
        deferred_failure_runtime.enqueue_validation_succeeded(
            owner_tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
        ),
        Err(EnqueueError::DuplicateCompletionOwnership),
        "Busy-deferred failure cannot coalesce an incoming success"
    );
    assert!(deferred_failure_runtime.fail_closed);

    let deferred_success_directory =
        TempDir::new().expect("temporary deferred validation-success directory");
    let (mut deferred_success_runtime, context, _keys) = authenticated_network_runtime(
        &deferred_success_directory,
        RuntimeQueueConfig::new(8, 1, 1),
    );
    let owner_tag = deferred_success_runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x9C);
    deferred_success_runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &manifest,
            DeferredBodyPipelineStageForTest::ValidationSucceeded,
        )
        .expect("stage Busy-deferred validation success");
    assert_eq!(
        deferred_success_runtime.enqueue_validation_failed(
            owner_tag,
            manifest.round,
            manifest.subject,
        ),
        Err(EnqueueError::DuplicateCompletionOwnership),
        "Busy-deferred success cannot coalesce an incoming failure"
    );
    assert!(deferred_success_runtime.fail_closed);

    let atomic_directory = TempDir::new().expect("temporary atomic validation directory");
    let (mut atomic_runtime, context, _keys) =
        authenticated_network_runtime(&atomic_directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = atomic_runtime.round_tag();
    let manifests = [0x9D, 0x9E, 0x9F, 0xA0].map(|seed| runtime_manifest(&context, seed));
    let failures = manifests
        .iter()
        .map(|manifest| (owner_tag, manifest.round, manifest.subject))
        .collect::<Vec<_>>();
    let next_ordinal_before_wrong_class = atomic_runtime.ingress.next_admission_ordinal;
    let (wrong_tag, wrong_round, wrong_subject) = failures[0];
    assert_eq!(
        atomic_runtime
            .ingress
            .enqueue_completion_batch(vec![TaggedCommand::new(
                wrong_tag,
                CommandClass::Normal,
                AdapterCommand::ValidationFailed {
                    round: wrong_round,
                    subject: wrong_subject,
                },
                Instant::now(),
            )]),
        Err(EnqueueError::FailClosed),
        "a batch API cannot relabel non-completion traffic as trusted completion work"
    );
    assert_eq!(atomic_runtime.queued_commands(), 0);
    assert_eq!(
        atomic_runtime.ingress.next_admission_ordinal, next_ordinal_before_wrong_class,
        "rejected batch traffic cannot spend an admission ordinal"
    );
    assert_eq!(
        atomic_runtime.enqueue_validation_failures_atomically(&failures),
        Err(EnqueueError::Full)
    );
    assert_eq!(
        atomic_runtime.queued_commands(),
        0,
        "a capacity failure cannot publish an earlier member of the batch"
    );
    atomic_runtime
        .enqueue_validation_failures_atomically(&failures[..3])
        .expect("the complete fitting batch is admitted atomically");
    assert_eq!(atomic_runtime.queued_commands(), 3);
    for (queued, (tag, round, subject)) in atomic_runtime
        .ingress
        .commands
        .iter()
        .zip(failures.iter().copied())
    {
        assert_eq!(queued.tag, tag);
        assert!(matches!(
            &queued.command,
            AdapterCommand::ValidationFailed {
                round: queued_round,
                subject: queued_subject,
            } if *queued_round == round && *queued_subject == subject
        ));
    }
    atomic_runtime
        .enqueue_validation_failures_atomically(&failures[..3])
        .expect("exact pre-owned rows coalesce without spending capacity");
    assert_eq!(atomic_runtime.queued_commands(), 3);

    let conflict_directory =
        TempDir::new().expect("temporary conflicting atomic validation directory");
    let (mut conflict_runtime, conflict_context, _keys) =
        authenticated_network_runtime(&conflict_directory, RuntimeQueueConfig::new(4, 1, 1));
    let conflict_tag = conflict_runtime.round_tag();
    let vacant = runtime_manifest(&conflict_context, 0xA1);
    let conflicting = runtime_manifest(&conflict_context, 0xA2);
    let durable = DurableBodyReceipt::for_test(
        conflict_context.id(),
        conflicting.round,
        conflicting.subject,
        HashOf::new(&conflicting),
    );
    stage_completion_for_queue_test(
        &mut conflict_runtime,
        conflict_tag,
        AdapterCommand::ValidationSucceeded {
            round: conflicting.round,
            subject: conflicting.subject,
            receipt: ValidatedBodyReceipt::for_test(durable),
        },
    );
    assert_eq!(
        conflict_runtime.enqueue_validation_failures_atomically(&[
            (conflict_tag, vacant.round, vacant.subject),
            (conflict_tag, conflicting.round, conflicting.subject),
        ]),
        Err(EnqueueError::DuplicateCompletionOwnership)
    );
    assert_eq!(
        conflict_runtime.queued_commands(),
        1,
        "the vacant prefix cannot become visible before a later conflict"
    );
    assert!(conflict_runtime.fail_closed);
}

#[test]
fn conflicting_local_and_validated_receipts_do_not_coalesce() {
    let validation_directory = TempDir::new().expect("temporary execution commitment directory");
    let (mut validation_runtime, context, _keys) =
        authenticated_network_runtime(&validation_directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = validation_runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x98);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let exact_validated = ValidatedBodyReceipt::for_test(durable.clone());
    let conflicting_validated = ValidatedBodyReceipt::for_test_with_commitment(
        durable,
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting parent state"),
            Hash::new(b"conflicting post state"),
            Hash::new(b"conflicting ordinary writes"),
            1,
            Hash::new(b"conflicting executed body"),
        ),
    );
    stage_completion_for_queue_test(
        &mut validation_runtime,
        owner_tag,
        AdapterCommand::ValidationSucceeded {
            round: manifest.round,
            subject: manifest.subject,
            receipt: exact_validated,
        },
    );
    assert_eq!(
        validation_runtime.enqueue_validation_succeeded(
            owner_tag,
            manifest.round,
            manifest.subject,
            conflicting_validated,
        ),
        Err(EnqueueError::DuplicateCompletionOwnership)
    );
    assert!(validation_runtime.fail_closed);

    let proposal_directory = TempDir::new().expect("temporary local proposal directory");
    let (mut proposal_runtime, context, _keys) =
        authenticated_network_runtime(&proposal_directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = proposal_runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x99);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    stage_completion_for_queue_test(
        &mut proposal_runtime,
        owner_tag,
        AdapterCommand::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable,
            validated_receipt: validated,
        },
    );

    let mut conflicting_manifest = manifest.clone();
    conflicting_manifest.chunk_hashes[0] = Hash::new(b"conflicting local proposal chunk");
    conflicting_manifest.chunk_root = Hash::new(b"conflicting local proposal root");
    let conflicting_durable = DurableBodyReceipt::for_test(
        context.id(),
        conflicting_manifest.round,
        conflicting_manifest.subject,
        HashOf::new(&conflicting_manifest),
    );
    let conflicting_validated = ValidatedBodyReceipt::for_test(conflicting_durable.clone());
    assert_eq!(
        proposal_runtime.enqueue_local_proposal(
            owner_tag,
            conflicting_manifest,
            conflicting_durable,
            conflicting_validated,
        ),
        Err(EnqueueError::DuplicateCompletionOwnership)
    );
    assert!(proposal_runtime.fail_closed);
}

#[test]
fn applied_body_pipeline_phases_suppress_retries_before_ordinal_allocation() {
    const PHASE_INVENTORY: [&str; 4] = [
        "body_available",
        "body_stored",
        "validation_succeeded",
        "signature_completed",
    ];

    let directory = TempDir::new().expect("temporary production phase-inventory directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for production dispatch");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x9A))
        .expect("enqueue authenticated proposal");
    let proposal_effects = match runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("dispatch proposal")
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
    let mut suppressed_phases = Vec::new();

    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("enqueue body reconstruction completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch body reconstruction"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
    ));
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("an applied BodyAvailable retry is a monotone stutter");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("body_available");

    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
        .expect("enqueue durable-store completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch durable-store completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
    ));
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
        .expect("an applied BodyStored retry is a monotone stutter");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("body_stored");

    let validated = ValidatedBodyReceipt::for_test(durable);
    runtime
        .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
        .expect("enqueue validation completion");
    let (signature_tag, signature_preimage) = match runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("dispatch validation completion")
    {
        RuntimeStep::Advanced(effects) => match effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                },
            ] => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected validation effects: {effects:?}"),
        },
        RuntimeStep::Idle => panic!("validation completion unexpectedly idle"),
    };
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
        .expect("an applied validation retry is a monotone stutter");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("validation_succeeded");

    let signature = Signature::new(keys[0].private_key(), &signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature(signature_tag, signature.clone())
        .expect("enqueue exact signature completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch exact signature completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
    ));
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_signature(signature_tag, signature)
        .expect("an applied signature retry is a monotone stutter");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    suppressed_phases.push("signature_completed");

    assert_eq!(
        runtime
            .retire_body_pipeline_completions(tag, manifest.round, manifest.subject)
            .expect("no applied callback remains physically owned"),
        RetiredBodyPipelineCompletions::default()
    );
    assert_eq!(suppressed_phases, PHASE_INVENTORY);
}

#[test]
fn applied_validation_failure_suppresses_retry_and_rejects_opposite_outcome() {
    const PHASE_INVENTORY: [&str; 1] = ["validation_failed"];

    let directory = TempDir::new().expect("temporary failed-validation phase directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for failed-validation dispatch");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x9B))
        .expect("enqueue authenticated proposal");
    let (tag, manifest) = match runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("dispatch proposal")
    {
        RuntimeStep::Advanced(effects) => match effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        },
        RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
    };
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("enqueue body reconstruction completion");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch body reconstruction"),
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
        .expect("enqueue durable-store completion");
    let validation_step = runtime
        .step(now)
        .expect("dispatch durable-store completion");
    runtime
        .take_last_scheduler_ownership()
        .expect("durable-store completion retains scheduler ownership");
    let RuntimeStep::Advanced(validation_effects) = validation_step else {
        panic!("durable-store completion unexpectedly idled")
    };
    assert!(matches!(
        validation_effects.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let validation_ownership = runtime
        .take_effect_ownership(validation_effects.len())
        .expect("ValidateBody retains its exact lifecycle owner")
        .pop()
        .expect("ValidateBody emits one owned effect");

    runtime
        .enqueue_validation_failed_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            &validation_ownership,
        )
        .expect("enqueue deterministic validation failure");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch deterministic validation failure"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_validation_failed(tag, manifest.round, manifest.subject)
        .expect("an applied failed-validation retry is a monotone stutter");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);

    // A ValidateBody effect can have been authorized while the reducer was
    // still Durable but reach the executor only after another exact task
    // records the same deterministic terminal. Its bound owner is
    // consumed by that monotone fact; it must not fail-stop the peer or
    // allocate a replacement FIFO lifecycle.
    assert_eq!(
        runtime.driver.preflight_runtime_command_admission(
            tag,
            &AdapterCommand::ValidationFailed {
                round: manifest.round,
                subject: manifest.subject,
            },
        ),
        RuntimeCommandAdmissionPreflight::CoalesceOwned {
            causal_lifecycle_key: validation_ownership
                .owner()
                .causal_origin()
                .lifecycle_key
                .clone(),
            admission_ordinal: validation_ownership.owner().lifecycle_ordinal(),
        },
    );
    runtime
        .enqueue_validation_failed_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            &validation_ownership,
        )
        .expect("late exact validation owner terminates against the recorded rejection");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert!(!runtime.fail_closed);
    assert_eq!(["validation_failed"], PHASE_INVENTORY);

    assert_eq!(
        runtime.enqueue_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
        ),
        Err(EnqueueError::FailClosed),
        "opposite deterministic outcomes for one durable body conflict"
    );
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert!(runtime.fail_closed);
}

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
fn durable_timeout_coalesces_late_local_proposal_validate_owner() {
    let directory = TempDir::new().expect("temporary timeout-race directory");
    let (fixture_context, _) = authenticated_runtime_context();
    let leader = fixture_context.leader(0);
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(leader),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime while local proposal validation is in flight");
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x9D);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let exact_validate_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&validate_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 90_013)],
    )
    .expect("bind the late ValidateBody capability")
    .pop()
    .expect("one ValidateBody effect retains one owner");

    let deadline = now + runtime.round_timeout();
    let RuntimeStep::Advanced(timeout_effects) = runtime
        .step(deadline)
        .expect("durably install the timeout intent")
    else {
        panic!("timeout dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout dispatch retains exact scheduler ownership");
    let timeout_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout Sign retains its lifecycle owner");
    assert_eq!(timeout_ownership.len(), 1);
    let (sign_tag, signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    let signature = Signature::new(
        keys[usize::try_from(leader).expect("leader index fits usize")].private_key(),
        &signature_preimage,
    )
    .payload()
    .to_vec();
    let signed_timeout = runtime
        .driver
        .signature_completed(sign_tag, signature)
        .expect("finish the durable timeout signature");
    assert_eq!(
        signed_timeout.disposition(),
        crate::sumeragi::v2_core::StepDisposition::Applied,
    );

    let next_ordinal = runtime.ingress.next_admission_ordinal;
    assert_eq!(
        runtime.driver.preflight_runtime_command_admission(
            tag,
            &AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable.clone(),
                validated_receipt: validated.clone(),
            },
        ),
        RuntimeCommandAdmissionPreflight::Coalesce,
    );

    runtime
        .enqueue_local_proposal_with_owner(tag, manifest, durable, validated, &exact_validate_owner)
        .expect("the exact late ValidateBody owner terminates at the closed view");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert!(!runtime.fail_closed);
}

#[test]
fn drained_internal_ignore_uses_exact_durable_tombstone_before_readmission() {
    const PHASE_INVENTORY: [&str; 2] = ["terminal_ignore", "restart_tombstone"];

    let directory = TempDir::new().expect("temporary runtime tombstone directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before draining the ownerless completion");
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x9D);
    let ordinal_before_first = runtime.ingress.next_admission_ordinal;
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("the first ownerless completion reaches its terminal reducer discard");
    assert_eq!(runtime.queued_commands(), 1);
    assert_ne!(runtime.ingress.next_admission_ordinal, ordinal_before_first);
    let original_ownership = RuntimeEffectOwnership::inherited(
        runtime.ingress.commands[0]
            .lifecycle_owner()
            .expect("first completion retains its exact lifecycle owner"),
    );
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
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
    let exact = restarted
        .reserve_body_available_with_owner(restarted_tag, manifest.clone(), &original_ownership)
        .expect("the durable tombstone coalesces only its retained owner");
    assert!(!exact.owns_new_slot());
    assert_eq!(
        exact.lifecycle_owner().as_ref(),
        Some(original_ownership.owner())
    );
    assert_eq!(restarted.ingress.next_admission_ordinal, next_ordinal);

    let foreign_ownership = RuntimeEffectOwnership::fresh_for_test(
        restarted_tag,
        original_ownership
            .owner()
            .lifecycle_ordinal()
            .checked_add(1)
            .expect("test lifecycle ordinal remains finite"),
    );
    assert_eq!(
        restarted.reserve_body_available_with_owner(restarted_tag, manifest, &foreign_ownership,),
        Err(EnqueueError::FailClosed),
        "a terminal semantic lifecycle cannot coalesce under a replacement owner",
    );
    assert_eq!(restarted.queued_commands(), 0);
    assert_eq!(restarted.ingress.next_admission_ordinal, next_ordinal);
    assert!(restarted.fail_closed);
    suppressed_phases.push("restart_tombstone");
    assert_eq!(suppressed_phases, PHASE_INVENTORY);
}

#[test]
fn queued_body_completion_coalesces_only_its_incumbent_owner() {
    let directory = TempDir::new().expect("temporary queued-owner directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xA7);
    runtime
        .enqueue_body_available(tag, manifest.clone())
        .expect("enqueue one exact body completion owner");
    let incumbent = RuntimeEffectOwnership::inherited(
        runtime.ingress.commands[0]
            .lifecycle_owner()
            .expect("queued body completion has one exact owner"),
    );
    let next_ordinal = runtime.ingress.next_admission_ordinal;

    let exact = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &incumbent)
        .expect("same-owner queued retry coalesces");
    assert!(!exact.owns_new_slot());
    assert_eq!(exact.lifecycle_owner().as_ref(), Some(incumbent.owner()));
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);

    let foreign = RuntimeEffectOwnership::fresh_for_test(
        tag,
        incumbent
            .owner()
            .lifecycle_ordinal()
            .checked_add(1)
            .expect("test lifecycle ordinal remains finite"),
    );
    assert_eq!(
        runtime.reserve_body_available_with_owner(tag, manifest, &foreign),
        Err(EnqueueError::FailClosed),
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert!(runtime.fail_closed);
}

#[test]
fn same_owner_wrong_stage_cannot_coalesce_a_body_completion() {
    let directory = TempDir::new().expect("temporary wrong-stage owner directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xA6);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let incumbent_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint the exact Store lifecycle");
    let incumbent = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            incumbent_ordinal,
        )],
    )
    .expect("bind the exact Store predecessor")
    .pop()
    .expect("one Store owns one candidate");
    stage_owned_completion_for_queue_test(
        &mut runtime,
        tag,
        AdapterCommand::BodyStored {
            round: manifest.round,
            subject: manifest.subject,
            receipt: durable.clone(),
        },
        &incumbent,
    );
    let retained_statement = runtime.ingress.commands[0].candidate_semantic_statement;
    let next_ordinal = runtime.ingress.next_admission_ordinal;

    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let wrong_stage = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&validate_effect),
        vec![RuntimeEffectOwnership::inherited(incumbent.owner().clone())],
    )
    .expect("bind the same owner to a different pipeline stage")
    .pop()
    .expect("one Validate owns one candidate");
    assert_eq!(wrong_stage.owner(), incumbent.owner());
    assert_eq!(
        runtime.enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable,
            &wrong_stage,
        ),
        Err(EnqueueError::FailClosed),
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        retained_statement,
    );
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert!(runtime.fail_closed);
}

#[test]
fn queued_fetch_completion_keeps_incumbent_and_rejects_conflicting_authority() {
    let directory = TempDir::new().expect("temporary fetch-completion owner directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xA8);
    let ordinary_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let bind_fetch = |effect: &AdapterEffect, ordinal| {
        bind_adapter_effect_batch_ownership(
            std::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind exact test Fetch ownership")
        .pop()
        .expect("one Fetch owns one candidate")
    };

    let incumbent_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint incumbent Fetch lifecycle");
    let incumbent = bind_fetch(&ordinary_fetch, incumbent_ordinal);
    let first = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &incumbent)
        .expect("reserve completion under the first Fetch owner");
    runtime
        .commit_body_available(first)
        .expect("publish the first exact completion");
    assert_eq!(runtime.queued_commands(), 1);

    let retry = incumbent.clone();
    assert_eq!(retry.owner(), incumbent.owner());
    let coalesced_retry = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &retry)
        .expect("an exact late Fetch retry keeps the queued incumbent");
    assert!(!coalesced_retry.owns_new_slot());
    assert_eq!(
        coalesced_retry.lifecycle_owner().as_ref(),
        Some(incumbent.owner())
    );
    runtime
        .commit_body_available(coalesced_retry)
        .expect("coalesced retry publishes no second completion");

    let mut prepare = signed_runtime_quorum_certificate(&context, &keys, 0xA9);
    prepare.phase = wire::GlobalPhase::Prepare;
    prepare.round = manifest.round;
    prepare.proposal_round = manifest.round;
    prepare.subject = manifest.subject;
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(prepare.clone()),
    };
    let upgrade_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint independently admitted certified carrier");
    let upgrade = bind_fetch(&certified_fetch, upgrade_ordinal);
    assert_ne!(upgrade.owner(), incumbent.owner());
    let coalesced_upgrade = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &upgrade)
        .expect("a late certified Fetch keeps the exact queued completion owner");
    assert!(!coalesced_upgrade.owns_new_slot());
    assert_eq!(
        coalesced_upgrade.lifecycle_owner().as_ref(),
        Some(incumbent.owner())
    );
    runtime
        .commit_body_available(coalesced_upgrade)
        .expect("authority upgrade publishes no second completion");
    let upgraded_statement = upgrade
        .candidate_semantic_statement()
        .expect("certified Fetch carries its complete authority statement");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(upgraded_statement),
        "the incumbent owner must retain the strongest admitted authority",
    );

    let mut conflicting_prepare = prepare;
    conflicting_prepare.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting queued parent state"),
            Hash::new(b"conflicting queued post state"),
            Hash::new(b"conflicting queued writes"),
            1,
            Hash::new(b"conflicting queued block"),
        );
    let conflicting_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(conflicting_prepare),
    };
    let conflicting_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint independently admitted conflicting carrier");
    let conflicting = bind_fetch(&conflicting_fetch, conflicting_ordinal);
    assert_eq!(
        runtime.reserve_body_available_with_owner(tag, manifest, &conflicting),
        Err(EnqueueError::FailClosed),
        "a second Prepare commitment cannot masquerade as another upgrade",
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(upgraded_statement),
        "conflicting authority must not rewrite the retained statement",
    );
    assert!(runtime.fail_closed);
}

#[test]
fn busy_deferred_store_completion_keeps_incumbent_and_rejects_conflicting_authority() {
    let directory = TempDir::new().expect("temporary deferred-store owner directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0xAA);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let incumbent_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint incumbent Store lifecycle");
    let incumbent_store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            incumbent_ordinal,
        )],
    )
    .expect("bind the incumbent ordinary Store")
    .pop()
    .expect("one Store owns one candidate");
    let incumbent_statement = incumbent_store
        .candidate_semantic_statement()
        .expect("ordinary Store carries its exact body statement");

    let deferred_before = runtime.driver.all_deferred_admission_ordinals();
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            tag,
            &manifest,
            DeferredBodyPipelineStageForTest::BodyStored,
        )
        .expect("stage the exact Busy-deferred Store completion");
    let deferred_ordinals = runtime
        .driver
        .all_deferred_admission_ordinals()
        .difference(&deferred_before)
        .copied()
        .collect::<Vec<_>>();
    let [deferred_ordinal] = deferred_ordinals.as_slice() else {
        panic!("one Store completion owns one Busy ordinal")
    };
    bind_deferred_lifecycle_owner_for_test(
        &mut runtime,
        *deferred_ordinal,
        incumbent_store.owner().clone(),
    );
    let deferred = runtime
        .deferred_lifecycle_ownership
        .remove(deferred_ordinal)
        .expect("Busy Store has one runtime ownership wrapper")
        .with_candidate_semantic_statement(Some(incumbent_statement))
        .expect("attach the exact ordinary Store statement");
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(*deferred_ordinal, deferred)
            .is_none()
    );

    let evidence = BodyPipelineCompletionEvidence::BodyStored {
        round: manifest.round,
        subject: manifest.subject,
        receipt: durable.clone(),
    };
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_exact_owner_ordinals(tag, &evidence),
        vec![*deferred_ordinal],
    );

    let mut prepare = signed_runtime_quorum_certificate(&context, &keys, 0xAB);
    prepare.phase = wire::GlobalPhase::Prepare;
    prepare.round = manifest.round;
    prepare.proposal_round = manifest.round;
    prepare.subject = manifest.subject;
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(prepare.clone()),
    };
    let upgrade_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint independently admitted certified Store carrier");
    let certified_fetch_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, upgrade_ordinal)],
    )
    .expect("bind the certified Fetch parent")
    .pop()
    .expect("one Fetch owns one candidate");
    let upgraded_store = certified_fetch_owner
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("certified Fetch passes its authority to Store");
    let upgraded_statement = upgraded_store
        .candidate_semantic_statement()
        .expect("certified Store retains its Prepare statement");
    assert_ne!(upgraded_store.owner(), incumbent_store.owner());

    runtime
        .enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &upgraded_store,
        )
        .expect("certified Store retry coalesces under the Busy incumbent");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(
        runtime.driver.all_deferred_admission_ordinals(),
        BTreeSet::from([*deferred_ordinal]),
    );
    let retained = runtime
        .deferred_lifecycle_ownership
        .get(deferred_ordinal)
        .expect("authority upgrade retains the Busy wrapper");
    assert_eq!(retained.owner(), incumbent_store.owner());
    assert_eq!(
        retained.candidate_semantic_statement,
        Some(upgraded_statement),
        "the Busy incumbent must retain the strongest admitted authority",
    );
    assert!(retained.validate_exact());
    assert!(!runtime.fail_closed);

    let mut conflicting_prepare = prepare;
    conflicting_prepare.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting deferred parent state"),
            Hash::new(b"conflicting deferred post state"),
            Hash::new(b"conflicting deferred writes"),
            1,
            Hash::new(b"conflicting deferred block"),
        );
    let conflicting_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(conflicting_prepare),
    };
    let conflicting_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint independently admitted conflicting Store carrier");
    let conflicting_fetch_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&conflicting_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            conflicting_ordinal,
        )],
    )
    .expect("bind the conflicting Fetch parent")
    .pop()
    .expect("one conflicting Fetch owns one candidate");
    let conflicting_store = conflicting_fetch_owner
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("conflicting Fetch passes its authority to Store");
    assert_eq!(
        runtime.enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable,
            &conflicting_store,
        ),
        Err(EnqueueError::FailClosed),
        "a second Prepare commitment cannot masquerade as a Store upgrade",
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(
        runtime.driver.all_deferred_admission_ordinals(),
        BTreeSet::from([*deferred_ordinal]),
    );
    let retained = runtime
        .deferred_lifecycle_ownership
        .get(deferred_ordinal)
        .expect("conflicting authority cannot retire the Busy wrapper");
    assert_eq!(retained.owner(), incumbent_store.owner());
    assert_eq!(
        retained.candidate_semantic_statement,
        Some(upgraded_statement),
    );
    assert!(retained.validate_exact());
    assert!(runtime.fail_closed);
}

#[test]
fn owned_validation_batch_refines_authority_only_after_atomic_commit() {
    let directory = TempDir::new().expect("temporary owned validation batch directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let tag = runtime.round_tag();
    let incumbent_manifest = runtime_manifest(&context, 0xAC);
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: incumbent_manifest.round,
        subject: incumbent_manifest.subject,
    };
    let incumbent_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint incumbent Validate lifecycle");
    let incumbent = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&validate_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            incumbent_ordinal,
        )],
    )
    .expect("bind the incumbent ordinary Validate")
    .pop()
    .expect("one Validate owns one candidate");
    runtime
        .enqueue_validation_failed_with_owner(
            tag,
            incumbent_manifest.round,
            incumbent_manifest.subject,
            &incumbent,
        )
        .expect("queue the incumbent validation failure");
    let incumbent_statement = incumbent
        .candidate_semantic_statement()
        .expect("ordinary Validate carries an exact body statement");
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(incumbent_statement),
    );

    let mut prepare = signed_runtime_quorum_certificate(&context, &keys, 0xAD);
    prepare.phase = wire::GlobalPhase::Prepare;
    prepare.round = incumbent_manifest.round;
    prepare.proposal_round = incumbent_manifest.round;
    prepare.subject = incumbent_manifest.subject;
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round: incumbent_manifest.round,
        subject: incumbent_manifest.subject,
        manifest: Some(incumbent_manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(prepare),
    };
    let upgrade_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint certified Validate carrier");
    let certified_fetch_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, upgrade_ordinal)],
    )
    .expect("bind the certified Fetch parent")
    .pop()
    .expect("one Fetch owns one candidate");
    let upgraded_validate = certified_fetch_owner
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("certified Fetch passes its authority to Validate");
    let upgraded_statement = upgraded_validate
        .candidate_semantic_statement()
        .expect("certified Validate retains its Prepare statement");

    let mut batch = vec![(
        tag,
        incumbent_manifest.round,
        incumbent_manifest.subject,
        upgraded_validate,
    )];
    for marker in [0xAE, 0xAF, 0xB0] {
        let manifest = runtime_manifest(&context, marker);
        let effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let ordinal = runtime
            .ingress
            .mint_non_fifo_lifecycle_ordinal()
            .expect("mint vacant Validate lifecycle");
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind one vacant Validate owner")
        .pop()
        .expect("one Validate owns one candidate");
        batch.push((tag, manifest.round, manifest.subject, ownership));
    }
    let first_vacant_statement = batch[1]
        .3
        .candidate_semantic_statement()
        .expect("vacant Validate carries its exact statement");
    let next_ordinal = runtime.ingress.next_admission_ordinal;
    assert_eq!(
        runtime.enqueue_validation_failures_atomically_with_owners(&batch),
        Err(EnqueueError::Full),
        "capacity rejection must precede every authority refinement",
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(incumbent_statement),
        "a rejected batch cannot strengthen an earlier coalesced member",
    );
    assert!(!runtime.fail_closed);

    runtime
        .enqueue_validation_failures_atomically_with_owners(&batch[..2])
        .expect("a fitting batch atomically refines and publishes its vacant member");
    assert_eq!(runtime.queued_commands(), 2);
    let retained_incumbent = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| {
            matches!(
                &queued.command,
                AdapterCommand::ValidationFailed { round, subject }
                    if *round == incumbent_manifest.round
                        && *subject == incumbent_manifest.subject
            )
        })
        .expect("the incumbent validation failure remains queued");
    assert_eq!(
        retained_incumbent.candidate_semantic_statement,
        Some(upgraded_statement),
    );
    let first_vacant_subject = batch[1].2;
    let retained_vacant = runtime
        .ingress
        .commands
        .iter()
        .find(|queued| {
            matches!(
                &queued.command,
                AdapterCommand::ValidationFailed { subject, .. }
                    if *subject == first_vacant_subject
            )
        })
        .expect("the fitting vacant validation failure was published");
    assert_eq!(
        retained_vacant.candidate_semantic_statement,
        Some(first_vacant_statement),
        "new owner-aware batch commands must retain typed authority",
    );
    assert!(!runtime.fail_closed);
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
    foreign_context.network_id = crate::sumeragi::synthetic_network_id("foreign-runtime-preflight");
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
    let capacity_before_dormant = runtime.remaining_completion_capacity();
    let dormant_reservation = runtime
        .ingress
        .reserve_canonical_body_available_internal(
            owner_tag,
            dormant_manifest.clone(),
            Some(&dormant_owner),
            None,
            None,
        )
        .expect("reserve an unpublished token for one exact lifecycle owner");
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before_dormant - 1,
        "the exact completion spends one physical position",
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
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before_dormant - 1,
        "mismatched bulk retirement preserves the exact capacity charge",
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
            None,
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
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
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
    let canonical_chunks = wire::encode_payload_chunks(dispatch_context.da_layout, &canonical_body)
        .expect("canonically encode the conflicting Busy body");
    // Deliberate negative data: the alternate body has canonical RS16
    // geometry, while the manifest remains bound to the original proposal
    // subject so BodyAvailable exercises exact conflict retirement.
    let canonical_manifest = wire::PayloadManifest::derive(
        &dispatch_context,
        busy_proposal.round,
        busy_proposal.subject,
        u64::try_from(canonical_body.len()).expect("small canonical body length fits u64"),
        &canonical_chunks,
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
    let queued_canonical_chunks =
        wire::encode_payload_chunks(queued_context.da_layout, &queued_canonical_body)
            .expect("canonically encode the conflicting queued body");
    // Deliberate negative data: retain the queued proposal's original
    // subject while deriving over the alternate body's complete RS16
    // sequence so this remains a semantic conflict, not malformed chunks.
    let queued_canonical_manifest = wire::PayloadManifest::derive(
        &queued_context,
        queued_proposal.round,
        queued_proposal.subject,
        u64::try_from(queued_canonical_body.len())
            .expect("small queued canonical body length fits u64"),
        &queued_canonical_chunks,
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
