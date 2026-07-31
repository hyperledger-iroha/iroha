// Backpressure and exact-output scheduling worker regression tests.
// Included lexically by v2_worker::tests to preserve canonical test names.

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

    #[test]
    fn unavailable_admission_racing_retirement_is_nonfatal() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let message = merge_share_message(b"unavailable reply retirement race");
        let mut routes = NetworkReplyRouteTestFixture::new(peer.clone());
        let route = routes.mint(peer.clone());
        let mut pending =
            PendingExactOutput::new(1, 1, 1, &[]).expect("one unavailable-race reply attempt fits");
        pending
            .enqueue(
                PendingExactFanout::new_with_routes(
                    vec![message],
                    vec![peer],
                    vec![ExactTargetRoute::Reply(route.clone())],
                )
                .expect("one unavailable-race reply fanout"),
            )
            .expect("retain the unavailable-race reply");

        assert_eq!(
            pending.drive_with_budget_ack(
                usize::MAX,
                |_post, _ticket, attempted, _timeout_attempt| {
                    let ExactTargetRoute::Reply(attempted) = attempted else {
                        panic!("unavailable-race response changed route kind")
                    };
                    assert!(attempted.same_delivery(&route));
                    assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
                    assert!(routes.retire(&route));
                    Ok(ExactOutputAttemptOutcome::Unavailable)
                }
            ),
            Ok(ExactOutputDriveOutcome::Drained)
        );
        let target = &pending.fanouts[0].targets[0];
        assert_eq!(target.message_index, 0);
        assert!(target.current.is_none());
        assert!(target.parked);
        assert!(!pending.source_fifo_owners.is_empty());
    }

    #[test]
    fn ordinary_reply_late_old_flush_after_reconnect_advances_exactly_once() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let messages = vec![
            merge_share_message(b"ordinary late flush first"),
            merge_share_message(b"ordinary late flush second"),
        ];
        let first_hash = HashOf::new(&messages[0]);
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 1);
        let old_route = routes.mint_via(peer.clone(), hub.clone());
        let mut pending =
            PendingExactOutput::new(1, 2, 1, &[]).expect("one two-message reply attempt fits");
        assert_eq!(
            pending.enqueue_owned_reply_transfer(
                PendingExactFanout::new_with_reply_routes(
                    messages.clone(),
                    peer.clone(),
                    NetworkReplyRoutes::try_from_route(old_route.clone())
                        .expect("old source route set"),
                )
                .expect("two-message ordinary reply fanout"),
            ),
            Ok(ExactFanoutOwnership::Owned)
        );

        let mut old_control = None;
        let mut cloned_completion_identity = None;
        assert_eq!(
            pending.drive_with_budget_ack(1, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                assert_eq!(HashOf::new(&post.data), first_hash);
                let ExactTargetRoute::Reply(route) = route else {
                    panic!("ordinary reply changed route kind")
                };
                assert!(route.same_delivery(&old_route));
                let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
                cloned_completion_identity = Some(ack.identity().clone());
                old_control = Some(control);
                Ok(ExactOutputAttemptOutcome::ReplyFlush(ack))
            }),
            Ok(ExactOutputDriveOutcome::BudgetExhausted {
                closest_backpressure_rank: None,
            })
        );
        assert_eq!(pending.fanouts[0].targets[0].message_index, 0);
        assert!(pending.fanouts[0].targets[0].pending_flush.is_some());

        assert!(routes.retire(&old_route));
        let replacement_route = routes.mint_via(peer.clone(), hub);
        assert_eq!(
            pending.enqueue_owned_reply_transfer(
                PendingExactFanout::new_with_reply_routes(
                    messages,
                    peer,
                    NetworkReplyRoutes::try_from_route(replacement_route.clone())
                        .expect("replacement source route set"),
                )
                .expect("replacement route retry candidate"),
            ),
            Ok(ExactFanoutOwnership::Owned)
        );
        let target = &pending.fanouts[0].targets[0];
        assert_eq!(target.message_index, 0);
        assert!(target.pending_flush.is_some());
        assert!(matches!(
            &target.route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&replacement_route)
        ));

        assert!(
            old_control
                .as_mut()
                .expect("old writer retains its sole completion controller")
                .flush()
        );
        pending
            .poll_reply_flushes()
            .expect("late old flush retains canonical source authority");
        let target = &pending.fanouts[0].targets[0];
        assert_eq!(target.message_index, 1);
        assert!(target.pending_flush.is_none());
        assert_eq!(pending.ownership_units, 1);
        assert!(
            !cloned_completion_identity
                .as_ref()
                .expect("retain clone-shared completion identity")
                .claim_writer_flush_once(),
            "a clone of the consumed writer occurrence cannot advance another cursor"
        );

        pending
            .poll_reply_flushes()
            .expect("polling after terminal ownership consumption is idempotent");
        assert_eq!(pending.fanouts[0].targets[0].message_index, 1);
        assert_eq!(pending.ownership_units, 1);
    }

    #[test]
    fn closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &peer);
        let chunk = match &chunk_message {
            CertifiedMergeSidecarMessage::Chunk(chunk) => chunk.clone(),
            CertifiedMergeSidecarMessage::Request(_)
            | CertifiedMergeSidecarMessage::Close(_)
            | CertifiedMergeSidecarMessage::CloseAck(_)
            | CertifiedMergeSidecarMessage::GenerationHint(_) => {
                unreachable!("sidecar fixture returns one response chunk")
            }
        };
        let rollover_claim = ExactOutputRolloverClaim::CertifiedSidecarChunk {
            scope: service.exact_output_scope(),
            target: peer.clone(),
            transfer: CertifiedSidecarTransferIdentity::from_chunk(&chunk),
            chunk_index: chunk.chunk_index,
            chunk_count: chunk.chunk_count,
            response_hash: HashOf::new(&chunk),
        };
        let shared_payload = Arc::new(chunk_message);
        let message = NetworkMessage::CertifiedMergeSidecar(Arc::clone(&shared_payload));
        let response_class = exact_output_class(&message).expect("classify sidecar response");
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
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone()).expect("source B route set"),
            )
            .expect("retain both authenticated response sources");

        let mut pending =
            PendingExactOutput::new(2, 1, 2, &[]).expect("two-source sidecar response corridor");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::claimed_with_reply_routes(
                        vec![message.clone()],
                        peer.clone(),
                        reply_routes,
                        rollover_claim.clone(),
                    )
                    .expect("valid two-source sidecar claim")
                    .expect("two-source sidecar fanout"),
                )
                .expect("retain both sidecar sources"),
            ExactFanoutOwnership::Owned
        );
        let fifo_id = pending.fanouts[0]
            .fifo_id
            .expect("sidecar fanout owns stable FIFO age");
        let (mut source_a_flush_control, source_a_flush_ack, _source_a_admission) =
            certified_sidecar_flush_fixture(&chunk, &route_a);
        let mut source_a_flush_ack = Some(source_a_flush_ack);
        assert_eq!(
            pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
                let NetworkMessage::CertifiedMergeSidecar(payload) = &post.data else {
                    panic!("sidecar fanout reconstructed another network payload")
                };
                assert!(
                    Arc::ptr_eq(payload, &shared_payload),
                    "source A and B must retain the worker's immutable payload carrier"
                );
                if matches!(route, ExactTargetRoute::Reply(route) if route.same_source(&route_a)) {
                    assert!(ticket.is_none());
                    return Ok(ExactOutputAttemptOutcome::SidecarFlush(
                        source_a_flush_ack
                            .take()
                            .expect("source A sidecar chunk is handed to one writer"),
                    ));
                }
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_source(&route_b)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 17,
                })
            }),
            Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 17 })
        );
        assert!(source_a_flush_ack.is_none());
        assert_eq!(pending.pending_sidecar_flushes(), 1);
        assert!(pending.admitted_sidecar_chunks.is_empty());
        assert!(source_a_flush_control.close());
        pending
            .poll_reply_flushes()
            .expect("closed exact writer identity remains well formed");
        assert_eq!(pending.pending_sidecar_flushes(), 0);
        assert!(
            pending.admitted_sidecar_chunks.is_empty(),
            "a closed writer without Flushed must not advance the sidecar cursor"
        );
        let a_index = pending.fanouts[0]
            .targets
            .iter()
            .position(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(&route_a))
            })
            .expect("source A target retains the current item");
        let b_index = pending.fanouts[0]
            .targets
            .iter()
            .position(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(&route_b))
            })
            .expect("source B target remains backpressured");
        assert_eq!(pending.fanouts[0].targets[a_index].message_index, 0);
        assert!(pending.fanouts[0].targets[a_index].current.is_none());
        assert!(pending.fanouts[0].targets[a_index].pending_flush.is_none());
        assert!(!pending.fanouts[0].target_is_complete(a_index));
        assert_eq!(pending.fanouts[0].targets[b_index].message_index, 0);
        assert!(pending.fanouts[0].targets[b_index].current.is_some());
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 2);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );

        let exact_duplicate = NetworkReplyRoutes::try_from_route(route_a.clone())
            .expect("exact pending-source duplicate");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::claimed_with_reply_routes(
                        vec![message.clone()],
                        peer.clone(),
                        exact_duplicate,
                        rollover_claim.clone(),
                    )
                    .expect("valid exact pending-source claim")
                    .expect("exact pending-source retry"),
                )
                .expect("exact duplicate coalesces without resetting the cursor"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets[a_index].message_index, 0);

        let later_a = routes
            .redeliver(&route_a)
            .expect("later delivery on the same source tenure");
        let later_delivery = NetworkReplyRoutes::try_from_route(later_a.clone())
            .expect("later pending-source delivery");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::claimed_with_reply_routes(
                        vec![message.clone()],
                        peer.clone(),
                        later_delivery,
                        rollover_claim.clone(),
                    )
                    .expect("valid later pending-source claim")
                    .expect("later pending-source retry"),
                )
                .expect("later delivery updates without resetting the cursor"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets[a_index].message_index, 0);
        assert!(matches!(
            &pending.fanouts[0].targets[a_index].route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&later_a)
        ));
        assert_eq!(pending.ownership_units, 2);

        assert!(routes.retire(&later_a));
        let reconnected_a = routes.mint_via(peer.clone(), hub_a.clone());
        let reconnect = NetworkReplyRoutes::try_from_route(reconnected_a.clone())
            .expect("same-source reconnect route set");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::claimed_with_reply_routes(
                        vec![message],
                        peer.clone(),
                        reconnect,
                        rollover_claim,
                    )
                    .expect("valid same-source reconnect claim")
                    .expect("same-source reconnect fanout"),
                )
                .expect("closed source reconnect preserves the current item"),
            ExactFanoutOwnership::Owned
        );
        let fanout = &pending.fanouts[0];
        let [NetworkMessage::CertifiedMergeSidecar(reconnected_payload)] =
            fanout.messages.as_slice()
        else {
            panic!("reconnected sidecar fanout changed payload kind")
        };
        assert!(
            Arc::ptr_eq(reconnected_payload, &shared_payload),
            "same-source reconnect must reuse the worker's current payload carrier"
        );
        assert_eq!(fanout.fifo_id, Some(fifo_id));
        assert_eq!(fanout.targets[a_index].message_index, 0);
        assert!(fanout.targets[a_index].current.is_none());
        assert!(fanout.targets[a_index].ticket.is_none());
        assert!(!fanout.target_is_complete(a_index));
        assert!(matches!(
            &fanout.targets[a_index].route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&reconnected_a)
        ));
        assert_eq!(fanout.targets[b_index].message_index, 0);
        assert!(fanout.targets[b_index].current.is_some());
        assert!(matches!(
            &fanout.targets[b_index].route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&route_b)
        ));
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 2);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );

        assert!(routes.retire(&reconnected_a));
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                let NetworkMessage::CertifiedMergeSidecar(payload) = &post.data else {
                    panic!("sidecar retry reconstructed another network payload")
                };
                assert!(Arc::ptr_eq(payload, &shared_payload));
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_source(&route_b)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 13,
                })
            }),
            Ok(Some(13))
        );
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 2);
        assert!(pending.fanouts[0].targets[a_index].parked);
        assert_eq!(pending.fanouts[0].targets[a_index].message_index, 0);
        let [NetworkMessage::CertifiedMergeSidecar(retry_payload)] =
            pending.fanouts[0].messages.as_slice()
        else {
            panic!("second reconnect changed sidecar payload kind")
        };
        assert!(Arc::ptr_eq(retry_payload, &shared_payload));
        assert!(!pending.fanouts[0].target_is_complete(a_index));
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );

        let second_reconnect_a = routes.mint_via(peer.clone(), hub_a);
        let retry_routes = NetworkReplyRoutes::try_from_route(second_reconnect_a.clone())
            .expect("second source A reconnect route set");
        let retry_messages = pending.fanouts[0].messages.clone();
        let retry_claim = pending.fanouts[0].rollover_claim.clone();
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::claimed_with_reply_routes(
                        retry_messages,
                        peer,
                        retry_routes,
                        retry_claim,
                    )
                    .expect("valid second source A reconnect claim")
                    .expect("second source A reconnect fanout"),
                )
                .expect("second reconnect restores source A's retained current item"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets[a_index].message_index, 0);
        assert!(!pending.fanouts[0].targets[a_index].parked);
        assert!(!pending.fanouts[0].target_is_complete(a_index));
        assert!(matches!(
            &pending.fanouts[0].targets[a_index].route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&second_reconnect_a)
        ));
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 2);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );
    }

    #[test]
    fn completed_sidecar_reconnect_preserves_terminal_cursor_without_capacity_charge() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &peer);
        let CertifiedMergeSidecarMessage::Chunk(chunk) = &chunk_message else {
            unreachable!("sidecar fixture returns one response chunk")
        };
        let rollover_claim = ExactOutputRolloverClaim::CertifiedSidecarChunk {
            scope: service.exact_output_scope(),
            target: peer.clone(),
            transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
            chunk_index: chunk.chunk_index,
            chunk_count: chunk.chunk_count,
            response_hash: HashOf::new(chunk),
        };
        let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(chunk_message));
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = routes.mint_via(peer.clone(), hub_a.clone());
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let response_class = exact_output_class(&message).expect("classify sidecar response");
        let source_a = ExactTargetRoute::Reply(route_a.clone()).source(&peer, response_class);
        let source_b = ExactTargetRoute::Reply(route_b.clone()).source(&peer, response_class);
        let fanout = |reply_routes: NetworkReplyRoutes| {
            PendingExactFanout::claimed_with_reply_routes(
                vec![message.clone()],
                peer.clone(),
                reply_routes,
                rollover_claim.clone(),
            )
            .expect("valid certified sidecar claim")
            .expect("one exact sidecar response")
        };

        let mut initial_routes =
            NetworkReplyRoutes::try_from_route(route_a.clone()).expect("source A route set");
        initial_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone()).expect("source B route set"),
            )
            .expect("retain both response sources");
        let retained = fanout(initial_routes);
        let a_index = retained
            .targets
            .iter()
            .position(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(&route_a))
            })
            .expect("source A target");
        let b_index = retained
            .targets
            .iter()
            .position(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(&route_b))
            })
            .expect("source B target");

        let mut pending =
            PendingExactOutput::new(2, 1, 2, &[]).expect("two shared ownership units fit");
        assert_eq!(
            pending
                .enqueue(retained)
                .expect("retain terminal A and live B"),
            ExactFanoutOwnership::Owned
        );
        let fifo_id = pending.fanouts[0]
            .fifo_id
            .expect("sidecar fanout owns stable FIFO age");
        assert_eq!(pending.ownership_units, 2);

        let mut source_a_flush_control = None;
        assert_eq!(
            pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                let ExactTargetRoute::Reply(route) = route else {
                    panic!("sidecar response must retain its exact reply route")
                };
                if route.same_source(&route_a) {
                    let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
                    source_a_flush_control = Some(control);
                    return Ok(ExactOutputAttemptOutcome::SidecarFlush(ack));
                }
                assert!(route.same_source(&route_b));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 19,
                })
            }),
            Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 19 })
        );
        assert!(
            source_a_flush_control
                .as_mut()
                .expect("source A owns its exact writer-flush witness")
                .flush()
        );
        pending
            .poll_reply_flushes()
            .expect("source A successful writer flush advances one cursor");
        assert_eq!(
            pending.fanouts[0].targets[a_index].message_index,
            pending.fanouts[0].messages.len()
        );
        assert_eq!(pending.fanouts[0].targets[b_index].message_index, 0);
        assert_eq!(pending.ownership_units, 1);
        let _applied_admission = pending
            .admitted_sidecar_chunks
            .pop_front()
            .expect("the successful writer flush publishes one lane receipt");
        assert!(pending.admitted_sidecar_chunks.is_empty());
        assert_eq!(pending.ownership_units, 1);

        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![merge_share_message(b"terminal sidecar capacity blocker")],
                        vec![peer.clone()],
                    )
                    .expect("one unrelated capacity blocker"),
                )
                .expect("fill the last shared ownership unit"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.shared_ownership_units, 2);

        assert!(routes.retire(&route_a));
        let reconnected_a = routes.mint_via(peer.clone(), hub_a);
        let reconnect = || {
            fanout(
                NetworkReplyRoutes::try_from_route(reconnected_a.clone())
                    .expect("reconnected source A route set"),
            )
        };
        assert!(
            pending
                .can_enqueue_owned_reply_transfer(reconnect())
                .expect("terminal replay reconnect preflight observes no new reservation"),
            "a terminal replay reconnect must fit even when shared capacity is full"
        );
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(reconnect())
                .expect("full corridor updates only the terminal route capability"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.ownership_units, 2);
        let terminal_a = &pending.fanouts[0].targets[a_index];
        assert_eq!(terminal_a.message_index, 1);
        assert!(matches!(
            &terminal_a.route,
            ExactTargetRoute::Reply(route) if route.same_delivery(&reconnected_a)
        ));
        assert!(
            !pending.source_fifo_owners.contains_key(&source_a),
            "terminal source A must not regain FIFO or reservation ownership"
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );

        assert_eq!(
            pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                if matches!(route, ExactTargetRoute::Topology) {
                    return Ok(ExactOutputAttemptOutcome::Admitted);
                }
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_source(&route_b)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 11,
                })
            }),
            Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 11 })
        );
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending.fanouts[0].targets[a_index].message_index,
            pending.fanouts[0].messages.len(),
            "service progress cannot reopen a terminal sidecar source"
        );
        assert!(!pending.source_fifo_owners.contains_key(&source_a));
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(pending.pending_sidecar_flushes(), 0);
        assert!(pending.admitted_sidecar_chunks.is_empty());
    }

    #[test]
    fn later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress()
     {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &peer);
        let CertifiedMergeSidecarMessage::Chunk(_chunk) = &chunk_message else {
            unreachable!("sidecar fixture returns one response chunk")
        };
        let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(chunk_message));
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let first_route = routes.mint_via(peer.clone(), hub_a.clone());
        let fanout = |route: &NetworkReplyRoute| {
            PendingExactFanout::new_with_reply_routes(
                vec![message.clone()],
                peer.clone(),
                NetworkReplyRoutes::try_from_route(route.clone()).expect("live reply route set"),
            )
            .expect("one exact sidecar response")
        };

        let mut pending = PendingExactOutput::new(3, 1, 2, &[])
            .expect("one response attempt and two capacity blockers fit");
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(&first_route))
                .expect("retain the first response attempt"),
            ExactFanoutOwnership::Owned
        );
        let mut flush_control = None;
        assert_eq!(
            pending.drive_with_budget_ack(1, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                let ExactTargetRoute::Reply(route) = route else {
                    panic!("sidecar response must retain its reply route")
                };
                assert!(route.same_delivery(&first_route));
                let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
                flush_control = Some(control);
                Ok(ExactOutputAttemptOutcome::SidecarFlush(ack))
            }),
            Ok(ExactOutputDriveOutcome::BudgetExhausted {
                closest_backpressure_rank: None,
            })
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.fanouts[0].targets[0].message_index, 0);
        assert_eq!(pending.pending_sidecar_flushes(), 1);

        for label in [
            b"pending flush capacity blocker a".as_slice(),
            b"pending flush capacity blocker b".as_slice(),
        ] {
            assert_eq!(
                pending
                    .enqueue(
                        PendingExactFanout::new(
                            vec![merge_share_message(label)],
                            vec![peer.clone()],
                        )
                        .expect("one unrelated capacity blocker"),
                    )
                    .expect("fill the shared exact-output corridor"),
                ExactFanoutOwnership::Owned
            );
        }
        assert_eq!(pending.shared_ownership_units, 3);

        let pending_later = routes
            .redeliver(&first_route)
            .expect("later delivery on the pending writer tenure");
        assert!(
            pending
                .can_enqueue_owned_reply_transfer(fanout(&pending_later))
                .expect("preflight recognizes retained flush ownership"),
            "a same-tenure replay consumes no additional shared capacity"
        );
        let blocker_count = pending.fanouts.len();
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(&pending_later))
                .expect("coalesce the pending same-tenure replay"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts.len(), blocker_count);
        assert_eq!(
            pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                assert!(matches!(route, ExactTargetRoute::Topology));
                assert!(
                    !matches!(post.data, NetworkMessage::CertifiedMergeSidecar(_)),
                    "same-tenure sidecar replay must not cross actor admission twice"
                );
                Ok(ExactOutputAttemptOutcome::Admitted)
            }),
            Ok(ExactOutputDriveOutcome::Drained)
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.pending_sidecar_flushes(), 1);
        assert_eq!(pending.shared_ownership_units, 1);

        assert!(
            flush_control
                .as_mut()
                .expect("first sidecar writer owns its flush controller")
                .flush()
        );
        pending
            .poll_reply_flushes()
            .expect("exact writer flush publishes one unapplied receipt");
        assert_eq!(pending.pending_sidecar_flushes(), 0);
        assert!(pending.fanouts.is_empty());
        assert_eq!(pending.shared_ownership_units, 0);
        assert_eq!(pending.admitted_sidecar_chunks.len(), 1);

        let unapplied_later = routes
            .redeliver(&pending_later)
            .expect("later delivery while the exact receipt remains unapplied");
        assert!(
            pending
                .can_enqueue_owned_reply_transfer(fanout(&unapplied_later))
                .expect("preflight recognizes the unapplied receipt")
        );
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(&unapplied_later))
                .expect("coalesce the unapplied same-tenure replay"),
            ExactFanoutOwnership::Owned
        );
        assert!(pending.fanouts.is_empty());
        assert_eq!(
            pending.drive_with_budget_ack(1, |_post, _ticket, _route, _timeout_attempt| {
                panic!("unapplied receipt must retain exact actor ownership")
            }),
            Ok(ExactOutputDriveOutcome::Drained)
        );

        let alternate = routes.mint_via(peer.clone(), hub_b);
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(&alternate))
                .expect("an alternate source keeps an independent attempt"),
            ExactFanoutOwnership::Owned
        );
        let reconnected = routes.mint_via(peer.clone(), hub_a);
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(&reconnected))
                .expect("a replacement tenure observes the unapplied exact receipt"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.fanouts[0].targets.len(), 1);
        assert!(pending.fanouts[0].targets.iter().all(|target| {
            target.message_index == 0 && target.current.is_none() && target.ticket.is_none()
        }));
        assert!(pending.fanouts[0].targets.iter().any(|target| {
            matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_delivery(&alternate))
        }));
        assert!(pending.fanouts[0].targets.iter().all(|target| {
            !matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(&reconnected))
        }));
        assert_eq!(pending.admitted_sidecar_chunks.len(), 1);
    }

    #[test]
    fn mixed_source_retry_retains_pending_flush_target_without_resetting_live_siblings() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &peer);
        let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(chunk_message));
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 3);
        let route_a = routes.mint_via(peer.clone(), hub_a);
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let route_c = routes.mint_via(peer.clone(), hub_c);
        let fanout = |reply_routes: NetworkReplyRoutes| {
            PendingExactFanout::new_with_reply_routes(
                vec![message.clone()],
                peer.clone(),
                reply_routes,
            )
            .expect("one exact sidecar response")
        };

        let mut pending = PendingExactOutput::new(3, 1, 3, &[])
            .expect("three authenticated response sources fit");
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(
                    NetworkReplyRoutes::try_from_route(route_a.clone())
                        .expect("source A route set"),
                ))
                .expect("retain source A response"),
            ExactFanoutOwnership::Owned
        );
        let mut flush_control = None;
        assert_eq!(
            pending.drive_with_budget_ack(1, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                let ExactTargetRoute::Reply(route) = route else {
                    panic!("sidecar response must retain source A")
                };
                assert!(route.same_delivery(&route_a));
                let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, route);
                flush_control = Some(control);
                Ok(ExactOutputAttemptOutcome::SidecarFlush(ack))
            }),
            Ok(ExactOutputDriveOutcome::BudgetExhausted {
                closest_backpressure_rank: None,
            })
        );
        assert_eq!(pending.pending_sidecar_flushes(), 1);
        assert_eq!(pending.fanouts.len(), 1);

        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(
                    NetworkReplyRoutes::try_from_route(route_c.clone())
                        .expect("source C route set"),
                ))
                .expect("retain independent source C"),
            ExactFanoutOwnership::Owned
        );
        let later_a = routes
            .redeliver(&route_a)
            .expect("rebind source A while its writer flush is pending");
        let mut mixed_routes =
            NetworkReplyRoutes::try_from_route(later_a.clone()).expect("later source A route set");
        mixed_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone())
                    .expect("new source B route set"),
            )
            .expect("candidate carries pending A and independent B");
        let mixed = fanout(mixed_routes);
        assert!(
            pending
                .can_enqueue_owned_reply_transfer(mixed)
                .expect("mixed-source preflight preserves exact ownership")
        );

        let mut mixed_routes =
            NetworkReplyRoutes::try_from_route(later_a.clone()).expect("later source A route set");
        mixed_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone())
                    .expect("new source B route set"),
            )
            .expect("rebuild the exact mixed-source candidate");
        assert_eq!(
            pending
                .enqueue_owned_reply_transfer(fanout(mixed_routes))
                .expect("merge pending A without losing live B or C"),
            ExactFanoutOwnership::Owned
        );

        assert_eq!(pending.fanouts.len(), 1);
        let retained = &pending.fanouts[0];
        assert_eq!(retained.targets.len(), 3);
        assert_eq!(
            retained
                .reply_routes
                .as_ref()
                .expect("mixed fanout retains route history")
                .len(),
            3
        );
        let target_for = |expected: &NetworkReplyRoute| {
            retained
                .targets
                .iter()
                .find(|target| {
                    matches!(&target.route, ExactTargetRoute::Reply(route) if route.same_source(expected))
                })
                .expect("retained fanout contains the expected source")
        };
        let pending_a = target_for(&later_a);
        assert_eq!(pending_a.message_index, 0);
        assert!(pending_a.current.is_none());
        assert!(pending_a.ticket.is_none());
        assert!(pending_a.pending_flush.is_some());
        assert_eq!(target_for(&route_b).message_index, 0);
        assert_eq!(target_for(&route_c).message_index, 0);
        assert_eq!(pending.ownership_units, 3);
        assert_eq!(pending.shared_ownership_units, 3);
        assert_eq!(retained.current_source_targets.len(), 3);

        assert_eq!(
            pending.drive_with_budget_ack(usize::MAX, |post, ticket, route, _timeout_attempt| {
                assert!(ticket.is_none());
                assert!(
                    !matches!(route, ExactTargetRoute::Reply(route) if route.same_source(&later_a)),
                    "source A already owns this exact chunk in the flush queue"
                );
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 9,
                })
            }),
            Ok(ExactOutputDriveOutcome::Backpressured { closest_rank: 9 })
        );
        assert_eq!(pending.pending_sidecar_flushes(), 1);
        assert!(flush_control.is_some());
    }

    #[test]
    fn sidecar_flush_ack_identity_mismatch_fails_closed() {
        let (service, _) = fixture();
        let requester = service.context.roster[1].validator.clone();
        let (_, message) = certified_sidecar_outputs(&service.local_peer, &requester);
        let CertifiedMergeSidecarMessage::Chunk(chunk) = message else {
            unreachable!("sidecar fixture returns one response chunk")
        };
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let route = routes.mint(requester.clone());
        let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
            CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
        ));
        let fanout = || {
            PendingExactFanout::new_with_reply_routes(
                vec![message.clone()],
                requester.clone(),
                NetworkReplyRoutes::try_from_route(route.clone()).expect("one reply route"),
            )
            .expect("one routed sidecar response")
        };
        let (_admission_control, _admission_ack, admission) =
            certified_sidecar_flush_fixture(&chunk, &route);
        let (mut substituted_control, substituted_ack, _substituted_admission) =
            certified_sidecar_flush_fixture(&chunk, &route);
        assert!(substituted_control.flush());

        let mut pending =
            PendingExactOutput::new(1, 1, 1, &[]).expect("one exact sidecar flush witness fits");
        assert_eq!(pending.enqueue(fanout()), Ok(ExactFanoutOwnership::Owned));
        pending.fanouts[0].targets[0].pending_flush = Some(PendingExactReplyFlush {
            sidecar_admission: Some(admission),
            flush_ack: substituted_ack,
            reply_writer_timeout_attempt: 0,
        });
        let error = pending
            .poll_reply_flushes()
            .expect_err("substituted writer occurrence must fail closed");
        assert!(error.contains("different actor output"));
        assert_eq!(pending.pending_sidecar_flushes(), 1);
        assert!(pending.admitted_sidecar_chunks.is_empty());

        let (mut exact_control, exact_ack, exact_admission) =
            certified_sidecar_flush_fixture(&chunk, &route);
        assert!(exact_control.flush());
        let mut exact_pending =
            PendingExactOutput::new(1, 1, 1, &[]).expect("one exact sidecar flush witness fits");
        assert_eq!(
            exact_pending.enqueue(fanout()),
            Ok(ExactFanoutOwnership::Owned)
        );
        exact_pending.fanouts[0].targets[0].pending_flush = Some(PendingExactReplyFlush {
            sidecar_admission: Some(exact_admission),
            flush_ack: exact_ack,
            reply_writer_timeout_attempt: 0,
        });
        exact_pending
            .poll_reply_flushes()
            .expect("the exact actor output satisfies the shared flush kernel");
        assert_eq!(exact_pending.pending_sidecar_flushes(), 0);
        assert!(exact_pending.fanouts.is_empty());
        assert_eq!(exact_pending.admitted_sidecar_chunks.len(), 1);
    }

    #[test]
    fn inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let message = merge_share_message(b"worker tombstone collision");
        let response_class = exact_output_class(&message).expect("classified response");
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 3);
        let route_a = routes.mint_via(peer.clone(), hub_a);
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let source_a = ExactTargetRoute::Reply(route_a.clone()).source(&peer, response_class);
        let source_b = ExactTargetRoute::Reply(route_b.clone()).source(&peer, response_class);
        let mut reply_routes =
            NetworkReplyRoutes::try_from_route(route_a.clone()).expect("source A route set");
        reply_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone()).expect("source B route set"),
            )
            .expect("retain two authenticated sources");
        let mut pending =
            PendingExactOutput::new(3, 1, 3, &[]).expect("three-source history corridor");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new_with_reply_routes(
                        vec![message.clone()],
                        peer.clone(),
                        reply_routes,
                    )
                    .expect("two-source retained fanout"),
                )
                .expect("retain source history"),
            ExactFanoutOwnership::Owned
        );
        assert!(routes.retire(&route_a));
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_delivery(&route_b)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 19,
                })
            }),
            Ok(Some(19))
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.ownership_units, 2);
        let parked_a = pending.fanouts[0]
            .targets
            .iter()
            .find(|target| {
                matches!(
                    &target.route,
                    ExactTargetRoute::Reply(route) if route.same_source(&route_a)
                )
            })
            .expect("retired source A keeps its independent target");
        assert!(parked_a.parked);
        assert_eq!(parked_a.message_index, 0);
        assert!(parked_a.current.is_none());
        assert!(parked_a.ticket.is_none());
        assert_eq!(pending.source_fifo_owners.len(), 2);
        let fifo_id = pending.fanouts[0]
            .fifo_id
            .expect("parked source retains its stable fanout age");
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );
        let fifo_before = pending.source_fifo_owners.clone();
        let collision = routes
            .forge_equal_ordinal_different_tenure(&route_a, peer.clone(), hub_c)
            .expect("forge cross-source reuse of the retired ordinal");
        assert!(route_a.equal_ordinal_different_tenure(&collision));
        let targets_before = pending.fanouts[0].targets.len();
        let reservations_before = pending.reservation_owner_counts.clone();
        assert!(matches!(
            NetworkReplyRoutes::try_from_route(collision),
            Err(NetworkReplyRouteError::EqualOrdinalDifferentTenure)
        ));
        assert_eq!(pending.fanouts[0].targets.len(), targets_before);
        assert_eq!(pending.reservation_owner_counts, reservations_before);
        assert_eq!(pending.source_fifo_owners, fifo_before);
        assert_eq!(pending.ownership_units, 2);
        assert!(
            pending.fanouts[0]
                .reply_routes
                .as_ref()
                .is_some_and(|history| history.iter().any(|route| route.same_delivery(&route_b)))
        );
    }

    #[test]
    fn owned_reply_history_merge_retries_candidate_retirement_after_prune() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let message = merge_share_message(b"worker route-history retirement race");
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = routes.mint_via(peer.clone(), hub_a);
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let retained = PendingExactFanout::new_with_reply_routes(
            vec![message.clone()],
            peer.clone(),
            NetworkReplyRoutes::try_from_route(route_a.clone()).expect("source A route history"),
        )
        .expect("retained source A fanout");
        let candidate = PendingExactFanout::new_with_reply_routes(
            vec![message],
            peer.clone(),
            NetworkReplyRoutes::try_from_route(route_b.clone()).expect("source B route history"),
        )
        .expect("candidate source B fanout");

        let mut hook_calls = 0usize;
        let plan = retained
            .reply_target_merge_plan_after_candidate_prune(&candidate, |attempt| {
                hook_calls = hook_calls.saturating_add(1);
                if attempt == 0 {
                    assert!(
                        routes.retire(&route_b),
                        "candidate retires after its owned-transfer prune"
                    );
                }
            })
            .expect("inactive-only retry prunes the raced candidate atomically");
        assert_eq!(hook_calls, 2);
        assert!(plan.targets.is_empty());
        assert_eq!(plan.reply_routes.len(), 1);
        assert!(
            plan.reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_a))
        );

        let collision = routes
            .forge_equal_ordinal_different_tenure(&route_b, peer, hub_c)
            .expect("forge reuse of the raced delivery ordinal");
        assert!(matches!(
            NetworkReplyRoutes::try_from_route(collision),
            Err(NetworkReplyRouteError::EqualOrdinalDifferentTenure)
        ));
        assert_eq!(plan.reply_routes.len(), 1);
        assert!(
            plan.reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_a))
        );
    }

    #[test]
    fn newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let first_digest = Hash::new(b"fallback first");
        let second_digest = Hash::new(b"fallback second");
        let messages = vec![
            merge_share_message(b"fallback first"),
            merge_share_message(b"fallback second"),
        ];
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = routes.mint_via(peer.clone(), hub_a.clone());
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let response_class = exact_output_class(&messages[0]).expect("classified response");
        let source_a = ExactTargetRoute::Reply(route_a.clone()).source(&peer, response_class);
        let source_b = ExactTargetRoute::Reply(route_b.clone()).source(&peer, response_class);
        let mut predecessor = PendingExactFanout::new_with_routes(
            messages.clone(),
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route_a.clone())],
        )
        .expect("two-message source A response");
        let returned_second = predecessor.messages[1].clone();
        let target = predecessor
            .targets
            .first_mut()
            .expect("response has one target");
        target.message_index = 1;
        target.current = Some(Post {
            data: returned_second,
            peer_id: peer.clone(),
            priority: Priority::High,
        });
        predecessor
            .rebuild_current_source_targets()
            .expect("manual fallback cursor has a valid local FIFO index");

        let mut pending = PendingExactOutput::new(2, 2, 2, &[])
            .expect("two independent authenticated sources fit");
        assert_eq!(
            pending.enqueue(predecessor).expect("predecessor fits"),
            ExactFanoutOwnership::Owned
        );
        let fifo_id = pending.fanouts[0]
            .fifo_id
            .expect("semantic response owns one stable FIFO age");
        assert!(routes.retire(&route_a));
        assert_eq!(
            pending.drive_with(|_post, _ticket, _route| {
                panic!("inactive source A must park before actor admission")
            }),
            Ok(None)
        );
        let parked_a = &pending.fanouts[0].targets[0];
        assert!(parked_a.parked);
        assert_eq!(parked_a.message_index, 1);
        assert!(parked_a.current.is_none());
        assert_eq!(pending.fanouts[0].fifo_id, Some(fifo_id));
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );

        let alternate = PendingExactFanout::new_with_routes(
            messages.clone(),
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route_b.clone())],
        )
        .expect("new source B response");
        assert_eq!(
            pending
                .enqueue(alternate)
                .expect("new source gets an independent bounded attempt"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets.len(), 2);
        assert!(pending.fanouts[0].targets[0].parked);
        assert_eq!(pending.fanouts[0].targets[0].message_index, 1);
        assert_eq!(pending.fanouts[0].targets[1].message_index, 0);
        assert!(!pending.fanouts[0].targets[1].parked);
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 2);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fifo_id]))
        );

        let mut admitted_b = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                assert!(ticket.is_none());
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_tenure(&route_b)
                ));
                admitted_b.push(merge_share_digest(&post.data));
                Ok(())
            }),
            Ok(None)
        );
        assert_eq!(admitted_b, vec![first_digest, second_digest]);
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.fanouts[0].targets[0].message_index, 1);
        assert!(pending.fanouts[0].targets[0].parked);
        assert!(pending.fanouts[0].target_is_complete(1));
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fifo_id]))
        );
        assert!(!pending.source_fifo_owners.contains_key(&source_b));

        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let route_c = routes.mint_via(peer.clone(), hub_c);
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new_with_routes(
                        messages.clone(),
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Reply(route_c)],
                    )
                    .expect("third authenticated source candidate"),
                )
                .expect("configured source geometry returns bounded backpressure"),
            ExactFanoutOwnership::SourceRetained
        );
        assert_eq!(pending.fanouts[0].targets.len(), 2);
        assert_eq!(pending.ownership_units, 1);

        let reconnected_a = routes.mint_via(peer.clone(), hub_a);
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new_with_routes(
                        messages,
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Reply(reconnected_a.clone())],
                    )
                    .expect("same-source reconnect response"),
                )
                .expect("source A reconnect reuses its retained ownership"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets[0].message_index, 1);
        assert!(!pending.fanouts[0].targets[0].parked);
        assert!(pending.fanouts[0].targets[0].current.is_none());
        let mut admitted_a = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                assert!(ticket.is_none());
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_tenure(&reconnected_a)
                ));
                admitted_a.push(merge_share_digest(&post.data));
                Ok(())
            }),
            Ok(None)
        );
        assert_eq!(admitted_a, vec![second_digest]);
        assert!(pending.fanouts.is_empty());

        let retired_without_alternate_source = routes.mint(peer.clone());
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new_with_routes(
                        vec![merge_share_message(
                            b"retired reply without alternate source"
                        )],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Reply(
                            retired_without_alternate_source.clone(),
                        )],
                    )
                    .expect("retirable reply output"),
                )
                .expect("retain reply before its tenure retires"),
            ExactFanoutOwnership::Owned
        );
        assert!(routes.retire(&retired_without_alternate_source));
        assert_eq!(
            pending.drive_with(|_post, _ticket, _route| {
                panic!("an already-inactive reply route must retire before actor admission")
            }),
            Ok(None)
        );
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(pending.fanouts.len(), 1);
        assert!(pending.fanouts[0].targets[0].parked);
        assert_eq!(pending.fanouts[0].targets[0].message_index, 0);
        assert!(!pending.source_fifo_owners.is_empty());

        let inactive_before_enqueue = routes.mint(peer.clone());
        assert!(routes.retire(&inactive_before_enqueue));
        let inactive_candidate = PendingExactFanout::new_with_routes(
            vec![merge_share_message(
                b"inactive before exact-output admission",
            )],
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(inactive_before_enqueue)],
        )
        .expect("inactive unowned candidate");
        assert!(
            pending
                .can_enqueue(&inactive_candidate)
                .expect_err("read-only admission rejects an already-dead source")
                .contains("inactive capability")
        );
        assert!(
            pending
                .enqueue(inactive_candidate)
                .expect_err("an all-dead candidate must be rejected atomically")
                .contains("inactive capability")
        );
        assert!(!pending.is_pending());
        assert_eq!(pending.ownership_units, 1);

        let retired_during_admission = routes.mint(peer.clone());
        let mut race_pending = PendingExactOutput::new(1, 1, 1, &[])
            .expect("one independent admission-race source fits");
        assert_eq!(
            race_pending
                .enqueue(
                    PendingExactFanout::new_with_routes(
                        vec![merge_share_message(b"reply retirement admission race")],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Reply(retired_during_admission.clone())],
                    )
                    .expect("racing reply output"),
                )
                .expect("retain reply before its admission race"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            race_pending.drive_with(|post, _ticket, _route| {
                assert!(routes.retire(&retired_during_admission));
                Err(NetworkActorAdmissionError::Rejected {
                    message: post,
                    reason: NetworkActorAdmissionRejection::InactiveReplyRoute,
                })
            }),
            Ok(None)
        );
        assert!(!race_pending.is_pending());
        assert_eq!(race_pending.ownership_units, 1);
        assert_eq!(race_pending.shared_ownership_units, 1);
        assert_eq!(race_pending.fanouts.len(), 1);
        assert!(race_pending.fanouts[0].targets[0].parked);
        assert!(!race_pending.source_fifo_owners.is_empty());

        let older_same_source = routes.mint(peer.clone());
        let younger_same_source = routes.mint(peer.clone());
        assert_eq!(
            older_same_source.source_key(),
            younger_same_source.source_key()
        );
        let mut blocked_pending = PendingExactOutput::new(1, 1, 2, std::slice::from_ref(&peer))
            .expect("one shared duplicate-source unit");
        let error = blocked_pending
            .enqueue(
                PendingExactFanout::new_with_routes(
                    vec![merge_share_message(b"duplicate-source retirement")],
                    vec![peer.clone(), peer.clone()],
                    vec![
                        ExactTargetRoute::Reply(older_same_source),
                        ExactTargetRoute::Reply(younger_same_source),
                    ],
                )
                .expect("malformed duplicate-source fanout fixture"),
            )
            .expect_err("one semantic request retains at most one attempt per source");
        assert!(error.contains("duplicated an authenticated source"));
        assert!(!blocked_pending.is_pending());

        let older_global_route = routes.mint(peer.clone());
        let younger_global_route = routes.mint(peer.clone());
        let global_class = exact_output_class(&merge_share_message(b"global FIFO class"))
            .expect("classified global FIFO response");
        let global_source =
            ExactTargetRoute::Reply(older_global_route.clone()).source(&peer, global_class);
        let mut global_pending =
            PendingExactOutput::new(2, 1, 1, &[]).expect("two global FIFO owners fit");
        for (route, label) in [
            (older_global_route.clone(), b"older global owner".as_slice()),
            (
                younger_global_route.clone(),
                b"younger global owner".as_slice(),
            ),
        ] {
            assert_eq!(
                global_pending
                    .enqueue(
                        PendingExactFanout::new_with_routes(
                            vec![merge_share_message(label)],
                            vec![peer.clone()],
                            vec![ExactTargetRoute::Reply(route)],
                        )
                        .expect("global FIFO reply fanout"),
                    )
                    .expect("global FIFO reply fanout fits"),
                ExactFanoutOwnership::Owned
            );
        }
        let older_fifo_id = global_pending.fanouts[0]
            .fifo_id
            .expect("older reply fanout has FIFO identity");
        let younger_fifo_id = global_pending.fanouts[1]
            .fifo_id
            .expect("younger reply fanout has FIFO identity");
        assert_eq!(
            global_pending.source_fifo_owners.get(&global_source),
            Some(&BTreeSet::from([older_fifo_id, younger_fifo_id]))
        );
        assert!(routes.retire(&younger_global_route));
        assert_eq!(
            global_pending.drive_with(|post, ticket, route| {
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_tenure(&older_global_route)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 23,
                })
            }),
            Ok(Some(23))
        );
        assert_eq!(global_pending.fanouts.len(), 2);
        assert_eq!(global_pending.ownership_units, 2);
        assert_eq!(global_pending.shared_ownership_units, 2);
        assert!(global_pending.fanouts[1].targets[0].parked);
        assert_eq!(
            global_pending.source_fifo_owners.get(&global_source),
            Some(&BTreeSet::from([older_fifo_id, younger_fifo_id]))
        );

        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut mixed_routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let live_route = mixed_routes.mint_via(peer.clone(), hub_a);
        let retired_route = mixed_routes.mint_via(peer.clone(), hub_b);
        assert!(mixed_routes.retire(&retired_route));
        let mut mixed_pending =
            PendingExactOutput::new(2, 1, 2, &[]).expect("two-source candidate corridor");
        let mixed_fanout = PendingExactFanout::new_with_routes(
            vec![merge_share_message(b"mixed live and retired sources")],
            vec![peer.clone(), peer],
            vec![
                ExactTargetRoute::Reply(live_route.clone()),
                ExactTargetRoute::Reply(retired_route),
            ],
        )
        .expect("mixed-liveness response fanout");
        assert!(
            mixed_pending
                .can_enqueue(&mixed_fanout)
                .expect_err("preflight must reject one inactive source")
                .contains("inactive capability")
        );
        assert!(
            mixed_pending
                .enqueue(mixed_fanout)
                .expect_err("one inactive source must reject the whole fanout")
                .contains("inactive capability")
        );
        assert_eq!(mixed_pending.ownership_units, 0);
        assert_eq!(mixed_pending.shared_ownership_units, 0);
        assert!(mixed_pending.reservation_owner_counts.is_empty());
        assert!(mixed_pending.source_fifo_owners.is_empty());
        assert!(!mixed_pending.is_pending());
    }

    #[test]
    fn owned_reply_transfer_retirement_after_validation_is_atomic() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = routes.mint_via(peer.clone(), hub_a);
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let message = merge_share_message(b"owned transfer retirement race");
        let mut pending =
            PendingExactOutput::new(2, 1, 2, &[]).expect("two independent owned reply sources fit");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new_with_routes(
                        vec![message.clone()],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Reply(route_a)],
                    )
                    .expect("retained source fanout"),
                )
                .expect("retain first source"),
            ExactFanoutOwnership::Owned
        );
        let mut candidate = PendingExactFanout::new_with_routes(
            vec![message],
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route_b.clone())],
        )
        .expect("owned alternate-source transfer");
        assert!(
            pending
                .validate_owned_reply_transfer(&mut candidate)
                .expect("candidate is live at strict validation")
        );
        let fifo_before = pending.source_fifo_owners.clone();
        let reservations_before = pending.reservation_owner_counts.clone();
        let units_before = pending.ownership_units;
        assert!(routes.retire(&route_b));
        assert_eq!(
            pending
                .enqueue_validated(candidate)
                .expect("post-validation retirement drops only the raced occurrence"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.source_fifo_owners, fifo_before);
        assert_eq!(pending.reservation_owner_counts, reservations_before);
        assert_eq!(pending.ownership_units, units_before);
        assert_eq!(pending.fanouts[0].targets.len(), 1);
    }

    #[test]
    fn a_b_a_hub_reconnect_preserves_each_source_cursor() {
        let (service, _) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let first_digest = Hash::new(b"independent route first");
        let second_digest = Hash::new(b"independent route second");
        let messages = vec![
            merge_share_message(b"independent route first"),
            merge_share_message(b"independent route second"),
        ];
        let response_class = exact_output_class(&messages[0]).expect("classified response");
        assert!(
            messages
                .iter()
                .all(|message| exact_output_class(message) == Ok(response_class))
        );
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = routes.mint_via(peer.clone(), hub_a.clone());
        let route_b = routes.mint_via(peer.clone(), hub_b);
        let mut fanout = PendingExactFanout::new_with_routes(
            messages.clone(),
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route_a.clone())],
        )
        .expect("first-source response fanout");
        fanout.targets[0].message_index = 1;
        fanout.targets[0].current = Some(Post {
            data: messages[1].clone(),
            peer_id: peer.clone(),
            priority: Priority::High,
        });
        fanout
            .rebuild_current_source_targets()
            .expect("advanced source A cursor remains indexed");

        let mut pending = PendingExactOutput::new(2, 2, 2, &[])
            .expect("two authenticated response sources fit exactly");
        assert_eq!(
            pending
                .enqueue(fanout)
                .expect("retain first source attempt"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new_with_routes(
                        messages.clone(),
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Reply(route_b.clone())],
                    )
                    .expect("second-source response retry"),
                )
                .expect("append the independent source attempt"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets.len(), 2);
        assert_eq!(pending.ownership_units, 2);
        let fanout_fifo_id = pending.fanouts[0]
            .fifo_id
            .expect("multi-source response has one stable FIFO identity");
        let source_a = ExactTargetRoute::Reply(route_a.clone()).source(&peer, response_class);
        let source_b = ExactTargetRoute::Reply(route_b.clone()).source(&peer, response_class);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                if matches!(route, ExactTargetRoute::Reply(route) if route.same_tenure(&route_a)) {
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 31,
                    });
                }
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_tenure(&route_b)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 29,
                })
            }),
            Ok(Some(29))
        );
        assert_eq!(pending.fanouts[0].targets[0].message_index, 1);
        assert!(pending.fanouts[0].targets[0].current.is_some());
        assert_eq!(pending.fanouts[0].targets[1].message_index, 0);
        assert!(pending.fanouts[0].targets[1].current.is_some());
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );
        assert!(routes.retire(&route_a));
        let route_a_reconnected = routes.mint_via(peer.clone(), hub_a.clone());
        let retry = PendingExactFanout::new_with_routes(
            messages,
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route_a_reconnected.clone())],
        )
        .expect("same-source reconnect retry");
        assert_eq!(
            pending.enqueue(retry).expect("merge A/B/A route ownership"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets.len(), 2);
        assert_eq!(pending.fanouts[0].targets[0].message_index, 1);
        assert!(pending.fanouts[0].targets[0].current.is_none());
        assert_eq!(pending.fanouts[0].targets[1].message_index, 0);
        assert!(pending.fanouts[0].targets[1].current.is_some());
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(
            pending.source_fifo_owners.get(&source_a),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );
        assert!(matches!(
            &pending.fanouts[0].targets[0].route,
            ExactTargetRoute::Reply(route) if route.same_tenure(&route_a_reconnected)
        ));
        assert!(matches!(
            &pending.fanouts[0].targets[1].route,
            ExactTargetRoute::Reply(route) if route.same_tenure(&route_b)
        ));

        let mut completed_a = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                assert!(ticket.is_none());
                if matches!(route, ExactTargetRoute::Reply(route) if route.same_tenure(&route_a_reconnected))
                {
                    completed_a.push(merge_share_digest(&post.data));
                    return Ok(());
                }
                assert!(matches!(
                    route,
                    ExactTargetRoute::Reply(route) if route.same_tenure(&route_b)
                ));
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 23,
                })
            }),
            Ok(Some(23))
        );
        assert_eq!(completed_a, vec![second_digest]);
        assert!(pending.fanouts[0].target_is_complete(0));
        assert_eq!(pending.fanouts[0].targets[1].message_index, 0);
        assert_eq!(pending.ownership_units, 1);
        assert!(!pending.source_fifo_owners.contains_key(&source_a));
        assert_eq!(
            pending.source_fifo_owners.get(&source_b),
            Some(&BTreeSet::from([fanout_fifo_id]))
        );

        assert!(routes.retire(&route_a_reconnected));
        let route_a_completed_reconnect = routes.mint_via(peer.clone(), hub_a);
        let completed_retry = PendingExactFanout::new_with_routes(
            vec![
                merge_share_message(b"independent route first"),
                merge_share_message(b"independent route second"),
            ],
            vec![peer.clone()],
            vec![ExactTargetRoute::Reply(route_a_completed_reconnect.clone())],
        )
        .expect("completed same-source reconnect retry");
        assert_eq!(
            pending
                .enqueue(completed_retry)
                .expect("completed reconnect preserves terminal ownership"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.fanouts[0].targets[0].message_index, 2);
        assert!(pending.fanouts[0].targets[0].current.is_none());
        assert_eq!(pending.fanouts[0].targets[1].message_index, 0);
        assert_eq!(pending.ownership_units, 1);
        assert!(!pending.source_fifo_owners.contains_key(&source_a));
        assert!(matches!(
            &pending.fanouts[0].targets[0].route,
            ExactTargetRoute::Reply(route) if route.same_tenure(&route_a_completed_reconnect)
        ));

        let mut admitted_a = Vec::new();
        let mut admitted_b = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, route| {
                assert!(ticket.is_none());
                let digest = merge_share_digest(&post.data);
                match route {
                    ExactTargetRoute::Reply(route)
                        if route.same_tenure(&route_a_completed_reconnect) =>
                    {
                        admitted_a.push(digest);
                    }
                    ExactTargetRoute::Reply(route) if route.same_tenure(&route_b) => {
                        admitted_b.push(digest);
                    }
                    _ => panic!("unexpected response route"),
                }
                Ok(())
            }),
            Ok(None)
        );
        assert!(admitted_a.is_empty());
        assert_eq!(admitted_b, vec![first_digest, second_digest]);
    }

    #[test]
    fn bulk_backpressure_does_not_block_reserved_lane_or_safety_output() {
        let (service, keys) = fixture();
        let peer = service.context.roster[1].validator.clone();
        let (_, artifact) = durable_finality_fixture(&service, &keys);
        let safety =
            ProductionV2Services::preencode_v2_network_message(global_commit_qc_message(&artifact))
                .expect("encode safety output");
        let lane = lane_commit_qc_message(peer.clone());
        let bulk = ProductionV2Services::preencode_v2_network_message(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk(
                manifest_hash(b"cross-class scheduler manifest"),
                0,
                b"bulk",
                0,
            ))),
        )
        .expect("encode bulk output");
        assert_eq!(exact_output_class(&safety), Ok(ExactOutputClass::Safety));
        assert_eq!(exact_output_class(&lane), Ok(ExactOutputClass::Lane));
        assert_eq!(exact_output_class(&bulk), Ok(ExactOutputClass::Bulk));
        assert!(
            PendingExactFanout::classified_with_routes(
                vec![bulk.clone(), safety.clone()],
                vec![peer.clone()],
                vec![ExactTargetRoute::Topology],
            )
            .is_err(),
            "a blocked lower-priority prefix must not own a later safety source"
        );

        let mut pending = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&peer))
            .expect("shared slot plus three reserved classes");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(vec![bulk], vec![peer.clone()]).expect("bulk fanout"),
                )
                .expect("bulk fanout within bounds"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 9,
                })
            }),
            Ok(Some(9))
        );

        for message in [safety, lane] {
            assert_eq!(
                pending
                    .enqueue(
                        PendingExactFanout::new(vec![message], vec![peer.clone()])
                            .expect("reserved class fanout"),
                    )
                    .expect("reserved class fanout within bounds"),
                ExactFanoutOwnership::Owned,
                "each unopened class for one semantic target has reserved ownership"
            );
        }

        let mut admitted = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                let class = exact_output_class(&post.data)
                    .expect("test messages have exact output classes");
                if class == ExactOutputClass::Bulk {
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 9,
                    });
                }
                assert!(ticket.is_none());
                admitted.push(class);
                Ok(())
            }),
            Ok(Some(9))
        );
        assert_eq!(
            admitted,
            vec![ExactOutputClass::Safety, ExactOutputClass::Lane]
        );
        assert_eq!(pending.fanouts.len(), 1);

        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                assert_eq!(exact_output_class(&post.data), Ok(ExactOutputClass::Bulk));
                assert!(ticket.is_none());
                Ok(())
            }),
            Ok(None)
        );
        assert!(!pending.is_pending());
    }

    #[test]
    fn non_roster_targets_cannot_consume_frozen_validator_reservations() {
        let (service, keys) = fixture();
        assert!(validate_shared_ownership_geometry(2, 1).is_err());
        assert_eq!(validate_shared_ownership_geometry(3, 1), Ok(()));
        let validator = service.context.roster[1].validator.clone();
        let frozen_validators = service
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let mut frozen_pending =
            PendingExactOutput::new(3, 1, frozen_validators.len(), &frozen_validators)
                .expect("source-sized shared pool plus frozen roster reservations");
        assert_eq!(
            frozen_pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(validator.clone())],
                        frozen_validators,
                    )
                    .expect("one full frozen-roster fanout"),
                )
                .expect("frozen reservations admit a roster wider than the reply-source bound"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            frozen_pending.shared_ownership_units, 0,
            "the first roster target/class occurrences use only frozen credits"
        );
        let observer_a = PeerId::new(KeyPair::random().public_key().clone());
        let observer_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut pending = PendingExactOutput::new(1, 1, 1, std::slice::from_ref(&validator))
            .expect("one shared slot plus frozen validator and control reservations");
        assert_eq!(pending.shared_ownership_unit_capacity, 1);
        assert_eq!(pending.reserved_target_classes.len(), 5);
        assert_eq!(pending.ownership_unit_capacity, 6);

        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(observer_a.clone())],
                        vec![observer_a.clone()],
                    )
                    .expect("first observer response"),
                )
                .expect("first observer response is within bounds"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                assert_eq!(post.peer_id, observer_a);
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 11,
                })
            }),
            Ok(Some(11))
        );

        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(observer_b.clone())],
                        vec![observer_b],
                    )
                    .expect("second observer response"),
                )
                .expect("second observer response is within bounds"),
            ExactFanoutOwnership::SourceRetained,
            "a novel non-roster identity must not claim a frozen validator slot"
        );

        let (_, artifact) = durable_finality_fixture(&service, &keys);
        let safety =
            ProductionV2Services::preencode_v2_network_message(global_commit_qc_message(&artifact))
                .expect("encode safety output");
        let lane = lane_commit_qc_message(validator.clone());
        let bulk = ProductionV2Services::preencode_v2_network_message(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk(
                manifest_hash(b"frozen reservation regression manifest"),
                0,
                b"bulk",
                0,
            ))),
        )
        .expect("encode bulk output");
        for message in [safety, lane, bulk] {
            assert_eq!(
                pending
                    .enqueue(
                        PendingExactFanout::new(vec![message], vec![validator.clone()])
                            .expect("frozen validator fanout"),
                    )
                    .expect("frozen validator fanout is within bounds"),
                ExactFanoutOwnership::Owned,
                "each frozen validator class retains its own slot"
            );
        }

        let mut admitted = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                if post.peer_id == observer_a {
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 11,
                    });
                }
                assert_eq!(post.peer_id, validator);
                assert!(ticket.is_none());
                admitted.push(exact_output_class(&post.data).expect("classified validator output"));
                Ok(())
            }),
            Ok(Some(11))
        );
        assert_eq!(
            admitted,
            vec![
                ExactOutputClass::Safety,
                ExactOutputClass::Lane,
                ExactOutputClass::Bulk,
            ]
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 1);
    }

    #[test]
    fn partial_fanout_progress_releases_only_the_completed_target_unit() {
        let (service, _) = fixture();
        let first = service.context.roster[1].validator.clone();
        let second = service.context.roster[2].validator.clone();
        let frozen = vec![first.clone(), second.clone()];
        let mut pending =
            PendingExactOutput::new(1, 1, 2, &frozen).expect("frozen two-validator corridor");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(first.clone())],
                        vec![first.clone(), second.clone()],
                    )
                    .expect("two-target lane fanout"),
                )
                .expect("two-target lane fanout is within bounds"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 0);

        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                if post.peer_id == first {
                    assert!(ticket.is_none());
                    return Ok(());
                }
                assert_eq!(post.peer_id, second);
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 13,
                })
            }),
            Ok(Some(13))
        );
        assert_eq!(pending.fanouts.len(), 1);
        assert_eq!(pending.ownership_units, 1);
        assert_eq!(pending.shared_ownership_units, 0);
        assert!(
            !pending
                .reservation_owner_counts
                .contains_key(&ExactTargetReservation {
                    semantic_target: first.clone(),
                    class: ExactOutputClass::Lane,
                    kind: ExactTargetReservationKind::Reliable,
                })
        );

        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(first.clone())],
                        vec![first.clone()],
                    )
                    .expect("new output for completed target"),
                )
                .expect("new completed-target output is within bounds"),
            ExactFanoutOwnership::Owned,
            "partial progress must free the completed target/class reservation"
        );
        assert_eq!(
            pending
                .reservation_owner_counts
                .get(&ExactTargetReservation {
                    semantic_target: first,
                    class: ExactOutputClass::Lane,
                    kind: ExactTargetReservationKind::Reliable,
                }),
            Some(&1)
        );
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 0);
    }

    #[test]
    fn ownership_units_reject_reservation_spill_and_release_exact_target() {
        let (service, _) = fixture();
        let constrained = service.context.roster[1].validator.clone();
        let alternate = service.context.roster[2].validator.clone();
        let observer = PeerId::new(KeyPair::random().public_key().clone());
        let frozen = vec![constrained.clone(), alternate.clone()];
        let mut pending =
            PendingExactOutput::new(1, 1, 2, &frozen).expect("frozen two-validator corridor");

        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(observer.clone())],
                        vec![observer],
                    )
                    .expect("observer fanout"),
                )
                .expect("observer consumes the only shared slot"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(constrained.clone())],
                        vec![constrained.clone(), alternate.clone()],
                    )
                    .expect("flexible validator fanout"),
                )
                .expect("flexible fanout owns both exact frozen units"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(constrained.clone())],
                        vec![constrained.clone()],
                    )
                    .expect("constrained validator fanout"),
                )
                .expect("duplicate target/class must consume shared ownership"),
            ExactFanoutOwnership::SourceRetained,
            "a multi-target fanout already owns the constrained unit; it cannot be undercharged"
        );
        assert_eq!(pending.ownership_units, 3);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                if post.peer_id == constrained {
                    assert!(ticket.is_none());
                    return Ok(());
                }
                Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 17,
                })
            }),
            Ok(Some(17))
        );
        assert_eq!(pending.ownership_units, 2);
        assert_eq!(pending.shared_ownership_units, 1);
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![lane_commit_qc_message(constrained.clone())],
                        vec![constrained],
                    )
                    .expect("released exact target fanout"),
                )
                .expect("released frozen unit remains independently available"),
            ExactFanoutOwnership::Owned
        );
    }

    #[test]
    fn backpressured_source_does_not_block_other_sources_or_consume_their_reserve() {
        let (service, _) = fixture();
        let blocked = service.context.roster[1].validator.clone();
        let same_fanout_responsive = service.context.roster[2].validator.clone();
        let later_fanout_responsive = service.context.roster[3].validator.clone();
        let observer = PeerId::new(KeyPair::random().public_key().clone());
        let oldest_first_digest = Hash::new(b"oldest blocked-peer fanout first");
        let oldest_second_digest = Hash::new(b"oldest blocked-peer fanout second");
        let responsive_digest = Hash::new(b"later responsive fanout");
        let later_blocked_digest = Hash::new(b"later blocked-peer fanout");
        let frozen = vec![
            blocked.clone(),
            same_fanout_responsive.clone(),
            later_fanout_responsive.clone(),
        ];
        let mut pending = PendingExactOutput::new(1, 2, 2, &frozen)
            .expect("one shared unit plus exact frozen target units");
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![merge_share_message(b"shared observer blocker")],
                        vec![observer.clone()],
                    )
                    .expect("observer blocker"),
                )
                .expect("observer consumes the shared ownership unit"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending
                .enqueue(
                    PendingExactFanout::new(
                        vec![
                            merge_share_message(b"oldest blocked-peer fanout first"),
                            merge_share_message(b"oldest blocked-peer fanout second"),
                        ],
                        vec![blocked.clone(), same_fanout_responsive.clone()],
                    )
                    .expect("mixed-target fanout"),
                )
                .expect("fanout within bounds"),
            ExactFanoutOwnership::Owned
        );

        let mut blocked_attempts = 0usize;
        let mut admitted = Vec::new();
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                if post.peer_id == observer {
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 7,
                    });
                }
                if post.peer_id == blocked {
                    blocked_attempts = blocked_attempts.saturating_add(1);
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 7,
                    });
                }
                assert!(ticket.is_none());
                admitted.push((post.peer_id, merge_share_digest(&post.data)));
                Ok(())
            }),
            Ok(Some(7))
        );
        assert_eq!(blocked_attempts, 1);
        assert_eq!(
            admitted,
            vec![
                (same_fanout_responsive.clone(), oldest_first_digest),
                (same_fanout_responsive.clone(), oldest_second_digest),
            ]
        );

        let responsive_fanout = PendingExactFanout::new(
            vec![merge_share_message(b"later responsive fanout")],
            vec![later_fanout_responsive.clone()],
        )
        .expect("later responsive fanout");
        assert_eq!(
            pending
                .enqueue(responsive_fanout)
                .expect("responsive fanout within bounds"),
            ExactFanoutOwnership::Owned
        );
        let later_blocked_fanout = PendingExactFanout::new(
            vec![merge_share_message(b"later blocked-peer fanout")],
            vec![blocked.clone()],
        )
        .expect("later same-source fanout");
        assert_eq!(
            pending
                .enqueue(later_blocked_fanout)
                .expect("same-source fanout within protocol bounds"),
            ExactFanoutOwnership::SourceRetained,
            "a blocked source cannot consume the slot reserved for another source/class"
        );

        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                if post.peer_id == observer {
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 7,
                    });
                }
                if post.peer_id == blocked {
                    blocked_attempts = blocked_attempts.saturating_add(1);
                    return Err(NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 7,
                    });
                }
                assert!(ticket.is_none());
                admitted.push((post.peer_id, merge_share_digest(&post.data)));
                Ok(())
            }),
            Ok(Some(7))
        );
        assert_eq!(blocked_attempts, 2);
        assert_eq!(
            admitted,
            vec![
                (same_fanout_responsive.clone(), oldest_first_digest),
                (same_fanout_responsive, oldest_second_digest),
                (later_fanout_responsive, responsive_digest),
            ]
        );
        assert_eq!(pending.fanouts.len(), 2);
        assert!(pending.fanouts[0].targets[0].current.is_some());
        assert!(pending.fanouts[1].targets[0].current.is_some());
        assert!(pending.fanouts[1].target_is_complete(1));

        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                assert!(ticket.is_none());
                if post.peer_id == blocked {
                    admitted.push((post.peer_id, merge_share_digest(&post.data)));
                } else {
                    assert_eq!(post.peer_id, observer);
                }
                Ok(())
            }),
            Ok(None)
        );
        assert!(!pending.is_pending());

        let later_blocked_fanout = PendingExactFanout::new(
            vec![merge_share_message(b"later blocked-peer fanout")],
            vec![blocked.clone()],
        )
        .expect("reconstructed same-source fanout");
        assert_eq!(
            pending
                .enqueue(later_blocked_fanout)
                .expect("reconstructed fanout within bounds"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            pending.drive_with(|post, ticket, _route| {
                assert_eq!(post.peer_id, blocked);
                assert!(ticket.is_none());
                admitted.push((post.peer_id, merge_share_digest(&post.data)));
                Ok(())
            }),
            Ok(None)
        );
        assert_eq!(
            admitted.last(),
            Some(&(blocked.clone(), later_blocked_digest)),
            "the producer-owned suffix becomes schedulable after the older FIFO head completes"
        );
        let admitted_to_recovered_target = admitted
            .iter()
            .filter_map(|(peer, digest)| (peer == &blocked).then_some(*digest))
            .collect::<Vec<_>>();
        assert_eq!(
            admitted_to_recovered_target,
            vec![
                oldest_first_digest,
                oldest_second_digest,
                later_blocked_digest,
            ]
        );
        assert!(!pending.is_pending());
    }
