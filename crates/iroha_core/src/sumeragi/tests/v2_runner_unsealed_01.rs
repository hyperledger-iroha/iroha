#[test]
fn open_height_lane_relay_drain_services_exactly_one_occurrence_per_turn() {
    let (mut adapter, keys) =
        super::super::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned);
    let (services, _) = super::super::v2_worker::tests::fixture();
    let sender = PeerId::new(keys[1].public_key().clone());
    let (lane_relay_tx, lane_relay_rx) = std::sync::mpsc::sync_channel(2);
    for certificate in [vec![0_u8], vec![1_u8]] {
        lane_relay_tx
            .try_send(LaneRelayMessage::QueuePlanAdmissionCertificate {
                sender: sender.clone(),
                certificate: Arc::new(certificate),
            })
            .expect("queue one authenticated relay occurrence");
    }

    assert!(
        drain_lane_relay_ingress(&lane_relay_rx, &mut adapter, &services, 0)
            .expect("service the first open-height relay occurrence")
    );
    assert!(
        drain_lane_relay_ingress(&lane_relay_rx, &mut adapter, &services, 0)
            .expect("service the second open-height relay occurrence"),
        "the first open-height turn must leave the second relay queued"
    );
    assert!(
        !drain_lane_relay_ingress(&lane_relay_rx, &mut adapter, &services, 0)
            .expect("observe the exhausted open-height relay queue")
    );
}

#[test]
fn direct_close_ack_retains_reply_route_from_lane_through_worker() {
    let super::super::v2_lane_work::tests::CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        local_validator,
        requester,
        request,
    } = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let mut services =
        super::super::v2_worker::tests::service_for_history_context_with_local_validator(
            kura,
            context,
            &validators,
            local_validator,
        );
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let reply_route = routes.mint(requester.clone());
    let mut close = CertifiedMergeSidecarCloseV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: request.service_generation,
        stream_epoch: request.stream_epoch,
        closed_through: request.semantic_sequence.get(),
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: requester.clone(),
        responder: request.responder,
    };
    close.close_id = close.canonical_close_id();
    assert_eq!(
        adapter.accept_relay_message(
            LaneRelayMessage::CertifiedMergeSidecar {
                sender: requester,
                reply_route: Some(reply_route.clone()),
                message: CertifiedMergeSidecarMessage::Close(close),
            },
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert!(matches!(
        adapter.next_effect(),
        Some(V2LaneWorkEffect::PostCertifiedMergeSidecar {
            reply_routes: Some(reply_routes),
            message,
            ..
        }) if reply_routes
            .iter()
            .any(|route| route.same_delivery(&reply_route))
            && matches!(
                message.as_ref(),
                CertifiedMergeSidecarMessage::CloseAck(_)
            )
    ));
    let mut malformed = adapter
        .next_effect()
        .expect("the direct CloseAck remains lane-owned before dispatch");
    let V2LaneWorkEffect::PostCertifiedMergeSidecar { reply_routes, .. } = &mut malformed else {
        unreachable!("the direct CloseAck keeps its sidecar effect kind")
    };
    *reply_routes = None;
    assert!(
        dispatch_lane_work_effect(&services, malformed)
            .expect_err("a responder control without its return route is malformed")
            .to_string()
            .contains("reply-route ownership")
    );
    dispatch_lane_work_effects(&mut adapter, &services, 1)
        .expect("dispatch the direct CloseAck through exact output");
    assert_eq!(adapter.effect_count(), 0);
    assert!(
        services
            .retains_reply_route_for_test(&reply_route)
            .expect("inspect direct CloseAck route in worker ownership")
    );
}
#[test]
fn closed_sidecar_prefix_handoff_requeues_only_failed_suffix() {
    let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let mut adapter = fixture.adapter;
    let responder = fixture.request.responder.clone();
    let second_requester = fixture
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &responder && peer != &fixture.requester)
        .expect("runner prefix retry fixture has a second remote requester");
    let mut second_request = fixture.request.clone();
    second_request.requester = second_requester;
    second_request.request_id = second_request.canonical_request_id();
    let requests = [fixture.request, second_request];
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    // The shared lane fixture reserves the production test corridor for
    // eight authenticated sources. Each capability must advertise that
    // exact geometry even though this case exercises one source.
    let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());
    for request in &requests {
        let reply_route = routes.mint_via(request.requester.clone(), hub.clone());
        assert_eq!(
            adapter
                .accept_certified_merge_sidecar_for_test(
                    request.requester.clone(),
                    reply_route.clone(),
                    request.clone(),
                )
                .expect("admit the exact Kura-backed request before closing it"),
            V2LaneIngressOutcome::Inserted
        );
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            closed_through: request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: request.requester.clone(),
            responder: responder.clone(),
        };
        close.close_id = close.canonical_close_id();
        assert_eq!(
            adapter.accept_relay_message(
                LaneRelayMessage::CertifiedMergeSidecar {
                    sender: request.requester.clone(),
                    reply_route: Some(reply_route),
                    message: CertifiedMergeSidecarMessage::Close(close),
                },
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
    }
    let mut first_applied = Vec::new();
    let mut calls = 0usize;
    let error = apply_certified_merge_sidecar_closed_prefixes_with(&mut adapter, |prefix| {
        calls = calls.saturating_add(1);
        if calls == 2 {
            Err("injected exact-output close failure".to_owned())
        } else {
            first_applied.push(prefix.clone());
            Ok(())
        }
    })
    .expect_err("the second exact-output close fails");
    assert!(matches!(
        error,
        V2RunnerError::Service(ref reason)
            if reason == "injected exact-output close failure"
    ));
    assert_eq!(first_applied.len(), 1);
    let mut retry_applied = Vec::new();
    apply_certified_merge_sidecar_closed_prefixes_with(&mut adapter, |prefix| {
        retry_applied.push(prefix.clone());
        Ok(())
    })
    .expect("retry applies the retained failed suffix");
    assert_eq!(retry_applied.len(), 1);
    assert_ne!(
        retry_applied[0], first_applied[0],
        "the already-applied prefix must not be repeated"
    );
    let applied_requesters = first_applied
        .iter()
        .chain(&retry_applied)
        .map(|prefix| prefix.requester.clone())
        .collect::<BTreeSet<_>>();
    let requesters = requests
        .into_iter()
        .map(|request| request.requester)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        applied_requesters, requesters,
        "the successful prefix plus the retried suffix cover the exact drained batch"
    );
    apply_certified_merge_sidecar_closed_prefixes_with(&mut adapter, |_| {
        panic!("a confirmed handoff must leave no prefix for another retry")
    })
    .expect("the confirmed handoff is empty");
}
#[test]
fn relayed_generation_hint_preserves_reply_route_from_lane_through_worker() {
    let super::super::v2_lane_work::tests::CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        local_validator,
        requester,
        request,
    } = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let changed_roster = super::super::v2_lane_work::tests::changed_merge_sidecar_server_roster();
    adapter
        .transition_merge_sidecar_responder_roster_for_test(&changed_roster)
        .expect("advance the quiescent responder generation");
    let mut services =
        super::super::v2_worker::tests::service_for_history_context_with_local_validator(
            kura,
            context,
            &validators,
            local_validator,
        );
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
    let route_a = routes.mint_via(requester.clone(), hub_a);
    let route_b = routes.mint_via(requester.clone(), hub_b);
    assert_eq!(
        adapter.accept_relay_message(
            LaneRelayMessage::CertifiedMergeSidecar {
                sender: requester.clone(),
                reply_route: Some(route_a.clone()),
                message: CertifiedMergeSidecarMessage::Request(request.clone()),
            },
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.accept_relay_message(
            LaneRelayMessage::CertifiedMergeSidecar {
                sender: requester.clone(),
                reply_route: Some(route_b.clone()),
                message: CertifiedMergeSidecarMessage::Request(request),
            },
            0,
        ),
        V2LaneIngressOutcome::Inserted,
        "an alternate authenticated delivery joins the same stateless Hint"
    );
    assert!(matches!(
        adapter.next_effect(),
        Some(V2LaneWorkEffect::PostCertifiedMergeSidecar {
            reply_routes: Some(reply_routes),
            message,
            ..
        }) if reply_routes.len() == 2
            && reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_a))
            && reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_b))
            && matches!(
                message.as_ref(),
                CertifiedMergeSidecarMessage::GenerationHint(_)
            )
    ));
    let mut malformed = adapter
        .next_effect()
        .expect("the routed GenerationHint remains lane-owned before dispatch");
    let V2LaneWorkEffect::PostCertifiedMergeSidecar { reply_routes, .. } = &mut malformed else {
        unreachable!("the GenerationHint keeps its sidecar effect kind")
    };
    *reply_routes = None;
    assert!(
        dispatch_lane_work_effect(&services, malformed)
            .expect_err("a GenerationHint without reply-route ownership is malformed")
            .to_string()
            .contains("reply-route ownership")
    );
    assert!(routes.retire(&route_a));
    dispatch_lane_work_effects(&mut adapter, &services, 1)
        .expect("dispatch the routed GenerationHint through exact output");
    assert_eq!(adapter.effect_count(), 0);
    assert!(
        !services
            .retains_reply_route_for_test(&route_a)
            .expect("the retired Hint source must not reach worker ownership")
    );
    assert!(
        services
            .retains_reply_route_for_test(&route_b)
            .expect("the live Hint sibling must remain in worker ownership")
    );
}
#[test]
fn runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling() {
    let (mut services, keys) = super::super::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 7,
        })
    });
    let local = PeerId::new(keys[0].public_key().clone());
    let requester = PeerId::new(keys[1].public_key().clone());
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = route_fixture.mint_via(requester.clone(), hub_a);
    let route_b = route_fixture.mint_via(requester.clone(), hub_b);
    let mut reply_routes =
        NetworkReplyRoutes::try_from_route(route_a.clone()).expect("first sidecar response source");
    reply_routes
        .merge(
            &NetworkReplyRoutes::try_from_route(route_b.clone())
                .expect("second sidecar response source"),
        )
        .expect("attach independent sidecar response source");
    let effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
        peer: requester.clone(),
        reply_routes: Some(reply_routes),
        message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
            local,
            requester,
            b"runner prune retired source",
        ))),
    };
    let (mut lane_work, _) =
        super::super::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned);
    assert!(lane_work.requeue_effect(effect));
    assert!(route_fixture.retire(&route_a));
    let queue_plan_scans = services
        .queue_plan_test_kura()
        .pending_queue_plan_admission_inventory_scans
        .load(Ordering::Relaxed);
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("a retired owned route cannot poison its live sibling");
    assert_eq!(
        services
            .queue_plan_test_kura()
            .pending_queue_plan_admission_inventory_scans
            .load(Ordering::Relaxed),
        queue_plan_scans
    );
    assert_eq!(lane_work.effect_count(), 0);
    assert!(
        !services
            .retains_reply_route_for_test(&route_a)
            .expect("inspect retired route ownership")
    );
    assert!(
        services
            .retains_reply_route_for_test(&route_b)
            .expect("inspect live sibling ownership")
    );
}
macro_rules! queue_plan_batch_case { ($($tokens:tt)*) => {{ $($tokens)* }}; }
#[test]
fn queue_plan_batch_scans_once_and_reuses_exact_sources() {
    queue_plan_batch_case! { let (services, _) = super::super::v2_worker::tests::fixture(); let (mut lane_work, keys) = super::super::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned); let view = 0; let (network_id, peer) = services.queue_plan_test_route(view); lane_work.set_queue_plan_test_network_id(network_id); super::super::v2_lane_work::tests::prepare_queue_plan_test(&mut lane_work, &keys); let mut effects = Vec::new(); for tag in [0x61, 0x62] { let (_, bytes) = super::super::v2_lane_work::tests::queue_plan_test_certificate(&lane_work, &keys, tag); services.queue_plan_test_kura().persist_pending_queue_plan_admission_certificate(&bytes).unwrap(); let effect = V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { peer: peer.clone(), view, certificate: Arc::new(bytes) }; assert!(lane_work.requeue_effect(effect.clone())); effects.push(effect); } let counters = || { let kura = services.queue_plan_test_kura(); (kura.pending_queue_plan_admission_inventory_scans.load(Ordering::Relaxed), kura.pending_queue_plan_admission_exact_reads.load(Ordering::Relaxed), kura.pending_queue_plan_admission_batch_validations.load(Ordering::Relaxed)) }; let before = counters(); dispatch_lane_work_effects_with_progress(&mut lane_work, &services, 2).unwrap(); assert_eq!(counters(), (before.0 + 1, before.1 + 2, before.2 + 2)); let mut missing_sources = services.queue_plan_admission_batch_sources().unwrap(); let missing = &effects[0]; let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { certificate, .. } = missing else { unreachable!() }; services.queue_plan_test_kura().remove_pending_queue_plan_admission_certificate(Hash::new(certificate.as_slice())).unwrap(); assert!(services.can_retain_lane_work_effect_from_snapshot(missing, Some(&mut missing_sources)).is_err()); let mut tampered_sources = services.queue_plan_admission_batch_sources().unwrap(); let tampered = &effects[1]; let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { certificate, .. } = tampered else { unreachable!() }; tampered_sources.corrupt_for_test(Hash::new(certificate.as_slice())); assert!(services.can_retain_lane_work_effect_from_snapshot(tampered, Some(&mut tampered_sources)).is_err()); }
}
#[test]
fn runner_preflight_enqueue_race_retains_sidecar_source_until_capacity_reopens() {
    let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let local_peer = fixture.request.responder.clone();
    let mut lane_work = fixture.adapter;
    let mut services =
        super::super::v2_worker::tests::service_for_history_context_with_local_validator(
            fixture.kura,
            fixture.context,
            &fixture.validators,
            fixture.local_validator,
        );
    services
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install one shared race slot plus frozen target reservations");
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let hub_c = PeerId::new(KeyPair::random().public_key().clone());
    // The lane fixture's production seam is configured for eight
    // authenticated reply sources. Route capabilities advertise that
    // exact geometry, even though this race uses only three sources.
    let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
    let actual_route = routes.mint_via(fixture.requester.clone(), hub_a);
    assert_eq!(
        lane_work
            .accept_certified_merge_sidecar_for_test(
                fixture.requester.clone(),
                actual_route.clone(),
                fixture.request,
            )
            .expect("materialize the source-owned sidecar effect"),
        V2LaneIngressOutcome::Inserted
    );
    let actual_effect = lane_work
        .next_effect()
        .expect("materialized chunk enters the lane queue");
    assert!(
        services
            .can_retain_lane_work_effect(&actual_effect)
            .expect("race preflight validates the empty corridor")
    );
    let _ = lane_work
        .drain_effects(1)
        .pop()
        .expect("take the exact effect after successful preflight");
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 5,
        })
    });
    let filler_a = routes.mint_via(fixture.requester.clone(), hub_b);
    let filler_b = routes.mint_via(fixture.requester.clone(), hub_c);
    for (route, label) in [
        (filler_a.clone(), b"runner race filler A".as_slice()),
        (filler_b.clone(), b"runner race filler B".as_slice()),
    ] {
        let reply_routes =
            NetworkReplyRoutes::try_from_route(route).expect("live filler source route");
        assert!(matches!(
            dispatch_lane_work_effect(
                &services,
                V2LaneWorkEffect::PostCertifiedMergeSidecar {
                    peer: fixture.requester.clone(),
                    reply_routes: Some(reply_routes),
                    message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
                        local_peer.clone(),
                        fixture.requester.clone(),
                        label,
                    ))),
                },
            )
            .expect("fill exact target/class ownership after preflight"),
            LaneWorkEffectDispatch::Complete
        ));
    }
    let retained_effect = match dispatch_lane_work_effect(&services, actual_effect)
        .expect("the enqueue race is bounded source backpressure")
    {
        LaneWorkEffectDispatch::SourceRetained(effect) => effect,
        LaneWorkEffectDispatch::Complete => {
            panic!("post-preflight capacity race must return the exact source owner")
        }
    };
    assert!(!services.exact_output_restart_required_for_test());
    let filler_controls = Arc::new(Mutex::new(Vec::new()));
    let filler_controls_for_hook = Arc::clone(&filler_controls);
    let mut filler_routes = vec![
        (Hash::new(b"runner race filler A"), filler_a),
        (Hash::new(b"runner race filler B"), filler_b),
    ];
    services.set_exact_output_flush_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let request_id = match &post.data {
            NetworkMessage::CertifiedMergeSidecar(message) => match message.as_ref() {
                CertifiedMergeSidecarMessage::Chunk(chunk) => &chunk.request_id,
                CertifiedMergeSidecarMessage::Request(_)
                | CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_) => {
                    panic!("retained sidecar filler changed into a request")
                }
            },
            _ => panic!("retained sidecar filler changed its network message kind"),
        };
        let index = filler_routes
            .iter()
            .position(|(expected, _)| expected == request_id)
            .expect("retained filler preserves its immutable request identity");
        let (_, route) = filler_routes.remove(index);
        let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, &route);
        filler_controls_for_hook
            .lock()
            .expect("retain filler writer controls")
            .push(control);
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(ack))
    });
    services
        .retry_pending_exact_output()
        .expect("responsive filler writers release exact ownership capacity");
    {
        let mut controls = filler_controls.lock().expect("flush filler controls");
        assert_eq!(controls.len(), 2);
        for control in controls.iter_mut() {
            assert!(control.flush());
        }
    }
    assert!(
        services
            .retry_pending_exact_output()
            .expect("flushed filler receipts remain owned until lane delivery")
    );
    assert_eq!(
        services
            .drain_certified_merge_sidecar_chunk_admissions(2)
            .expect("drain both successfully flushed filler receipts")
            .len(),
        2
    );
    assert!(
        !services
            .has_pending_exact_output()
            .expect("drained filler receipts release their exact ownership")
    );
    let actual_admissions = Arc::new(AtomicUsize::new(0));
    let actual_admissions_for_hook = Arc::clone(&actual_admissions);
    let actual_route_for_hook = actual_route.clone();
    let actual_control = Arc::new(Mutex::new(None));
    let actual_control_for_hook = Arc::clone(&actual_control);
    services.set_exact_output_flush_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        actual_admissions_for_hook.fetch_add(1, Ordering::Relaxed);
        let (control, ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&post, &actual_route_for_hook);
        assert!(
            actual_control_for_hook
                .lock()
                .expect("retain actual writer control")
                .replace(control)
                .is_none()
        );
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(ack))
    });
    assert!(lane_work.requeue_effect(retained_effect));
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("retained source dispatches after capacity reopens");
    assert_eq!(lane_work.effect_count(), 0);
    assert_eq!(actual_admissions.load(Ordering::Relaxed), 1);
    assert!(
        actual_control
            .lock()
            .expect("lock actual writer control")
            .as_mut()
            .expect("actual source reached writer admission")
            .flush()
    );
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("actual writer flush advances the source exactly once");
    assert_eq!(actual_admissions.load(Ordering::Relaxed), 1);
    assert!(
        !services
            .has_pending_exact_output()
            .expect("actual source receipt is fully applied")
    );
    assert!(!services.exact_output_restart_required_for_test());
}
#[test]
fn runner_dispatch_advances_certified_sidecar_only_after_writer_flush() {
    let (mut services, keys) = super::super::v2_worker::tests::fixture();
    let local = PeerId::new(keys[0].public_key().clone());
    let requester = PeerId::new(keys[1].public_key().clone());
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let route = route_fixture.mint(requester.clone());
    let route_for_hook = route.clone();
    let flush_control = Arc::new(Mutex::new(None));
    let flush_control_for_hook = Arc::clone(&flush_control);
    services.set_exact_output_flush_admission_hook(move |post, _| {
        let (control, flush_ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&post, &route_for_hook);
        assert!(
            flush_control_for_hook
                .lock()
                .expect("lock exact test writer-flush control")
                .replace(control)
                .is_none(),
            "one sidecar occurrence owns one exact writer-flush control"
        );
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(flush_ack))
    });
    let reply_routes = NetworkReplyRoutes::try_from_route(route).expect("live reply route set");
    let chunk = runner_sidecar_chunk(local, requester.clone(), b"runner admitted sidecar request");
    dispatch_lane_work_effect(
        &services,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: requester,
            reply_routes: Some(reply_routes),
            message: Arc::new(CertifiedMergeSidecarMessage::Chunk(chunk)),
        },
    )
    .expect("runner hands the certified chunk to exact output");
    assert!(
        services
            .has_pending_exact_output()
            .expect("receipt remains process-locally owned")
    );
    assert!(
        services
            .retry_pending_exact_output()
            .expect("pending writer completion remains visible to retry callers")
    );
    assert!(
        services
            .drain_certified_merge_sidecar_chunk_admissions(2)
            .expect("poll pending sidecar writer completion")
            .is_empty(),
        "actor admission alone must not advance the source cursor"
    );
    assert!(
        flush_control
            .lock()
            .expect("lock exact test writer-flush control")
            .as_mut()
            .expect("runner admission minted the exact writer-flush control")
            .flush()
    );
    assert!(
        services
            .retry_pending_exact_output()
            .expect("writer-flushed receipt remains owned until lane application")
    );
    assert_eq!(
        services
            .drain_certified_merge_sidecar_chunk_admissions(2)
            .expect("drain writer-flushed sidecar receipt")
            .len(),
        1
    );
    assert!(
        !services
            .has_pending_exact_output()
            .expect("receipt ownership is released after drain")
    );
    let (mut closed_services, keys) = super::super::v2_worker::tests::fixture();
    let local = PeerId::new(keys[0].public_key().clone());
    let requester = PeerId::new(keys[1].public_key().clone());
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let route = route_fixture.mint(requester.clone());
    let route_for_hook = route.clone();
    let close_control = Arc::new(Mutex::new(None));
    let close_control_for_hook = Arc::clone(&close_control);
    closed_services.set_exact_output_flush_admission_hook(move |post, _| {
        let (control, close_ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&post, &route_for_hook);
        assert!(
            close_control_for_hook
                .lock()
                .expect("lock exact test closed control")
                .replace(control)
                .is_none(),
            "one closed occurrence owns one exact writer-flush control"
        );
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(close_ack))
    });
    let reply_routes =
        NetworkReplyRoutes::try_from_route(route).expect("live closed-path reply route set");
    dispatch_lane_work_effect(
        &closed_services,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: requester.clone(),
            reply_routes: Some(reply_routes),
            message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
                local,
                requester,
                b"runner closed sidecar request",
            ))),
        },
    )
    .expect("runner retains a second sidecar completion");
    assert!(
        close_control
            .lock()
            .expect("lock exact test closed control")
            .as_mut()
            .expect("runner admission minted the exact closed control")
            .close()
    );
    assert!(
        closed_services
            .drain_certified_merge_sidecar_chunk_admissions(2)
            .expect("closed writer completion is harmless")
            .is_empty(),
        "closed writer ownership must never produce a cursor receipt"
    );
    assert!(
        closed_services
            .has_pending_exact_output()
            .expect("closed completion retains the current item for exact retry"),
        "a closed writer acknowledgement cannot erase an active source's current item"
    );
}
#[test]
fn runner_dispatch_retired_admission_race_emits_no_sidecar_receipt() {
    let (mut services, keys) = super::super::v2_worker::tests::fixture();
    let local = PeerId::new(keys[0].public_key().clone());
    let requester = PeerId::new(keys[1].public_key().clone());
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let route = route_fixture.mint(requester.clone());
    let route_for_hook = route.clone();
    services.set_exact_output_flush_admission_hook(move |_, _| {
        assert!(route_fixture.retire(&route_for_hook));
        Ok(super::super::v2_worker::ExactOutputTestAdmission::Retired)
    });
    let reply_routes =
        NetworkReplyRoutes::try_from_route(route).expect("initially live reply route set");
    dispatch_lane_work_effect(
        &services,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: requester.clone(),
            reply_routes: Some(reply_routes),
            message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
                local,
                requester,
                b"runner retired admission race",
            ))),
        },
    )
    .expect("tenure cancellation retires only the exact occurrence");
    assert!(
        services
            .drain_certified_merge_sidecar_chunk_admissions(1)
            .expect("retired occurrence owns no receipt")
            .is_empty()
    );
    assert!(
        !services
            .has_pending_exact_output()
            .expect("retired occurrence releases worker ownership")
    );
}
#[test]
fn runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once() {
    let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let mut lane_work = fixture.adapter;
    let mut services =
        super::super::v2_worker::tests::service_for_history_context_with_local_validator(
            fixture.kura,
            fixture.context,
            &fixture.validators,
            fixture.local_validator,
        );
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::new(hub);
    let first_route = routes.mint(fixture.requester.clone());
    assert_eq!(
        lane_work
            .accept_certified_merge_sidecar_for_test(
                fixture.requester.clone(),
                first_route.clone(),
                fixture.request.clone(),
            )
            .expect("materialize the first Kura-backed chunk"),
        V2LaneIngressOutcome::Inserted
    );
    let first_message = match lane_work.next_effect() {
        Some(V2LaneWorkEffect::PostCertifiedMergeSidecar { message, .. })
            if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(_)) =>
        {
            message
        }
        other => panic!("expected first Kura-backed sidecar chunk, got {other:?}"),
    };
    let first_route_for_hook = first_route.clone();
    let close_control = Arc::new(Mutex::new(None));
    let close_control_for_hook = Arc::clone(&close_control);
    services.set_exact_output_flush_admission_hook(move |post, _| {
        let (control, close_ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&post, &first_route_for_hook);
        assert!(
            close_control_for_hook
                .lock()
                .expect("lock first exact sidecar control")
                .replace(control)
                .is_none(),
            "first chunk owns one exact writer-flush control"
        );
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(close_ack))
    });
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("dispatch first chunk without advancing its cursor");
    assert!(routes.retire(&first_route));
    let reconnected_route = routes.mint(fixture.requester.clone());
    assert_eq!(
        lane_work
            .accept_certified_merge_sidecar_for_test(
                fixture.requester.clone(),
                reconnected_route.clone(),
                fixture.request,
            )
            .expect("reconnect rematerializes the retained current chunk"),
        V2LaneIngressOutcome::Inserted
    );
    match lane_work.next_effect() {
        Some(V2LaneWorkEffect::PostCertifiedMergeSidecar {
            reply_routes: Some(reply_routes),
            message,
            ..
        }) => {
            assert!(matches!(
                message.as_ref(),
                CertifiedMergeSidecarMessage::Chunk(_)
            ));
            assert_eq!(
                message, first_message,
                "reconnect must preserve the exact current chunk even when its bounded transport cache was rematerialized"
            );
            assert!(
                reply_routes
                    .iter()
                    .any(|route| route.same_delivery(&reconnected_route))
            );
        }
        other => panic!("expected reconnected current sidecar chunk, got {other:?}"),
    }
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("the worker absorbs the reconnect into its retained source attempt");
    assert_eq!(
        lane_work.effect_count(),
        0,
        "the route capability transfers exactly once into worker ownership"
    );
    assert!(
        services
            .has_pending_exact_output()
            .expect("old writer flush remains process-locally owned")
    );
    let reconnected_route_for_hook = reconnected_route.clone();
    let flush_control = Arc::new(Mutex::new(None));
    let flush_control_for_hook = Arc::clone(&flush_control);
    services.set_exact_output_flush_admission_hook(move |post, _| {
        let (control, flush_ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&post, &reconnected_route_for_hook);
        assert!(
            flush_control_for_hook
                .lock()
                .expect("lock reconnected exact sidecar control")
                .replace(control)
                .is_none(),
            "reconnected chunk owns one exact writer-flush control"
        );
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(flush_ack))
    });
    assert!(
        close_control
            .lock()
            .expect("lock first exact sidecar control")
            .as_mut()
            .expect("first chunk admission minted its exact closed control")
            .close()
    );
    retry_exact_output_and_apply_sidecar_admissions(&mut lane_work, &services, 1)
        .expect("closed old writer retries the retained current chunk on the reconnect");
    assert_eq!(lane_work.effect_count(), 0);
    assert!(
        flush_control
            .lock()
            .expect("lock reconnected exact sidecar control")
            .as_mut()
            .expect("reconnected admission minted its exact flush control")
            .flush()
    );
    retry_exact_output_and_apply_sidecar_admissions(&mut lane_work, &services, 1)
        .expect("writer flush advances exactly the reconnected source cursor");
    assert_eq!(lane_work.effect_count(), 0);
    assert!(
        !services
            .has_pending_exact_output()
            .expect("writer-flushed receipt is fully applied")
    );
}
#[test]
fn runner_old_flushed_sidecar_receipt_cancels_queued_reconnect_retry() {
    let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let mut lane_work = fixture.adapter;
    let mut services =
        super::super::v2_worker::tests::service_for_history_context_with_local_validator(
            fixture.kura,
            fixture.context,
            &fixture.validators,
            fixture.local_validator,
        );
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::new(hub);
    let first_route = routes.mint(fixture.requester.clone());
    assert_eq!(
        lane_work
            .accept_certified_merge_sidecar_for_test(
                fixture.requester.clone(),
                first_route.clone(),
                fixture.request.clone(),
            )
            .expect("materialize the first exact sidecar chunk"),
        V2LaneIngressOutcome::Inserted
    );
    let first_route_for_hook = first_route.clone();
    let flush_control = Arc::new(Mutex::new(None));
    let flush_control_for_hook = Arc::clone(&flush_control);
    services.set_exact_output_flush_admission_hook(move |post, _| {
        let (control, ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&post, &first_route_for_hook);
        assert!(
            flush_control_for_hook
                .lock()
                .expect("lock old exact writer control")
                .replace(control)
                .is_none()
        );
        Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(ack))
    });
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("dispatch the first route chunk");
    assert!(routes.retire(&first_route));
    let reconnected_route = routes.mint(fixture.requester.clone());
    assert_eq!(
        lane_work
            .accept_certified_merge_sidecar_for_test(
                fixture.requester.clone(),
                reconnected_route.clone(),
                fixture.request,
            )
            .expect("reconnect retains the old current chunk"),
        V2LaneIngressOutcome::Inserted
    );
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("the worker absorbs the reconnect into its retained source attempt");
    assert_eq!(lane_work.effect_count(), 0);
    assert!(
        services
            .has_pending_exact_output()
            .expect("the worker owns the reconnect while the old flush is pending")
    );
    assert!(
        flush_control
            .lock()
            .expect("lock old exact writer control")
            .as_mut()
            .expect("first admission installed its writer control")
            .flush()
    );
    retry_exact_output_and_apply_sidecar_admissions(&mut lane_work, &services, 1)
        .expect("late old flush advances the rebound source without identity mismatch");
    assert_eq!(
        lane_work.effect_count(),
        0,
        "the terminal old flush must remove the queued reconnect current chunk"
    );
    assert!(
        !services
            .retains_reply_route_for_test(&reconnected_route)
            .expect("reconnect was never transferred into worker ownership")
    );
    assert!(
        !services
            .has_pending_exact_output()
            .expect("the terminal receipt is fully applied exactly once")
    );
    assert!(!services.exact_output_restart_required_for_test());
}
#[test]
fn runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route() {
    let (services, keys) = super::super::v2_worker::tests::fixture();
    let local = PeerId::new(keys[0].public_key().clone());
    let requester = PeerId::new(keys[1].public_key().clone());
    let chunk = runner_sidecar_chunk(local, requester.clone(), b"runner missing sidecar route");
    let error = dispatch_lane_work_effect(
        &services,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: requester,
            reply_routes: None,
            message: Arc::new(CertifiedMergeSidecarMessage::Chunk(chunk)),
        },
    )
    .expect_err("runner must reject a sidecar response without local reply authority");
    assert!(error.to_string().contains("reply-route ownership"));
}
#[test]
fn runner_dispatch_rejects_durable_response_without_reply_routes() {
    let history = super::super::v2_lane_work::tests::durable_lane_history_fixture();
    let requester = history.certificate.commit_qc.validator_set[1].clone();
    let services = super::super::v2_worker::tests::service_for_history_context(
        history.kura,
        history.context,
        &history.validators,
    );
    let mut effect = V2LaneWorkEffect::PostDurableLaneCertificate {
        peer: requester,
        reply_routes: None,
        ingress_ownership: None,
        certificate: history.certificate,
    };
    assert!(
        retain_active_owned_reply_routes(&mut effect),
        "the scheduler prefilter must leave malformed ownership for strict dispatch"
    );
    let error = dispatch_lane_work_effect(&services, effect)
        .expect_err("runner must reject a durable response without local reply authority");
    assert!(
        error
            .to_string()
            .contains("lost its authenticated reply routes")
    );
}
#[test]
fn snapshot_successor_time_is_exact_bounded_and_restart_deterministic() {
    let height_started_at = Instant::now();
    let round_timeout = Duration::from_secs(20);
    assert_eq!(
        initial_block_sync_deadline(height_started_at, round_timeout, false),
        deadline_after(height_started_at, round_timeout),
        "an ordinary live height keeps the full quiet round before discovery"
    );
    assert_eq!(
        initial_block_sync_deadline(height_started_at, round_timeout, true),
        height_started_at,
        "a recovered or historically synchronized height probes immediately"
    );
    assert!(!retain_eager_block_sync(false, false));
    assert!(retain_eager_block_sync(true, false));
    assert!(retain_eager_block_sync(false, true));
    let anchor = wire::SnapshotBootstrapAnchor {
        snapshot_height: 99,
        snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"snapshot tip")),
        snapshot_block_creation_time_ms: 50_000,
        snapshot_state_hash: Hash::new(b"snapshot state"),
    };
    let cadence = Duration::from_millis(750);
    let first = snapshot_successor_logical_time(&anchor, cadence)
        .expect("representable snapshot successor time");
    let restarted = snapshot_successor_logical_time(&anchor, cadence)
        .expect("restart derives the same successor time");
    assert_eq!(first, Duration::from_millis(50_750));
    assert_eq!(restarted, first);
    assert!(matches!(
        snapshot_successor_logical_time(&anchor, Duration::ZERO),
        Err(V2RunnerError::InvalidSnapshotBootstrapCadence)
    ));
    assert!(matches!(
        snapshot_successor_logical_time(&anchor, Duration::from_nanos(999_999)),
        Err(V2RunnerError::InvalidSnapshotBootstrapCadence)
    ));
    let overflowing = wire::SnapshotBootstrapAnchor {
        snapshot_block_creation_time_ms: u64::MAX,
        ..anchor
    };
    assert!(matches!(
        snapshot_successor_logical_time(&overflowing, Duration::from_millis(1)),
        Err(V2RunnerError::V2BlockTimeOverflow)
    ));
}
#[test]
fn unsupported_storage_platform_rejects_runner_voter_and_admits_observer() {
    let admit_role = |role| require_validator_storage_platform(role == NodeRole::Validator, false);
    assert!(matches!(
        admit_role(NodeRole::Validator),
        Err(super::super::v2_lane_work::V2LaneWorkError::UnsupportedValidatorStoragePlatform)
    ));
    assert_eq!(
        admit_role(NodeRole::Observer),
        Ok(()),
        "an unsupported host may enter only the explicitly non-voting runner path"
    );
}
#[test]
fn explicit_observer_never_votes_even_when_present_in_roster() {
    let (context, keys) = context();
    let peer = PeerId::new(keys[0].public_key().clone());
    assert_eq!(
        local_validator_index(&context, &peer, NodeRole::Observer).expect("observer"),
        None
    );
    assert_eq!(
        local_validator_index(
            &context,
            &PeerId::new(
                KeyPair::try_from_seed(vec![0x55; 32], Algorithm::BlsNormal)
                    .expect("deterministic non-member key")
                    .public_key()
                    .clone()
            ),
            NodeRole::Validator
        )
        .expect("an out-of-roster configured validator remains a non-voting process"),
        None
    );
}
#[test]
fn runtime_queue_reserves_progress_and_completions() {
    let config = SumeragiV2Config {
        format_version: SUMERAGI_V2_CONFIG_FORMAT_VERSION,
        protocol_version: wire::PROTOCOL_VERSION,
        mode: wire::ConsensusMode::Permissioned,
        block_cadence_ms: 1_000,
        limits: SumeragiV2Limits {
            max_transactions: 512,
            max_payload_bytes: 16 * 1024 * 1024,
            max_queue_scan: 2_048,
            control_queue_capacity: 128,
            runtime_command_capacity: 8,
            runtime_progress_reserve: 2,
            runtime_completion_reserve: 2,
            body_queue_capacity: 16,
            authenticated_non_validator_source_capacity: 2,
            body_bytes: 160 * 1024 * 1024,
            body_source_bytes: 32 * 1024 * 1024,
            chunk_queue_capacity: 64,
            effect_work_capacity: 2,
            ready_body_capacity: 8,
            ready_body_bytes: 32 * 1024 * 1024,
            certified_request_capacity: 8,
            authenticated_merge_qc_capacity: 64,
            merge_leader_body_frame_headroom_bytes: 1024 * 1024,
            autonomous_carrier_headroom_bytes: 1024 * 1024,
            autonomous_producer_recheck_ms: 100,
            historical_recovery_stuck_attempts: 32,
            historical_recovery_retry_tier_attempts: 4,
            historical_recovery_max_retry_tier: 6,
            sidecar_service_burst: 8,
            merge_sidecar_inbound_session_capacity: 32,
            merge_sidecar_inbound_sessions_per_peer: 4,
            merge_sidecar_inbound_assembly_bytes: 64 * 1024 * 1024,
            merge_sidecar_inbound_assembly_bytes_per_peer: 32 * 1024 * 1024,
            merge_sidecar_deferred_block_capacity: 128,
            merge_sidecar_future_block_distance: 64,
            merge_sidecar_request_timeout_ms: 10_000,
            merge_sidecar_outbound_sessions_per_source: 2,
            merge_sidecar_outbound_bytes_per_source: 16 * 1024 * 1024,
            merge_sidecar_server_request_gates_per_source: 4,
            pending_certified_merge_entry_capacity: 1_024,
            pending_queue_plan_admission_capacity: 1_024,
            pending_control_sidecar_bytes: 256 * 1024 * 1024,
            merge_signing_guard_record_capacity: 1_024,
            merge_signing_guard_record_bytes: 16 * 1024 * 1024 + 64 * 1024,
            merge_signing_guard_total_bytes: 256 * 1024 * 1024,
            native_amx_signing_guard_record_capacity: 524_288,
            native_amx_signing_guard_record_bytes: 16 * 1024,
            native_amx_signing_guard_anchor_bytes: 4 * 1024,
        },
        key_policy: SumeragiV2KeyPolicy {
            activation_lead_blocks: 1,
            overlap_grace_blocks: 1,
            expiry_grace_blocks: 1,
            allowed_algorithms: vec![Algorithm::BlsNormal],
        },
    };
    assert!(runtime_queue_config(&config).is_ok());
    assert!(effect_queue_config(&config).is_ok());
    assert!(
        lane_work_limits(
            &config,
            1,
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            Duration::from_millis(10),
            Duration::from_secs(1),
        )
        .is_ok()
    );
    let mut insufficient_native = config.clone();
    insufficient_native
        .limits
        .native_amx_signing_guard_record_capacity = 130_559;
    assert!(matches!(
        lane_work_limits(
            &insufficient_native,
            1,
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            Duration::from_millis(10),
            Duration::from_secs(1),
        ),
        Err(V2RunnerError::NativeAmxSigningCapacity {
            configured: 130_559,
            required: 130_560,
        })
    ));
    let mut largest_supported_native = config.clone();
    largest_supported_native.limits.max_transactions = 4_096;
    largest_supported_native
        .limits
        .native_amx_signing_guard_record_capacity = 1_044_480;
    assert!(
        lane_work_limits(
            &largest_supported_native,
            1,
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            Duration::from_millis(10),
            Duration::from_secs(1),
        )
        .is_ok(),
        "the largest transaction bound covered by the hard signing journal must remain operable"
    );
    let mut unsupported_native = largest_supported_native;
    unsupported_native
        .limits
        .native_amx_signing_guard_record_capacity = 1_044_479;
    assert!(matches!(
        lane_work_limits(
            &unsupported_native,
            1,
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            Duration::from_millis(10),
            Duration::from_secs(1),
        ),
        Err(V2RunnerError::NativeAmxSigningCapacity {
            configured: 1_044_479,
            required: 1_044_480,
        })
    ));
    let mut invalid = config;
    invalid.limits.effect_work_capacity = 3;
    assert!(matches!(
        effect_queue_config(&invalid),
        Err(V2RunnerError::EffectWorkExceedsCompletionReserve {
            pending: 3,
            reserve: 2,
        })
    ));
}
#[test]
fn commit_certificate_runtime_backpressure_remains_retryable() {
    let admission = Err(CommitCertificateAdmissionError::Enqueue(
        NetworkIngressError::Backpressure(
            crate::sumeragi::v2_runtime::EnqueueError::ReservedCapacity,
        ),
    ));
    assert!(matches!(
        commit_certificate_admission_completed(admission),
        Ok(false)
    ));
    assert!(matches!(
        commit_certificate_admission_completed(Ok(())),
        Ok(true)
    ));
}
