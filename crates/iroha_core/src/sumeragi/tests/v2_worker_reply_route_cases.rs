// Reply-route and exact-output worker regression tests.
// Included lexically by v2_worker::tests to preserve canonical test names.
#[test]
fn responder_control_retry_coalesces_alternate_return_route() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    for message in [
        certified_sidecar_close_ack(&service.local_peer, &peer, 17),
        certified_sidecar_generation_hint(&service.local_peer, &peer, 18),
    ] {
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let first_route = routes.mint_via(peer.clone(), hub_a);
        let second_route = routes.mint_via(peer.clone(), hub_b);
        let fanout_for = |message: CertifiedMergeSidecarMessage, route: NetworkReplyRoute| {
            let message_hash = HashOf::new(&message);
            PendingExactFanout::claimed_with_reply_routes(
                vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(message))],
                peer.clone(),
                NetworkReplyRoutes::try_from_route(route)
                    .expect("worker retry keeps one actor-owned return route"),
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: service.exact_output_scope(),
                    target: peer.clone(),
                    message_hash,
                },
            )
            .expect("valid actor-owned worker sidecar-control rollover claim")
            .expect("one exact actor-owned worker sidecar-control fanout")
        };
        let first = fanout_for(message.clone(), first_route);
        let second = fanout_for(message, second_route);
        let ExactTargetRoute::Reply(first_route) = &first.targets[0].route else {
            unreachable!("first responder control keeps an exact reply route")
        };
        let ExactTargetRoute::Reply(second_route) = &second.targets[0].route else {
            unreachable!("second responder control keeps an exact reply route")
        };
        assert!(!first_route.same_delivery(second_route));
        let first_route = first_route.clone();
        let second_route = second_route.clone();
        let mut pending = PendingExactOutput::new(4, 1, 2, std::slice::from_ref(&peer))
            .expect("two independent control sources fit the bounded corridor");
        assert_eq!(
            pending.enqueue_owned_reply_transfer(first),
            Ok(ExactFanoutOwnership::Owned)
        );
        assert_eq!(
            pending.enqueue_owned_reply_transfer(second),
            Ok(ExactFanoutOwnership::Owned),
            "exact retry must merge before same-target responder-control dedup"
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.fanouts[0].targets.len(), 2);
        assert!(pending.fanouts[0].targets.iter().any(
                |target| matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_delivery(&first_route))
            ));
        assert!(pending.fanouts[0].targets.iter().any(
                |target| matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_delivery(&second_route))
            ));
        let control_reservation = ExactTargetReservation {
            semantic_target: peer.clone(),
            class: ExactOutputClass::Lane,
            kind: ExactTargetReservationKind::SidecarReplyControl,
        };
        assert_eq!(
            pending.reservation_owner_counts.get(&control_reservation),
            Some(&1),
            "alternate exact routes share one fanout-level control credit"
        );
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 0);
        let mut admitted_routes = Vec::new();
        assert_eq!(
            pending.drive_with_budget(1, |_post, ticket, route| {
                assert!(ticket.is_none());
                let ExactTargetRoute::Reply(route) = route else {
                    panic!("responder control changed route kind")
                };
                admitted_routes.push(route.clone());
                Ok(())
            }),
            Ok(ExactOutputDriveOutcome::BudgetExhausted {
                closest_backpressure_rank: None,
            })
        );
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(
            pending.reservation_owner_counts.get(&control_reservation),
            Some(&1),
            "the fanout credit survives until its last exact route flushes"
        );
        assert_eq!(
            pending.drive_with_budget(1, |_post, ticket, route| {
                assert!(ticket.is_none());
                let ExactTargetRoute::Reply(route) = route else {
                    panic!("responder control changed route kind")
                };
                admitted_routes.push(route.clone());
                Ok(())
            }),
            Ok(ExactOutputDriveOutcome::Drained)
        );
        assert_eq!(admitted_routes.len(), 2);
        assert!(
            admitted_routes
                .iter()
                .any(|route| route.same_delivery(&first_route))
        );
        assert!(
            admitted_routes
                .iter()
                .any(|route| route.same_delivery(&second_route))
        );
        assert_eq!(pending.ownership_units, 0);
        assert_eq!(pending.shared_ownership_units, 0);
        assert!(
            !pending
                .reservation_owner_counts
                .contains_key(&control_reservation)
        );
    }
}
#[test]
fn responder_control_uses_ordinary_reply_flush_ownership() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let expected_route = routes.mint_via(peer.clone(), hub_a);
    let fanout = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &peer,
        certified_sidecar_close_ack(&service.local_peer, &peer, 19),
        NetworkReplyRoutes::try_from_route(expected_route.clone())
            .expect("CloseAck keeps one exact reply route"),
    );
    let mut pending = PendingExactOutput::new(2, 1, 1, std::slice::from_ref(&peer))
        .expect("one exact responder control fits");
    assert_eq!(
        pending.enqueue_owned_reply_transfer(fanout),
        Ok(ExactFanoutOwnership::Owned)
    );
    let mut flush_control = None;
    let _ = pending
        .drive_with_budget_ack(1, |post, _ticket, route, timeout_attempt| {
            let ExactTargetRoute::Reply(route) = route else {
                unreachable!("CloseAck must not use topology output")
            };
            assert!(route.same_delivery(&expected_route));
            assert!(matches!(
                &post.data,
                NetworkMessage::CertifiedMergeSidecar(message)
                    if matches!(
                        message.as_ref(),
                        CertifiedMergeSidecarMessage::CloseAck(_)
                    )
            ));
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            flush_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("retain the ordinary CloseAck writer-flush receipt");
    assert!(pending.admitted_sidecar_chunks.is_empty());
    assert!(
        pending.fanouts[0].targets[0]
            .pending_flush
            .as_ref()
            .is_some_and(|flush| flush.sidecar_admission.is_none()),
        "responder controls must never enter chunk-admission ownership"
    );
    assert!(
        routes.mark_reply_unwritable_while_delivery_active(&expected_route),
        "the old route may drain while its admitted write still owns a flush"
    );
    let alternate_route = routes.mint_via(peer.clone(), hub_b);
    let alternate_message = certified_sidecar_generation_hint(&service.local_peer, &peer, 21);
    let alternate = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &peer,
        alternate_message.clone(),
        NetworkReplyRoutes::try_from_route(alternate_route.clone())
            .expect("the alternate source attempt has one live reply route"),
    );
    assert_eq!(
        pending.can_enqueue(&alternate),
        Ok(false),
        "a pending old writer flush cannot be replaced"
    );
    assert_eq!(
        pending.enqueue(alternate),
        Ok(ExactFanoutOwnership::SourceRetained)
    );
    assert!(pending.fanouts[0].targets[0].pending_flush.is_some());
    assert!(matches!(
        pending.fanouts[0].messages.as_slice(),
        [NetworkMessage::CertifiedMergeSidecar(message)]
            if matches!(message.as_ref(), CertifiedMergeSidecarMessage::CloseAck(_))
    ));
    assert!(
        flush_control
            .as_mut()
            .expect("the exact reply minted one writer receipt")
            .flush()
    );
    pending
        .poll_reply_flushes()
        .expect("consume the ordinary CloseAck writer flush");
    assert!(pending.fanouts.is_empty());
    assert!(pending.admitted_sidecar_chunks.is_empty());
    let alternate = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &peer,
        alternate_message,
        NetworkReplyRoutes::try_from_route(alternate_route)
            .expect("the retained alternate control keeps its exact route"),
    );
    assert_eq!(pending.can_enqueue(&alternate), Ok(true));
    assert_eq!(pending.enqueue(alternate), Ok(ExactFanoutOwnership::Owned));
}
#[test]
fn generation_hint_uses_ordinary_reply_flush_ownership() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let fanout = certified_sidecar_control_fanout(
        service.exact_output_scope(),
        &peer,
        certified_sidecar_generation_hint(&service.local_peer, &peer, 20),
    );
    let ExactTargetRoute::Reply(expected_route) = &fanout.targets[0].route else {
        unreachable!("GenerationHint keeps an exact reply route")
    };
    let expected_route = expected_route.clone();
    let mut pending = PendingExactOutput::new(2, 1, 1, std::slice::from_ref(&peer))
        .expect("one exact GenerationHint fits");
    assert_eq!(
        pending.enqueue_owned_reply_transfer(fanout),
        Ok(ExactFanoutOwnership::Owned)
    );
    let mut flush_control = None;
    let _ = pending
        .drive_with_budget_ack(1, |post, _ticket, route, timeout_attempt| {
            let ExactTargetRoute::Reply(route) = route else {
                unreachable!("GenerationHint must not use topology output")
            };
            assert!(route.same_delivery(&expected_route));
            assert!(matches!(
                &post.data,
                NetworkMessage::CertifiedMergeSidecar(message)
                    if matches!(
                        message.as_ref(),
                        CertifiedMergeSidecarMessage::GenerationHint(_)
                    )
            ));
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            flush_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("retain the ordinary GenerationHint writer-flush receipt");
    assert!(pending.admitted_sidecar_chunks.is_empty());
    assert!(
        pending.fanouts[0].targets[0]
            .pending_flush
            .as_ref()
            .is_some_and(|flush| flush.sidecar_admission.is_none()),
        "GenerationHint must not enter chunk-admission ownership"
    );
    assert!(
        flush_control
            .as_mut()
            .expect("the exact reply minted one writer receipt")
            .flush()
    );
    pending
        .poll_reply_flushes()
        .expect("consume the ordinary GenerationHint writer flush");
    assert!(pending.fanouts.is_empty());
    assert!(pending.admitted_sidecar_chunks.is_empty());
}
#[test]
fn writable_responder_control_source_retains_one_distinct_pending_control() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let scope = service.exact_output_scope();
    let mut controls = Vec::new();
    for ordinal in 0..32 {
        controls.push(certified_sidecar_generation_hint(
            &service.local_peer,
            &peer,
            ordinal,
        ));
        controls.push(certified_sidecar_close_ack(
            &service.local_peer,
            &peer,
            ordinal,
        ));
    }
    let retry_after_drain = controls
        .get(1)
        .expect("the adversarial control set contains a close acknowledgement")
        .clone();
    let first_control_hash = HashOf::new(
        controls
            .first()
            .expect("the adversarial control set is non-empty"),
    );
    let mut pending = PendingExactOutput::new(controls.len(), 1, 1, &[])
        .expect("the unconstrained test corridor could retain every control");
    for (index, message) in controls.into_iter().enumerate() {
        match &message {
            CertifiedMergeSidecarMessage::GenerationHint(hint) => {
                assert_eq!(hint.hint_id, hint.canonical_hint_id());
            }
            CertifiedMergeSidecarMessage::CloseAck(ack) => {
                assert_eq!(ack.close_id, ack.canonical_close_id());
            }
            _ => unreachable!("the adversarial set contains only responder controls"),
        }
        let fanout = certified_sidecar_control_fanout(scope, &peer, message);
        assert!(
            fanout.is_retryable_certified_sidecar_responder_control_fanout(),
            "only stateless responder controls exercise the bounded-control rule"
        );
        let expected_available = index == 0;
        assert_eq!(pending.can_enqueue(&fanout), Ok(expected_available));
        assert_eq!(
            pending.enqueue(fanout),
            Ok(if expected_available {
                ExactFanoutOwnership::Owned
            } else {
                ExactFanoutOwnership::SourceRetained
            }),
            "one distinct pending control remains owned by upstream lane work"
        );
        assert_eq!(
            pending.fanouts.len(),
            1,
            "distinct canonical responder controls must not accumulate"
        );
        let retained = pending
            .fanouts
            .front()
            .expect("one responder control remains retained");
        let [NetworkMessage::CertifiedMergeSidecar(retained)] = retained.messages.as_slice() else {
            unreachable!("retained worker control keeps its exact sidecar message")
        };
        assert_eq!(
            HashOf::new(retained.as_ref()),
            first_control_hash,
            "a writable incumbent is never displaced by a distinct control"
        );
    }
    let mut drained = Vec::new();
    assert_eq!(
        pending.drive_with(|post, ticket, route| {
            assert!(ticket.is_none());
            assert!(matches!(route, ExactTargetRoute::Reply(_)));
            let NetworkMessage::CertifiedMergeSidecar(message) = post.data else {
                unreachable!("drained worker control keeps its exact sidecar message")
            };
            drained.push(HashOf::new(message.as_ref()));
            Ok(())
        }),
        Ok(None)
    );
    assert_eq!(drained, vec![first_control_hash]);
    assert!(pending.fanouts.is_empty());
    let retried_hash = HashOf::new(&retry_after_drain);
    let retried = certified_sidecar_control_fanout(scope, &peer, retry_after_drain);
    assert_eq!(pending.can_enqueue(&retried), Ok(true));
    assert_eq!(pending.enqueue(retried), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(pending.fanouts.len(), 1);
    let [NetworkMessage::CertifiedMergeSidecar(retained)] = pending.fanouts[0].messages.as_slice()
    else {
        unreachable!("retried worker control keeps its exact sidecar message")
    };
    assert_eq!(
        HashOf::new(retained.as_ref()),
        retried_hash,
        "a retried control may enter once the prior control drains"
    );
    assert!(matches!(
        retained.as_ref(),
        CertifiedMergeSidecarMessage::CloseAck(_)
    ));
}
#[test]
fn ordinary_exact_output_does_not_suppress_retryable_sidecar_control_ownership() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let scope = service.exact_output_scope();
    let progress = merge_share_message(b"progress before retryable sidecar control");
    let progress_hash = HashOf::new(&progress);
    let mut pending = PendingExactOutput::new(8, 1, 1, &[]).expect("eight exact fanouts fit");
    assert_eq!(
        pending.enqueue(
            PendingExactFanout::new(vec![progress], vec![peer.clone()])
                .expect("one exact progress fanout"),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    let control = certified_sidecar_control_fanout(
        scope,
        &peer,
        certified_sidecar_generation_hint(&service.local_peer, &peer, 91),
    );
    assert_eq!(pending.can_enqueue(&control), Ok(true));
    assert_eq!(
        pending.enqueue(control),
        Ok(ExactFanoutOwnership::Owned),
        "ordinary output for the target does not own its responder-control occurrence"
    );
    assert_eq!(
        pending.fanouts.len(),
        2,
        "one ordinary fanout and one retryable responder control coexist"
    );
    assert_eq!(pending.fanouts[0].message_hashes, vec![progress_hash]);
    assert!(pending.fanouts[1].is_retryable_certified_sidecar_responder_control_fanout());
    let (request_message, chunk_message) = certified_sidecar_outputs(&service.local_peer, &peer);
    let CertifiedMergeSidecarMessage::Request(request) = &request_message else {
        unreachable!("worker sidecar fixture returns one request")
    };
    let request_transfer = CertifiedSidecarTransferIdentity::from_request(request);
    let request_hash = HashOf::new(request);
    let request_service_generation = request.service_generation;
    let request_stream_epoch = request.stream_epoch;
    let request_semantic_sequence = request.semantic_sequence;
    let request_fanout = PendingExactFanout::claimed(
        vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(
            request_message,
        ))],
        vec![peer.clone()],
        ExactOutputRolloverClaim::CertifiedSidecarRequest {
            scope,
            target: peer.clone(),
            transfer: request_transfer,
            request_hash,
        },
    )
    .expect("valid exact sidecar request claim")
    .expect("one exact sidecar request fanout");
    let mut close = crate::merge_sidecar::CertifiedMergeSidecarCloseV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: request_service_generation,
        stream_epoch: request_stream_epoch,
        closed_through: request_semantic_sequence.get(),
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: service.local_peer.clone(),
        responder: peer.clone(),
    };
    close.close_id = close.canonical_close_id();
    let close_message = CertifiedMergeSidecarMessage::Close(close);
    let close_fanout = certified_sidecar_control_fanout(scope, &peer, close_message);
    let CertifiedMergeSidecarMessage::Chunk(chunk) = &chunk_message else {
        unreachable!("worker sidecar fixture returns one response chunk")
    };
    let chunk_transfer = CertifiedSidecarTransferIdentity::from_chunk(chunk);
    let chunk_index = chunk.chunk_index;
    let chunk_count = chunk.chunk_count;
    let response_hash = HashOf::new(chunk);
    let chunk_fanout = PendingExactFanout::claimed(
        vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(
            chunk_message,
        ))],
        vec![peer.clone()],
        ExactOutputRolloverClaim::CertifiedSidecarChunk {
            scope,
            target: peer.clone(),
            transfer: chunk_transfer,
            chunk_index,
            chunk_count,
            response_hash,
        },
    )
    .expect("valid exact sidecar chunk claim")
    .expect("one exact sidecar chunk fanout");
    for fanout in [request_fanout, close_fanout, chunk_fanout] {
        assert!(
            !fanout.is_retryable_certified_sidecar_responder_control_fanout(),
            "request, close, and chunk output retain ordinary exact ownership"
        );
        assert_eq!(pending.can_enqueue(&fanout), Ok(true));
        assert_eq!(pending.enqueue(fanout), Ok(ExactFanoutOwnership::Owned));
    }
    assert_eq!(
        pending.fanouts.len(),
        5,
        "request, close, and chunk fanouts still enter behind existing progress"
    );
    let later_progress = PendingExactFanout::new(
        vec![merge_share_message(
            b"non-control progress after retryable sidecar control",
        )],
        vec![peer],
    )
    .expect("one later exact progress fanout");
    assert_eq!(pending.can_enqueue(&later_progress), Ok(true));
    assert_eq!(
        pending.enqueue(later_progress),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert_eq!(
        pending.fanouts.len(),
        6,
        "the bounded-control exception must not change non-control admission"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn generation_fence_cancels_only_older_request_and_close_for_exact_endpoint() {
    let (service, _) = fixture();
    let requester = service.local_peer.clone();
    let responder = service.context.roster[1].validator.clone();
    let other_responder = service.context.roster[2].validator.clone();
    let ack_target = service.context.roster[3].validator.clone();
    let scope = service.exact_output_scope();
    let generation = |value| {
        crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(value).expect("test service generation is non-zero"),
        )
    };
    let old_generation = generation(1);
    let current_generation = generation(3);
    let newer_generation = generation(4);
    let request =
        |target: &PeerId,
         service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1,
         ordinal: u64| {
            let (message, _) = certified_sidecar_outputs(&requester, target);
            let CertifiedMergeSidecarMessage::Request(mut request) = message else {
                unreachable!("worker sidecar fixture returns one request")
            };
            request.service_generation = service_generation;
            request.stream_epoch = CertifiedMergeSidecarStreamEpochV1(
                NonZeroU64::new(ordinal).expect("test request stream epoch is non-zero"),
            );
            request.semantic_sequence = CertifiedMergeSidecarSemanticSequenceV1(
                NonZeroU64::new(ordinal).expect("test request semantic sequence is non-zero"),
            );
            request.request_id = request.canonical_request_id();
            request
        };
    let request_fanout = |request: CertifiedMergeSidecarRequestV1| {
        let target = request.responder.clone();
        let transfer = CertifiedSidecarTransferIdentity::from_request(&request);
        let request_hash = HashOf::new(&request);
        PendingExactFanout::claimed(
            vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(
                CertifiedMergeSidecarMessage::Request(request),
            ))],
            vec![target.clone()],
            ExactOutputRolloverClaim::CertifiedSidecarRequest {
                scope,
                target,
                transfer,
                request_hash,
            },
        )
        .expect("valid exact sidecar request claim")
        .expect("one exact sidecar request fanout")
    };
    let close =
        |target: &PeerId,
         service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1,
         ordinal: u64| {
            let mut close = crate::merge_sidecar::CertifiedMergeSidecarCloseV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation,
                stream_epoch: CertifiedMergeSidecarStreamEpochV1(
                    NonZeroU64::new(ordinal).expect("test close stream epoch is non-zero"),
                ),
                closed_through: ordinal,
                close_id: Hash::prehashed([0; Hash::LENGTH]),
                requester: requester.clone(),
                responder: target.clone(),
            };
            close.close_id = close.canonical_close_id();
            close
        };
    let close_fanout = |close: crate::merge_sidecar::CertifiedMergeSidecarCloseV1| {
        let target = close.responder.clone();
        certified_sidecar_control_fanout(scope, &target, CertifiedMergeSidecarMessage::Close(close))
    };

    let stale_request = request(&responder, old_generation, 1);
    let observed_message_hash = HashOf::new(&stale_request).into();
    let mut hint = CertifiedMergeSidecarGenerationHintV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        observed_generation: old_generation,
        current_generation,
        observed_message_hash,
        hint_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: requester.clone(),
        responder: responder.clone(),
    };
    hint.hint_id = hint.canonical_hint_id();

    let (_, chunk_message) = certified_sidecar_outputs(&responder, &requester);
    let CertifiedMergeSidecarMessage::Chunk(chunk) = &chunk_message else {
        unreachable!("worker sidecar fixture returns one chunk")
    };
    assert_eq!(chunk.requester, requester);
    assert_eq!(chunk.responder, responder);
    let chunk_target = chunk.requester.clone();
    let chunk_transfer = CertifiedSidecarTransferIdentity::from_chunk(chunk);
    let chunk_index = chunk.chunk_index;
    let chunk_count = chunk.chunk_count;
    let chunk_hash = HashOf::new(chunk);
    let chunk_fanout = PendingExactFanout::claimed(
        vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(
            chunk_message,
        ))],
        vec![chunk_target.clone()],
        ExactOutputRolloverClaim::CertifiedSidecarChunk {
            scope,
            target: chunk_target,
            transfer: chunk_transfer,
            chunk_index,
            chunk_count,
            response_hash: chunk_hash,
        },
    )
    .expect("valid exact sidecar chunk claim")
    .expect("one exact sidecar chunk fanout");
    let cases = vec![
        (
            "older request for authenticated endpoint",
            true,
            request_fanout(stale_request),
        ),
        (
            "older close for authenticated endpoint",
            true,
            close_fanout(close(&responder, old_generation, 2)),
        ),
        (
            "equal-generation request",
            false,
            request_fanout(request(&responder, current_generation, 3)),
        ),
        (
            "newer-generation request",
            false,
            request_fanout(request(&responder, newer_generation, 4)),
        ),
        (
            "equal-generation close",
            false,
            close_fanout(close(&responder, current_generation, 5)),
        ),
        (
            "newer-generation close",
            false,
            close_fanout(close(&responder, newer_generation, 6)),
        ),
        (
            "older request for another responder",
            false,
            request_fanout(request(&other_responder, old_generation, 7)),
        ),
        (
            "older close for another responder",
            false,
            close_fanout(close(&other_responder, old_generation, 8)),
        ),
        (
            "CloseAck singleton",
            false,
            certified_sidecar_control_fanout(
                scope,
                &ack_target,
                certified_sidecar_close_ack(&service.local_peer, &ack_target, 9),
            ),
        ),
        (
            "GenerationHint singleton",
            false,
            certified_sidecar_control_fanout(
                scope,
                &requester,
                CertifiedMergeSidecarMessage::GenerationHint(hint.clone()),
            ),
        ),
        ("Chunk singleton", false, chunk_fanout),
    ];
    let expected = cases
        .iter()
        .map(|(label, cancel, fanout)| {
            (
                *label,
                *cancel,
                fanout
                    .message_hashes
                    .first()
                    .copied()
                    .expect("singleton fanout has one message hash"),
            )
        })
        .collect::<Vec<_>>();
    let frozen_targets = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut pending = PendingExactOutput::new(32, 1, 1, &frozen_targets)
        .expect("all exact boundary cases fit the bounded output corridor");
    for (label, _, fanout) in cases {
        assert_eq!(fanout.messages.len(), 1, "{label} must stay singleton");
        assert_eq!(
            pending.enqueue(fanout),
            Ok(ExactFanoutOwnership::Owned),
            "{label} must be ranked before cancellation"
        );
    }
    assert_eq!(
        pending.cancel_obsolete_certified_merge_sidecar_generation_hints(&[hint]),
        Ok(2)
    );
    let retained_hashes = pending
        .fanouts
        .iter()
        .flat_map(|fanout| fanout.message_hashes.iter().copied())
        .collect::<Vec<_>>();
    for (label, cancelled, message_hash) in expected {
        assert_eq!(
            retained_hashes.contains(&message_hash),
            !cancelled,
            "generation-fence boundary mismatch for {label}"
        );
    }
    assert_eq!(pending.fanouts.len(), 9);
}
#[test]
fn unrelated_parked_reply_does_not_suppress_responsive_target_control() {
    let (service, _) = fixture();
    let peer_a = service.context.roster[1].validator.clone();
    let peer_b = service.context.roster[2].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 1);
    let route_a = routes.mint_via(peer_a.clone(), hub_a);
    let reply_routes =
        NetworkReplyRoutes::try_from_route(route_a.clone()).expect("one live reply route");
    let parked_candidate = PendingExactFanout::new_with_reply_routes(
        vec![merge_share_message(
            b"unrelated reply parked before peer B control",
        )],
        peer_a.clone(),
        reply_routes,
    )
    .expect("one unrelated exact reply fanout");
    let frozen = vec![peer_a.clone(), peer_b.clone()];
    let mut pending = PendingExactOutput::new(3, 1, 1, &frozen)
        .expect("ordinary and sidecar progress reservations for each frozen target");
    assert_eq!(
        pending.enqueue_owned_reply_transfer(parked_candidate),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert!(routes.retire(&route_a));
    assert_eq!(
        pending.drive_with(|_post, _ticket, _route| {
            panic!("the retired reply route must park before actor admission")
        }),
        Ok(None)
    );
    assert_eq!(pending.fanouts.len(), 1);
    assert!(pending.fanouts[0].targets[0].parked);
    let control_b = certified_sidecar_control_fanout(
        service.exact_output_scope(),
        &peer_b,
        certified_sidecar_generation_hint(&service.local_peer, &peer_b, 201),
    );
    assert_eq!(
        pending.can_enqueue(&control_b),
        Ok(true),
        "peer B keeps its independent frozen target reservation"
    );
    assert_eq!(pending.enqueue(control_b), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(
        pending.fanouts.len(),
        2,
        "peer A's parked reply cannot impersonate peer B's control owner"
    );
    let mut admitted = Vec::new();
    assert_eq!(
        pending.drive_with(|post, ticket, route| {
            assert!(ticket.is_none());
            assert!(matches!(route, ExactTargetRoute::Reply(_)));
            admitted.push(post.peer_id);
            Ok(())
        }),
        Ok(None)
    );
    assert_eq!(admitted, vec![peer_b]);
    assert_eq!(
        pending.fanouts.len(),
        1,
        "only the unrelated parked reply remains"
    );
    assert!(pending.fanouts[0].targets[0].parked);
}
#[test]
fn generation_hint_uses_exact_reply_ownership_without_topology_fallback() {
    let (service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let control = certified_sidecar_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 251),
    );
    assert_eq!(
        control.certified_sidecar_topology_progress_target(),
        None,
        "GenerationHint must not consume topology-progress ownership"
    );
    assert!(
        matches!(
            control.targets.as_slice(),
            [PendingExactTarget {
                route: ExactTargetRoute::Reply(_),
                ..
            }]
        ),
        "GenerationHint must retain an exact reply route"
    );
    assert!(control.is_retryable_certified_sidecar_responder_control_fanout());
    let mut pending = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&target))
        .expect("one exact generation Hint fits its responder-control reservation");
    assert_eq!(pending.can_enqueue(&control), Ok(true));
    assert_eq!(pending.enqueue(control), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 0);
    assert_eq!(
        pending
            .reservation_owner_counts
            .get(&ExactTargetReservation {
                semantic_target: target.clone(),
                class: ExactOutputClass::Lane,
                kind: ExactTargetReservationKind::SidecarReplyControl,
            }),
        Some(&1)
    );
    assert!(
        !pending
            .reservation_owner_counts
            .contains_key(&ExactTargetReservation {
                semantic_target: target.clone(),
                class: ExactOutputClass::Lane,
                kind: ExactTargetReservationKind::SidecarTopologyProgress,
            })
    );
    let blocker = PeerId::new(KeyPair::random().public_key().clone());
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 1);
    let parked_route = routes.mint_via(target.clone(), hub);
    let parked_reply = PendingExactFanout::new_with_reply_routes(
        vec![merge_share_message(
            b"same-target reliable reply before exact GenerationHint",
        )],
        target.clone(),
        NetworkReplyRoutes::try_from_route(parked_route.clone())
            .expect("one live same-target reply route"),
    )
    .expect("one same-target reliable reply");
    let mut saturated = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&target))
        .expect("one shared unit plus frozen responder-control reservation");
    assert_eq!(
        saturated.enqueue_owned_reply_transfer(parked_reply),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert!(routes.retire(&parked_route));
    assert_eq!(
        saturated.drive_with(|_post, _ticket, _route| {
            panic!("the retired same-target reliable reply must park")
        }),
        Ok(None)
    );
    assert_eq!(
        saturated.enqueue(
            PendingExactFanout::new(
                vec![merge_share_message(
                    b"non-frozen blocker saturates shared ownership before Hint",
                )],
                vec![blocker],
            )
            .expect("one non-frozen blocking fanout"),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert_eq!(saturated.shared_ownership_units, 1);
    let hint = certified_sidecar_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 252),
    );
    assert_eq!(
        saturated.can_enqueue(&hint),
        Ok(true),
        "parked reliable output and a full shared pool cannot starve the exact Hint"
    );
    assert_eq!(saturated.enqueue(hint), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(saturated.ownership_units, 3);
    assert_eq!(saturated.shared_ownership_units, 1);
    assert_eq!(
        saturated
            .reservation_owner_counts
            .get(&ExactTargetReservation {
                semantic_target: target,
                class: ExactOutputClass::Lane,
                kind: ExactTargetReservationKind::SidecarReplyControl,
            }),
        Some(&1)
    );
}
#[test]
fn stranded_responder_control_is_replaced_by_a_writable_authenticated_trigger() {
    let (service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let scope = service.exact_output_scope();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let hub_c = PeerId::new(KeyPair::random().public_key().clone());
    let hub_d = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 4);
    let mut pending = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&target))
        .expect("one frozen responder-control slot plus one shared unit");
    let unwritable_route = routes.mint_via(target.clone(), hub_a);
    let unwritable = certified_sidecar_reply_control_fanout(
        scope,
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 281),
        NetworkReplyRoutes::try_from_route(unwritable_route.clone())
            .expect("one exact old responder-control route"),
    );
    assert_eq!(pending.enqueue(unwritable), Ok(ExactFanoutOwnership::Owned));
    let old_fifo_id = pending.fanouts[0]
        .fifo_id
        .expect("the old responder control owns FIFO age");
    let old_source =
        ExactTargetRoute::Reply(unwritable_route.clone()).source(&target, ExactOutputClass::Lane);
    let writable_route = routes.mint_via(target.clone(), hub_b);
    let older_same_source = PendingExactFanout::new_with_reply_routes(
        vec![merge_share_message(
            b"older exact reply sharing the replacement source",
        )],
        target.clone(),
        NetworkReplyRoutes::try_from_route(writable_route.clone())
            .expect("one older exact reply source"),
    )
    .expect("one older same-source exact reply");
    let older_message_hash = older_same_source.message_hashes[0];
    assert_eq!(
        pending.enqueue_owned_reply_transfer(older_same_source),
        Ok(ExactFanoutOwnership::Owned)
    );
    let older_fifo_id = pending.fanouts[1]
        .fifo_id
        .expect("the older same-source reply owns FIFO age");
    let replacement_source =
        ExactTargetRoute::Reply(writable_route.clone()).source(&target, ExactOutputClass::Lane);
    assert_eq!(
        pending.source_fifo_owners.get(&replacement_source),
        Some(&BTreeSet::from([older_fifo_id]))
    );
    assert!(
        routes.mark_reply_unwritable_while_delivery_active(&unwritable_route),
        "the old source enters its monotonic draining interval"
    );
    assert!(!pending.fanouts[0].targets[0].parked);
    let writable = certified_sidecar_reply_control_fanout(
        scope,
        &target,
        certified_sidecar_close_ack(&service.local_peer, &target, 282),
        NetworkReplyRoutes::try_from_route(writable_route.clone())
            .expect("one exact replacement responder-control route"),
    );
    let replacement_message_hash = writable.message_hashes[0];
    assert_eq!(
        pending.can_enqueue(&writable),
        Ok(true),
        "writability is checked before the worker has a chance to park the old route"
    );
    pending.next_fanout_fifo_id = ExactFanoutFifoId::MAX;
    assert_eq!(pending.enqueue(writable), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(pending.fanouts.len(), 2);
    let replacement = pending
        .fanouts
        .back()
        .expect("the replacement rejoins at the worker tail");
    let replacement_fifo_id = replacement
        .fifo_id
        .expect("the replacement owns fresh FIFO age");
    assert_ne!(
        replacement_fifo_id, old_fifo_id,
        "a different authenticated source must not inherit old source age"
    );
    assert!(matches!(
        replacement.messages.as_slice(),
        [NetworkMessage::CertifiedMergeSidecar(message)]
            if matches!(message.as_ref(), CertifiedMergeSidecarMessage::CloseAck(_))
    ));
    assert!(matches!(
        replacement.targets.as_slice(),
        [PendingExactTarget {
            route: ExactTargetRoute::Reply(route),
            parked: false,
            ..
        }] if route.same_delivery(&writable_route)
    ));
    assert_eq!(pending.ownership_units, 2);
    assert_eq!(pending.shared_ownership_units, 0);
    assert!(!pending.source_fifo_owners.contains_key(&old_source));
    assert_eq!(
        pending.source_fifo_owners.get(&replacement_source),
        Some(&BTreeSet::from([older_fifo_id, replacement_fifo_id])),
        "forced rebase and replacement must keep older exact-source work first"
    );
    pending.next_fanout_fifo_id = ExactFanoutFifoId::MAX;
    pending
        .rebase_source_fifo()
        .expect("a later FIFO exhaustion preserves replacement age");
    let older_rebased_fifo_id = pending.fanouts[0]
        .fifo_id
        .expect("the older reply keeps a rebased FIFO identity");
    let replacement_rebased_fifo_id = pending.fanouts[1]
        .fifo_id
        .expect("the replacement keeps a rebased FIFO identity");
    assert_eq!(
        pending.source_fifo_owners.get(&replacement_source),
        Some(&BTreeSet::from([
            older_rebased_fifo_id,
            replacement_rebased_fifo_id,
        ])),
        "later rebasing must preserve the tail-rejoined replacement's age"
    );
    let mut admitted_hashes = Vec::new();
    assert_eq!(
        pending.drive_with(|post, ticket, route| {
            assert!(ticket.is_none());
            assert_eq!(post.peer_id, target);
            assert!(matches!(
                route,
                ExactTargetRoute::Reply(route)
                    if route.same_delivery(&writable_route)
            ));
            admitted_hashes.push(HashOf::new(&post.data));
            Ok(())
        }),
        Ok(None)
    );
    assert_eq!(
        admitted_hashes,
        vec![older_message_hash, replacement_message_hash],
        "fresh replacement work must not inherit the retired occurrence's queue age"
    );
    let retired_route = routes.mint_via(target.clone(), hub_c);
    let retired = certified_sidecar_reply_control_fanout(
        scope,
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 283),
        NetworkReplyRoutes::try_from_route(retired_route.clone())
            .expect("one exact route which will retire"),
    );
    assert_eq!(pending.enqueue(retired), Ok(ExactFanoutOwnership::Owned));
    assert!(routes.retire(&retired_route));
    assert_eq!(
        pending.drive_with(|_post, _ticket, _route| {
            panic!("a retired responder-control route must park before admission")
        }),
        Ok(None)
    );
    assert!(pending.fanouts[0].targets[0].parked);
    let live_after_park = routes.mint_via(target.clone(), hub_d);
    let replacement = certified_sidecar_reply_control_fanout(
        scope,
        &target,
        certified_sidecar_close_ack(&service.local_peer, &target, 284),
        NetworkReplyRoutes::try_from_route(live_after_park.clone())
            .expect("one exact live route after parking"),
    );
    assert_eq!(
        pending.enqueue(replacement),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert_eq!(pending.fanouts.len(), 1);
    assert!(matches!(
        pending.fanouts[0].targets.as_slice(),
        [PendingExactTarget {
            route: ExactTargetRoute::Reply(route),
            parked: false,
            ..
        }] if route.same_delivery(&live_after_park)
    ));
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 0);
}
#[test]
fn responder_control_replacement_cancels_ticket_after_index_commit() {
    let (service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let old_route = routes.mint_via(target.clone(), hub_a);
    let old = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 285),
        NetworkReplyRoutes::try_from_route(old_route.clone())
            .expect("one old responder-control route"),
    );
    let mut pending = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&target))
        .expect("one dedicated responder-control slot");
    assert_eq!(pending.enqueue(old), Ok(ExactFanoutOwnership::Owned));
    let old_post = Post {
        data: pending.fanouts[0].messages[0].clone(),
        peer_id: target.clone(),
        priority: Priority::High,
    };
    let (ticket_fixture, ticket) =
        NetworkActorAdmissionTicketTestFixture::for_reply(&old_post, &old_route);
    assert_eq!(ticket.rank(), Some(1));
    assert_eq!(ticket_fixture.waiter_count(), 1);
    assert_eq!(ticket_fixture.ticket_drop_cancellations(), 0);
    pending.fanouts[0].targets[0].current = Some(old_post);
    pending.fanouts[0].targets[0].ticket = Some(ticket);
    assert!(routes.mark_reply_unwritable_while_delivery_active(&old_route));
    let replacement_route = routes.mint_via(target.clone(), hub_b);
    let replacement = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_close_ack(&service.local_peer, &target, 286),
        NetworkReplyRoutes::try_from_route(replacement_route.clone())
            .expect("one replacement responder-control route"),
    );
    let replacement_hash = replacement.message_hashes[0];
    let replacement_source =
        ExactTargetRoute::Reply(replacement_route.clone()).source(&target, ExactOutputClass::Lane);
    let retired = pending
        .commit_stranded_responder_control_replacement(replacement)
        .expect("replacement planning must remain valid")
        .expect("the unwritable responder control must be replaced");
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.next_fanout_index, 0);
    assert_eq!(pending.fanouts[0].message_hashes, vec![replacement_hash]);
    let replacement_fifo_id = pending.fanouts[0]
        .fifo_id
        .expect("replacement owns a committed FIFO identity");
    assert_eq!(
        pending.next_fanout_fifo_id,
        replacement_fifo_id
            .checked_add(1)
            .expect("bounded replacement FIFO identity must advance")
    );
    assert_eq!(
        pending.source_fifo_owners,
        BTreeMap::from([(replacement_source, BTreeSet::from([replacement_fifo_id]))])
    );
    assert_eq!(pending.target_is_global_head(0, 0), Ok(true));
    assert_eq!(
        pending.reservation_owner_counts,
        BTreeMap::from([(
            ExactTargetReservation {
                semantic_target: target,
                class: ExactOutputClass::Lane,
                kind: ExactTargetReservationKind::SidecarReplyControl,
            },
            1,
        )])
    );
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 0);
    assert!(pending.fanouts[0].targets[0].ticket.is_none());
    assert_eq!(
        retired.targets[0]
            .ticket
            .as_ref()
            .and_then(NetworkActorAdmissionTicket::rank),
        Some(1),
        "the retired external owner remains live after every worker index commits"
    );
    assert_eq!(ticket_fixture.waiter_count(), 1);
    assert_eq!(
        ticket_fixture.ticket_drop_cancellations(),
        0,
        "the commit phase cannot trigger actor cancellation"
    );
    drop(retired);
    assert_eq!(ticket_fixture.waiter_count(), 0);
    assert_eq!(
        ticket_fixture.ticket_drop_cancellations(),
        1,
        "terminal destruction cancels the exact actor waiter once"
    );
    drop(pending);
    assert_eq!(
        ticket_fixture.ticket_drop_cancellations(),
        1,
        "the replacement owns no alias of the retired actor ticket"
    );
}
#[test]
fn multi_route_stranded_responder_control_uses_one_dedicated_reservation() {
    let (service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let blocker = PeerId::new(KeyPair::random().public_key().clone());
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let hub_c = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let old_route = routes.mint_via(target.clone(), hub_a);
    let old = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_close_ack(&service.local_peer, &target, 285),
        NetworkReplyRoutes::try_from_route(old_route.clone())
            .expect("one old responder-control route"),
    );
    let mut pending = PendingExactOutput::new(1, 1, 2, std::slice::from_ref(&target))
        .expect("one shared unit and two candidate reply sources");
    assert_eq!(pending.enqueue(old), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(
        pending.enqueue(
            PendingExactFanout::new(
                vec![merge_share_message(
                    b"shared-unit blocker for fail-atomic responder replacement",
                )],
                vec![blocker],
            )
            .expect("one non-frozen blocker"),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    let old_post = Post {
        data: pending.fanouts[0].messages[0].clone(),
        peer_id: target.clone(),
        priority: Priority::High,
    };
    pending.fanouts[0].targets[0].current = Some(old_post);
    assert!(
        routes.mark_reply_unwritable_while_delivery_active(&old_route),
        "the actor-returned old source becomes unwritable"
    );
    let route_b = routes.mint_via(target.clone(), hub_b);
    let route_c = routes.mint_via(target.clone(), hub_c);
    let mut candidate_routes = NetworkReplyRoutes::try_from_route(route_b)
        .expect("the replacement starts with one source");
    candidate_routes
        .merge(
            &NetworkReplyRoutes::try_from_route(route_c)
                .expect("the replacement has a second source"),
        )
        .expect("two live replacement sources fit their route bound");
    let candidate = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 286),
        candidate_routes,
    );
    assert_eq!(
        pending.can_enqueue(&candidate),
        Ok(true),
        "alternate authenticated routes must not borrow shared capacity"
    );
    assert_eq!(pending.enqueue(candidate), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(pending.fanouts.len(), 2);
    let replacement = pending
        .fanouts
        .back()
        .expect("the multi-route replacement rejoins at the tail");
    assert_eq!(replacement.targets.len(), 2);
    assert_eq!(
        pending
            .reservation_owner_counts
            .get(&ExactTargetReservation {
                semantic_target: target,
                class: ExactOutputClass::Lane,
                kind: ExactTargetReservationKind::SidecarReplyControl,
            }),
        Some(&1),
        "all exact routes share the target's one responder-control credit"
    );
    assert_eq!(pending.ownership_units, 2);
    assert_eq!(
        pending.shared_ownership_units, 1,
        "only the unrelated non-frozen blocker consumes shared capacity"
    );
}
#[test]
fn stranded_responder_control_fifo_collision_is_fail_atomic() {
    let (service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let blocker = PeerId::new(KeyPair::random().public_key().clone());
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 1);
    let old_route = routes.mint_via(target.clone(), hub_a);
    let old = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_close_ack(&service.local_peer, &target, 287),
        NetworkReplyRoutes::try_from_route(old_route.clone())
            .expect("one old responder-control route"),
    );
    let mut pending = PendingExactOutput::new(2, 1, 1, std::slice::from_ref(&target))
        .expect("one control plus one independent blocker");
    assert_eq!(pending.enqueue(old), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(
        pending.enqueue(
            PendingExactFanout::new(
                vec![merge_share_message(
                    b"live FIFO owner colliding with the replacement sequence",
                )],
                vec![blocker],
            )
            .expect("one independent FIFO owner"),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    let old_post = Post {
        data: pending.fanouts[0].messages[0].clone(),
        peer_id: target.clone(),
        priority: Priority::High,
    };
    pending.fanouts[0].targets[0].current = Some(old_post);
    assert!(routes.mark_reply_unwritable_while_delivery_active(&old_route));
    let replacement_route = routes.mint_via(target.clone(), hub_b);
    let candidate = certified_sidecar_reply_control_fanout(
        service.exact_output_scope(),
        &target,
        certified_sidecar_generation_hint(&service.local_peer, &target, 288),
        NetworkReplyRoutes::try_from_route(replacement_route).expect("one live replacement route"),
    );
    let colliding_fifo_id = pending.fanouts[1]
        .fifo_id
        .expect("the independent blocker owns one FIFO identity");
    pending.next_fanout_fifo_id = colliding_fifo_id;
    let fanout_fifo_before = pending
        .fanouts
        .iter()
        .map(|fanout| fanout.fifo_id)
        .collect::<Vec<_>>();
    let next_fanout_index_before = pending.next_fanout_index;
    let source_owners_before = pending.source_fifo_owners.clone();
    let reservations_before = pending.reservation_owner_counts.clone();
    let ownership_units_before = pending.ownership_units;
    let shared_units_before = pending.shared_ownership_units;
    let retained_hash_before = pending.fanouts[0].message_hashes.clone();
    assert_eq!(pending.can_enqueue(&candidate), Ok(true));
    assert_eq!(
        pending.enqueue(candidate),
        Err("Sumeragi v2 responder-control replacement reused a live FIFO identity".to_owned())
    );
    assert_eq!(pending.fanouts.len(), 2);
    assert_eq!(
        pending
            .fanouts
            .iter()
            .map(|fanout| fanout.fifo_id)
            .collect::<Vec<_>>(),
        fanout_fifo_before
    );
    assert_eq!(pending.next_fanout_fifo_id, colliding_fifo_id);
    assert_eq!(pending.next_fanout_index, next_fanout_index_before);
    assert_eq!(pending.source_fifo_owners, source_owners_before);
    assert_eq!(pending.reservation_owner_counts, reservations_before);
    assert_eq!(pending.ownership_units, ownership_units_before);
    assert_eq!(pending.shared_ownership_units, shared_units_before);
    assert_eq!(pending.fanouts[0].message_hashes, retained_hash_before);
    assert!(
        pending.fanouts[0].targets[0].current.is_some(),
        "failed planning preserves the actor-returned old attempt"
    );
}
#[test]
fn parked_same_target_reply_and_full_shared_pool_do_not_block_request_or_close() {
    let (service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let scope = service.exact_output_scope();
    let cases = [
        (
            "request",
            certified_sidecar_request_fanout(scope, &service.local_peer, &target),
            certified_sidecar_request_fanout(scope, &service.local_peer, &target),
        ),
        (
            "close",
            certified_sidecar_control_fanout(
                scope,
                &target,
                certified_sidecar_close(&service.local_peer, &target, 271),
            ),
            certified_sidecar_control_fanout(
                scope,
                &target,
                certified_sidecar_close(&service.local_peer, &target, 272),
            ),
        ),
    ];
    for (kind, progress, later_progress) in cases {
        assert_eq!(
            progress.certified_sidecar_topology_progress_target(),
            Some(&target)
        );
        assert!(
            !progress.is_retryable_certified_sidecar_responder_control_fanout(),
            "{kind} must remain non-droppable"
        );
        let blocker = PeerId::new(KeyPair::random().public_key().clone());
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 1);
        let parked_route = routes.mint_via(target.clone(), hub);
        let parked_reply = PendingExactFanout::new_with_reply_routes(
            vec![merge_share_message(
                b"same target parked reply before non-droppable sidecar progress",
            )],
            target.clone(),
            NetworkReplyRoutes::try_from_route(parked_route.clone())
                .expect("one live same-target reply route"),
        )
        .expect("one same-target reply fanout");
        let mut pending = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&target))
            .expect("one shared unit plus frozen topology-progress reservation");
        assert_eq!(
            pending.enqueue_owned_reply_transfer(parked_reply),
            Ok(ExactFanoutOwnership::Owned)
        );
        assert!(routes.retire(&parked_route));
        assert_eq!(
            pending.drive_with(|_post, _ticket, _route| {
                panic!("the retired same-target reply must park before actor admission")
            }),
            Ok(None)
        );
        assert!(pending.fanouts[0].targets[0].parked);
        assert_eq!(
            pending.enqueue(
                PendingExactFanout::new(
                    vec![merge_share_message(
                        b"non-frozen blocker saturates shared ownership",
                    )],
                    vec![blocker.clone()],
                )
                .expect("one non-frozen blocking fanout"),
            ),
            Ok(ExactFanoutOwnership::Owned)
        );
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending.can_enqueue(&progress),
            Ok(true),
            "{kind} keeps the frozen target's independent topology-progress credit"
        );
        assert_eq!(pending.enqueue(progress), Ok(ExactFanoutOwnership::Owned));
        assert_eq!(pending.ownership_units, 3);
        assert_eq!(pending.shared_ownership_units, 1);
        let progress_reservation = ExactTargetReservation {
            semantic_target: target.clone(),
            class: ExactOutputClass::Lane,
            kind: ExactTargetReservationKind::SidecarTopologyProgress,
        };
        assert_eq!(
            pending.reservation_owner_counts.get(&progress_reservation),
            Some(&1)
        );
        assert_eq!(
            pending.can_enqueue(&later_progress),
            Ok(false),
            "a second non-droppable {kind} must not bypass the saturated shared bound"
        );
        assert_eq!(
            pending.enqueue(later_progress),
            Ok(ExactFanoutOwnership::SourceRetained),
            "a second non-droppable {kind} remains owned by lane work"
        );
        assert_eq!(pending.fanouts.len(), 3);
        assert_eq!(
            pending.reservation_owner_counts.get(&progress_reservation),
            Some(&1)
        );
        let mut progress_admitted = false;
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                assert!(ticket.is_none());
                assert!(matches!(route, ExactTargetRoute::Topology));
                if post.peer_id == blocker {
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 37,
                    });
                }
                assert_eq!(post.peer_id, target);
                let NetworkMessage::CertifiedMergeSidecar(message) = &post.data else {
                    panic!("sidecar progress changed message kind")
                };
                match (kind, message.as_ref()) {
                    ("request", CertifiedMergeSidecarMessage::Request(_))
                    | ("close", CertifiedMergeSidecarMessage::Close(_)) => {}
                    other => panic!("sidecar progress changed variant: {other:?}"),
                }
                progress_admitted = true;
                Ok(())
            }),
            Ok(Some(37)),
            "shared-pool backpressure must not suppress the responsive target's {kind}"
        );
        assert!(progress_admitted);
        assert_eq!(pending.fanouts.len(), 2);
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 1);
        assert!(
            !pending
                .reservation_owner_counts
                .contains_key(&progress_reservation)
        );
        let retried_progress = match kind {
            "request" => certified_sidecar_request_fanout(scope, &service.local_peer, &target),
            "close" => certified_sidecar_control_fanout(
                scope,
                &target,
                certified_sidecar_close(&service.local_peer, &target, 273),
            ),
            _ => unreachable!("the bounded topology-progress cases are exhaustive"),
        };
        assert_eq!(
            pending.can_enqueue(&retried_progress),
            Ok(true),
            "the retained {kind} can retry as soon as the topology-progress credit drains"
        );
        assert_eq!(
            pending.enqueue(retried_progress),
            Ok(ExactFanoutOwnership::Owned)
        );
        assert_eq!(pending.ownership_units, 3);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending.reservation_owner_counts.get(&progress_reservation),
            Some(&1)
        );
    }
}
#[test]
fn non_frozen_responder_controls_remain_strictly_shared_bounded() {
    let (service, _) = fixture();
    let first = PeerId::new(KeyPair::random().public_key().clone());
    let second = PeerId::new(KeyPair::random().public_key().clone());
    let scope = service.exact_output_scope();
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one shared ownership unit");
    let first_control = certified_sidecar_control_fanout(
        scope,
        &first,
        certified_sidecar_generation_hint(&service.local_peer, &first, 261),
    );
    assert_eq!(
        pending.enqueue(first_control),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 1);
    let second_control = certified_sidecar_control_fanout(
        scope,
        &second,
        certified_sidecar_close_ack(&service.local_peer, &second, 262),
    );
    assert_eq!(pending.can_enqueue(&second_control), Ok(false));
    assert_eq!(
        pending.enqueue(second_control),
        Ok(ExactFanoutOwnership::SourceRetained),
        "arbitrary non-frozen targets receive no unbounded control side channel"
    );
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 1);
}
#[test]
fn retryable_sidecar_controls_are_bounded_and_fair_per_semantic_target() {
    let (service, _) = fixture();
    let peer_a = service.context.roster[1].validator.clone();
    let peer_b = service.context.roster[2].validator.clone();
    let scope = service.exact_output_scope();
    let frozen = vec![peer_a.clone(), peer_b.clone()];
    let mut pending = PendingExactOutput::new(3, 1, 1, &frozen)
        .expect("reliable and responder-control reservations for each frozen target");
    for (peer, message) in [
        (
            peer_a.clone(),
            certified_sidecar_generation_hint(&service.local_peer, &peer_a, 301),
        ),
        (
            peer_b.clone(),
            certified_sidecar_close_ack(&service.local_peer, &peer_b, 301),
        ),
    ] {
        let fanout = certified_sidecar_control_fanout(scope, &peer, message);
        assert_eq!(pending.can_enqueue(&fanout), Ok(true));
        assert_eq!(pending.enqueue(fanout), Ok(ExactFanoutOwnership::Owned));
    }
    assert_eq!(pending.fanouts.len(), 2);
    assert_eq!(pending.ownership_units, 2);
    assert_eq!(
        pending.shared_ownership_units, 0,
        "one control for each frozen target consumes only its reply-control reservation"
    );
    for ordinal in 302..334 {
        for (peer, message) in [
            (
                peer_a.clone(),
                certified_sidecar_close_ack(&service.local_peer, &peer_a, ordinal),
            ),
            (
                peer_b.clone(),
                certified_sidecar_generation_hint(&service.local_peer, &peer_b, ordinal),
            ),
        ] {
            let duplicate = certified_sidecar_control_fanout(scope, &peer, message);
            assert_eq!(
                pending.can_enqueue(&duplicate),
                Ok(false),
                "a distinct same-target control remains source-owned"
            );
            assert_eq!(
                pending.enqueue(duplicate),
                Ok(ExactFanoutOwnership::SourceRetained)
            );
        }
        assert_eq!(
            pending.fanouts.len(),
            2,
            "each semantic target retains at most one retryable control"
        );
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 0);
    }
    let retained_targets = pending
        .fanouts
        .iter()
        .filter_map(PendingExactFanout::retryable_certified_sidecar_responder_control_target)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        retained_targets,
        BTreeSet::from([peer_a.clone(), peer_b.clone()])
    );
    let mut peer_b_admitted = false;
    assert_eq!(
        pending.drive_with(|post, ticket, route| {
            assert!(ticket.is_none());
            if post.peer_id == peer_a {
                assert!(matches!(route, ExactTargetRoute::Reply(_)));
                return Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 29,
                });
            }
            assert_eq!(post.peer_id, peer_b);
            assert!(matches!(route, ExactTargetRoute::Reply(_)));
            peer_b_admitted = true;
            Ok(())
        }),
        Ok(Some(29)),
        "peer A backpressure must not suppress peer B's independent control"
    );
    assert!(peer_b_admitted);
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(
        pending.fanouts[0].retryable_certified_sidecar_responder_control_target(),
        Some(&peer_a)
    );
    assert_eq!(
        pending.drive_with(|post, ticket, route| {
            assert!(ticket.is_none());
            assert!(matches!(route, ExactTargetRoute::Reply(_)));
            assert_eq!(post.peer_id, peer_a);
            Ok(())
        }),
        Ok(None)
    );
    assert!(pending.fanouts.is_empty());
    assert_eq!(pending.ownership_units, 0);
    assert_eq!(pending.shared_ownership_units, 0);
}
#[test]
fn reliable_flush_projection_preserves_exact_stream_epoch() {
    let (service, _) = fixture();
    let requester = service.context.roster[1].validator.clone();
    let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &requester);
    let CertifiedMergeSidecarMessage::Chunk(mut chunk) = chunk_message else {
        unreachable!("sidecar fixture returns one response chunk")
    };
    let stream_epoch = CertifiedMergeSidecarStreamEpochV1(
        NonZeroU64::new(73).expect("non-zero adversarial stream epoch"),
    );
    let service_generation = crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1(
        NonZeroU64::new(29).expect("non-zero adversarial service generation"),
    );
    chunk.service_generation = service_generation;
    chunk.stream_epoch = stream_epoch;
    chunk.semantic_sequence = CertifiedMergeSidecarSemanticSequenceV1(
        NonZeroU64::new(11).expect("adversarial semantic sequence is non-zero"),
    );
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let route = routes.mint(requester);
    let (_control, _ack, admission) = certified_sidecar_flush_fixture(&chunk, &route);
    let projection = reliable_flush_trace_projection(
        &admission,
        NetworkReplyFlushAckStatus::Flushed,
        1,
        0,
        0,
        1,
        2,
    )
    .expect("project an exact successful sidecar flush");
    assert_eq!(projection.service_generation, service_generation.get());
    assert_eq!(projection.stream_epoch, stream_epoch.get());
    assert_eq!(projection.semantic_sequence, 11);
    assert!(
        production_reliable_flush_trace_refines_outbound_ownership_kernel(projection),
        "the live worker projection must retain the non-zero stream incarnation"
    );
    let timed_out = reliable_flush_trace_projection(
        &admission,
        NetworkReplyFlushAckStatus::TimedOut,
        1,
        0,
        0,
        0,
        2,
    )
    .expect("project a non-advancing sidecar writer timeout");
    assert_eq!(timed_out.status, 3);
    assert_eq!(
        timed_out.message_cursor_after, timed_out.message_cursor_before,
        "TimedOut must reuse the existing non-advancing ownership transition"
    );
    assert!(
        production_reliable_flush_trace_refines_outbound_ownership_kernel(timed_out),
        "TimedOut must refine the same no-progress kernel edge as Closed"
    );
    let mut absent_epoch = projection;
    absent_epoch.stream_epoch = 0;
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(absent_epoch),
        "the refinement kernel must reject a projection that erases stream incarnation"
    );
    let mut absent_generation = projection;
    absent_generation.service_generation = 0;
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(absent_generation),
        "the refinement kernel must reject an erased responder generation"
    );
    let mut absent_sequence = projection;
    absent_sequence.semantic_sequence = 0;
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(absent_sequence),
        "the refinement kernel must reject an erased semantic occurrence"
    );
}
#[test]
fn certified_sidecar_transfer_identity_binds_stream_epoch() {
    let (service, _) = fixture();
    let responder = service.context.roster[1].validator.clone();
    let (request_message, chunk_message) =
        certified_sidecar_outputs(&service.local_peer, &responder);
    let CertifiedMergeSidecarMessage::Request(request) = request_message else {
        unreachable!("sidecar fixture returns one request")
    };
    let CertifiedMergeSidecarMessage::Chunk(chunk) = chunk_message else {
        unreachable!("sidecar fixture returns one response chunk")
    };
    let successor_epoch = CertifiedMergeSidecarStreamEpochV1(
        NonZeroU64::new(
            request
                .stream_epoch
                .get()
                .checked_add(1)
                .expect("fixture stream epoch has a successor"),
        )
        .expect("successor stream epoch is non-zero"),
    );
    let mut successor_request = request.clone();
    successor_request.stream_epoch = successor_epoch;
    let mut successor_chunk = chunk.clone();
    successor_chunk.stream_epoch = successor_epoch;
    assert_ne!(
        CertifiedSidecarTransferIdentity::from_request(&request),
        CertifiedSidecarTransferIdentity::from_request(&successor_request)
    );
    assert_ne!(
        CertifiedSidecarTransferIdentity::from_chunk(&chunk),
        CertifiedSidecarTransferIdentity::from_chunk(&successor_chunk)
    );
    let mut successor_generation_request = request.clone();
    successor_generation_request.service_generation =
        crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(
                request
                    .service_generation
                    .get()
                    .checked_add(1)
                    .expect("fixture service generation has a successor"),
            )
            .expect("successor service generation is non-zero"),
        );
    let mut successor_generation_chunk = chunk.clone();
    successor_generation_chunk.service_generation = successor_generation_request.service_generation;
    assert_ne!(
        CertifiedSidecarTransferIdentity::from_request(&request),
        CertifiedSidecarTransferIdentity::from_request(&successor_generation_request)
    );
    assert_ne!(
        CertifiedSidecarTransferIdentity::from_chunk(&chunk),
        CertifiedSidecarTransferIdentity::from_chunk(&successor_generation_chunk)
    );
}
#[test]
fn certified_sidecar_close_cancels_only_the_exact_stream_epoch() {
    let (service, _) = fixture();
    let requester = service.context.roster[1].validator.clone();
    let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &requester);
    let CertifiedMergeSidecarMessage::Chunk(old_chunk) = chunk_message else {
        unreachable!("sidecar fixture returns one response chunk")
    };
    let old_epoch = old_chunk.stream_epoch;
    let mut successor_chunk = old_chunk.clone();
    successor_chunk.stream_epoch = CertifiedMergeSidecarStreamEpochV1(
        NonZeroU64::new(
            old_epoch
                .get()
                .checked_add(1)
                .expect("fixture stream epoch has a successor"),
        )
        .expect("successor stream epoch is non-zero"),
    );
    let sidecar_fanout = |chunk: &CertifiedMergeSidecarChunkV1| {
        let rollover_claim = ExactOutputRolloverClaim::CertifiedSidecarChunk {
            scope: service.exact_output_scope(),
            target: requester.clone(),
            transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
            chunk_index: chunk.chunk_index,
            chunk_count: chunk.chunk_count,
            response_hash: HashOf::new(chunk),
        };
        PendingExactFanout::claimed(
            vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(
                CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
            ))],
            vec![requester.clone()],
            rollover_claim,
        )
        .expect("valid epoch-bound sidecar rollover claim")
        .expect("one exact epoch-bound sidecar fanout")
    };
    let mut pending =
        PendingExactOutput::new(4, 1, 1, &[]).expect("two fanouts and admissions fit");
    for chunk in [&old_chunk, &successor_chunk] {
        assert_eq!(
            pending.enqueue(sidecar_fanout(chunk)),
            Ok(ExactFanoutOwnership::Owned)
        );
    }
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let route = routes.mint(requester.clone());
    for chunk in [&old_chunk, &successor_chunk] {
        let (_control, _ack, admission) = certified_sidecar_flush_fixture(chunk, &route);
        pending.admitted_sidecar_chunks.push_back(admission);
    }
    assert_eq!(pending.fanouts.len(), 2);
    assert_eq!(pending.admitted_sidecar_chunks.len(), 2);
    assert_eq!(
        pending
            .close_certified_sidecar_prefix(&CertifiedMergeSidecarClosedPrefix {
                requester,
                service_generation: old_chunk.service_generation,
                stream_epoch: old_epoch,
                closed_through: old_chunk.semantic_sequence.get(),
            })
            .expect("close only the terminated stream incarnation"),
        1
    );
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.admitted_sidecar_chunks.len(), 1);
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 1);
    assert_eq!(pending.source_fifo_owners.len(), 1);
    assert!(matches!(
        &pending
            .fanouts
            .front()
            .expect("successor stream fanout remains")
            .rollover_claim,
        ExactOutputRolloverClaim::CertifiedSidecarChunk { transfer, .. }
            if transfer.stream_epoch == successor_chunk.stream_epoch
    ));
    assert_eq!(
        pending
            .admitted_sidecar_chunks
            .front()
            .expect("successor stream admission remains")
            .projection()
            .stream_epoch,
        successor_chunk.stream_epoch
    );
}
#[test]
fn generation_close_dominance_cancels_queued_and_admitted_sidecar_occurrences() {
    let (service, _) = fixture();
    let requester = service.context.roster[1].validator.clone();
    let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &requester);
    let CertifiedMergeSidecarMessage::Chunk(base_chunk) = chunk_message else {
        unreachable!("sidecar fixture returns one response chunk")
    };
    let generation = |value| {
        crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(value).expect("test generation is non-zero"),
        )
    };
    let epoch = |value| {
        CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(value).expect("test stream epoch is non-zero"),
        )
    };
    let occurrence = |service_generation, stream_epoch, semantic_sequence| {
        let mut chunk = base_chunk.clone();
        chunk.service_generation = generation(service_generation);
        chunk.stream_epoch = epoch(stream_epoch);
        chunk.semantic_sequence = CertifiedMergeSidecarSemanticSequenceV1(
            NonZeroU64::new(semantic_sequence)
                .expect("generation-close semantic sequence is non-zero"),
        );
        chunk.request_id = Hash::new_from_chunks(&[
            b"worker generation-close occurrence",
            &service_generation.to_le_bytes(),
            &stream_epoch.to_le_bytes(),
            &semantic_sequence.to_le_bytes(),
        ]);
        chunk
    };
    let occurrences = [
        occurrence(1, 1, 1),
        occurrence(1, 2, 2),
        occurrence(2, 2, 1),
        occurrence(2, 3, 4),
        occurrence(2, 3, 5),
    ];
    let sidecar_fanout = |chunk: &CertifiedMergeSidecarChunkV1| {
        PendingExactFanout::claimed(
            vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(
                CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
            ))],
            vec![requester.clone()],
            ExactOutputRolloverClaim::CertifiedSidecarChunk {
                scope: service.exact_output_scope(),
                target: requester.clone(),
                transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
                chunk_index: chunk.chunk_index,
                chunk_count: chunk.chunk_count,
                response_hash: HashOf::new(chunk),
            },
        )
        .expect("valid generation-bound sidecar rollover claim")
        .expect("one exact generation-bound sidecar fanout")
    };
    let mut pending =
        PendingExactOutput::new(occurrences.len(), 1, 1, &[]).expect("bounded occurrences fit");
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let route = routes.mint(requester.clone());
    for chunk in &occurrences {
        assert_eq!(
            pending.enqueue(sidecar_fanout(chunk)),
            Ok(ExactFanoutOwnership::Owned)
        );
        let (_control, _ack, admission) = certified_sidecar_flush_fixture(chunk, &route);
        pending.admitted_sidecar_chunks.push_back(admission);
    }
    assert_eq!(pending.fanouts.len(), occurrences.len());
    assert_eq!(pending.admitted_sidecar_chunks.len(), occurrences.len());
    assert_eq!(
        pending
            .close_certified_sidecar_prefix(&CertifiedMergeSidecarClosedPrefix {
                requester,
                service_generation: generation(2),
                stream_epoch: epoch(3),
                closed_through: 4,
            })
            .expect("generation-wide close preserves only uncovered occurrences"),
        4
    );
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.admitted_sidecar_chunks.len(), 1);
    assert_eq!(pending.sidecar_control_units(), 1);
    assert_eq!(pending.ownership_units, 1);
    let retained = pending
        .admitted_sidecar_chunks
        .front()
        .expect("the same-generation successor remains")
        .projection();
    assert_eq!(retained.service_generation, generation(2));
    assert_eq!(retained.stream_epoch, epoch(3));
    assert_eq!(retained.semantic_sequence.get(), 5);
}
#[test]
fn actor_backpressure_retains_exact_final_lane_commit_qc_post() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("bounded output corridor");
    pending
        .enqueue(
            PendingExactFanout::new(
                vec![lane_commit_qc_message(peer.clone())],
                vec![peer.clone()],
            )
            .expect("non-empty final QC fanout"),
        )
        .expect("retain final QC fanout");
    assert_eq!(pending.source_fifo_owners.len(), 1);
    assert_eq!(pending.fanouts[0].current_source_targets.len(), 1);
    assert_eq!(
        pending.drive_with(|post, ticket, _route| {
            assert!(ticket.is_none());
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 3,
            })
        }),
        Ok(Some(3))
    );
    let retained = pending
        .fanouts
        .front()
        .and_then(|fanout| fanout.targets[0].current.as_ref())
        .expect("actor-returned final QC post remains owned");
    assert_eq!(retained.peer_id, peer);
    assert_eq!(retained.priority, Priority::High);
    let NetworkMessage::SumeragiBlock(wire) = &retained.data else {
        panic!("retained output must be a lane CommitQC");
    };
    let BlockMessage::LaneBlockQc(qc) = wire.as_message() else {
        panic!("retained Sumeragi output must be a lane CommitQC");
    };
    assert_eq!(qc.body.phase, CertPhase::Commit);
    let mut admitted = Vec::new();
    assert_eq!(
        pending.drive_with(|post, ticket, _route| {
            assert!(ticket.is_none());
            admitted.push(post.peer_id);
            Ok(())
        }),
        Ok(None)
    );
    assert_eq!(admitted, vec![peer]);
    assert!(!pending.is_pending());
    assert!(pending.source_fifo_owners.is_empty());
}
#[test]
fn actor_backpressure_retains_complete_merge_share_fanout() {
    let (service, _) = fixture();
    let peers = service.remote_voters();
    let digest = Hash::new(b"outbound corridor merge share");
    let message = merge_share_message(b"outbound corridor merge share");
    let mut pending = PendingExactOutput::new(1, 1, peers.len(), &peers)
        .expect("bounded merge output corridor with exact frozen targets");
    pending
        .enqueue(
            PendingExactFanout::new(vec![message], peers.clone()).expect("non-empty merge fanout"),
        )
        .expect("retain merge fanout");
    assert_eq!(
        pending.drive_with(|post, ticket, _route| {
            assert!(ticket.is_none());
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 2,
            })
        }),
        Ok(Some(2))
    );
    let retained = pending
        .fanouts
        .front()
        .and_then(|fanout| fanout.targets[0].current.as_ref())
        .expect("actor-returned merge post remains owned");
    assert_eq!(retained.peer_id, peers[0]);
    let NetworkMessage::MergeCommitteeSignature(signature) = &retained.data else {
        panic!("retained output must be the merge share");
    };
    assert_eq!(signature.message_digest, digest);
    let mut admitted = Vec::new();
    assert_eq!(
        pending.drive_with(|post, ticket, _route| {
            assert!(ticket.is_none());
            let NetworkMessage::MergeCommitteeSignature(signature) = &post.data else {
                panic!("every fanout post must retain the merge share");
            };
            assert_eq!(signature.message_digest, digest);
            admitted.push(post.peer_id);
            Ok(())
        }),
        Ok(None)
    );
    assert_eq!(admitted, peers);
    assert!(!pending.is_pending());
}
#[test]
fn same_tenure_updates_and_reconnect_preserve_current_item() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let second_digest = Hash::new(b"source update second");
    let messages = vec![
        merge_share_message(b"source update first"),
        merge_share_message(b"source update second"),
    ];
    let response_class = exact_output_class(&messages[0]).expect("classified response");
    assert!(
        messages
            .iter()
            .all(|message| exact_output_class(message) == Ok(response_class))
    );
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let prior_route = routes.mint(peer.clone());
    let prior_source = ExactTargetRoute::Reply(prior_route.clone()).source(&peer, response_class);
    let mut predecessor = PendingExactFanout::new_with_routes(
        messages.clone(),
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(prior_route.clone())],
    )
    .expect("two-message exact response");
    let returned_second = predecessor.messages[1].clone();
    let target = predecessor
        .targets
        .first_mut()
        .expect("response has one target");
    target.message_index = 1;
    target.current = Some(Post {
        data: returned_second.clone(),
        peer_id: peer.clone(),
        priority: Priority::High,
    });
    predecessor
        .rebuild_current_source_targets()
        .expect("manual predecessor cursor has a valid local FIFO index");
    let mut pending = PendingExactOutput::new(1, 2, 1, &[]).expect("one-response corridor");
    assert_eq!(
        pending.enqueue(predecessor).expect("predecessor fits"),
        ExactFanoutOwnership::Owned
    );
    assert_eq!(pending.ownership_units, 1);
    let predecessor_fifo_id = pending.fanouts[0]
        .fifo_id
        .expect("retained predecessor has a stable FIFO identity");
    assert_eq!(
        pending
            .source_fifo_owners
            .get(&prior_source)
            .and_then(BTreeSet::first),
        Some(&predecessor_fifo_id)
    );
    let same_tenure_retry = PendingExactFanout::new_with_routes(
        messages.clone(),
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(prior_route.clone())],
    )
    .expect("same-tenure exact retry");
    assert_eq!(
        pending
            .enqueue(same_tenure_retry)
            .expect("same-tenure retry coalesces"),
        ExactFanoutOwnership::Owned
    );
    let retained = pending
        .fanouts
        .front()
        .and_then(|fanout| fanout.targets.first())
        .expect("predecessor remains queued");
    assert_eq!(retained.message_index, 1);
    assert_eq!(
        retained
            .current
            .as_ref()
            .map(|post| HashOf::new(&post.data)),
        Some(HashOf::new(&returned_second))
    );
    let later_delivery = routes
        .redeliver(&prior_route)
        .expect("same-tenure later delivery");
    let later_delivery_retry = PendingExactFanout::new_with_routes(
        messages.clone(),
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(later_delivery.clone())],
    )
    .expect("later-delivery exact retry");
    assert_eq!(
        pending
            .enqueue(later_delivery_retry)
            .expect("later delivery updates only its source route"),
        ExactFanoutOwnership::Owned
    );
    let retained = &pending.fanouts[0].targets[0];
    assert_eq!(retained.message_index, 1);
    assert_eq!(
        retained
            .current
            .as_ref()
            .map(|post| HashOf::new(&post.data)),
        Some(HashOf::new(&returned_second))
    );
    assert!(matches!(
        &retained.route,
        ExactTargetRoute::Reply(route) if route.same_delivery(&later_delivery)
    ));
    let stale_retry = PendingExactFanout::new_with_routes(
        messages.clone(),
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(prior_route.clone())],
    )
    .expect("out-of-order same-source retry");
    assert!(
        pending
            .enqueue(stale_retry)
            .expect_err("an older delivery must be rejected atomically")
            .contains("stale capability")
    );
    let retained = &pending.fanouts[0].targets[0];
    assert_eq!(retained.message_index, 1);
    assert_eq!(
        retained
            .current
            .as_ref()
            .map(|post| HashOf::new(&post.data)),
        Some(HashOf::new(&returned_second))
    );
    assert!(matches!(
        &retained.route,
        ExactTargetRoute::Reply(route) if route.same_delivery(&later_delivery)
    ));
    assert!(routes.retire(&prior_route));
    assert_eq!(
        pending.drive_with(|_post, _ticket, _route| {
            panic!("inactive source must park before actor admission")
        }),
        Ok(None)
    );
    let parked = &pending.fanouts[0].targets[0];
    assert!(parked.parked);
    assert_eq!(parked.message_index, 1);
    assert!(parked.current.is_none());
    assert!(parked.ticket.is_none());
    assert_eq!(pending.fanouts[0].fifo_id, Some(predecessor_fifo_id));
    assert_eq!(pending.fanouts[0].message_hashes.len(), 2);
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 1);
    assert_eq!(pending.reservation_owner_counts.values().sum::<usize>(), 1);
    assert_eq!(
        pending.source_fifo_owners.get(&prior_source),
        Some(&BTreeSet::from([predecessor_fifo_id]))
    );
    let reconnected_route = routes.mint(peer.clone());
    let reconnected_source =
        ExactTargetRoute::Reply(reconnected_route.clone()).source(&peer, response_class);
    let reconnect = PendingExactFanout::new_with_routes(
        messages,
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(reconnected_route.clone())],
    )
    .expect("same-source reconnect retry");
    assert_eq!(
        pending
            .enqueue(reconnect)
            .expect("same-source reconnect updates its route"),
        ExactFanoutOwnership::Owned
    );
    let resumed = pending
        .fanouts
        .front()
        .and_then(|fanout| fanout.targets.first())
        .expect("reconnected source remains queued");
    assert_eq!(resumed.message_index, 1);
    assert!(!resumed.parked);
    assert!(resumed.current.is_none());
    assert!(resumed.ticket.is_none());
    assert!(matches!(
        &resumed.route,
        ExactTargetRoute::Reply(route) if route.same_tenure(&reconnected_route)
    ));
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 1);
    assert_eq!(pending.reservation_owner_counts.values().sum::<usize>(), 1);
    assert_eq!(prior_source, reconnected_source);
    assert_eq!(
        pending
            .source_fifo_owners
            .get(&reconnected_source)
            .and_then(BTreeSet::first),
        Some(&predecessor_fifo_id),
        "reconnect must retain the authenticated source's FIFO age"
    );
    let mut admitted = Vec::new();
    let mut admit = |post: Post<NetworkMessage>,
                     ticket: Option<NetworkActorAdmissionTicket>,
                     route: &ExactTargetRoute| {
        assert!(ticket.is_none());
        assert!(matches!(
            route,
            ExactTargetRoute::Reply(route) if route.same_tenure(&reconnected_route)
        ));
        admitted.push(merge_share_digest(&post.data));
        Ok(())
    };
    assert_eq!(pending.drive_with(&mut admit), Ok(None));
    assert_eq!(admitted, vec![second_digest]);
    assert_eq!(pending.ownership_units, 0);
    assert_eq!(pending.shared_ownership_units, 0);
    assert!(pending.source_fifo_owners.is_empty());
    let replay_bulk = ProductionV2Services::preencode_v2_network_message(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk(
            manifest_hash(b"source reconnect bulk"),
            0,
            b"source reconnect bulk",
            0,
        ))),
    )
    .expect("encode reconnect bulk output");
    assert_eq!(exact_output_class(&replay_bulk), Ok(ExactOutputClass::Bulk));
    let replay_messages = vec![lane_commit_qc_message(peer.clone()), replay_bulk.clone()];
    let mut replay_routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let replay_prior_route = replay_routes.mint(peer.clone());
    let mut replay_predecessor = PendingExactFanout::new_with_routes(
        replay_messages.clone(),
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(replay_prior_route.clone())],
    )
    .expect("mixed-class replay predecessor");
    let replay_returned = replay_predecessor.messages[1].clone();
    let replay_target = replay_predecessor
        .targets
        .first_mut()
        .expect("mixed-class predecessor has one target");
    replay_target.message_index = 1;
    replay_target.current = Some(Post {
        data: replay_returned,
        peer_id: peer.clone(),
        priority: Priority::High,
    });
    replay_predecessor
        .rebuild_current_source_targets()
        .expect("mixed-class predecessor cursor has a valid local FIFO index");
    let mut replay_pending = PendingExactOutput::new(1, 2, 1, std::slice::from_ref(&peer))
        .expect("one shared replay unit plus frozen target units");
    assert_eq!(
        replay_pending
            .enqueue(replay_predecessor)
            .expect("retain replay predecessor"),
        ExactFanoutOwnership::Owned
    );
    assert_eq!(
        replay_pending
            .enqueue(
                PendingExactFanout::new(
                    vec![lane_commit_qc_message(peer.clone())],
                    vec![peer.clone()],
                )
                .expect("newer lane owner"),
            )
            .expect("newer lane owner uses its frozen unit"),
        ExactFanoutOwnership::Owned
    );
    let blocker_hub = PeerId::new(KeyPair::random().public_key().clone());
    let blocker_route = replay_routes.mint_via(peer.clone(), blocker_hub);
    assert_eq!(
        replay_pending
            .enqueue(
                PendingExactFanout::new_with_routes(
                    vec![replay_bulk],
                    vec![peer.clone()],
                    vec![ExactTargetRoute::Reply(blocker_route.clone())],
                )
                .expect("duplicate bulk reservation blocker"),
            )
            .expect("bulk blocker consumes the only shared unit"),
        ExactFanoutOwnership::Owned
    );
    assert_eq!(replay_pending.shared_ownership_units, 1);
    assert!(replay_routes.retire(&replay_prior_route));
    let replay_reconnected_route = replay_routes.mint(peer.clone());
    let source_index_before = replay_pending.source_fifo_owners.clone();
    let ownership_before = replay_pending.reservation_owner_counts.clone();
    assert_eq!(
        replay_pending
            .enqueue(
                PendingExactFanout::new_with_routes(
                    replay_messages.clone(),
                    vec![peer.clone()],
                    vec![ExactTargetRoute::Reply(replay_reconnected_route.clone())],
                )
                .expect("same-source reconnect under full shared capacity"),
            )
            .expect("reconnect reuses its already-owned suffix reservation"),
        ExactFanoutOwnership::Owned
    );
    assert_eq!(replay_pending.source_fifo_owners, source_index_before);
    assert_eq!(replay_pending.reservation_owner_counts, ownership_before);
    let replay_target = &replay_pending.fanouts[0].targets[0];
    assert_eq!(replay_target.message_index, 1);
    assert!(replay_target.current.is_none());
    assert!(matches!(
        &replay_target.route,
        ExactTargetRoute::Reply(route) if route.same_tenure(&replay_reconnected_route)
    ));
    assert!(replay_routes.retire(&blocker_route));
    let blocker_index = replay_pending
        .fanouts
        .iter()
        .position(|fanout| {
            matches!(
                &fanout.targets[0].route,
                ExactTargetRoute::Reply(route) if route.same_tenure(&blocker_route)
            )
        })
        .expect("retired capacity blocker remains queued");
    replay_pending
        .retire_inactive_reply_target(blocker_index, 0)
        .expect("retiring the blocker parks its payload without erasing ownership");
    assert!(replay_pending.fanouts[blocker_index].targets[0].parked);
    assert_eq!(replay_pending.shared_ownership_units, 1);
    assert!(replay_pending.source_fifo_owners.values().any(|owners| {
        owners.contains(
            &replay_pending.fanouts[blocker_index]
                .fifo_id
                .expect("parked fanout retains stable age"),
        )
    }));
    assert_eq!(
        replay_pending
            .enqueue(
                PendingExactFanout::new_with_routes(
                    replay_messages.clone(),
                    vec![peer],
                    vec![ExactTargetRoute::Reply(replay_reconnected_route.clone())],
                )
                .expect("exact reconnect retry under retained capacity"),
            )
            .expect("exact retry reuses the retained source reservation"),
        ExactFanoutOwnership::Owned
    );
    let replay_target = &replay_pending.fanouts[0].targets[0];
    assert_eq!(replay_target.message_index, 1);
    assert!(replay_target.current.is_none());
    assert!(matches!(
        &replay_target.route,
        ExactTargetRoute::Reply(route) if route.same_tenure(&replay_reconnected_route)
    ));
    assert_eq!(replay_pending.shared_ownership_units, 1);
}
#[test]
fn delayed_old_tenure_delivery_cannot_replace_newer_worker_reply_route() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let message = merge_share_message(b"worker rejects delayed superseded tenure");
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 1);
    let old_route = routes.mint_via(peer.clone(), hub.clone());
    let current_route = routes.mint_via(peer.clone(), hub);
    let delayed_old_route = routes
        .redeliver(&old_route)
        .expect("old tenure delivers after the replacement was observed");
    assert_eq!(
        delayed_old_route.source_update_from(&current_route),
        Err(NetworkReplyRouteError::Stale)
    );
    let fanout = |route: &NetworkReplyRoute| {
        PendingExactFanout::new_with_reply_routes(
            vec![message.clone()],
            peer.clone(),
            NetworkReplyRoutes::try_from_route(route.clone()).expect("one live route"),
        )
        .expect("one ordinary reply fanout")
    };
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one reply source attempt fits");
    assert_eq!(
        pending.enqueue_owned_reply_transfer(fanout(&current_route)),
        Ok(ExactFanoutOwnership::Owned)
    );
    let ownership_before = pending.reservation_owner_counts.clone();
    let source_fifo_before = pending.source_fifo_owners.clone();
    let error = pending
        .enqueue_owned_reply_transfer(fanout(&delayed_old_route))
        .expect_err("superseded tenure cannot rebind the worker target");
    assert!(error.contains("stale capability"));
    let target = &pending.fanouts[0].targets[0];
    assert_eq!(target.message_index, 0);
    assert!(target.current.is_none());
    assert!(matches!(
        &target.route,
        ExactTargetRoute::Reply(route) if route.same_delivery(&current_route)
    ));
    assert_eq!(pending.reservation_owner_counts, ownership_before);
    assert_eq!(pending.source_fifo_owners, source_fifo_before);
}
