fn certified_sidecar_close_ack(
    local: &PeerId,
    peer: &PeerId,
    ordinal: u64,
) -> CertifiedMergeSidecarMessage {
    let stream_epoch = ordinal
        .checked_add(1)
        .expect("worker close acknowledgement epoch does not overflow");
    let mut ack = crate::merge_sidecar::CertifiedMergeSidecarCloseAckV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(stream_epoch).expect("worker close acknowledgement epoch is non-zero"),
        ),
        closed_through: stream_epoch,
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: peer.clone(),
        responder: local.clone(),
    };
    ack.close_id = ack.canonical_close_id();
    CertifiedMergeSidecarMessage::CloseAck(ack)
}
fn certified_sidecar_control_fanout(
    scope: ExactOutputCreationScope,
    peer: &PeerId,
    message: CertifiedMergeSidecarMessage,
) -> PendingExactFanout {
    let reply_control = matches!(
        &message,
        CertifiedMergeSidecarMessage::CloseAck(_) | CertifiedMergeSidecarMessage::GenerationHint(_)
    );
    if reply_control {
        let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
        let reply_route = routes.mint(peer.clone());
        return certified_sidecar_reply_control_fanout(
            scope,
            peer,
            message,
            NetworkReplyRoutes::try_from_route(reply_route)
                .expect("worker responder control keeps one exact return route"),
        );
    }
    let message_hash = HashOf::new(&message);
    let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(message));
    let claim = ExactOutputRolloverClaim::CertifiedSidecarControl {
        scope,
        target: peer.clone(),
        message_hash,
    };
    let fanout = PendingExactFanout::claimed(vec![message], vec![peer.clone()], claim);
    fanout
        .expect("valid worker sidecar-control rollover claim")
        .expect("one exact worker sidecar-control fanout")
}
fn certified_sidecar_reply_control_fanout(
    scope: ExactOutputCreationScope,
    peer: &PeerId,
    message: CertifiedMergeSidecarMessage,
    reply_routes: NetworkReplyRoutes,
) -> PendingExactFanout {
    assert!(matches!(
        &message,
        CertifiedMergeSidecarMessage::CloseAck(_) | CertifiedMergeSidecarMessage::GenerationHint(_)
    ));
    let message_hash = HashOf::new(&message);
    let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(message));
    let claim = ExactOutputRolloverClaim::CertifiedSidecarControl {
        scope,
        target: peer.clone(),
        message_hash,
    };
    PendingExactFanout::claimed_with_reply_routes(vec![message], peer.clone(), reply_routes, claim)
        .expect("valid worker responder-control rollover claim")
        .expect("one exact worker responder-control fanout")
}
fn certified_sidecar_request_fanout(
    scope: ExactOutputCreationScope,
    local: &PeerId,
    peer: &PeerId,
) -> PendingExactFanout {
    let (message, _) = certified_sidecar_outputs(local, peer);
    let CertifiedMergeSidecarMessage::Request(request) = &message else {
        unreachable!("worker sidecar fixture returns one request")
    };
    let transfer = CertifiedSidecarTransferIdentity::from_request(request);
    let request_hash = HashOf::new(request);
    PendingExactFanout::claimed(
        vec![NetworkMessage::CertifiedMergeSidecar(Arc::new(message))],
        vec![peer.clone()],
        ExactOutputRolloverClaim::CertifiedSidecarRequest {
            scope,
            target: peer.clone(),
            transfer,
            request_hash,
        },
    )
    .expect("valid worker sidecar-request rollover claim")
    .expect("one exact worker sidecar-request fanout")
}
fn certified_sidecar_flush_fixture(
    chunk: &CertifiedMergeSidecarChunkV1,
    route: &NetworkReplyRoute,
) -> (
    NetworkReplyFlushAckTestFixture,
    NetworkReplyFlushAck,
    CertifiedMergeSidecarChunkAdmission,
) {
    let post = Post {
        data: NetworkMessage::CertifiedMergeSidecar(Arc::new(CertifiedMergeSidecarMessage::Chunk(
            chunk.clone(),
        ))),
        peer_id: chunk.requester.clone(),
        priority: Priority::High,
    };
    let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
    let admission = CertifiedMergeSidecarChunkAdmission::from_admitted_reply(
        &post,
        route,
        0,
        1,
        ack.identity(),
    )
    .expect("bind exact worker-side sidecar flush fixture");
    (control, ack, admission)
}
fn merge_share_digest(message: &NetworkMessage) -> Hash {
    let NetworkMessage::MergeCommitteeSignature(signature) = message else {
        panic!("expected exact merge-share output");
    };
    signature.message_digest
}
include!("v2_worker_reply_route_cases.rs");
#[test]
fn pending_reply_flush_fifo_head_quiesces_later_same_source_fanout() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let routed = |message| {
        PendingExactFanout::new_with_routes(
            vec![message],
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route.clone())],
        )
        .expect("one exact reply fanout")
    };
    let mut pending =
        PendingExactOutput::new(2, 1, 1, &[]).expect("two same-source reply fanouts fit");
    assert_eq!(
        pending.enqueue(routed(merge_share_message(b"older pending flush"))),
        Ok(ExactFanoutOwnership::Owned)
    );
    let mut flush_control = None;
    assert_eq!(
        pending.drive_with_budget_ack(1, |post, ticket, attempted, timeout_attempt| {
            assert!(ticket.is_none());
            let ExactTargetRoute::Reply(attempted) = attempted else {
                panic!("exact reply changed route kind")
            };
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                attempted,
                timeout_attempt,
            );
            flush_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        }),
        Ok(ExactOutputDriveOutcome::BudgetExhausted {
            closest_backpressure_rank: None,
        })
    );
    assert!(pending.fanouts[0].targets[0].pending_flush.is_some());
    assert_eq!(
        pending.enqueue(routed(merge_share_message(b"later same source"))),
        Ok(ExactFanoutOwnership::Owned)
    );
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |_post, _ticket, _route, _timeout_attempt| {
            panic!("a later same-source fanout must wait behind its pending flush head")
        },),
        Ok(ExactOutputDriveOutcome::Drained)
    );
    assert_eq!(pending.fanouts.len(), 2);
    assert!(pending.fanouts[0].targets[0].pending_flush.is_some());
    assert!(pending.fanouts[1].has_dispatchable_target());
    drop(flush_control);
}
#[test]
fn ordinary_reply_timeout_grows_only_its_source_attempt_while_sibling_progresses() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let message = merge_share_message(b"ordinary reply close and reconnect");
    let payload_hash = HashOf::new(&message);
    let response_class = exact_output_class(&message).expect("classify ordinary reply");
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = routes.mint_via(peer.clone(), hub_a.clone());
    let route_b = routes.mint_via(peer.clone(), hub_b);
    let source_a = ExactTargetRoute::Reply(route_a.clone()).source(&peer, response_class);
    let source_b = ExactTargetRoute::Reply(route_b.clone()).source(&peer, response_class);
    let mut reply_routes =
        NetworkReplyRoutes::try_from_route(route_a.clone()).expect("source A route set");
    reply_routes
        .merge(&NetworkReplyRoutes::try_from_route(route_b.clone()).expect("source B route set"))
        .expect("retain both authenticated sources");
    let mut pending =
        PendingExactOutput::new(2, 1, 2, &[]).expect("two ordinary reply attempts fit");
    assert_eq!(
        pending.enqueue_owned_reply_transfer(
            PendingExactFanout::new_with_reply_routes(
                vec![message.clone()],
                peer.clone(),
                reply_routes,
            )
            .expect("two-source ordinary reply fanout"),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    let fifo_id = pending.fanouts[0]
        .fifo_id
        .expect("ordinary reply fanout owns stable FIFO age");
    let source_a_index = pending.fanouts[0]
            .targets
            .iter()
            .position(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(&route_a))
            })
            .expect("fanout retains source A");
    let mut source_a_control = None;
    let mut source_b_control = None;
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, timeout_attempt| {
            assert!(ticket.is_none());
            assert_eq!(timeout_attempt, 0);
            assert_eq!(HashOf::new(&post.data), payload_hash);
            let ExactTargetRoute::Reply(route) = route else {
                panic!("ordinary reply must retain its authenticated route")
            };
            let (mut control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            if route.same_source(&route_a) {
                source_a_control = Some(control);
            } else {
                assert!(route.same_source(&route_b));
                assert!(control.flush());
                source_b_control = Some(control);
            }
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        }),
        Ok(ExactOutputDriveOutcome::Drained)
    );
    assert!(source_a_control.is_some());
    assert!(source_b_control.is_some());
    assert_eq!(pending.ownership_units, 2);
    assert_eq!(
        pending.fanouts[0]
            .targets
            .iter()
            .filter(|target| target.pending_flush.is_some())
            .count(),
        2
    );
    pending
        .poll_reply_flushes()
        .expect("source B writer flush advances independently");
    let target_a = &pending.fanouts[0].targets[source_a_index];
    assert_eq!(target_a.message_index, 0);
    assert!(target_a.pending_flush.is_some());
    assert_eq!(HashOf::new(&pending.fanouts[0].messages[0]), payload_hash);
    assert!(pending.source_fifo_owners.contains_key(&source_a));
    assert!(!pending.source_fifo_owners.contains_key(&source_b));
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(target_a.reply_writer_timeout_attempt, 0);
    assert!(routes.mark_reply_unwritable_while_delivery_active(&route_a));
    assert!(
        source_a_control
            .as_mut()
            .expect("source A retains its sole writer controller")
            .timeout()
    );
    pending
        .poll_reply_flushes()
        .expect("timed-out source A acknowledgement retains the exact current item");
    let target_a = &pending.fanouts[0].targets[source_a_index];
    assert_eq!(target_a.message_index, 0);
    assert_eq!(target_a.reply_writer_timeout_attempt, 1);
    assert!(target_a.current.is_none());
    assert!(target_a.pending_flush.is_none());
    assert!(target_a.parked);
    assert_eq!(HashOf::new(&pending.fanouts[0].messages[0]), payload_hash);
    assert_eq!(
        pending.source_fifo_owners.get(&source_a),
        Some(&BTreeSet::from([fifo_id]))
    );
    assert_eq!(pending.ownership_units, 1);
    assert!(routes.retire(&route_a));
    let reconnected_a = routes.mint_via(peer.clone(), hub_a);
    assert_eq!(
        pending.enqueue_owned_reply_transfer(
            PendingExactFanout::new_with_reply_routes(
                vec![message],
                peer,
                NetworkReplyRoutes::try_from_route(reconnected_a.clone())
                    .expect("reconnected source A route set"),
            )
            .expect("same-source reconnect candidate"),
        ),
        Ok(ExactFanoutOwnership::Owned)
    );
    let target_a = &pending.fanouts[0].targets[source_a_index];
    assert_eq!(target_a.message_index, 0);
    assert!(target_a.pending_flush.is_none());
    assert!(matches!(
        &target_a.route,
        ExactTargetRoute::Reply(route) if route.same_delivery(&reconnected_a)
    ));
    let mut retry_control = None;
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, timeout_attempt| {
            assert!(ticket.is_none());
            assert_eq!(
                timeout_attempt, 1,
                "same-source reconnect preserves its adaptive timeout generation"
            );
            assert_eq!(HashOf::new(&post.data), payload_hash);
            let ExactTargetRoute::Reply(route) = route else {
                panic!("reconnected reply changed route kind")
            };
            assert!(route.same_delivery(&reconnected_a));
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            retry_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        }),
        Ok(ExactOutputDriveOutcome::Drained)
    );
    assert!(
        retry_control
            .as_mut()
            .expect("replacement writer owns the retried item")
            .flush()
    );
    pending
        .poll_reply_flushes()
        .expect("replacement writer completes the retained item");
    assert!(pending.fanouts.is_empty());
    assert_eq!(pending.ownership_units, 0);
    assert!(pending.source_fifo_owners.is_empty());
}
#[test]
fn closed_flush_on_delivery_active_unwritable_route_parks_without_cursor_advance() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let message = merge_share_message(b"closed draining reply");
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let mut pending =
        PendingExactOutput::new(1, 1, 1, &[]).expect("one draining reply attempt fits");
    pending
        .enqueue(
            PendingExactFanout::new_with_routes(
                vec![message.clone()],
                vec![peer],
                vec![ExactTargetRoute::Reply(route.clone())],
            )
            .expect("one draining reply fanout"),
        )
        .expect("retain the draining reply");
    let mut control = None;
    pending
        .drive_with_budget_ack(usize::MAX, |post, ticket, attempted, _timeout_attempt| {
            assert!(ticket.is_none());
            let ExactTargetRoute::Reply(attempted) = attempted else {
                panic!("draining response changed route kind")
            };
            let (writer, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, attempted);
            control = Some(writer);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("queue the exact writer flush");
    assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
    assert!(control.as_mut().expect("writer controller").close());
    pending
        .poll_reply_flushes()
        .expect("closed draining flush is a recoverable route transition");
    let target = &pending.fanouts[0].targets[0];
    assert_eq!(target.message_index, 0);
    assert_eq!(
        target.reply_writer_timeout_attempt, 0,
        "ordinary Closed must not grow the adaptive timeout generation"
    );
    assert!(target.current.is_none());
    assert!(target.pending_flush.is_none());
    assert!(target.parked);
    assert_eq!(pending.fanouts[0].messages.len(), 1);
    assert_eq!(
        norito::to_bytes(&pending.fanouts[0].messages[0]).expect("encode retained draining reply"),
        norito::to_bytes(&message).expect("encode expected draining reply"),
        "parking an unwritable delivery-active route must retain the exact payload"
    );
    assert!(!pending.source_fifo_owners.is_empty());
}
#[test]
fn adaptive_reply_timeout_grows_closed_preserves_and_flushed_resets_attempt() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let messages = vec![
        merge_share_message(b"adaptive timeout first"),
        merge_share_message(b"adaptive timeout second"),
    ];
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let mut pending =
        PendingExactOutput::new(1, 2, 1, &[]).expect("one adaptive reply source fits");
    pending
        .enqueue(
            PendingExactFanout::new_with_routes(
                messages,
                vec![peer],
                vec![ExactTargetRoute::Reply(route)],
            )
            .expect("two-message adaptive reply fanout"),
        )
        .expect("retain the adaptive reply fanout");
    let mut timeout_control = None;
    pending
        .drive_with_budget_ack(1, |post, _ticket, route, timeout_attempt| {
            assert_eq!(timeout_attempt, 0);
            let ExactTargetRoute::Reply(route) = route else {
                panic!("adaptive reply changed route kind")
            };
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            timeout_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("admit the first adaptive reply occurrence");
    assert!(
        timeout_control
            .as_mut()
            .expect("first writer controller")
            .timeout()
    );
    pending
        .poll_reply_flushes()
        .expect("timeout retains the exact current item");
    assert_eq!(pending.fanouts[0].targets[0].message_index, 0);
    assert_eq!(
        pending.fanouts[0].targets[0].reply_writer_timeout_attempt,
        1
    );
    let mut closed_control = None;
    pending
        .drive_with_budget_ack(1, |post, _ticket, route, timeout_attempt| {
            assert_eq!(timeout_attempt, 1);
            let ExactTargetRoute::Reply(route) = route else {
                panic!("adaptive retry changed route kind")
            };
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            closed_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("admit the retry whose writer will close");
    assert!(
        closed_control
            .as_mut()
            .expect("closed writer controller")
            .close()
    );
    pending
        .poll_reply_flushes()
        .expect("Closed retains the adaptive generation");
    assert_eq!(
        pending.fanouts[0].targets[0].reply_writer_timeout_attempt,
        1
    );
    let mut flushed_control = None;
    pending
        .drive_with_budget_ack(1, |post, _ticket, route, timeout_attempt| {
            assert_eq!(timeout_attempt, 1);
            let ExactTargetRoute::Reply(route) = route else {
                panic!("adaptive flush retry changed route kind")
            };
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
                &post,
                route,
                timeout_attempt,
            );
            flushed_control = Some(control);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("admit the retry whose writer will flush");
    assert!(
        flushed_control
            .as_mut()
            .expect("successful writer controller")
            .flush()
    );
    pending
        .poll_reply_flushes()
        .expect("Flushed advances and resets the adaptive generation");
    let target = &pending.fanouts[0].targets[0];
    assert_eq!(target.message_index, 1);
    assert_eq!(target.reply_writer_timeout_attempt, 0);
}
#[test]
fn reply_flush_attempt_identity_mismatch_fails_without_cursor_or_attempt_advance() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let message = merge_share_message(b"reply timeout-attempt identity mismatch");
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let mut pending =
        PendingExactOutput::new(1, 1, 1, &[]).expect("one adaptive reply source fits");
    pending
        .enqueue(
            PendingExactFanout::new_with_routes(
                vec![message],
                vec![peer],
                vec![ExactTargetRoute::Reply(route)],
            )
            .expect("one adaptive reply fanout"),
        )
        .expect("retain the adaptive reply fanout");
    pending.fanouts[0].targets[0].reply_writer_timeout_attempt = 1;
    let error = pending
        .drive_with_budget_ack(1, |post, ticket, attempted, timeout_attempt| {
            assert!(ticket.is_none());
            assert_eq!(timeout_attempt, 1);
            let ExactTargetRoute::Reply(attempted) = attempted else {
                panic!("adaptive reply changed route kind")
            };
            let (_attempt_zero_control, attempt_zero_ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, attempted);
            assert_eq!(
                attempt_zero_ack.identity().reply_writer_timeout_attempt(),
                0
            );
            Ok(ExactOutputAttemptOutcome::ReplyFlush(attempt_zero_ack))
        })
        .expect_err("attempt-zero acknowledgement must not satisfy attempt one");
    assert!(error.contains("timeout-attempt identity"));
    let target = &pending.fanouts[0].targets[0];
    assert_eq!(target.message_index, 0);
    assert_eq!(target.reply_writer_timeout_attempt, 1);
    assert!(target.current.is_none());
    assert!(target.ticket.is_none());
    assert!(target.pending_flush.is_none());
    assert_eq!(pending.ownership_units, 1);
}
include!("v2_worker_backpressure_cases.rs");
/// Exercise a dead-target output through synthesized durable-height handoff.
///
/// The fixture validates the production output/handoff contract only; it
/// does not execute the preceding QC-to-application pipeline.
pub(in crate::sumeragi) fn production_output_handoff_with_dead_target() -> wire::HeightContext {
    let (mut service, keys) = fixture();
    let context = service.context.clone();
    let (receipt, artifact) = durable_finality_fixture(&service, &keys);
    let blocked = service.context.roster[1].validator.clone();
    let later_responsive = service.context.roster[3].validator.clone();
    let lane_qc = lane_commit_qc(blocked.clone());
    let lane_message = BlockMessage::LaneBlockQc(lane_qc.clone());
    let lane_authority = DurableLaneRolloverAuthority::for_test(&artifact, &lane_message);
    let blocked_for_hook = blocked.clone();
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_hook = Arc::clone(&attempts);
    let admitted = Arc::new(Mutex::new(Vec::new()));
    let admitted_for_hook = Arc::clone(&admitted);
    service.set_exact_output_admission_hook(move |post, ticket| {
        if post.peer_id == blocked_for_hook {
            attempts_for_hook.fetch_add(1, Ordering::Relaxed);
            return Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            });
        }
        assert!(ticket.is_none());
        let kind = match &post.data {
            NetworkMessage::SumeragiBlock(wire)
                if matches!(wire.as_message(), BlockMessage::LaneBlockQc(_)) =>
            {
                "lane-qc"
            }
            NetworkMessage::MergeCommitteeSignature(_) => "merge-share",
            other => panic!("unexpected production output fixture: {other:?}"),
        };
        admitted_for_hook
            .lock()
            .expect("record admitted production output")
            .push((post.peer_id, kind));
        Ok(())
    });
    service
        .post_lane_block(blocked.clone(), lane_message.clone())
        .expect("retain finalized-height lane certificate for blocked target");
    assert!(
        service
            .has_pending_exact_output()
            .expect("inspect pending production output")
    );
    service
        .post_lane_block(later_responsive.clone(), lane_message)
        .expect("later responsive fanout enters the non-full corridor");
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    let admitted = admitted.lock().expect("inspect admitted production output");
    assert_eq!(
        admitted
            .iter()
            .filter(|(peer, _)| peer == &later_responsive)
            .cloned()
            .collect::<Vec<_>>(),
        vec![(later_responsive, "lane-qc")]
    );
    drop(admitted);
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect surviving production target");
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.fanouts[0].peers[0], blocked);
    assert!(pending.fanouts[0].targets[0].current.is_some());
    drop(pending);
    service.broadcast_merge_to_voters(merge_share(b"rollover merge share"));
    let (sidecar_request, _sidecar_chunk) =
        certified_sidecar_outputs(&service.local_peer, &blocked);
    service.post_certified_merge_sidecar(blocked.clone(), sidecar_request);
    assert_eq!(
        service
            .lock_pending_exact_output()
            .expect("inspect typed rollover outputs")
            .fanouts
            .len(),
        3,
        "lane, merge-share, and locally initiated sidecar request stay owned"
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &receipt,
                &artifact,
                &lane_authority,
            )
            .expect("durable application supersedes dead-target output"),
        3
    );
    assert!(
        !service
            .has_pending_exact_output()
            .expect("inspect applied-height output handoff")
    );
    assert!(!service.output_guard.restart_required());
    context
}
#[test]
fn production_output_path_serves_later_fanout_while_target_stays_backpressured() {
    let _ = production_output_handoff_with_dead_target();
}
#[test]
fn response_outputs_without_exact_routes_fail_stop() {
    let (mut service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let messages = non_retireable_lane_transport_messages(peer.clone());
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    for message in &messages {
        let effect = V2LaneWorkEffect::PostLaneBlock {
            peer: peer.clone(),
            message: message.clone(),
        };
        assert_eq!(service.can_retain_lane_work_effect(&effect), Ok(true));
        assert_eq!(
            service.post_lane_block(peer.clone(), message.clone()),
            Ok(()),
            "every authenticated non-retireable lane message must enter exact ownership"
        );
    }
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect retained non-retireable lane transport");
    assert_eq!(pending.fanouts.len(), messages.len());
    for (fanout, expected) in pending.fanouts.iter().zip(&messages) {
        assert!(matches!(
            &fanout.rollover_claim,
            ExactOutputRolloverClaim::NonRetireableLaneTransport {
                target,
                message_hash,
            } if target == &peer && *message_hash == HashOf::new(expected)
        ));
        assert!(matches!(
            fanout.messages.as_slice(),
            [NetworkMessage::SumeragiBlock(envelope)]
                if HashOf::new(envelope.as_message()) == HashOf::new(expected)
        ));
    }
    drop(pending);
    assert!(!service.output_guard.restart_required());
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    assert!(
        service
            .post_lane_block(peer, BlockMessage::invalid_wire_sentinel())
            .is_err(),
        "the lane-only transport must reject decode-only global traffic"
    );
    assert!(service.output_guard.restart_required());
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    service.post_native_amx(
        peer,
        native_amx_output(&service.context, service.local_peer.clone()),
    );
    assert!(service.output_guard.restart_required());
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let (_request, chunk) = certified_sidecar_outputs(&service.local_peer, &peer);
    service.post_certified_merge_sidecar(peer, chunk);
    assert!(service.output_guard.restart_required());
}
#[test]
fn locally_authorized_autonomous_transport_has_durable_rollover_claim() {
    let (mut service, _) = fixture();
    let target = service.context.roster[1].validator.clone();
    let messages = non_retireable_lane_transport_messages(service.local_peer.clone());
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    for message in &messages {
        service
            .post_lane_block(target.clone(), message.clone())
            .expect("retain locally reconstructable autonomous transport");
    }
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect autonomous rollover claims");
    assert_eq!(pending.fanouts.len(), messages.len());
    let expected_scope = service.exact_output_scope();
    for fanout in &pending.fanouts {
        assert!(matches!(
            &fanout.rollover_claim,
            ExactOutputRolloverClaim::AutonomousLane {
                scope,
                local_peer,
                proposal_height: 1,
            } if *scope == expected_scope && local_peer == &service.local_peer
        ));
    }
}
#[test]
fn generation_hint_requires_exact_reply_route_ownership() {
    let (mut service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let hint = Arc::new(certified_sidecar_generation_hint(
        &service.local_peer,
        &peer,
        401,
    ));
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());
    let reply_route = routes.mint_via(peer.clone(), hub);
    let reply_routes = NetworkReplyRoutes::try_from_route(reply_route.clone())
        .expect("one live GenerationHint reply route");
    let effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
        peer: peer.clone(),
        reply_routes: Some(reply_routes.clone()),
        message: Arc::clone(&hint),
    };
    assert_eq!(service.can_retain_lane_work_effect(&effect), Ok(true));
    assert_eq!(
        service
            .post_certified_merge_sidecar_with_reply_routes(
                peer.clone(),
                Some(reply_routes),
                Arc::clone(&hint),
            )
            .expect("routed GenerationHint has exact reply ownership"),
        ExactFanoutOwnership::Owned
    );
    assert!(!service.output_guard.restart_required());
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect retained routed GenerationHint");
    assert_eq!(pending.fanouts.len(), 1);
    let fanout = &pending.fanouts[0];
    assert!(matches!(
        fanout.messages.as_slice(),
        [NetworkMessage::CertifiedMergeSidecar(message)]
            if matches!(
                message.as_ref(),
                CertifiedMergeSidecarMessage::GenerationHint(_)
            )
    ));
    assert_eq!(
        fanout.messages[0].topic(),
        iroha_p2p::network::message::Topic::Consensus
    );
    assert_eq!(fanout.peers, vec![peer.clone()]);
    assert!(matches!(
        fanout.targets.as_slice(),
        [PendingExactTarget {
            route: ExactTargetRoute::Reply(retained),
            ..
        }] if retained.same_delivery(&reply_route)
    ));
    assert_eq!(fanout.certified_sidecar_topology_progress_target(), None);
    drop(pending);
    let (missing_route_service, _) = fixture();
    let missing_route_peer = missing_route_service.context.roster[1].validator.clone();
    let missing_route_hint = Arc::new(certified_sidecar_generation_hint(
        &missing_route_service.local_peer,
        &missing_route_peer,
        402,
    ));
    let missing_route_effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
        peer: missing_route_peer.clone(),
        reply_routes: None,
        message: Arc::clone(&missing_route_hint),
    };
    assert!(
        missing_route_service
            .can_retain_lane_work_effect(&missing_route_effect)
            .expect_err("GenerationHint missing reply-route ownership must fail preflight")
            .contains("reply-route ownership")
    );
    assert!(
        missing_route_service
            .post_certified_merge_sidecar_with_reply_routes(
                missing_route_peer,
                None,
                missing_route_hint,
            )
            .expect_err(
                "GenerationHint missing reply-route ownership must fail exact-output admission",
            )
            .contains("reply-route ownership")
    );
    assert!(missing_route_service.output_guard.restart_required());
}
#[test]
fn lane_drain_vote_uses_one_authenticated_exact_output_claim() {
    let (service, keys) = fixture();
    let target = service.context.roster[1].validator.clone();
    let vote = lane_drain_vote(&keys[0]);
    let effect = V2LaneWorkEffect::PostLaneDrainVote {
        peer: target.clone(),
        vote: vote.clone(),
    };
    assert_eq!(service.can_retain_lane_work_effect(&effect), Ok(true));
    service.post_lane_drain_vote(target.clone(), vote.clone());
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect retained lane-drain output");
    assert_eq!(pending.fanouts.len(), 1);
    let fanout = &pending.fanouts[0];
    assert_eq!(fanout.semantic_peers(), vec![target.clone()]);
    assert!(matches!(
        fanout.messages.as_slice(),
        [NetworkMessage::LaneDrainVote(queued)] if queued.as_ref() == &vote
    ));
    assert!(matches!(
        &fanout.rollover_claim,
        ExactOutputRolloverClaim::LaneDrainVote {
            target: claimed_target,
            vote_hash,
            ..
        } if claimed_target == &target && *vote_hash == HashOf::new(&vote)
    ));
    drop(pending);
    let mut tampered = vote;
    tampered.bls_signature[0] ^= 0x01;
    let error = service
        .can_retain_lane_work_effect(&V2LaneWorkEffect::PostLaneDrainVote {
            peer: target,
            vote: tampered,
        })
        .expect_err("tampered drain vote must fail before corridor reservation");
    assert!(error.contains("invalid vote evidence"));
}
#[test]
fn sidecar_receipts_use_a_separate_bounded_control_queue() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let (_, chunk) = certified_sidecar_outputs(&service.local_peer, &peer);
    let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(chunk));
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let fanout = || {
        PendingExactFanout::new_with_routes(
            vec![message.clone()],
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route.clone())],
        )
        .expect("one routed sidecar response")
    };
    let mut pending = PendingExactOutput::new(1, 1, 1, &[])
        .expect("one ownership unit and one receipt-control unit");
    assert_eq!(pending.sidecar_admission_capacity, 1);
    assert_eq!(pending.enqueue(fanout()), Ok(ExactFanoutOwnership::Owned));
    let mut first_flush_control = None;
    assert_eq!(
        pending.drive_with_budget_ack(1, |post, ticket, route, _timeout_attempt| {
            assert!(ticket.is_none());
            let ExactTargetRoute::Reply(route) = route else {
                panic!("sidecar response must retain its reply route")
            };
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
            first_flush_control = Some(control);
            Ok(ExactOutputAttemptOutcome::SidecarFlush(ack))
        }),
        Ok(ExactOutputDriveOutcome::BudgetExhausted {
            closest_backpressure_rank: None,
        })
    );
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.pending_sidecar_flushes(), 1);
    assert!(pending.admitted_sidecar_chunks.is_empty());
    assert!(
        first_flush_control
            .as_mut()
            .expect("first sidecar writer owns its flush controller")
            .flush()
    );
    pending
        .poll_reply_flushes()
        .expect("first exact writer flush publishes its receipt");
    assert_eq!(pending.ownership_units, 0);
    assert_eq!(pending.pending_sidecar_flushes(), 0);
    assert_eq!(pending.admitted_sidecar_chunks.len(), 1);
    assert_eq!(pending.enqueue(fanout()), Ok(ExactFanoutOwnership::Owned));
    assert_eq!(
        pending.drive_with_budget_ack(1, |_post, _ticket, _route, _timeout_attempt| {
            panic!("a full receipt queue must stop before actor admission")
        }),
        Ok(ExactOutputDriveOutcome::ReceiptBackpressured)
    );
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.admitted_sidecar_chunks.len(), 1);
    pending
        .admitted_sidecar_chunks
        .pop_front()
        .expect("release the first bounded receipt");
    let mut second_flush_control = None;
    assert_eq!(
        pending.drive_with_budget_ack(1, |post, ticket, route, _timeout_attempt| {
            assert!(ticket.is_none());
            let ExactTargetRoute::Reply(route) = route else {
                panic!("sidecar response must retain its reply route")
            };
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
            second_flush_control = Some(control);
            Ok(ExactOutputAttemptOutcome::SidecarFlush(ack))
        }),
        Ok(ExactOutputDriveOutcome::BudgetExhausted {
            closest_backpressure_rank: None,
        })
    );
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.pending_sidecar_flushes(), 1);
    assert!(pending.admitted_sidecar_chunks.is_empty());
    assert!(
        second_flush_control
            .as_mut()
            .expect("second sidecar writer owns its flush controller")
            .flush()
    );
    pending
        .poll_reply_flushes()
        .expect("second exact writer flush publishes its receipt");
    assert_eq!(pending.ownership_units, 0);
    assert_eq!(pending.pending_sidecar_flushes(), 0);
    assert_eq!(pending.admitted_sidecar_chunks.len(), 1);
}
include!("v2_worker_recovered_lifecycle_output_cases.rs");
#[test]
fn actor_backpressure_cannot_change_returned_payload_identity() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let original = merge_share_message(b"original exact output");
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    pending
        .enqueue(
            PendingExactFanout::new(vec![original], vec![peer]).expect("original exact fanout"),
        )
        .expect("retain original exact fanout");
    let error = pending
        .drive_with(|mut post, ticket, _route| {
            post.data = merge_share_message(b"mutated returned output");
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            })
        })
        .expect_err("the actor cannot substitute a same-target payload");
    assert!(error.contains("changed an exact output payload"));
    assert!(pending.is_pending());
    assert!(pending.fanouts[0].targets[0].current.is_none());
}
#[test]
fn exact_output_retry_rejects_a_different_message_identity() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let reply_route = routes.mint(peer.clone());
    let original = merge_share_message(b"retained exact output");
    let mut retained = PendingExactFanout::new_with_routes(
        vec![original.clone()],
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(reply_route.clone())],
    )
    .expect("retained exact fanout");
    let exact_retry = PendingExactFanout::new_with_routes(
        vec![original],
        vec![peer.clone()],
        vec![ExactTargetRoute::Reply(reply_route.clone())],
    )
    .expect("exact same-tenure retransmission");
    let conflicting = PendingExactFanout::new_with_routes(
        vec![merge_share_message(b"conflicting exact output")],
        vec![peer],
        vec![ExactTargetRoute::Reply(reply_route)],
    )
    .expect("conflicting retransmission");
    assert!(retained.can_coalesce_retry(&exact_retry));
    assert_ne!(retained.message_hashes, conflicting.message_hashes);
    assert!(
        !retained
            .coalesce_retry(&conflicting)
            .expect("conflicting retry is structurally valid")
    );
}
#[test]
fn outbound_corridor_capacity_keeps_the_owned_front_bounded() {
    let (service, _) = fixture();
    let first_peer = service.context.roster[1].validator.clone();
    let second_peer = service.context.roster[2].validator.clone();
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    assert_eq!(
        pending
            .enqueue(
                PendingExactFanout::new(
                    vec![lane_commit_qc_message(first_peer.clone())],
                    vec![first_peer],
                )
                .expect("first final QC fanout"),
            )
            .expect("first fanout is within protocol bounds"),
        ExactFanoutOwnership::Owned
    );
    assert_eq!(
        pending
            .enqueue(
                PendingExactFanout::new(
                    vec![lane_commit_qc_message(second_peer.clone())],
                    vec![second_peer],
                )
                .expect("second final QC fanout"),
            )
            .expect("second fanout is within protocol bounds"),
        ExactFanoutOwnership::SourceRetained
    );
    assert_eq!(pending.fanouts.len(), 1);
}
#[test]
fn applied_height_handoff_rejects_output_without_reconstruction() {
    let (service, keys) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    pending
        .enqueue(
            PendingExactFanout::new(
                vec![merge_share_message(b"manual untyped output")],
                vec![peer],
            )
            .expect("non-empty exact-only fanout"),
        )
        .expect("retain exact-only fanout");
    let error = pending
        .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
        .expect_err("exact-only output cannot enter durable reconstruction handoff");
    assert!(error.contains("no typed applied-height rollover claim"));
    assert!(pending.is_pending());
    let mut other_context = service.context.clone();
    other_context.height = other_context.height.saturating_add(1);
    let native = native_amx_output(&other_context, service.local_peer.clone());
    let native_hash = HashOf::new(&native);
    let native_round = native_amx_message_body(&native)
        .expect("valid Native AMX fixture round")
        .round;
    let wrong_scope = ExactOutputCreationScope {
        context_id: native_round.context_id,
        height: native_round.height,
    };
    let mut wrong = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    wrong
        .enqueue(
            PendingExactFanout::claimed(
                vec![NetworkMessage::NativeAmx(Arc::new(native))],
                vec![service.context.roster[1].validator.clone()],
                ExactOutputRolloverClaim::NativeAmx {
                    scope: wrong_scope,
                    round: native_round,
                    message_hash: native_hash,
                },
            )
            .expect("internally exact wrong-scope claim")
            .expect("non-empty wrong-scope fanout"),
        )
        .expect("retain wrong-scope Native AMX output");
    let error = wrong
        .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
        .expect_err("another height's typed claim must fail closed");
    assert!(error.contains("another creation scope"));
    assert!(wrong.is_pending());
}
#[test]
fn closed_network_actor_fails_stop_before_later_output() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    service
        .lock_pending_exact_output()
        .expect("lock output corridor")
        .enqueue(
            PendingExactFanout::new(vec![lane_commit_qc_message(peer.clone())], vec![peer])
                .expect("non-empty final QC fanout"),
        )
        .expect("retain final QC before actor admission");
    let error = service
        .retry_pending_exact_output()
        .expect_err("a permanently closed network actor must fail stop");
    assert!(error.contains("network actor closed"));
    assert!(service.output_guard.restart_required());
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect fail-stop output ownership");
    assert_eq!(pending.fanouts.len(), 1);
    let retained = pending.fanouts[0].targets[0]
        .current
        .as_ref()
        .expect("closed actor returned the exact final QC post");
    assert!(matches!(&retained.data, NetworkMessage::SumeragiBlock(_)));
}
#[test]
fn full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    assert_eq!(
        pending
            .enqueue(
                PendingExactFanout::new(
                    vec![lane_commit_qc_message(peer.clone())],
                    vec![peer.clone()],
                )
                .expect("valid progress fanout"),
            )
            .expect("valid fanout enters corridor"),
        ExactFanoutOwnership::Owned
    );
    let error = PendingExactFanout::classified_with_routes(
        vec![NetworkMessage::Health],
        vec![peer],
        vec![ExactTargetRoute::Topology],
    )
    .expect_err("a non-progress route has no reliable scheduler class");
    assert!(error.contains("no reliable progress class"));
    assert_eq!(pending.fanouts.len(), 1);
    assert!(!service.output_guard.restart_required());
}
fn locked_candidate_subject(label: &[u8]) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        payload_hash: Hash::new(label),
    }
}
fn locked_candidate_tag(view: u64) -> EventTag {
    EventTag::new(1, view, Generation::new(view + 1))
}
fn locked_candidate_round(service: &ProductionV2Services, view: u64) -> wire::ConsensusRound {
    wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view,
    }
}
fn attach_locked_candidate_io(
    service: &mut ProductionV2Services,
    capacity: usize,
) -> V2IoCommandReceiver {
    let (command_tx, command_rx, admission) = test_io_command_channel(capacity);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(capacity);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    command_rx
}
fn detach_locked_candidate_io(service: &mut ProductionV2Services) {
    drop(service.io.take());
}
#[test]
fn locked_candidate_requests_coalesce_by_immutable_subject() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"coalesced locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue the one physical acquisition");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("coalesce an exact retransmission");
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("rebind the same acquisition to a later view");
    let same_view_rebound = EventTag::new(1, 1, Generation::new(3));
    service
        .request_locked_candidate(
            same_view_rebound,
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("rebind the same acquisition to a newer same-view generation");
    let commands = command_rx.try_iter().collect::<Vec<_>>();
    assert!(matches!(
        commands.as_slice(),
        [V2IoCommand::LoadCandidate { subject: queued, .. }] if *queued == subject
    ));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("one acquisition owner");
    assert_eq!(acquisition.subject, subject);
    assert_eq!(acquisition.consumer, same_view_rebound);
    assert_eq!(acquisition.pending_count(), 1);
    detach_locked_candidate_io(&mut service);
}
#[test]
fn locked_candidate_completion_uses_latest_consumer_without_reloading() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"rebound locked candidate");
    let canonical_wire = b"exact durable body".to_vec();
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue initial load");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the one exact-subject candidate load"),
    };
    service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject,
            canonical_wire: canonical_wire.clone(),
        })
        .expect("complete the physical load");
    service
        .request_locked_candidate(
            locked_candidate_tag(3),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("advance the ready result consumer");
    let first = service
        .take_loaded_candidate()
        .expect("deliver ready bytes to the latest view");
    assert_eq!(first.tag(), locked_candidate_tag(3));
    assert_eq!(first.round(), locked_candidate_round(&service, 0));
    assert_eq!(first.subject(), subject);
    assert_eq!(first.into_canonical_wire(), canonical_wire);
    assert!(service.take_loaded_candidate().is_none());
    service
        .request_locked_candidate(
            locked_candidate_tag(4),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("rebind retained ready bytes once more");
    let second = service
        .take_loaded_candidate()
        .expect("redeliver retained bytes without another read");
    assert_eq!(second.tag(), locked_candidate_tag(4));
    assert_eq!(second.subject(), subject);
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    detach_locked_candidate_io(&mut service);
}
#[test]
fn locked_candidate_consumer_rebind_rejects_stale_or_regressive_tags() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"monotonic locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(2),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue current-view acquisition");
    let stale = service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect_err("a stale consumer must not replace the latest binding");
    assert!(stale.contains("did not advance monotonically"));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("original acquisition remains owned");
    assert_eq!(acquisition.consumer, locked_candidate_tag(2));
    assert!(service.output_guard.restart_required());
    assert_eq!(command_rx.try_iter().count(), 1);
    detach_locked_candidate_io(&mut service);
}
#[test]
fn locked_candidate_duplicate_or_wrong_completion_is_rejected() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"owned locked candidate");
    let wrong = locked_candidate_subject(b"conflicting locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue owned acquisition");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the owned candidate load"),
    };
    let completion_error = service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject: wrong,
            canonical_wire: b"wrong body".to_vec(),
        })
        .expect_err("wrong-subject completion must be rejected");
    assert!(completion_error.contains("different acquisition subject"));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("exact acquisition remains owned");
    assert_eq!(acquisition.subject, subject);
    assert!(matches!(
        &acquisition.state,
        LockedCandidateAcquisitionState::Loading { .. }
    ));
    service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject,
            canonical_wire: b"exact body".to_vec(),
        })
        .expect("complete the exact acquisition");
    let duplicate = service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject,
            canonical_wire: b"exact body".to_vec(),
        })
        .expect_err("duplicate completion must be rejected");
    assert!(duplicate.contains("completed more than once"));
    detach_locked_candidate_io(&mut service);
}
#[test]
fn locked_candidate_future_completion_is_rejected_without_replacing_owner() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"future completion owner");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue owned acquisition");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the owned candidate load"),
    };
    let future_id = LockedCandidateAcquisitionId(
        acquisition_id
            .0
            .checked_add(1)
            .expect("test acquisition ID has a successor"),
    );
    let future = service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id: future_id,
            subject,
            canonical_wire: b"forged future body".to_vec(),
        })
        .expect_err("an unissued future completion must fail closed");
    assert!(future.contains("unknown future acquisition ID"));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("the issued acquisition remains owned");
    assert_eq!(acquisition.subject, subject);
    assert!(matches!(
        acquisition.state,
        LockedCandidateAcquisitionState::Loading {
            acquisition_id: owned,
            subject: owned_subject,
        } if owned == acquisition_id && owned_subject == subject
    ));
    assert!(service.take_loaded_candidate().is_none());
    detach_locked_candidate_io(&mut service);
}
#[test]
fn higher_different_lock_replaces_load_and_retires_stale_completion() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let original = locked_candidate_subject(b"original locked candidate");
    let replacement = locked_candidate_subject(b"higher locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            original,
        )
        .expect("queue original acquisition");
    let original_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        }) if subject == original => acquisition_id,
        _ => panic!("expected original candidate load"),
    };
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 1),
            replacement,
        )
        .expect("a higher lock replaces the desired subject");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: original_id,
                subject: original,
                canonical_wire: b"superseded body".to_vec(),
            })
            .expect("retire superseded physical result"),
        None
    );
    let replacement_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        }) if subject == replacement => acquisition_id,
        _ => panic!("expected one replacement candidate load"),
    };
    assert!(replacement_id > original_id);
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: original_id,
                subject: original,
                canonical_wire: b"late duplicate".to_vec(),
            })
            .expect("late superseded completion is non-fatal"),
        None
    );
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: replacement_id,
                subject: replacement,
                canonical_wire: b"replacement body".to_vec(),
            })
            .expect("complete replacement acquisition"),
        Some(locked_candidate_tag(1))
    );
    let loaded = service
        .take_loaded_candidate()
        .expect("deliver only the higher locked body");
    assert_eq!(loaded.tag(), locked_candidate_tag(1));
    assert_eq!(loaded.round(), locked_candidate_round(&service, 1));
    assert_eq!(loaded.subject(), replacement);
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    detach_locked_candidate_io(&mut service);
}
#[test]
fn superseded_locked_candidate_failure_starts_latest_acquisition() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let original = locked_candidate_subject(b"failing original candidate");
    let replacement = locked_candidate_subject(b"replacement after failure");
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            original,
        )
        .expect("queue original acquisition");
    let original_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        }) if subject == original => acquisition_id,
        _ => panic!("expected original candidate load"),
    };
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 1),
            replacement,
        )
        .expect("install higher same-incarnation lock");
    assert_eq!(
        service
            .locked_candidate_load_failed(
                original_id,
                original,
                "superseded read failure".to_owned(),
            )
            .expect("superseded failure must retire non-fatally"),
        None
    );
    assert!(!service.output_guard.restart_required());
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate { subject, .. }) if subject == replacement
    ));
    detach_locked_candidate_io(&mut service);
}
#[test]
fn unavailable_locked_candidate_waits_for_matching_durable_store() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"not-yet-durable locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue initial acquisition");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected initial candidate load"),
    };
    assert_eq!(
        service
            .locked_candidate_load_unavailable(acquisition_id, subject)
            .expect("local absence is a recoverable state"),
        None
    );
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("waiting acquisition remains owned");
    assert!(matches!(
        &acquisition.state,
        LockedCandidateAcquisitionState::Waiting { .. }
    ));
    assert_eq!(acquisition.pending_count(), 1);
    assert!(!service.output_guard.restart_required());
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("same request coalesces while certified recovery runs");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    service
        .retry_locked_candidate_after_store(locked_candidate_subject(b"unrelated body"))
        .expect("unrelated store cannot steal retry ownership");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    service
        .retry_locked_candidate_after_store(subject)
        .expect("matching durable store requeues exactly once");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate { subject: queued, .. }) if queued == subject
    ));
    detach_locked_candidate_io(&mut service);
}
#[test]
fn unavailable_locked_candidate_rebinds_latest_consumer_before_retry() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"waiting rebound candidate");
    let canonical_wire = b"recovered exact body".to_vec();
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue initial acquisition");
    let initial_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected initial candidate load"),
    };
    service
        .locked_candidate_load_unavailable(initial_id, subject)
        .expect("local absence waits for certified recovery");
    service
        .request_locked_candidate(
            locked_candidate_tag(7),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("same lock rebinds while durable recovery is pending");
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("waiting acquisition remains owned");
    assert_eq!(acquisition.consumer, locked_candidate_tag(7));
    assert!(matches!(
        acquisition.state,
        LockedCandidateAcquisitionState::Waiting {
            acquisition_id,
            subject: waiting_subject,
        } if acquisition_id == initial_id && waiting_subject == subject
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    service
        .retry_locked_candidate_after_store(subject)
        .expect("matching durable store starts one replacement read");
    let retry_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the matching durable retry"),
    };
    assert!(retry_id > initial_id);
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: retry_id,
                subject,
                canonical_wire: canonical_wire.clone(),
            })
            .expect("complete the recovered exact acquisition"),
        Some(locked_candidate_tag(7))
    );
    let loaded = service
        .take_loaded_candidate()
        .expect("deliver recovered bytes only to the latest consumer");
    assert_eq!(loaded.tag(), locked_candidate_tag(7));
    assert_eq!(loaded.round(), locked_candidate_round(&service, 0));
    assert_eq!(loaded.subject(), subject);
    assert_eq!(loaded.into_canonical_wire(), canonical_wire);
    assert!(service.take_loaded_candidate().is_none());
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    detach_locked_candidate_io(&mut service);
}
fn proposal_body_and_payload(
    context: &wire::HeightContext,
    keys: &[KeyPair],
) -> (Vec<u8>, EncodedV2Payload, wire::Proposal) {
    let (canonical_wire, payload) = proposal_body_and_payload_at_view(context, keys, 0);
    let round = payload.manifest().round;
    let proposer = context.leader(round.view);
    let proposal = wire::Proposal {
        round,
        proposer,
        subject: payload.manifest().subject,
        manifest: payload.manifest().clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    (canonical_wire, payload, proposal)
}
fn proposal_body_and_payload_at_view(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
) -> (Vec<u8>, EncodedV2Payload) {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let proposer = context.leader(round.view);
    let proposer_index = usize::try_from(proposer).expect("fixture proposer index");
    // The immutable body was created by the genesis authority in view 0;
    // `view` is the certified round in which that exact body is proposed
    // or reproposed after restart.
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).expect("non-zero fixture height"),
        None,
        None,
        None,
        1_000,
        0,
    );
    let signature = SignatureOf::try_from_hash(keys[proposer_index].private_key(), header.hash())
        .expect("sign fixture block header");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(proposer), signature),
        header,
        Vec::new(),
    );
    let canonical_wire = block.encode_wire().expect("canonical fixture block");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let payload = encode_payload(context, round, subject, &canonical_wire)
        .expect("encode fixture proposal payload");
    (canonical_wire, payload)
}
fn allow_fixture_block_payload(context: &mut wire::HeightContext) {
    context.da_layout = wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 1_024,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 16_384,
        max_chunk_count: 32,
    };
    context.validate().expect("widened fixture context");
}
fn fixture_with_block_payload() -> (ProductionV2Services, Vec<KeyPair>) {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    (service, keys)
}
type ConsensusRouteObservation = (PeerId, wire::ConsensusMessageV2);
fn install_consensus_route_observer(
    service: &mut ProductionV2Services,
) -> Arc<Mutex<Vec<ConsensusRouteObservation>>> {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&observations);
    service.set_exact_output_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let NetworkMessage::SumeragiBlock(envelope) = &post.data else {
            panic!("consensus routing emitted a non-Sumeragi message");
        };
        let BlockMessage::V2(message) = envelope.as_message() else {
            panic!("consensus routing emitted a lane message");
        };
        observed
            .lock()
            .expect("lock consensus route observations")
            .push((post.peer_id, message.clone()));
        Ok(())
    });
    observations
}
fn take_consensus_route_observations(
    observations: &Mutex<Vec<ConsensusRouteObservation>>,
) -> Vec<ConsensusRouteObservation> {
    std::mem::take(
        &mut *observations
            .lock()
            .expect("inspect consensus route observations"),
    )
}
fn proposal_route_targets(
    observations: &[ConsensusRouteObservation],
    round: wire::ConsensusRound,
    manifest: &wire::PayloadManifest,
) -> BTreeSet<PeerId> {
    observations
        .iter()
        .filter_map(|(peer, message)| match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal)
                if proposal.round == round && proposal.manifest == *manifest =>
            {
                Some(peer.clone())
            }
            _ => None,
        })
        .collect()
}
fn chunk_route_targets(
    observations: &[ConsensusRouteObservation],
    manifest: &wire::PayloadManifest,
) -> BTreeSet<PeerId> {
    let manifest_hash = HashOf::new(manifest);
    observations
        .iter()
        .filter_map(|(peer, message)| match &message.payload {
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk)
                if chunk.manifest_hash == manifest_hash =>
            {
                Some(peer.clone())
            }
            _ => None,
        })
        .collect()
}
fn set_local_validator(
    service: &mut ProductionV2Services,
    keys: &[KeyPair],
    validator: wire::ValidatorIndex,
) {
    let index = usize::try_from(validator).expect("fixture validator index");
    service.local_validator = Some(validator);
    service.local_peer = service.context.roster[index].validator.clone();
    service.key_pair = keys[index].clone();
}
fn routing_vote(service: &ProductionV2Services, view: u64, phase: wire::GlobalPhase) -> wire::Vote {
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view,
    };
    wire::Vote {
        round,
        proposal_round: round,
        phase,
        subject: wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"routing vote block")),
            payload_hash: Hash::new(b"routing vote payload"),
        },
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"routing vote parent state"),
            Hash::new(b"routing vote post state"),
            Hash::new(b"routing vote ordinary writes"),
            1,
            Hash::new(b"routing vote executed block wire"),
        ),
        signer: service
            .local_validator
            .expect("routing fixture is a voting validator"),
        signature: vec![0xA5; 48],
    }
}
#[test]
#[allow(clippy::too_many_lines)]
fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound() {
    use super::super::v2_lifecycle_coordinator::{
        RecoveredLifecycleSignClassV1, RecoveredLifecycleSignDispatchIdentityV1,
    };
    let (mut service, keys) = fixture_with_block_payload();
    let directory = TempDir::new().expect("temporary recovered Proposal output store");
    let body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open exact recovered Proposal output store");
    let body_store_identity = body_store.instance_identity();
    let output_guard = ConsensusOutputGuard::isolated();
    let (_, payload, mut proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = service.active_tag;
    let request = super::super::v2::SignRequest::Proposal(proposal.clone());
    let dispatch_key = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        91,
        tag,
        &request,
        RecoveredLifecycleSignClassV1::ControlProposal,
    )
    .expect("mint exact recovered Proposal dispatch identity")
    .key();
    let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
    proposal.signature =
        Signature::new(keys[proposer].private_key(), &proposal.signature_preimage())
            .payload()
            .to_vec();
    set_local_validator(&mut service, &keys, proposal.proposer);
    let service_context = service.context.clone();
    let service_io = install_lifecycle_planner_io_for_validator_for_test(
        &mut service,
        service_context.clone(),
        tag,
        proposal.proposer,
        Arc::clone(&output_guard),
        body_store,
        body_store_identity.clone(),
        1,
    );
    install_local_signer_for_test(&mut service, &keys[proposer]);
    service
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install adversarial one-unit output corridor");
    let authority_context = service_context;
    let authority = |identity: V2BodyStoreInstanceIdentity, guard: Arc<ConsensusOutputGuard>| {
        super::super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1::for_test(
            &authority_context,
            dispatch_key,
            tag,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
            payload.clone(),
            identity,
            guard,
        )
        .expect("fixture Proposal output authority is structurally exact")
    };
    let foreign_directory = TempDir::new().expect("temporary foreign Proposal output store");
    let foreign_identity = V2BodyStore::open(foreign_directory.path(), service.context.clone())
        .expect("open foreign Proposal output store")
        .instance_identity();
    assert!(
        service
            .capture_recovered_lifecycle_proposal_exact_output(authority(
                foreign_identity,
                Arc::clone(&output_guard),
            ))
            .is_err(),
        "a same-context Proposal cannot cross a foreign body-store owner"
    );
    let foreign_guard = ConsensusOutputGuard::isolated();
    assert!(
        service
            .capture_recovered_lifecycle_proposal_exact_output(authority(
                body_store_identity.clone(),
                foreign_guard,
            ))
            .is_err(),
        "a same-store Proposal cannot cross a foreign output guard"
    );
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let blocking_vote = routing_vote(&service, 0, wire::GlobalPhase::Prepare);
    assert_eq!(
        service
            .broadcast_consensus(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(blocking_vote),
            ))
            .expect("retain one exact blocking owner"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    let before = {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect the exact blocking owner");
        (
            pending.fanouts.len(),
            pending.source_fifo_owners.clone(),
            pending.reservation_owner_counts.clone(),
            pending.ownership_units,
            pending.shared_ownership_units,
            pending.next_fanout_fifo_id,
        )
    };
    let expected_batch_first_fifo = before.5;
    let retry_authority = match service
        .capture_recovered_lifecycle_proposal_exact_output(authority(
            body_store_identity.clone(),
            Arc::clone(&output_guard),
        ))
        .expect("capacity pressure is a typed Proposal retry")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority) => authority,
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(_) => {
            panic!("aggregate Proposal demand must not fit behind the blocking owner")
        }
    };
    let after = {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect unchanged pressured corridor");
        (
            pending.fanouts.len(),
            pending.source_fifo_owners.clone(),
            pending.reservation_owner_counts.clone(),
            pending.ownership_units,
            pending.shared_ownership_units,
            pending.next_fanout_fifo_id,
        )
    };
    assert_eq!(
        after, before,
        "capacity failure cannot install either fanout"
    );
    assert!(service.fast_path_proposals.is_empty());
    assert!(!service.output_guard.restart_required());
    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("drain the exact blocking owner")
    );
    let retry_authority = match service
        .capture_recovered_lifecycle_proposal_exact_output(retry_authority)
        .expect("the unchanged Proposal authority reserves after capacity recovers")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(reservation) => {
            reservation.abort_before_publication()
        }
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty corridor must reserve the aggregate Proposal output")
        }
    };
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("typed abort leaves the corridor empty");
        assert!(pending.fanouts.is_empty());
        assert!(pending.source_fifo_owners.is_empty());
        assert!(pending.reservation_owner_counts.is_empty());
    }
    match service
        .capture_recovered_lifecycle_proposal_exact_output(retry_authority)
        .expect("retry the exact authority after typed abort")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(reservation) => {
            reservation.commit_after_publication();
        }
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty corridor must retain the complete Proposal batch")
        }
    }
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect committed atomic Proposal batch");
        let expected_peers = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
        assert_eq!(pending.fanouts.len(), 2);
        assert!(matches!(
            &pending.fanouts[0].rollover_claim,
            ExactOutputRolloverClaim::GlobalV2(_)
        ));
        assert!(matches!(
            &pending.fanouts[1].rollover_claim,
            ExactOutputRolloverClaim::PayloadChunks { .. }
        ));
        assert_eq!(
            pending
                .fanouts
                .iter()
                .map(|fanout| fanout.fifo_id)
                .collect::<Vec<_>>(),
            vec![
                Some(expected_batch_first_fifo),
                expected_batch_first_fifo.checked_add(1),
            ],
            "the inseparable control/chunk pair owns adjacent FIFO identities"
        );
        for fanout in &pending.fanouts {
            assert_eq!(
                fanout.peers.iter().cloned().collect::<BTreeSet<_>>(),
                expected_peers,
                "restart Proposal dissemination targets every remote voter"
            );
        }
        let control = &pending.fanouts[0];
        let chunks = &pending.fanouts[1];
        let [NetworkMessage::SumeragiBlock(control)] = control.messages.as_slice() else {
            panic!("the first fanout must retain one Proposal control")
        };
        assert!(matches!(
            control.as_message(),
            BlockMessage::V2(message)
                if message.payload
                    == wire::ConsensusMessageV2Payload::Proposal(proposal.clone())
        ));
        let manifest = payload.manifest();
        let signer = &service.context.roster[proposer].validator;
        let mut observed_chunk_indices = BTreeSet::new();
        for encoded in &chunks.messages {
            let NetworkMessage::SumeragiBlock(envelope) = encoded else {
                panic!("the second fanout must retain only payload chunks")
            };
            let BlockMessage::V2(message) = envelope.as_message() else {
                panic!("the recovered Proposal chunk changed protocol lane")
            };
            let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload else {
                panic!("the recovered Proposal chunk fanout mixed message classes")
            };
            chunk
                .validate(&service.context, manifest)
                .expect("the retained chunk matches its canonical manifest");
            let signature = Signature::try_from_bytes(&chunk.signature)
                .expect("the retained chunk signature is canonical");
            signature
                .verify(
                    signer.public_key(),
                    &chunk
                        .signature_preimage(&service.context, manifest)
                        .expect("the retained chunk has a canonical signature preimage"),
                )
                .expect("the retained chunk is signed by the recovered proposer");
            assert!(observed_chunk_indices.insert(chunk.index));
        }
        assert_eq!(
            observed_chunk_indices.len(),
            manifest.chunk_hashes.len(),
            "the exact batch retains every canonical chunk once"
        );
        assert_eq!(
            pending.ownership_units,
            pending.reservation_owner_counts.values().sum::<usize>()
        );
        assert!(!pending.source_fifo_owners.is_empty());
    }
    assert!(
        service.fast_path_proposals.is_empty(),
        "recovered restart dissemination deliberately targets all voters without mutating live fast-path state"
    );
    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("drain both committed Proposal fanouts")
    );
    let retirement_authority = match service
        .capture_recovered_lifecycle_proposal_exact_output(authority(
            body_store_identity,
            Arc::clone(&output_guard),
        ))
        .expect("reserve one final exact Proposal before Decision")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(reservation) => {
            reservation.abort_before_publication()
        }
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty corridor must reserve the pre-Decision Proposal")
        }
    };
    service
        .retire_candidate_work_after_decision(proposal.round, proposal.subject)
        .expect("retire Proposal work at the durable Decision boundary");
    assert!(
        service
            .capture_recovered_lifecycle_proposal_exact_output(retirement_authority)
            .is_err(),
        "a pre-Decision Proposal authority cannot cross candidate retirement"
    );
    assert!(!service.output_guard.restart_required());
    service_io.detach(&mut service);
}
#[test]
fn prepare_and_commit_votes_reach_every_remote_voter_across_views() {
    let (mut service, _) = fixture();
    let observations = install_consensus_route_observer(&mut service);
    let roster_len = u64::try_from(service.context.roster.len()).expect("fixture roster length");
    let expected = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    for view in 0..roster_len {
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view,
        };
        for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
            let vote = routing_vote(&service, view, phase);
            service
                .broadcast_consensus(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(vote),
                ))
                .expect("route phase vote to every remote voter");
            let routed = take_consensus_route_observations(&observations);
            let targets = routed
                .iter()
                .filter_map(|(peer, message)| match &message.payload {
                    wire::ConsensusMessageV2Payload::Vote(vote)
                        if vote.round == round && vote.phase == phase =>
                    {
                        Some(peer.clone())
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                targets, expected,
                "phase vote fanout differs in view {view}"
            );
            assert_eq!(routed.len(), expected.len());
        }
    }
}
#[test]
fn first_proposal_routes_manifest_control_to_all_and_chunks_to_set_a() {
    let (mut service, keys) = fixture_with_block_payload();
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    set_local_validator(&mut service, &keys, proposal.proposer);
    let manifest = payload.manifest().clone();
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("retain first proposal chunks");
    let committee = service
        .committee_for_round(proposal.round)
        .expect("project first proposal committee");
    let expected_control = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    let expected_chunks = service
        .remote_voters_for_indices(committee.set_a())
        .expect("resolve first proposal Set A")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let observations = install_consensus_route_observer(&mut service);
    service
        .broadcast_consensus(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        ))
        .expect("broadcast first proposal");
    let routed = take_consensus_route_observations(&observations);
    assert_eq!(
        proposal_route_targets(&routed, proposal.round, &manifest),
        expected_control
    );
    assert_eq!(chunk_route_targets(&routed, &manifest), expected_chunks);
    assert!(routed.iter().all(|(_, message)| matches!(
        &message.payload,
        wire::ConsensusMessageV2Payload::Proposal(routed)
            if routed.manifest == manifest
    ) || matches!(
        &message.payload,
        wire::ConsensusMessageV2Payload::PayloadChunk(_)
    )));
}
#[test]
fn same_round_proposal_retransmission_expands_chunks_to_set_b_and_all_voters() {
    let (mut service, keys) = fixture_with_block_payload();
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    set_local_validator(&mut service, &keys, proposal.proposer);
    let manifest = payload.manifest().clone();
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("retain proposal chunks");
    let committee = service
        .committee_for_round(proposal.round)
        .expect("project proposal committee");
    let expected_fast = service
        .remote_voters_for_indices(committee.set_a())
        .expect("resolve Set A")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_set_b = service
        .remote_voters_for_indices(committee.set_b())
        .expect("resolve Set B")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_all = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    assert!(!expected_set_b.is_empty());
    let observations = install_consensus_route_observer(&mut service);
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal.clone()));
    service
        .broadcast_consensus(message.clone())
        .expect("broadcast first proposal occurrence");
    let first = take_consensus_route_observations(&observations);
    assert_eq!(chunk_route_targets(&first, &manifest), expected_fast);
    service
        .broadcast_consensus(message)
        .expect("broadcast same-round proposal retransmission");
    let retransmission = take_consensus_route_observations(&observations);
    let retransmitted_chunks = chunk_route_targets(&retransmission, &manifest);
    assert_eq!(retransmitted_chunks, expected_all);
    assert!(expected_set_b.is_subset(&retransmitted_chunks));
    assert_eq!(
        proposal_route_targets(&retransmission, proposal.round, &manifest),
        expected_all
    );
}
#[test]
fn proposal_broadcast_reports_source_retained_until_corridor_acceptance() {
    let (mut service, keys) = fixture_with_block_payload();
    service
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install one-unit adversarial output corridor");
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let blocking_vote = routing_vote(&service, 0, wire::GlobalPhase::Prepare);
    assert_eq!(
        service
            .broadcast_consensus(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(blocking_vote),
            ))
            .expect("the first control transfers into the exact corridor"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    assert!(
        service
            .has_pending_exact_output()
            .expect("inspect actor-backpressured control")
    );
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    set_local_validator(&mut service, &keys, proposal.proposer);
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("retain proposal chunks before broadcast");
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal.clone()));
    assert_eq!(
        service
            .broadcast_consensus(message.clone())
            .expect("corridor pressure is a typed ownership disposition"),
        ConsensusBroadcastDisposition::SourceRetained,
        "a full same-class corridor must not masquerade as Proposal acceptance"
    );
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect atomic Proposal rejection");
        assert_eq!(
            pending.fanouts.len(),
            1,
            "capacity pressure must retain only the pre-existing owner"
        );
        assert!(pending.fanouts.iter().all(|fanout| !matches!(
            &fanout.rollover_claim,
            ExactOutputRolloverClaim::PayloadChunks { .. }
        )));
    }
    assert!(
        !service.fast_path_proposals.contains(&proposal.round),
        "failed aggregate admission cannot consume the first-send marker"
    );
    assert!(!service.output_guard.restart_required());
    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("network recovery drains the previously accepted exact suffix")
    );
    assert_eq!(
        service
            .broadcast_consensus(message)
            .expect("the retained Proposal source retries after corridor recovery"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    assert!(
        !service
            .has_pending_exact_output()
            .expect("accepted retransmission drains immediately")
    );
    assert!(service.fast_path_proposals.contains(&proposal.round));
    assert!(!service.output_guard.restart_required());
}
#[test]
fn certified_view_transition_resets_fast_path_before_new_set_a_fanout() {
    let (mut service, keys) = fixture_with_block_payload();
    let old_round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: service.active_tag.view(),
    };
    assert!(service.fast_path_proposals.insert(old_round));
    let new_tag = EventTag::new(
        service.active_tag.height(),
        service.active_tag.view() + 1,
        Generation::new(service.active_tag.generation().get() + 1),
    );
    service
        .entered_view(
            new_tag,
            timeout_certificate_at_view(&service, old_round.view),
        )
        .expect("install certified successor view");
    assert!(service.fast_path_proposals.is_empty());
    let (_, payload) = proposal_body_and_payload_at_view(&service.context, &keys, new_tag.view());
    let manifest = payload.manifest().clone();
    let proposal = wire::Proposal {
        round: manifest.round,
        proposer: service.context.leader(manifest.round.view),
        subject: manifest.subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: timeout_certificate_at_view(&service, old_round.view),
            highest_prepare_qc: None,
        }),
        signature: vec![0xA5; 48],
    };
    set_local_validator(&mut service, &keys, proposal.proposer);
    service
        .register_outbound_payload(new_tag, payload)
        .expect("retain new-view proposal chunks");
    let committee = service
        .committee_for_round(proposal.round)
        .expect("project new-view committee");
    let expected_set_a = service
        .remote_voters_for_indices(committee.set_a())
        .expect("resolve new-view Set A")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_all = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    assert_ne!(expected_set_a, expected_all);
    let observations = install_consensus_route_observer(&mut service);
    service
        .broadcast_consensus(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        ))
        .expect("broadcast first proposal in certified successor view");
    let routed = take_consensus_route_observations(&observations);
    assert_eq!(chunk_route_targets(&routed, &manifest), expected_set_a);
    assert_eq!(
        proposal_route_targets(&routed, proposal.round, &manifest),
        expected_all
    );
    assert_eq!(
        service.fast_path_proposals,
        BTreeSet::from([proposal.round])
    );
}
fn install_temporary_chunk_root(service: &mut ProductionV2Services) -> TempDir {
    let directory = TempDir::new().expect("temporary chunk root");
    service.chunk_root = directory.path().to_path_buf();
    directory
}
fn certified_fetch_task(
    service: &ProductionV2Services,
    id: u64,
    tag: EventTag,
    manifest: Option<wire::PayloadManifest>,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> BodyFetchTask {
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"fetch fixture parent state"),
            Hash::new(b"fetch fixture post state"),
            Hash::new(b"fetch fixture writes"),
            1,
            Hash::new(b"fetch fixture block"),
        ),
        signers: vec![0],
        aggregate_signature: vec![1],
    };
    let request = wire::CertifiedBodyRequest {
        round,
        subject,
        certificate,
        requester: service.local_peer.clone(),
        signature: vec![1],
    };
    BodyFetchTask::certified_for_test(id, tag, manifest, vec![service.local_peer.clone()], request)
}
#[test]
fn certified_fetch_fans_out_to_every_frozen_roster_archive() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let request = certified_fetch_task(&service, 62, tag, None, proposal.round, proposal.subject)
        .certified_request()
        .expect("fixture certified request")
        .clone();
    assert_eq!(request.certificate.signers, vec![0]);
    let sources = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let expected_targets = sources
        .iter()
        .filter(|peer| *peer != &service.local_peer)
        .cloned()
        .collect::<Vec<_>>();
    let task = BodyFetchTask::certified_for_test(62, tag, None, sources.clone(), request);
    let admitted = Arc::new(Mutex::new(Vec::new()));
    let admitted_for_hook = Arc::clone(&admitted);
    service.set_exact_output_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let NetworkMessage::SumeragiBlock(envelope) = &post.data else {
            panic!("certified archive fanout emitted a non-Sumeragi message")
        };
        assert!(matches!(
            envelope.as_message(),
            BlockMessage::V2(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                )
        ));
        admitted_for_hook
            .lock()
            .expect("record frozen-roster archive target")
            .push(post.peer_id);
        Ok(())
    });
    service
        .enqueue_body_fetch(task.clone())
        .expect("fan out one certified request to every remote archive");
    assert_eq!(task.sources(), sources.as_slice());
    assert_eq!(
        admitted
            .lock()
            .expect("inspect frozen-roster archive targets")
            .as_slice(),
        expected_targets.as_slice()
    );
    assert_eq!(
        expected_targets,
        service.context.roster[1..]
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>(),
        "every remote fixture archive is intentionally outside the one-signer QC"
    );
}
#[test]
fn replayed_proposal_signature_restores_exact_durable_payload() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let directory = TempDir::new().expect("temporary body store");
    let mut body_store =
        V2BodyStore::open(directory.path(), service.context.clone()).expect("open body store");
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let _receipt = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("store exact proposal body");
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
    let task =
        ConsensusSignTask::for_test(7, tag, super::super::v2::SignRequest::Proposal(proposal));
    let expected_work_id = task.id();
    let completion =
        sign_consensus_task(&body_store, &service.context, &keys[proposer], task, true)
            .expect("sign replayed proposal");
    let V2IoCompletion::Signature {
        work_id,
        signature,
        outbound_payload: Some(restored),
    } = completion
    else {
        panic!("proposal replay must restore its outbound payload");
    };
    assert_eq!(work_id, expected_work_id);
    assert!(!signature.is_empty());
    assert_eq!(restored, payload);
}
include!("v2_worker_nonzero_view_restart.rs");
#[test]
fn replayed_proposal_signature_rejects_missing_durable_payload() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let directory = TempDir::new().expect("temporary body store");
    let body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open empty body store");
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
    let error = match sign_consensus_task(
        &body_store,
        &service.context,
        &keys[proposer],
        ConsensusSignTask::for_test(8, tag, super::super::v2::SignRequest::Proposal(proposal)),
        true,
    ) {
        Ok(_) => panic!("missing durable proposal body must fail closed"),
        Err(error) => error,
    };
    assert!(error.contains("no durable exact body"));
}
