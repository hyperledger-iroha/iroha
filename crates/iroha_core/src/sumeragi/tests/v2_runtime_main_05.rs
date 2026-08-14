#[test]
fn body_available_rebind_coalesces_exact_busy_deferred_destination_owner() {
    let directory = TempDir::new().expect("temporary destination-coalescing directory");
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
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8C))
        .expect("enqueue authenticated proposal");
    let proposal_effects = match runtime.step(now).expect("dispatch proposal") {
        RuntimeStep::Advanced(effects) => effects,
        RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("proposal dispatch publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let (source_tag, manifest) = match proposal_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let proposal_effect_ownership = runtime
        .take_effect_ownership(proposal_effects.len())
        .expect("FetchBody retains the proposal lifecycle owner");
    assert_eq!(proposal_effect_ownership.len(), 1);
    let body_reservation = runtime
        .reserve_body_available_with_owner(
            source_tag,
            manifest.clone(),
            &proposal_effect_ownership[0],
        )
        .expect("reserve body reconstruction under the FetchBody owner");
    runtime
        .commit_body_available(body_reservation)
        .expect("publish the owned body reconstruction completion");
    let RuntimeStep::Advanced(body_effects) =
        runtime.step(now).expect("dispatch body reconstruction")
    else {
        panic!("body reconstruction unexpectedly idled")
    };
    assert!(matches!(
        body_effects.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("body reconstruction publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let body_effect_ownership = runtime
        .take_effect_ownership(body_effects.len())
        .expect("StoreBody retains the FetchBody lifecycle owner");
    assert_eq!(body_effect_ownership.len(), 1);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &body_effect_ownership[0],
        )
        .expect("enqueue durable-store completion");
    let store_effect = body_effects[0].clone();
    let retry_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint an independently admitted Store retry");
    let retry_store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            source_tag,
            retry_ordinal,
        )],
    )
    .expect("bind the independently admitted Store retry")
    .pop()
    .expect("one Store retry owns one candidate");
    assert_ne!(retry_store.owner(), body_effect_ownership[0].owner());
    runtime
        .enqueue_body_stored_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &retry_store,
        )
        .expect("a late exact Store retry keeps the queued incumbent completion");
    let mut prepare = signed_runtime_quorum_certificate(&context, &keys, 0x8D);
    prepare.phase = wire::GlobalPhase::Prepare;
    prepare.round = manifest.round;
    prepare.proposal_round = manifest.round;
    prepare.subject = manifest.subject;
    let certified_fetch = AdapterEffect::FetchBody {
        tag: source_tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(prepare.clone()),
    };
    let upgrade_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint an independently admitted certified carrier");
    let certified_fetch_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(
            source_tag,
            upgrade_ordinal,
        )],
    )
    .expect("bind the independently admitted certified Fetch")
    .pop()
    .expect("one certified Fetch owns one candidate");
    let certified_store_owner = certified_fetch_owner
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("certified Fetch passes its authority to Store");
    runtime
        .enqueue_body_stored_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &certified_store_owner,
        )
        .expect("a late certified Store carrier keeps the queued incumbent completion");
    assert_eq!(runtime.queued_commands(), 1);
    let RuntimeStep::Advanced(store_effects) = runtime
        .step(now)
        .expect("dispatch durable-store completion")
    else {
        panic!("durable-store completion unexpectedly idled")
    };
    assert!(matches!(
        store_effects.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("durable-store completion publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let store_effect_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("ValidateBody retains the body pipeline lifecycle owner");
    assert_eq!(store_effect_ownership.len(), 1);
    runtime
        .enqueue_validation_succeeded_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
            &store_effect_ownership[0],
        )
        .expect("enqueue validation completion");
    let RuntimeStep::Advanced(validation_effects) =
        runtime.step(now).expect("dispatch validation completion")
    else {
        panic!("validation completion unexpectedly idled")
    };
    let (sign_tag, sign_preimage) = match validation_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected validation effects: {effects:?}"),
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("validation completion publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let sign_effect_ownership = runtime
        .take_effect_ownership(validation_effects.len())
        .expect("Prepare Sign retains the body pipeline lifecycle owner");
    assert_eq!(sign_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![sign_effect_ownership[0].owner().clone()])
        .expect("publish the pending Prepare signer owner");
    let rebound = EventTag::new(
        source_tag.height(),
        source_tag.view() + 1,
        Generation::new(source_tag.generation().get() + 1),
    );
    // The body reconstructed above already owns a terminal serviced-
    // candidate record. Use another exact body to exercise the live Busy
    // lane instead of asking the adapter to resurrect that terminal.
    let rebound_manifest = runtime_manifest(&context, 0x8E);
    let (body_ordinal, body_owner) = defer_persistent_body_available_for_test(
        &mut runtime,
        source_tag,
        &rebound_manifest,
        b"body-available-retirement-owner",
    );
    let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
        manifest: rebound_manifest.clone(),
    };
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
        (1, 1),
        "the current tag owns the real Busy-deferred completion"
    );
    observe_enter_view_for_test(&mut runtime, source_tag, rebound, &rebound_manifest);
    assert_eq!(
        runtime
            .driver
            .rebind_deferred_body_available(source_tag, rebound, &rebound_manifest),
        1,
        "the seam models an exact destination owner already transferred by another path"
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (1, 1),
        "the destination must be owned by the real Busy-deferred lane"
    );
    assert!(
        runtime
            .driver
            .deferred_body_available_has_persistent_producer(rebound, &rebound_manifest)
            .expect("validate the rebound durable producer"),
        "the destination must retain the sole persistent producer root"
    );
    stage_completion_for_queue_test(
        &mut runtime,
        source_tag,
        AdapterCommand::BodyAvailable {
            manifest: rebound_manifest.clone(),
        },
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(
        runtime
            .rebind_body_available(source_tag, rebound, &rebound_manifest)
            .expect("exact destination ownership coalesces the source")
    );
    assert!(!runtime.fail_closed);
    assert_eq!(runtime.queued_commands(), 0, "the source owner was retired");
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (1, 1),
        "ordinary-source coalescing retains exactly one persistent destination owner"
    );
    assert_eq!(
        runtime
            .deferred_lifecycle_ownership
            .get(&body_ordinal)
            .map(RuntimeDeferredLifecycleOwnership::owner),
        Some(&body_owner),
        "coalescing cannot retire the wrapper of the retained Busy owner"
    );
    assert!(
        !runtime
            .rebind_body_available(source_tag, rebound, &rebound_manifest)
            .expect("an idempotent retry finds no remaining source owner")
    );
    let same_view_rebound = EventTag::new(
        rebound.height(),
        rebound.view(),
        Generation::new(rebound.generation().get() + 1),
    );
    observe_enter_view_for_test(&mut runtime, rebound, same_view_rebound, &rebound_manifest);
    assert!(
        runtime
            .rebind_body_available(rebound, same_view_rebound, &rebound_manifest)
            .expect("same-view generation supersession transfers the Busy-deferred owner")
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(same_view_rebound, &evidence),
        (1, 1),
        "same-view rebinding leaves exactly one Busy-deferred destination"
    );
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .contains_key(&body_ordinal)
    );
    assert!(
        runtime
            .retire_body_available(same_view_rebound, &rebound_manifest)
            .expect("the unique destination owner remains retireable")
    );
    assert!(
        !runtime
            .deferred_lifecycle_ownership
            .contains_key(&body_ordinal),
        "retirement cannot leave the drained Busy owner at the global minimum"
    );
    assert!(runtime.deferred_ingress_ownership.is_empty());
    let signature = Signature::new(keys[0].private_key(), &sign_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(sign_tag, signature, &sign_effect_ownership[0])
        .expect("complete the retained Prepare signer under its original owner");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the external signer after its completion is admitted");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch the retained Prepare completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
    ));
    // Exercise the opposite coalescing direction: a Busy source loses to
    // an already-installed FIFO destination. The adapter occurrence and
    // its sealed runtime wrapper must retire in the same transition.
    let retirement_directory = TempDir::new().expect("temporary Busy-source coalescing directory");
    let (mut retirement_runtime, retirement_context, _keys) =
        authenticated_network_runtime(&retirement_directory, RuntimeQueueConfig::new(8, 1, 1));
    let retirement_source = retirement_runtime.round_tag();
    let retirement_manifest = runtime_manifest(&retirement_context, 0x8F);
    retirement_runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            retirement_source,
            &retirement_manifest,
            DeferredBodyPipelineStageForTest::BodyAvailable,
        )
        .expect("stage the exact Busy source completion");
    let retirement_ordinals = retirement_runtime
        .driver
        .all_deferred_admission_ordinals()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(retirement_ordinals.len(), 1);
    let retirement_ordinal = retirement_ordinals[0];
    bind_local_deferred_lifecycle_for_test(
        &mut retirement_runtime,
        retirement_ordinal,
        b"body-available-rebind-retirement-owner",
    );
    let retirement_rebound = EventTag::new(
        retirement_source.height(),
        retirement_source.view() + 1,
        Generation::new(retirement_source.generation().get() + 1),
    );
    observe_enter_view_for_test(
        &mut retirement_runtime,
        retirement_source,
        retirement_rebound,
        &retirement_manifest,
    );
    stage_completion_for_queue_test(
        &mut retirement_runtime,
        retirement_rebound,
        AdapterCommand::BodyAvailable {
            manifest: retirement_manifest.clone(),
        },
    );
    assert!(
        retirement_runtime
            .rebind_body_available(retirement_source, retirement_rebound, &retirement_manifest,)
            .expect("the existing FIFO destination coalesces the Busy source")
    );
    assert!(
        !retirement_runtime
            .deferred_lifecycle_ownership
            .contains_key(&retirement_ordinal),
        "Busy-source coalescing cannot leave its runtime wrapper alive"
    );
    assert!(
        !retirement_runtime
            .driver
            .all_deferred_admission_ordinals()
            .contains(&retirement_ordinal)
    );
    assert_eq!(retirement_runtime.queued_commands(), 1);
    assert!(
        retirement_runtime
            .retire_body_available(retirement_rebound, &retirement_manifest)
            .expect("the retained FIFO destination remains uniquely retireable")
    );
    assert_eq!(retirement_runtime.queued_commands(), 0);
}
#[test]
fn queued_body_terminal_adopts_only_authority_upgrades_and_rejects_same_authority_owners() {
    let directory = TempDir::new().expect("temporary body-terminal visibility directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for terminal-visibility dispatch");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8D))
        .expect("enqueue authenticated proposal");
    let RuntimeStep::Advanced(fetch_effects) = runtime.step(now).expect("dispatch proposal") else {
        panic!("proposal dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("proposal dispatch publishes scheduler ownership");
    let (tag, manifest) = match fetch_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let fetch_ownership = runtime
        .take_effect_ownership(fetch_effects.len())
        .expect("take exact FetchBody ownership");
    let reservation = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &fetch_ownership[0])
        .expect("reserve exact body reconstruction");
    runtime
        .commit_body_available(reservation)
        .expect("publish body reconstruction");
    let RuntimeStep::Advanced(store_effects) =
        runtime.step(now).expect("dispatch body reconstruction")
    else {
        panic!("body reconstruction unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("body reconstruction publishes scheduler ownership");
    assert!(matches!(
        store_effects.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    let store_effect = store_effects[0].clone();
    let store_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("take exact StoreBody ownership");
    let commit = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"terminal upgrade parent state"),
            Hash::new(b"terminal upgrade post state"),
            Hash::new(b"terminal upgrade writes"),
            1,
            Hash::new(b"terminal upgrade block"),
        ),
        signers: Vec::new(),
        aggregate_signature: Vec::new(),
    };
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(commit),
    };
    let certified_fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 9_901)],
    )
    .expect("bind a distinct Commit-authorized Fetch owner")
    .pop()
    .expect("one Commit Fetch owner");
    let certified_store_ownership = certified_fetch_ownership
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("Commit Fetch authorizes the exact Store stage");
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &store_ownership[0],
        )
        .expect("queue exact durable-store terminal");
    assert_ne!(
        certified_store_ownership.candidate_semantic_identity(),
        store_ownership[0].candidate_semantic_identity(),
        "Commit authority deliberately changes the route-neutral candidate identity"
    );
    assert!(
        runtime
            .body_pipeline_candidate_has_terminal(&store_effect, &certified_store_ownership)
            .expect("Commit Store observes the ordinary queued terminal")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(!runtime.fail_closed);
    let RuntimeStep::Advanced(validate_effects) =
        runtime.step(now).expect("dispatch durable-store terminal")
    else {
        panic!("durable-store terminal unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("durable-store dispatch publishes scheduler ownership");
    assert!(matches!(
        validate_effects.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let validate_effect = validate_effects[0].clone();
    let validate_ownership = runtime
        .take_effect_ownership(validate_effects.len())
        .expect("take exact ValidateBody ownership");
    let certified_validate_ownership = certified_store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("Commit Store authorizes the exact Validate stage");
    runtime
        .enqueue_validation_succeeded_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
            &validate_ownership[0],
        )
        .expect("queue exact validation terminal");
    assert_eq!(
        certified_validate_ownership.candidate_semantic_identity(),
        validate_ownership[0].candidate_semantic_identity(),
        "the Store terminal refinement carries Commit authority into deterministic validation"
    );
    assert!(
        runtime
            .body_pipeline_candidate_has_terminal(&validate_effect, &validate_ownership[0],)
            .expect("the incumbent Commit Validate observes its queued terminal")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(!runtime.fail_closed);
    assert_ne!(
        certified_validate_ownership.owner(),
        validate_ownership[0].owner(),
        "the negative retry must carry a distinct lifecycle owner"
    );
    assert!(
        runtime
            .body_pipeline_candidate_has_terminal(&validate_effect, &certified_validate_ownership,)
            .is_err(),
        "same-authority terminal retry must reject a foreign owner"
    );
    assert!(runtime.fail_closed);
}
#[test]
fn queued_store_terminal_query_refines_prepare_to_commit_under_incumbent() {
    let directory = TempDir::new().expect("temporary terminal-refinement directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x8E);
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
    let mut commit = signed_runtime_quorum_certificate(&context, &keys, 0x8F);
    commit.round = manifest.round;
    commit.proposal_round = manifest.round;
    commit.subject = manifest.subject;
    let mut prepare = commit.clone();
    prepare.phase = wire::GlobalPhase::Prepare;
    let fetch = |certificate| AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(certificate),
    };
    let bind_store = |effect: &AdapterEffect, ordinal| {
        bind_adapter_effect_batch_ownership(
            std::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind one certified FetchBody carrier")
        .pop()
        .expect("one FetchBody carrier owns one candidate")
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("carry certified authority into StoreBody")
    };
    let prepare_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint Prepare-authorized StoreBody owner");
    let prepare_store = bind_store(&fetch(prepare), prepare_ordinal);
    let prepare_statement = prepare_store
        .candidate_semantic_statement()
        .expect("Prepare-authorized StoreBody carries its exact statement");
    let evidence = BodyPipelineCompletionEvidence::BodyStored {
        round: manifest.round,
        subject: manifest.subject,
        receipt: durable.clone(),
    };
    assert!(prepare_store.exactly_authorizes_body_pipeline_successor(
        &store_effect,
        tag,
        &evidence,
    ));
    stage_owned_completion_for_queue_test(
        &mut runtime,
        tag,
        AdapterCommand::BodyStored {
            round: manifest.round,
            subject: manifest.subject,
            receipt: durable,
        },
        &prepare_store,
    );
    let incumbent = runtime.ingress.commands[0]
        .lifecycle_owner()
        .expect("queued terminal retains its incumbent owner");
    let commit_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint Commit-authorized StoreBody retry owner");
    let commit_store = bind_store(&fetch(commit), commit_ordinal);
    let commit_statement = commit_store
        .candidate_semantic_statement()
        .expect("Commit-authorized StoreBody carries its exact statement");
    assert_ne!(commit_store.owner(), &incumbent);
    let planned = runtime
        .plan_body_pipeline_candidate_terminal(&store_effect, &commit_store)
        .expect("terminal query accepts the monotonic Commit refinement")
        .expect("queued StoreBody terminal produces one incumbent-owner plan");
    assert_eq!(planned.owner(), &incumbent);
    assert_eq!(
        planned.candidate_semantic_statement(),
        Some(commit_statement)
    );
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(prepare_statement),
        "planning cannot refine terminal authority before the caller's total gate"
    );
    runtime
        .commit_body_pipeline_candidate_terminal(&store_effect, &planned)
        .expect("commit the checked monotonic terminal refinement");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.commands[0]
            .lifecycle_owner()
            .expect("authority refinement preserves the incumbent owner"),
        incumbent,
    );
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(commit_statement),
    );
    assert!(!runtime.fail_closed);
}
#[test]
fn local_proposal_ready_is_owned_by_its_validate_predecessor() {
    let (context, _) = authenticated_runtime_context();
    let manifest = runtime_manifest(&context, 0x90);
    let tag = EventTag::new(context.height, manifest.round.view, Generation::new(4));
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
        manifest: manifest.clone(),
        durable_receipt: durable.clone(),
        validated_receipt: ValidatedBodyReceipt::for_test(durable),
    };
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 9_090)],
    )
    .expect("bind exact StoreBody ownership")
    .pop()
    .expect("one StoreBody effect owns one candidate");
    let validate = store
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry the body owner into ValidateBody");
    assert!(!store.binds_body_pipeline_completion_predecessor(&evidence));
    assert!(validate.binds_body_pipeline_completion_predecessor(&evidence));
    assert!(!store.exactly_authorizes_body_pipeline_successor(&store_effect, tag, &evidence,));
    assert!(validate.exactly_authorizes_body_pipeline_successor(&validate_effect, tag, &evidence,));
}
#[test]
fn body_available_rebind_rejects_two_persistent_roots_before_mutation() {
    let directory = TempDir::new().expect("temporary persistent-root conflict directory");
    let (mut runtime, context, _keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before opening a signer fence");
    let deadline = now + runtime.round_timeout();
    let RuntimeStep::Advanced(timeout_effects) = runtime
        .step(deadline)
        .expect("open a runtime-owned TimeoutVote signer fence")
    else {
        panic!("timeout dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout dispatch retains exact scheduler ownership");
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    let timeout_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("TimeoutVote Sign retains its lifecycle owner");
    let [timeout_ownership] = timeout_ownership.as_slice() else {
        panic!("TimeoutVote Sign has one exact owner")
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
        .expect("publish the pending TimeoutVote signer owner");
    let source_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x90);
    let (source_ordinal, source_owner) = defer_persistent_body_available_for_test(
        &mut runtime,
        source_tag,
        &manifest,
        b"persistent-body-source",
    );
    let rebound = EventTag::new(
        source_tag.height(),
        source_tag.view() + 1,
        Generation::new(source_tag.generation().get() + 1),
    );
    observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
    let (destination_ordinal, destination_owner) =
        defer_persistent_body_available_with_owner_for_test(
            &mut runtime,
            rebound,
            &manifest,
            source_owner.clone(),
        );
    assert_ne!(source_ordinal, destination_ordinal);
    assert_eq!(source_owner, destination_owner);
    let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
        manifest: manifest.clone(),
    };
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
        (1, 1)
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (1, 1)
    );
    let adapter_ordinals_before = runtime.driver.all_deferred_admission_ordinals();
    let wrapper_ordinals_before = runtime
        .deferred_lifecycle_ownership
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        runtime
            .rebind_body_available(source_tag, rebound, &manifest)
            .expect_err("two persistent roots must fail before either owner is retired"),
        "Sumeragi v2 body completion has two persistent producer roots"
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.driver.all_deferred_admission_ordinals(),
        adapter_ordinals_before,
        "persistent-root preflight cannot mutate either adapter occurrence"
    );
    assert_eq!(
        runtime
            .deferred_lifecycle_ownership
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        wrapper_ordinals_before,
        "persistent-root preflight cannot retire either runtime wrapper"
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
        (1, 1)
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (1, 1)
    );
    assert!(
        runtime
            .driver
            .deferred_body_available_has_persistent_producer(source_tag, &manifest)
            .expect("source persistent root remains exact")
    );
    assert!(
        runtime
            .driver
            .deferred_body_available_has_persistent_producer(rebound, &manifest)
            .expect("destination persistent root remains exact")
    );
}
#[test]
fn body_available_rebind_rejects_busy_source_and_restored_ingress_destination_before_mutation() {
    let directory = TempDir::new().expect("temporary cross-carrier conflict directory");
    let (mut runtime, context, _keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before opening a signer fence");
    let deadline = now + runtime.round_timeout();
    let RuntimeStep::Advanced(timeout_effects) = runtime
        .step(deadline)
        .expect("open a runtime-owned TimeoutVote signer fence")
    else {
        panic!("timeout dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout dispatch retains exact scheduler ownership");
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    let timeout_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("TimeoutVote Sign retains its lifecycle owner");
    let [timeout_ownership] = timeout_ownership.as_slice() else {
        panic!("TimeoutVote Sign has one exact owner")
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
        .expect("publish the pending TimeoutVote signer owner");
    let source_tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x91);
    let (source_ordinal, source_owner) = defer_persistent_body_available_for_test(
        &mut runtime,
        source_tag,
        &manifest,
        b"persistent-body-busy-source",
    );
    let rebound = EventTag::new(
        source_tag.height(),
        source_tag.view() + 1,
        Generation::new(source_tag.generation().get() + 1),
    );
    observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);

    // Inject exact stage-7 ingress metadata to exercise fail-closed
    // cross-carrier defense; full restart reachability is covered elsewhere.
    let destination_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve the restored destination lifecycle");
    let destination = runtime
        .restored_tagged_command(
            rebound,
            CommandClass::Completion,
            AdapterCommand::BodyAvailable {
                manifest: manifest.clone(),
            },
            now,
            Hash::new(b"persistent-body-restored-destination"),
            destination_ordinal,
            RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE,
        )
        .expect("reconstruct the independent persistent destination");
    let destination_owner = destination
        .lifecycle_owner()
        .expect("restored destination retains its exact lifecycle owner");
    runtime
        .ingress
        .enqueue(destination)
        .expect("stage the restored persistent destination");
    assert_ne!(source_ordinal, destination_ordinal);
    assert_ne!(source_owner, destination_owner);
    let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
        manifest: manifest.clone(),
    };
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
        (1, 1)
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (0, 0)
    );
    assert_eq!(
        runtime
            .ingress
            .restored_body_available_retirement(rebound, |queued| queued == &manifest),
        Ok(Some(RestoredProducerRetirement {
            causal_lifecycle_key: Hash::new(b"persistent-body-restored-destination"),
            admission_ordinal: destination_ordinal,
            producer_stage: RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE,
        }))
    );
    let adapter_ordinals_before = runtime.driver.all_deferred_admission_ordinals();
    let wrapper_ownership_before = runtime.deferred_lifecycle_ownership.clone();
    let queue_before = runtime.ingress.ownership_snapshot();
    assert_eq!(
        runtime
            .rebind_body_available(source_tag, rebound, &manifest)
            .expect_err("two persistent roots must fail before either owner is retired"),
        "Sumeragi v2 body completion has two persistent producer roots"
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.driver.all_deferred_admission_ordinals(),
        adapter_ordinals_before
    );
    assert_eq!(
        runtime.deferred_lifecycle_ownership,
        wrapper_ownership_before
    );
    assert_eq!(runtime.ingress.ownership_snapshot(), queue_before);
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
        (1, 1)
    );
    assert!(
        runtime
            .driver
            .deferred_body_available_has_persistent_producer(source_tag, &manifest)
            .expect("source persistent root remains exact")
    );
    assert!(
        runtime
            .ingress
            .restored_body_available_retirement(rebound, |queued| queued == &manifest)
            .expect("destination restored root remains exact")
            .is_some()
    );
}
#[test]
fn body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation() {
    {
        let directory = TempDir::new().expect("temporary destination-conflict directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let source_tag = runtime.round_tag();
        let rebound = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8D);
        observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
        let mut conflicting = manifest.clone();
        conflicting.chunk_hashes[0] = Hash::new(b"conflicting rebound chunk");
        conflicting.chunk_root = Hash::new(b"conflicting rebound root");
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue unique source owner");
        runtime
            .ingress
            .enqueue_canonical_body_available(rebound, conflicting.clone())
            .expect("test seam stages conflicting destination evidence");
        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("conflicting destination evidence must fail closed"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 2);
        assert!(runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::BodyAvailable { manifest: queued_manifest }
                if queued.tag == source_tag && queued_manifest == &manifest
        )));
        assert!(runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::BodyAvailable { manifest: queued_manifest }
                if queued.tag == rebound && queued_manifest == &conflicting
        )));
        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("fail-closed runtime rejects a second conflicting rebind"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(source_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }
    {
        let directory = TempDir::new().expect("temporary destination-duplicate directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let source_tag = runtime.round_tag();
        let rebound = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8E);
        observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue unique source owner");
        for _ in 0..2 {
            runtime
                .ingress
                .enqueue_canonical_body_available(rebound, manifest.clone())
                .expect("test seam creates duplicate destination ownership");
        }
        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("duplicate destination ownership must fail closed"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 3);
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .filter(|queued| queued.tag == source_tag)
                .count(),
            1,
            "destination preflight must retain the source owner"
        );
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .filter(|queued| queued.tag == rebound)
                .count(),
            2,
            "destination preflight must not mutate duplicate owners"
        );
        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("fail-closed runtime rejects a second duplicate rebind"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(source_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }
}
#[test]
fn duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation() {
    {
        let directory = TempDir::new().expect("temporary duplicate-rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x8E);
        for _ in 0..2 {
            runtime
                .ingress
                .enqueue_canonical_body_available(owner_tag, manifest.clone())
                .expect("test seam creates duplicate ingress ownership");
        }
        let rebound = EventTag::new(
            owner_tag.height(),
            owner_tag.view() + 1,
            Generation::new(owner_tag.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, owner_tag, rebound, &manifest);
        assert_eq!(
            runtime
                .rebind_body_available(owner_tag, rebound, &manifest)
                .expect_err("duplicate ownership must prevent rebind"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 2);
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .all(|queued| queued.tag == owner_tag),
            "preflight must leave every duplicate owner at its original tag"
        );
        assert_eq!(
            runtime
                .rebind_body_available(owner_tag, rebound, &manifest)
                .expect_err("fail-closed runtime must reject a second rebind"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(owner_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }
    {
        let directory = TempDir::new().expect("temporary duplicate-retirement directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x8F);
        for _ in 0..2 {
            runtime
                .ingress
                .enqueue_canonical_body_available(owner_tag, manifest.clone())
                .expect("test seam creates duplicate ingress ownership");
        }
        assert_eq!(
            runtime
                .retire_body_available(owner_tag, &manifest)
                .expect_err("duplicate ownership must prevent retirement"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            2,
            "preflight must not mutate duplicate serialized owners"
        );
        assert_eq!(
            runtime
                .retire_body_available(owner_tag, &manifest)
                .expect_err("fail-closed runtime must reject a second retirement"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(owner_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }
}
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
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch durable-store completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
    ));
    runtime
        .enqueue_validation_failed(tag, manifest.round, manifest.subject)
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
    let late_validation = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let late_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&late_validation),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            next_ordinal
                .expect("live validation flow retains an unused lifecycle ordinal")
                .checked_add(100)
                .expect("test lifecycle ordinal remains finite"),
        )],
    )
    .expect("bind one late exact validation owner")
    .pop()
    .expect("one validation owner");
    assert_eq!(
        runtime.driver.preflight_runtime_command_admission(
            tag,
            &AdapterCommand::ValidationFailed {
                round: manifest.round,
                subject: manifest.subject,
            },
        ),
        RuntimeCommandAdmissionPreflight::Coalesce,
    );
    runtime
        .enqueue_validation_failed_with_owner(tag, manifest.round, manifest.subject, &late_owner)
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
