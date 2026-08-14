#[test]
fn typed_finality_handoff_fences_changed_roster_after_sealing_active_writer() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        local_validator,
        requester,
        request,
        ..
    } = certified_sidecar_server_fixture();
    let (service_owner, transport_owner) = durable_exact_output_handoff_owner_pair();
    adapter.exact_output_handoff_owner = transport_owner;
    let mut service = service_for_history_context_with_local_validator_and_handoff_owner(
        Arc::clone(&kura),
        context,
        &validators,
        local_validator,
        service_owner,
    );
    let (durable_receipt, artifact) = durable_finality_fixture(&service, &validators);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"typed sidecar rollover has no winning lane output"),
    );
    let predecessor_generation = request.service_generation;
    let predecessor_roster = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let predecessor_roster_digest = canonical_merge_sidecar_roster_digest(&predecessor_roster);
    let reply_source_capacity = adapter.limits.reply_source_capacity.get();
    let sidecar_limits = adapter.limits.merge_sidecar_limits;
    let hub = PeerId::new(
        KeyPair::try_from_seed(vec![0xE6; 32], Algorithm::BlsNormal)
            .expect("deterministic typed-rollover hub")
            .public_key()
            .clone(),
    );
    let mut routes =
        NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), reply_source_capacity);
    let route = routes.mint_via(requester.clone(), hub);
    assert!(route.is_active() && route.is_reply_writable());
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_for_test(
                requester.clone(),
                route.clone(),
                request.clone(),
            )
            .expect("materialize one active predecessor response"),
        V2LaneIngressOutcome::Inserted
    );
    let effect = adapter
        .sidecar_effects
        .pop_front()
        .expect("active response owns one lane effect");
    let V2LaneWorkEffect::PostCertifiedMergeSidecar {
        peer,
        reply_routes: Some(reply_routes),
        message,
    } = effect
    else {
        panic!("active response must preserve its exact reply routes")
    };
    assert_eq!(peer, requester);
    let writer_route = reply_routes
        .iter()
        .next()
        .expect("singleton reply-route history")
        .clone();
    assert!(writer_route.same_delivery(&route));
    let writer_control = Arc::new(Mutex::new(None));
    let writer_control_for_hook = Arc::clone(&writer_control);
    service.set_exact_output_flush_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, &writer_route);
        *writer_control_for_hook
            .lock()
            .expect("retain predecessor writer control") = Some(control);
        Ok(ExactOutputTestAdmission::SidecarFlush(ack))
    });
    assert_eq!(
        service
            .post_certified_merge_sidecar_with_reply_routes(
                requester.clone(),
                Some(reply_routes),
                message,
            )
            .expect("dispatch predecessor chunk into exact writer ownership"),
        super::super::v2_worker::ExactFanoutOwnership::Owned
    );
    assert!(
        service
            .has_pending_exact_output()
            .expect("inspect predecessor writer")
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &durable_receipt,
                &artifact,
                &lane_authority,
            )
            .expect("durable finality supersedes the active writer"),
        1
    );
    let exact_output_handoff = service
        .seal_applied_height_output_handoff(&durable_receipt, &artifact, &lane_authority)
        .expect("seal the final empty writer corridor");
    let output_guard = Arc::clone(&adapter.output_guard);
    let lifecycle_root = adapter
        .merge_sidecars
        .lifecycle_root_high_water_path_for_test();
    let predecessor_root =
        std::fs::read(&lifecycle_root).expect("read the active predecessor V3 root");
    let replacement = PeerId::new(
        KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::BlsNormal)
            .expect("deterministic replacement validator")
            .public_key()
            .clone(),
    );
    let successor = immediate_successor_context(&artifact, Some(replacement));
    let successor_roster = successor
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let successor_roster_digest = canonical_merge_sidecar_roster_digest(&successor_roster);
    let retained = adapter
        .into_retained_merge_sidecars(exact_output_handoff, &artifact, &successor)
        .expect("exact service/transport owner binds the retained predecessor");
    let successor_construction = output_guard
        .begin_fail_stop_operation()
        .expect("successor construction starts while output remains open");
    let mut successor_transport = retained
        .rehydrate_for_successor(
            &successor,
            reply_source_capacity,
            sidecar_limits,
            successor_roster.len(),
            successor_roster_digest.clone(),
            Instant::now(),
        )
        .expect("sealed exact output durably fences active predecessor ownership");
    successor_construction.complete();
    assert!(
        !output_guard.restart_required(),
        "successful authority-gated rollover keeps successor output open"
    );
    assert_ne!(
        std::fs::read(&lifecycle_root).expect("read successor V3 root"),
        predecessor_root,
        "changed roster must publish a new root-anchored generation fence"
    );
    assert_eq!(
        successor_transport.server_service_generation_for_test(),
        CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(
                predecessor_generation
                    .get()
                    .checked_add(1)
                    .expect("test generation remains representable"),
            )
            .expect("successor service generation remains non-zero"),
        )
    );
    assert_eq!(
        successor_transport.server_roster_digest_for_test(),
        &successor_roster_digest
    );
    assert_ne!(
        successor_transport.server_roster_digest_for_test(),
        &predecessor_roster_digest
    );
    assert_eq!(successor_transport.server_stream_count_for_test(), 0);
    assert_eq!(successor_transport.server_request_gate_count_for_test(), 0);
    assert_eq!(
        successor_transport.server_request_attempt_count_for_test(),
        0
    );
    assert_eq!(
        successor_transport.retained_outbound_attempt_count_for_test(),
        0
    );
    assert_eq!(successor_transport.retained_outbound_bytes_for_test(), 0);
    assert!(
        successor_transport
            .drain_closed_server_prefixes()
            .is_empty(),
        "forced roster fencing must not forge requester-authenticated close prefixes"
    );
    let stale = successor_transport
        .admit_server_request(
            &requester,
            &request,
            Some(&route),
            &request.responder,
            Instant::now(),
        )
        .expect("stale predecessor request receives the successor fence");
    let ServerRequestAdmission::GenerationHint(post) = stale else {
        panic!("stale predecessor request must not recreate responder ownership")
    };
    assert!(
        post.reply_route
            .as_ref()
            .is_some_and(|retained| retained.same_delivery(&route))
    );
    let CertifiedMergeSidecarMessage::GenerationHint(hint) = post.message.as_ref() else {
        panic!("changed-roster fence must emit a GenerationHint")
    };
    assert_eq!(
        hint.current_generation,
        successor_transport.server_service_generation_for_test()
    );
    assert_eq!(hint.observed_generation, predecessor_generation);
    assert!(
        route.is_active() && route.is_reply_writable(),
        "generation fencing does not mutate the independent route capability"
    );
    assert!(
        !writer_control
            .lock()
            .expect("inspect predecessor writer control")
            .as_mut()
            .expect("predecessor reached writer admission")
            .flush(),
        "the sealed predecessor writer cannot complete after handoff"
    );
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("late predecessor completion is a sealed no-op")
    );
}
#[test]
fn typed_finality_handoff_preserves_same_roster_current_chunk_for_retry() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        local_validator,
        requester,
        request,
        ..
    } = certified_sidecar_server_fixture();
    let (service_owner, transport_owner) = durable_exact_output_handoff_owner_pair();
    adapter.exact_output_handoff_owner = transport_owner;
    let mut service = service_for_history_context_with_local_validator_and_handoff_owner(
        kura,
        context,
        &validators,
        local_validator,
        service_owner,
    );
    let (durable_receipt, artifact) = durable_finality_fixture(&service, &validators);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"same-roster sidecar retry has no winning lane output"),
    );
    let predecessor_generation = request.service_generation;
    let reply_source_capacity = adapter.limits.reply_source_capacity.get();
    let sidecar_limits = adapter.limits.merge_sidecar_limits;
    let hub = PeerId::new(
        KeyPair::try_from_seed(vec![0xE8; 32], Algorithm::BlsNormal)
            .expect("deterministic same-roster rollover hub")
            .public_key()
            .clone(),
    );
    let mut routes =
        NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), reply_source_capacity);
    let route = routes.mint_via(requester.clone(), hub);
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_for_test(requester.clone(), route.clone(), request,)
            .expect("materialize one same-roster predecessor response"),
        V2LaneIngressOutcome::Inserted
    );
    let effect = adapter
        .sidecar_effects
        .pop_front()
        .expect("same-roster response owns one lane effect");
    let V2LaneWorkEffect::PostCertifiedMergeSidecar {
        peer,
        reply_routes: Some(reply_routes),
        message,
    } = effect
    else {
        panic!("same-roster response must preserve its exact reply routes")
    };
    assert_eq!(peer, requester);
    let expected_message = Arc::clone(&message);
    let writer_route = reply_routes
        .iter()
        .next()
        .expect("singleton same-roster reply-route history")
        .clone();
    assert!(writer_route.same_delivery(&route));
    let writer_control = Arc::new(Mutex::new(None));
    let writer_control_for_hook = Arc::clone(&writer_control);
    service.set_exact_output_flush_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, &writer_route);
        *writer_control_for_hook
            .lock()
            .expect("retain same-roster predecessor writer control") = Some(control);
        Ok(ExactOutputTestAdmission::SidecarFlush(ack))
    });
    assert_eq!(
        service
            .post_certified_merge_sidecar_with_reply_routes(
                requester.clone(),
                Some(reply_routes),
                message,
            )
            .expect("dispatch same-roster predecessor writer"),
        super::super::v2_worker::ExactFanoutOwnership::Owned
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &durable_receipt,
                &artifact,
                &lane_authority,
            )
            .expect("durable finality supersedes the old writer occurrence"),
        1
    );
    let exact_output_handoff = service
        .seal_applied_height_output_handoff(&durable_receipt, &artifact, &lane_authority)
        .expect("seal the empty same-roster writer corridor");
    let successor = immediate_successor_context(&artifact, None);
    let successor_roster = successor
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let successor_roster_digest = canonical_merge_sidecar_roster_digest(&successor_roster);
    let retained = adapter
        .into_retained_merge_sidecars(exact_output_handoff, &artifact, &successor)
        .expect("bind same-roster transport to its exact worker receipt");
    let mut transitioned = retained
        .rehydrate_for_successor(
            &successor,
            reply_source_capacity,
            sidecar_limits,
            successor_roster.len(),
            successor_roster_digest.clone(),
            Instant::now(),
        )
        .expect("same roster preserves responder ownership");
    assert_eq!(
        transitioned.server_service_generation_for_test(),
        predecessor_generation,
        "an equal roster must not force a responder generation rollover"
    );
    assert_eq!(
        transitioned.server_roster_digest_for_test(),
        &successor_roster_digest,
        "an equal roster must retain its exact responder identity"
    );
    assert_eq!(transitioned.server_stream_count_for_test(), 1);
    assert_eq!(transitioned.server_request_gate_count_for_test(), 1);
    assert_eq!(transitioned.retained_outbound_attempt_count_for_test(), 1);
    assert!(transitioned.retained_outbound_bytes_for_test() != 0);
    let retried = transitioned
        .drain_outbound_chunks_durable(1, Instant::now())
        .expect("retry the retained current chunk")
        .pop()
        .expect("same-roster rollover queues the current chunk exactly once");
    assert_eq!(retried.peer, requester);
    assert_eq!(retried.message, expected_message);
    assert!(
        retried
            .reply_route
            .as_ref()
            .is_some_and(|retry_route| retry_route.same_delivery(&route))
    );
    assert!(
        !writer_control
            .lock()
            .expect("inspect same-roster predecessor writer control")
            .as_mut()
            .expect("predecessor reached writer admission")
            .flush(),
        "the superseded predecessor writer cannot acknowledge the retried chunk"
    );
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("sealed predecessor retries are terminal no-ops")
    );
}
#[test]
fn typed_changed_roster_v3_lifecycle_failure_preserves_predecessor_pair() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        ..
    } = certified_sidecar_server_fixture();
    let (service_owner, transport_owner) = durable_exact_output_handoff_owner_pair();
    adapter.exact_output_handoff_owner = transport_owner;
    let service = service_for_history_context_with_handoff_owner(
        Arc::clone(&kura),
        context,
        &validators,
        service_owner,
    );
    let (receipt, artifact) = durable_finality_fixture(&service, &validators);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"typed V3 lifecycle crash has no winning lane output"),
    );
    let handoff = service
        .seal_applied_height_output_handoff(&receipt, &artifact, &lane_authority)
        .expect("seal the empty typed V3 lifecycle fixture");
    let predecessor_generation = adapter.merge_sidecars.server_service_generation_for_test();
    let predecessor_roster = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let predecessor_roster_digest = canonical_merge_sidecar_roster_digest(&predecessor_roster);
    let replacement = PeerId::new(
        KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal)
            .expect("deterministic V2-snapshot replacement validator")
            .public_key()
            .clone(),
    );
    let successor = immediate_successor_context(&artifact, Some(replacement));
    let successor_roster = successor
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let successor_roster_digest = canonical_merge_sidecar_roster_digest(&successor_roster);
    let reply_source_capacity = adapter.limits.reply_source_capacity.get();
    let sidecar_limits = adapter.limits.merge_sidecar_limits;
    let lifecycle_temp = adapter
        .merge_sidecars
        .lifecycle_journal_temp_path_for_test();
    let lifecycle_state = adapter
        .merge_sidecars
        .lifecycle_journal_state_path_for_test();
    let lifecycle_root = adapter
        .merge_sidecars
        .lifecycle_root_high_water_path_for_test();
    let predecessor_snapshot =
        std::fs::read(&lifecycle_state).expect("read the predecessor V3 lifecycle snapshot");
    let predecessor_root =
        std::fs::read(&lifecycle_root).expect("read the predecessor V3 root high-water");
    let output_guard = Arc::clone(&adapter.output_guard);
    adapter
        .merge_sidecars
        .obstruct_lifecycle_journal_temp_for_test();
    let retained = adapter
        .into_retained_merge_sidecars(handoff, &artifact, &successor)
        .expect("bind the V3 lifecycle fixture to its exact transport");
    let successor_construction = output_guard
        .begin_fail_stop_operation()
        .expect("successor construction starts while output remains open");
    let error = match retained.rehydrate_for_successor(
        &successor,
        reply_source_capacity,
        sidecar_limits,
        successor_roster.len(),
        successor_roster_digest.clone(),
        Instant::now(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("an obstructed V3 lifecycle snapshot cannot commit a successor"),
    };
    drop(successor_construction);
    assert!(matches!(
        error,
        V2LaneWorkError::InvalidContext(ref reason) if reason.contains("lifecycle")
    ));
    assert!(
        output_guard.restart_required(),
        "a failed durable successor transition must latch restart recovery"
    );
    assert_eq!(
        std::fs::read(&lifecycle_state).expect("reread the predecessor V3 lifecycle snapshot"),
        predecessor_snapshot,
        "the V3 state must retain the complete predecessor on write failure"
    );
    assert_eq!(
        std::fs::read(&lifecycle_root).expect("reread the predecessor V3 root high-water"),
        predecessor_root,
        "the V3 root must retain the predecessor commitment on write failure"
    );
    std::fs::remove_dir(lifecycle_temp).expect("remove the injected V3 state obstruction");
    let recovered_predecessor = MergeSidecarTransport::open_durable_with_server_stream_capacity(
        &kura.store_root(),
        reply_source_capacity,
        sidecar_limits,
        predecessor_roster.len(),
        predecessor_roster_digest.clone(),
    )
    .expect("restart reopens the complete predecessor V3 state");
    assert_eq!(
        recovered_predecessor.server_service_generation_for_test(),
        predecessor_generation
    );
    assert_eq!(
        recovered_predecessor.server_roster_digest_for_test(),
        &predecessor_roster_digest
    );
    assert_eq!(recovered_predecessor.server_stream_count_for_test(), 0);
    assert_eq!(
        recovered_predecessor.server_request_gate_count_for_test(),
        0
    );
    drop(recovered_predecessor);
    let recovered_successor = MergeSidecarTransport::open_durable_with_server_stream_capacity(
        &kura.store_root(),
        reply_source_capacity,
        sidecar_limits,
        successor_roster.len(),
        successor_roster_digest.clone(),
    )
    .expect("restart retries the terminal changed-roster transition");
    assert_eq!(
        recovered_successor
            .server_service_generation_for_test()
            .get(),
        predecessor_generation.get() + 1
    );
    assert_eq!(
        recovered_successor.server_roster_digest_for_test(),
        &successor_roster_digest
    );
    assert_eq!(recovered_successor.server_stream_count_for_test(), 0);
    assert_eq!(recovered_successor.server_request_gate_count_for_test(), 0);
}
