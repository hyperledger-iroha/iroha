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

    // A second live adapter producer for the same candidate is correctly
    // rejected by durable admission. Model the independent crash-restored
    // carrier which can coexist at this preflight boundary instead.
    let destination_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve the restored destination lifecycle");
    let destination_command = AdapterCommand::BodyAvailable {
        manifest: manifest.clone(),
    };
    let destination = runtime
        .restored_tagged_command(
            rebound,
            CommandClass::Completion,
            destination_command,
            now,
            Hash::new(b"persistent-body-destination"),
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
            causal_lifecycle_key: Hash::new(b"persistent-body-destination"),
            admission_ordinal: destination_ordinal,
            producer_stage: RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE,
        }))
    );
    let adapter_ordinals_before = runtime.driver.all_deferred_admission_ordinals();
    let wrapper_ordinals_before = runtime.deferred_lifecycle_ownership.clone();
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
        wrapper_ordinals_before
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
