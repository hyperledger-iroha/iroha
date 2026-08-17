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
    let retry_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint independently admitted retry carrier");
    let retry = bind_fetch(&ordinary_fetch, retry_ordinal);
    assert_ne!(retry.owner(), incumbent.owner());
    let (retry, retry_relation) = incumbent
        .adopt_incumbent_fetch_for_retry_or_authority(&retry, &ordinary_fetch)
        .expect("the exact retry adopts the incumbent physical Fetch owner");
    assert_eq!(retry_relation, RuntimeFetchAuthorityRelation::Same);
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
    let (upgrade, upgrade_relation) = incumbent
        .adopt_incumbent_fetch_for_retry_or_authority(&upgrade, &certified_fetch)
        .expect("the certified retry upgrades the incumbent physical Fetch owner");
    assert_eq!(upgrade_relation, RuntimeFetchAuthorityRelation::Upgrade);
    assert_eq!(upgrade.owner(), incumbent.owner());
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
fn periodic_retransmit_cannot_starve_admitted_work_when_every_step_arrives_late() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    for value in 1..=2 {
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(value),
        )
        .unwrap();
    }
    for seconds in [2, 4, 6, 8] {
        let _ = runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(seconds));
    }
    assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
    assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
    // Drain a periodic episode and the one-shot timeout before admitting
    // a new target. Every later runner entry is again exactly one whole
    // retransmit interval late. The drained timer's dormant semantic key
    // must not resurrect its old physical ordinal on each entry.
    let mut post_timeout = self::runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    post_timeout
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
        .expect("drain the first periodic episode");
    assert_eq!(post_timeout.driver.retransmits, vec![initial]);
    post_timeout
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
        .expect("emit the one-shot absolute timeout");
    assert_eq!(post_timeout.driver.timeouts, vec![initial]);
    enqueue_fake(
        &mut post_timeout,
        initial,
        CommandClass::Normal,
        FakeCommand::record(9),
    )
    .expect("admit work after the old periodic owner drained");
    post_timeout
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(12))
        .expect("the admitted target precedes the fresh periodic episode");
    assert_eq!(post_timeout.driver.delivered, vec![(initial, 9)]);
    assert_eq!(
        post_timeout.driver.retransmits,
        vec![initial],
        "a drained timer cannot reacquire its old position ahead of the target"
    );
    post_timeout
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(14))
        .expect("the freshly positioned periodic episode follows the target");
    assert_eq!(post_timeout.driver.retransmits, vec![initial, initial]);
}
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
    let fifo_step = runtime
        .step(start + Duration::from_secs(2))
        .expect("FIFO debt runs immediately after the periodic turn");
    let RuntimeStep::Advanced(fifo_effects) = fifo_step else {
        panic!("FIFO debt unexpectedly idled")
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("admitted FIFO publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    runtime
        .take_effect_ownership(fifo_effects.len())
        .expect("the FIFO executor consumes its exact effect ownership");
    assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
    assert_eq!(runtime.driver.retransmits, vec![initial]);
    assert!(runtime.driver.timeouts.is_empty());
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
    // The TC-like Progress owner predates the retransmit frozen by this turn,
    // so EnterView runs first and resets both clocks.
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
fn same_view_generation_upgrade_restarts_timeout_with_a_fresh_owner() {
    let start = Instant::now();
    let initial = EventTag::new(7, 0, Generation::new(11));
    let rebound = EventTag::new(7, 0, Generation::new(12));
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );
    runtime
        .step_and_take_scheduler_ownership_for_test(start + runtime.round_timeout())
        .expect("the first generation emits its timeout");
    assert_eq!(runtime.driver.timeouts, vec![initial]);

    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Progress,
        FakeCommand::enter_view(rebound),
    )
    .expect("admit the same-view generation upgrade");
    let rebound_at = start + runtime.round_timeout();
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(rebound_at),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(runtime.round_tag(), rebound);
    runtime
        .reconcile_active_view_producer(rebound, false)
        .expect("the nonleader test peer retires the rebound producer");

    runtime
        .step_and_take_scheduler_ownership_for_test(rebound_at + runtime.round_timeout())
        .expect("the rebound generation emits a fresh timeout");
    assert_eq!(runtime.driver.timeouts, vec![initial, rebound]);
    assert!(!runtime.fail_closed);
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
