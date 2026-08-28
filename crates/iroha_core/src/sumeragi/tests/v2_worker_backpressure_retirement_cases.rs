#[test]
fn applied_height_finality_releases_only_ticketless_global_topology_target() {
    let (service, keys) = fixture();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let removed = service.context.roster[1].validator.clone();
    let responsive = service.context.roster[2].validator.clone();
    let vote = routing_vote(&service, 0, wire::GlobalPhase::Commit);
    let message = ProductionV2Services::preencode_v2_network_message(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
    )
    .expect("encode exact GlobalV2 vote");
    let mut pending = PendingExactOutput::new(2, 1, 2, &[removed.clone(), responsive.clone()])
        .expect("two frozen validator targets fit");
    assert_eq!(
        pending
            .enqueue(
                PendingExactFanout::claimed(
                    vec![message.clone()],
                    vec![removed.clone(), responsive.clone()],
                    ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
                )
                .expect("valid GlobalV2 fanout")
                .expect("non-empty GlobalV2 fanout"),
            )
            .expect("retain both frozen targets"),
        ExactFanoutOwnership::Owned
    );

    let mut responsive_admissions = 0usize;
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            if post.peer_id == removed {
                return Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket: None,
                    rank: 1,
                });
            }
            assert_eq!(post.peer_id, responsive);
            responsive_admissions += 1;
            Ok(ExactOutputAttemptOutcome::Admitted)
        },),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 1 })
    );
    assert_eq!(responsive_admissions, 1);
    assert_eq!(pending.fanouts.len(), 1);
    assert_eq!(pending.fanouts[0].targets[0].message_index, 0);
    assert_eq!(pending.fanouts[0].targets[1].message_index, 1);

    pending.applied_height_finality = Some(artifact.clone());
    assert_eq!(
        pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
            assert_eq!(post.peer_id, removed);
            assert!(matches!(route, ExactTargetRoute::Topology));
            assert!(ticket.is_none());
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 1,
            })
        },),
        Ok(ExactOutputDriveOutcome::Drained)
    );
    assert!(!pending.is_pending());

    let mut manual =
        PendingExactOutput::new(1, 1, 1, &[removed.clone()]).expect("one manual target fits");
    manual.applied_height_finality = Some(artifact);
    manual
        .enqueue(PendingExactFanout::new(vec![message], vec![removed.clone()]).expect("fanout"))
        .expect("retain manual fanout");
    assert_eq!(
        manual.drive_with_budget_ack(usize::MAX, |post, _ticket, _route, _timeout_attempt| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket: None,
                rank: 7,
            })
        },),
        Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 7 })
    );
    assert!(manual.is_pending(), "manual output retains exact authority");
}

#[test]
fn closed_flush_racing_final_receiver_retirement_is_nonfatal() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let message = merge_share_message(b"closed reply retirement race");
    let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
    let route = routes.mint(peer.clone());
    let mut pending =
        PendingExactOutput::new(1, 1, 1, &[]).expect("one retirement-race reply attempt fits");
    pending
        .enqueue(
            PendingExactFanout::new_with_routes(
                vec![message],
                vec![peer],
                vec![ExactTargetRoute::Reply(route.clone())],
            )
            .expect("one retirement-race reply fanout"),
        )
        .expect("retain the retirement-race reply");
    let mut control = None;
    pending
        .drive_with_budget_ack(usize::MAX, |post, _ticket, attempted, _timeout_attempt| {
            let ExactTargetRoute::Reply(attempted) = attempted else {
                panic!("retirement-race response changed route kind")
            };
            let (writer, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, attempted);
            control = Some(writer);
            Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
        })
        .expect("queue the retirement-race writer flush");
    assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
    assert!(control.as_mut().expect("writer controller").close());
    assert!(routes.retire(&route));
    pending
        .poll_reply_flushes()
        .expect("final receiver retirement after Closed must not fail stop output");
    let target = &pending.fanouts[0].targets[0];
    assert_eq!(target.message_index, 0);
    assert!(target.current.is_none());
    assert!(target.pending_flush.is_none());
    assert!(target.parked);
    assert!(!pending.source_fifo_owners.is_empty());
}
