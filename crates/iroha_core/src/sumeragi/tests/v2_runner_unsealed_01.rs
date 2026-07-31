
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
        let changed_roster =
            super::super::v2_lane_work::tests::changed_merge_sidecar_server_roster();
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
        let V2LaneWorkEffect::PostCertifiedMergeSidecar { reply_routes, .. } = &mut malformed
        else {
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
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = route_fixture.mint_via(requester.clone(), hub_a);
        let route_b = route_fixture.mint_via(requester.clone(), hub_b);
        let mut reply_routes = NetworkReplyRoutes::try_from_route(route_a.clone())
            .expect("first sidecar response source");
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

        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("a retired owned route cannot poison its live sibling");
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
                        message: Arc::new(CertifiedMergeSidecarMessage::Chunk(
                            runner_sidecar_chunk(
                                local_peer.clone(),
                                fixture.requester.clone(),
                                label,
                            )
                        )),
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
        let chunk =
            runner_sidecar_chunk(local, requester.clone(), b"runner admitted sidecar request");

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
        let admit_role =
            |role| require_validator_storage_platform(role == NodeRole::Validator, false);
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
        assert!(
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
            .is_err()
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
                require_hsm: false,
                allowed_algorithms: vec![Algorithm::BlsNormal],
                allowed_hsm_providers: Vec::new(),
            },
        };
        assert!(runtime_queue_config(&config).is_ok());
        assert!(effect_queue_config(&config).is_ok());

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

    #[test]
    fn tag_roundtrip_rejects_another_height() {
        let (context, _) = context();
        let tag = EventTag::new(1, 3, Generation::new(7));
        assert_eq!(round_for_tag(&context, tag).expect("round").view, 3);
        assert!(matches!(
            round_for_tag(&context, EventTag::new(2, 0, Generation::new(7))),
            Err(V2RunnerError::StaleTag)
        ));
    }

    fn proposal_owner(
        context: &wire::HeightContext,
        tag: EventTag,
        lock: Option<(u64, wire::BlockSubject)>,
        decided_subject: Option<wire::BlockSubject>,
    ) -> LocalProposalOwner {
        LocalProposalOwner {
            tag,
            locked_body: lock.map(|(view, subject)| {
                (
                    wire::ConsensusRound {
                        context_id: context.id(),
                        height: context.height,
                        view,
                    },
                    subject,
                )
            }),
            decided_subject,
        }
    }

    fn proposal_subject(label: &[u8]) -> wire::BlockSubject {
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
            payload_hash: Hash::new(&[label, b" payload"].concat()),
        }
    }

    #[test]
    fn locked_body_recovery_is_independent_of_reproposal_gates() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(18));
        let subject = proposal_subject(b"nonleader locked-body recovery");
        let locked_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        };
        let leader = context.leader(tag.view());
        let nonleader = if leader == 0 { 1 } else { 0 };
        let directive =
            LocalProposalDirective::for_test(tag, leader, Some(locked_round), Some(subject), None);
        let owner = LocalProposalOwner::from(directive);
        let expected_request = Some((tag, locked_round, subject));

        for (local_validator, attempted, can_admit) in [
            (nonleader, None, true),
            (leader, Some(owner), true),
            (leader, None, false),
        ] {
            let plan = locked_body_recovery_plan(directive, local_validator, attempted, can_admit);
            assert_eq!(plan.request, expected_request);
            assert!(
                !plan.may_repropose,
                "body recovery must survive every local reproposal gate"
            );
        }

        let eligible = locked_body_recovery_plan(directive, leader, None, true);
        assert_eq!(eligible.request, expected_request);
        assert!(eligible.may_repropose);

        let decided = LocalProposalDirective::for_test(
            tag,
            leader,
            Some(locked_round),
            Some(subject),
            Some(subject),
        );
        assert_eq!(
            locked_body_recovery_plan(decided, leader, None, true),
            LockedBodyRecoveryPlan {
                request: None,
                may_repropose: false,
            }
        );
    }

    #[test]
    fn same_tag_higher_lock_retires_all_local_proposal_owners() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(11));
        let subject_a = proposal_subject(b"local owner A");
        let subject_b = proposal_subject(b"local owner B");
        let owner_a = proposal_owner(&context, tag, Some((2, subject_a)), None);
        let owner_b = proposal_owner(&context, tag, Some((4, subject_b)), None);
        let now = Instant::now();
        let mut state = LocalProposalState {
            attempted: Some(owner_a),
            submitted: Some((owner_a, subject_a)),
            heartbeat_only: Some(owner_a),
            candidate_work_wait: Some(CandidateWorkWait {
                owner: owner_a,
                started_at: now,
                next_retry: now,
            }),
            pending_events: Some(PendingLocalEvents {
                owner: owner_a,
                subject: subject_a,
                events: Vec::new(),
            }),
            global_selection: None,
        };

        state.reconcile(owner_b);

        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.heartbeat_only.is_none());
        assert!(state.candidate_work_wait.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn deferred_autonomous_work_timeout_arms_only_an_empty_heartbeat() {
        let (context, _) = context();
        let owner = proposal_owner(
            &context,
            EventTag::new(context.height, 3, Generation::new(17)),
            None,
            None,
        );
        let started_at = Instant::now();
        let wait_bound = Duration::from_secs(2);
        let mut state = LocalProposalState::default();

        state.defer_candidate_work(owner, started_at, wait_bound);
        assert_eq!(state.heartbeat_only, None);
        assert!(
            state
                .candidate_work_wait
                .is_some_and(|wait| wait.owner == owner && wait.started_at == started_at)
        );

        let expired_at = started_at
            .checked_add(wait_bound)
            .expect("fixture wait deadline is representable");
        state.defer_candidate_work(owner, expired_at, wait_bound);
        assert_eq!(state.heartbeat_only, Some(owner));
        assert!(state.candidate_work_wait.is_none());

        state.defer_candidate_work(owner, expired_at, wait_bound);
        assert_eq!(
            state.heartbeat_only,
            Some(owner),
            "repeated timeout handling must never re-arm ordinary candidate work"
        );
        assert!(state.candidate_work_wait.is_none());
    }

    #[test]
    fn first_same_subject_lock_preserves_pending_local_proposal_events() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(14));
        let subject = proposal_subject(b"first lock keeps local subject");
        let unlocked = proposal_owner(&context, tag, None, None);
        let locked = proposal_owner(&context, tag, Some((5, subject)), None);
        let mut state = LocalProposalState {
            attempted: Some(unlocked),
            submitted: Some((unlocked, subject)),
            pending_events: Some(PendingLocalEvents {
                owner: unlocked,
                subject,
                events: Vec::new(),
            }),
            ..LocalProposalState::default()
        };

        state.reconcile(locked);

        assert_eq!(state.attempted, Some(locked));
        assert_eq!(state.submitted, Some((locked, subject)));
        assert!(
            state
                .pending_events
                .as_ref()
                .is_some_and(|pending| { pending.owner == locked && pending.subject == subject })
        );
    }

    #[test]
    fn higher_same_subject_lock_retires_prior_origin_work() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(15));
        let subject = proposal_subject(b"higher lock retires old origin");
        let lower = proposal_owner(&context, tag, Some((2, subject)), None);
        let higher = proposal_owner(&context, tag, Some((4, subject)), None);
        let mut state = LocalProposalState {
            attempted: Some(lower),
            submitted: Some((lower, subject)),
            pending_events: Some(PendingLocalEvents {
                owner: lower,
                subject,
                events: Vec::new(),
            }),
            ..LocalProposalState::default()
        };

        assert_ne!(lower, higher);
        state.reconcile(higher);

        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn first_same_subject_lock_from_prior_view_retires_unlocked_work() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(16));
        let subject = proposal_subject(b"old-origin first lock");
        let unlocked = proposal_owner(&context, tag, None, None);
        let locked = proposal_owner(&context, tag, Some((4, subject)), None);
        let mut state = LocalProposalState {
            attempted: Some(unlocked),
            submitted: Some((unlocked, subject)),
            pending_events: Some(PendingLocalEvents {
                owner: unlocked,
                subject,
                events: Vec::new(),
            }),
            ..LocalProposalState::default()
        };

        state.reconcile(locked);

        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn late_old_rejection_cannot_arm_heartbeat_for_replacement_lock() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(12));
        let subject_a = proposal_subject(b"rejected old A");
        let subject_b = proposal_subject(b"current B");
        let owner_a = proposal_owner(&context, tag, Some((2, subject_a)), None);
        let owner_b = proposal_owner(&context, tag, Some((4, subject_b)), None);
        let proposal_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: tag.view(),
        };
        let mut state = LocalProposalState {
            submitted: Some((owner_a, subject_a)),
            ..LocalProposalState::default()
        };

        assert_eq!(
            state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_a,),
            LocalValidationDisposition::Ignored
        );
        assert_eq!(state.heartbeat_only, None);

        state.submitted = Some((owner_b, subject_b));
        assert_eq!(
            state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_b,),
            LocalValidationDisposition::RetryHeartbeat
        );
        assert_eq!(state.heartbeat_only, Some(owner_b));

        state.submitted = Some((owner_b, subject_b));
        assert_eq!(
            state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_b,),
            LocalValidationDisposition::FatalHeartbeat
        );
    }

    #[test]
    fn decision_retires_local_work_before_prepared_delivery() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 6, Generation::new(13));
        let subject = proposal_subject(b"decided proposal");
        let active = proposal_owner(&context, tag, Some((4, subject)), None);
        let decided = proposal_owner(&context, tag, Some((4, subject)), Some(subject));
        let mut state = LocalProposalState {
            attempted: Some(active),
            submitted: Some((active, subject)),
            heartbeat_only: None,
            candidate_work_wait: None,
            pending_events: Some(PendingLocalEvents {
                owner: active,
                subject,
                events: Vec::new(),
            }),
            global_selection: None,
        };

        assert!(state.take_prepared_events(decided, tag, subject).is_none());
        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn height_one_proposal_projects_staged_genesis_to_resultless_wire() {
        let key_pair = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let transaction = TransactionBuilder::new(
            ChainId::from("height-one-resultless-projection"),
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "staged genesis execution".to_owned())])
        .sign(key_pair.private_key());
        let entrypoint = transaction.hash_as_entrypoint();
        let mut staged =
            SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None);
        staged
            .set_transaction_results(
                Vec::new(),
                &[entrypoint],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach deterministic staged genesis results");
        assert!(staged.has_results());
        assert!(!staged.is_resultless_proposal());
        assert!(staged.header().result_merkle_root().is_some());

        let staged_header_hash = staged.header().hash();
        let staged_hash = staged.hash();
        let staged_signatures = staged.signatures().cloned().collect::<Vec<_>>();
        let staged_result_root = staged.header().result_merkle_root();
        let staged_execution_wire = staged.encode_wire().expect("encode staged execution image");
        let wire = canonical_height_one_proposal_wire(&staged)
            .expect("encode canonical height-one proposal");
        let proposal = decode_framed_signed_block(&wire).expect("decode height-one proposal");

        assert!(proposal.is_resultless_proposal());
        assert!(!proposal.has_results());
        assert!(proposal.header().result_merkle_root().is_none());
        assert_eq!(proposal.header().hash(), staged_header_hash);
        assert_eq!(proposal.hash(), staged_hash);
        assert_eq!(
            proposal.signatures().cloned().collect::<Vec<_>>(),
            staged_signatures
        );
        assert_eq!(
            staged.header().result_merkle_root(),
            staged_result_root,
            "proposal projection must not mutate the staged result root"
        );
        assert_eq!(
            staged
                .encode_wire()
                .expect("re-encode staged execution image"),
            staged_execution_wire,
            "proposal projection must not mutate the staged execution image"
        );
        assert_eq!(
            Hash::new(&wire),
            staged
                .canonical_proposal_wire_hash()
                .expect("hash canonical staged-genesis proposal"),
        );
    }

    #[test]
    fn exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift() {
        let (context, _) = context();
        let key_pair = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519)
            .expect("deterministic proposal key");
        let transaction = TransactionBuilder::new(
            ChainId::from("locked-reproposal-exact-body"),
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "immutable locked body".to_owned())])
        .sign(key_pair.private_key());
        let block = SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None)
            .canonical_resultless_proposal();
        assert!(block.is_resultless_proposal());
        let canonical_wire = block.encode_wire().expect("encode exact proposal body");
        let locked_subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let tag = EventTag::new(context.height, 3, Generation::new(17));

        let encoded = encode_exact_local_body(&context, tag, Some(locked_subject), &canonical_wire)
            .expect("encode unchanged locked body at the reproposal round");
        assert_eq!(
            encoded.manifest().round,
            round_for_tag(&context, tag).unwrap()
        );
        assert_eq!(encoded.manifest().subject, locked_subject);
        let (_, chunks) = encoded.into_parts();
        assert_eq!(chunks.concat(), canonical_wire);

        let foreign_subject = proposal_subject(b"foreign locked subject");
        assert!(matches!(
            encode_exact_local_body(&context, tag, Some(foreign_subject), &canonical_wire,),
            Err(V2RunnerError::LockedBodyMismatch)
        ));
    }

