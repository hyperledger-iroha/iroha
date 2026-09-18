// Regression coverage for retained Serve ownership across physical work states.

#[test]
fn lifecycle_serve_ownership_survives_worker_states_until_exact_acknowledgement() {
    let (sender, receiver, _admission) = test_io_command_channel(2);
    let request_hash = HashOf::from_untyped_unchecked(Hash::new(b"retained Serve ownership"));
    let expected = [
        LifecycleServeOwnershipV1 {
            lifecycle_ordinal: 7,
            request_hash,
            authority: LifecycleServeAuthorityKindV1::Claimed,
        },
        LifecycleServeOwnershipV1 {
            lifecycle_ordinal: 11,
            request_hash,
            authority: LifecycleServeAuthorityKindV1::TerminalReplay,
        },
    ];
    for owner in expected {
        receiver.queue.lock().lifecycle_serves.insert(
            owner.lifecycle_ordinal,
            V2IoTrackedLifecycleServeV1 {
                request_hash: owner.request_hash,
                authority: owner.authority,
                state: V2IoWorkState::Queued,
            },
        );
    }
    for physical_state in [
        V2IoWorkState::Queued,
        V2IoWorkState::Active,
        V2IoWorkState::CompletionPending,
    ] {
        for tracked in receiver.queue.lock().lifecycle_serves.values_mut() {
            tracked.state = physical_state;
        }
        assert_eq!(
            receiver.queue.lifecycle_serve_ownership_snapshot(),
            expected
        );
        assert_eq!(
            receiver.queue.lifecycle_serve_ownership_snapshot(),
            expected
        );
        assert!(
            receiver
                .queue
                .lock()
                .lifecycle_serves
                .values()
                .all(|tracked| { tracked.state == physical_state })
        );
    }
    receiver.queue.close_receiver();
    assert_eq!(
        receiver.queue.lifecycle_serve_ownership_snapshot(),
        expected
    );
    receiver
        .queue
        .acknowledge_lifecycle_certified_serve(11, request_hash);
    assert_eq!(
        receiver.queue.lifecycle_serve_ownership_snapshot(),
        expected[..1]
    );
    receiver
        .queue
        .acknowledge_lifecycle_certified_serve(7, request_hash);
    assert!(
        receiver
            .queue
            .lifecycle_serve_ownership_snapshot()
            .is_empty()
    );
    drop(receiver);
    drop(sender);
}

#[test]
fn lifecycle_serve_service_snapshot_distinguishes_absent_io_from_empty_ownership() {
    let (mut service, _) = fixture();
    assert!(service.lifecycle_serve_ownership_snapshot().is_none());
    assert_eq!(service.has_unleased_lifecycle_completion_work(), None);
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Shutdown)));
        drop(completion_tx);
    });
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: Some(worker),
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    assert_eq!(
        service.lifecycle_serve_ownership_snapshot(),
        Some(Vec::new())
    );
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(false));
    drop(service);
}

#[test]
fn unleased_validate_ownership_survives_physical_completion_and_excludes_runtime_work() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let (command_tx, command_rx, admission) = test_io_command_channel(2);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    service
        .io
        .as_ref()
        .expect("installed I/O owner")
        .command_tx
        .try_send(V2IoCommand::Store(BodyStoreTask::for_test(
            21,
            tag,
            payload.manifest().clone(),
            body,
        )))
        .expect("admit ordinary Runtime Store");
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(false));
    assert!(matches!(command_rx.try_recv(), Ok(V2IoCommand::Store(_))));
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(false));
    command_rx.complete_work(EffectWorkId::for_test(21));
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(false));
    service
        .io
        .as_ref()
        .expect("installed I/O owner")
        .command_tx
        .acknowledge_completion(EffectWorkId::for_test(21));

    let validate_keys = [7, 11].map(|ordinal| {
        LifecycleValidateDispatchKeyV1::for_test(
            &service.context,
            crate::sumeragi::v2_lifecycle_coordinator::LifecycleDigest::new([0xC1; 32]),
            ordinal,
            ordinal,
            0,
            crate::sumeragi::v2_lifecycle_coordinator::LifecycleDigest::new([ordinal as u8; 32]),
        )
        .expect("exact source-bound Validate key")
    });
    for key in validate_keys {
        command_rx.queue.lock().lifecycle_validates.insert(
            key,
            V2IoTrackedLifecycleValidateV1 {
                state: V2IoWorkState::Queued,
            },
        );
    }
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(true));
    for key in validate_keys {
        command_rx
            .queue
            .lock()
            .lifecycle_validates
            .get_mut(&key)
            .expect("queued Validate owner")
            .state = V2IoWorkState::Active;
        assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(true));
        command_rx.complete_lifecycle_validate_failure(key);
        assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(true));
    }
    command_rx
        .queue
        .acknowledge_lifecycle_validate(validate_keys[0]);
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(true));
    command_rx
        .queue
        .acknowledge_lifecycle_validate(validate_keys[1]);
    assert_eq!(service.has_unleased_lifecycle_completion_work(), Some(false));
    service.io.as_mut().expect("installed I/O owner").join = Some(thread::spawn(move || {
        assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Shutdown)));
        drop(completion_tx);
    }));
    drop(service);
}
