#[test]
fn late_passive_fetch_completion_opens_one_serve_predecessor_admission_and_steps() {
    let mut fixture = ProductionTransportFixture::new();
    let now = Instant::now();
    fixture
        .executor
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            now,
        )
        .expect("arm the production executor after startup");
    let fetch_tag = fixture.executor.current_tag();
    let fetch_ordinal = fixture
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve the passive Fetch lifecycle before Serve");
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        4_000,
        0,
    );
    let signature =
        SignatureOf::try_from_hash(fixture.validator_keys[0].private_key(), header.hash())
            .expect("late Fetch block signature");
    let block = SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
    let body = block
        .encode_wire()
        .expect("late Fetch canonical block wire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&body),
    };
    let manifest = canonical_payload_manifest(&fixture.context, fixture.round, subject, &body);
    let fetch = AdapterEffect::FetchBody {
        tag: fetch_tag,
        round: fixture.round,
        subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(
            fetch_tag,
            fetch_ordinal,
        )],
    )
    .expect("bind the passive Fetch to the shared actor ordinal");
    fixture
        .executor
        .retain_effect_batch(vec![fetch], ownership)
        .expect("retain the production-shaped Fetch effect");
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    assert_eq!(
        fixture
            .executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("dispatch the passive Fetch"),
        1
    );
    let task = services
        .fetch_tasks
        .first()
        .expect("Fetch service owns the passive request")
        .clone();
    assert_eq!(task.lifecycle_ordinal(), fetch_ordinal);
    let serve_ordinal = fixture
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve the selected Serve target after Fetch");
    let initial = fixture
        .executor
        .exact_serve_predecessor_observation(now, serve_ordinal, None)
        .expect("observe the selected Serve before Fetch completion");
    assert!(initial.should_open_predecessor_admission());
    assert!(
        !initial.has_runnable_predecessor(),
        "passive Fetch transport work alone cannot block Serve"
    );

    fixture
        .executor
        .complete_body_reconstruction(&task, manifest, body, &mut services)
        .expect("late reconstruction materializes BodyAvailable under the Fetch owner");
    let observation = fixture
        .executor
        .exact_serve_predecessor_observation(now, serve_ordinal, None)
        .expect("observe late BodyAvailable behind the selected Serve");
    assert!(observation.should_open_predecessor_admission());
    assert!(observation.has_runnable_predecessor());
    let retained_response_ordinal = fixture
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve an isolated retained-response target after Serve");
    assert!(
        fixture
            .executor
            .older_runtime_lifecycle_predates_retained_response(
                now,
                retained_response_ordinal,
            )
            .expect("exercise the published retained-response predecessor probe")
    );
    let repeated = fixture
        .executor
        .exact_serve_predecessor_observation(now, serve_ordinal, None)
        .expect("retained-response probing cannot reset selected-Serve state");
    assert!(repeated.should_open_predecessor_admission());
    assert!(repeated.has_runnable_predecessor());
    assert_eq!(
        fixture.executor.status().queued_runtime_completions,
        1,
        "the late Fetch successor is runnable inside serialized runtime"
    );

    assert_eq!(
        fixture
            .executor
            .step(now, &mut services)
            .expect("the reopened predecessor owns one serialized admission attempt"),
        EffectExecutorStep::Advanced { effects: 0 },
        "the orphan completion reaches one durable terminal without inventing reducer work"
    );
    assert_eq!(fixture.executor.status().queued_runtime_completions, 0);
    assert!(
        services.store_tasks.is_empty(),
        "an injected Fetch without matching reducer body work must not create a Store task"
    );
    assert!(fixture.executor.pending_fetches.is_empty());
    assert!(fixture.executor.pending_stores.is_empty());
    assert_eq!(
        fixture.executor.ready_bodies.len(),
        1,
        "the reconstructed body remains staged for later matching reducer work"
    );
    let terminal = fixture
        .executor
        .exact_serve_predecessor_observation(now, serve_ordinal, None)
        .expect("the durable terminal closes the selected Serve aperture");
    assert!(!terminal.should_open_predecessor_admission());
    assert!(!terminal.has_runnable_predecessor());
    assert!(!fixture.executor.status().fail_closed);
}
