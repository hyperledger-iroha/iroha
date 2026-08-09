// Upstream reply-route supersession regression retained through the merge.

#[test]
fn delayed_block_sync_reply_route_is_superseded_without_poisoning_output() {
    let history = durable_history_fixture();
    let mut service = successor_service_for_history_as(
        Arc::clone(&history.kura),
        &history.artifact,
        &history.validators,
        3,
    );
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });

    let request = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateRequest(wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            chain_id: history.artifact.height_context.chain_id.clone(),
            context_id: history.artifact.context_id(),
            height: history.artifact.height,
            requester: history.requester.clone(),
            signature: vec![0xA5],
        }),
    ));
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 1);
    let old_route = route_fixture.mint_via(history.requester.clone(), hub.clone());
    let current_route = route_fixture.mint_via(history.requester.clone(), hub.clone());
    let delayed_old_route = route_fixture
        .redeliver(&old_route)
        .expect("the retired connection can deliver one delayed occurrence");
    assert_eq!(
        delayed_old_route.source_update_from(&current_route),
        Err(NetworkReplyRouteError::Stale)
    );
    let (current_routes, current_ownership) = fair_ingress_route_owner(
        request.clone(),
        history.requester.clone(),
        hub.clone(),
        current_route.clone(),
    );
    let (delayed_routes, delayed_ownership) =
        fair_ingress_route_owner(request, history.requester.clone(), hub, delayed_old_route);

    let guard = Arc::clone(&service.output_guard);
    let operation = guard
        .begin_fail_stop_operation()
        .expect("current block-sync response owns a guarded operation");
    service
        .post_durable_history_response_on_reply_routes_with_permit(
            history.requester.clone(),
            current_routes,
            current_ownership,
            history.commit_response.clone(),
            operation.permit(),
        )
        .expect("current block-sync response enters exact output");
    operation.complete();

    let (fifo_before, reservations_before, source_fifo_before) = {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect retained current response");
        assert_eq!(pending.fanouts.len(), 1);
        let fanout = &pending.fanouts[0];
        assert!(matches!(
            &fanout.targets[0].route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&current_route)
        ));
        (
            fanout.fifo_id,
            pending.reservation_owner_counts.clone(),
            pending.source_fifo_owners.clone(),
        )
    };

    let operation = guard
        .begin_fail_stop_operation()
        .expect("delayed replay must not inherit a poisoned guard");
    service
        .post_durable_history_response_on_reply_routes_with_permit(
            history.requester.clone(),
            delayed_routes,
            delayed_ownership,
            history.commit_response.clone(),
            operation.permit(),
        )
        .expect("delayed authenticated replay is consumed as superseded");
    operation.complete();

    assert!(!guard.restart_required());
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect response after delayed replay");
    assert_eq!(pending.fanouts.len(), 1);
    let fanout = &pending.fanouts[0];
    assert_eq!(fanout.fifo_id, fifo_before);
    assert!(matches!(
        &fanout.targets[0].route,
        ExactTargetRoute::Reply(route) if route.same_delivery(&current_route)
    ));
    assert_eq!(pending.reservation_owner_counts, reservations_before);
    assert_eq!(pending.source_fifo_owners, source_fifo_before);
    assert_eq!(
        fanout
            .ingress_ownership
            .as_ref()
            .expect("both block-sync admissions retain bounded history")
            .admission_count,
        2
    );
}
