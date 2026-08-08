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
