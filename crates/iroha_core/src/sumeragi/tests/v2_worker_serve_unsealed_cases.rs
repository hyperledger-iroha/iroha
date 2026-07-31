// Unsealed certified-Serve lifecycle worker regression tests.
// Included lexically by v2_worker::tests to preserve canonical test names.

    #[test]
    fn dormant_serve_waiters_reattach_strictly_by_durable_scheduler_ordinal() {
        let (service, keys) = fixture_with_block_payload();
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let first = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let second = authenticated_serve_request(
            &service.context,
            &keys[2],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let via = service.context.roster[0].validator.clone();
        let (command_tx, _command_rx, _admission) = test_io_command_channel(4);
        let gate = CertifiedServeIngressGate {
            queue: Arc::clone(&command_tx.queue),
        };
        let first_carrier = gate
            .reserve(first.request(), &via, true, 10)
            .expect("reserve first exact Serve owner")
            .expect("current-height request owns a Serve reservation");
        let second_carrier = gate
            .reserve(second.request(), &via, true, 11)
            .expect("reserve second exact Serve owner")
            .expect("current-height request owns a Serve reservation");
        let first_scheduler_ordinal = first_carrier.id.0;
        let second_scheduler_ordinal = second_carrier.id.0;
        assert!(first_scheduler_ordinal < second_scheduler_ordinal);

        drop(second_carrier);
        drop(first_carrier);
        assert_eq!(
            gate.dormant_ingress_scheduler_ordinal()
                .expect("inspect carrierless durable head"),
            Some(first_scheduler_ordinal)
        );
        assert!(matches!(
            gate.reserve(second.request(), &via, true, 20),
            Err(CertifiedServeIngressReserveError::Busy)
        ));
        assert!(
            command_tx
                .try_begin_producer_episode()
                .expect("inspect producer exclusion")
                .is_none()
        );

        let resumed_first = gate
            .reserve(first.request(), &via, true, 21)
            .expect("reattach durable head")
            .expect("current-height retry retains Serve reservation");
        assert_eq!(resumed_first.id.0, first_scheduler_ordinal);
        assert_eq!(
            command_tx
                .serve_barrier()
                .expect("inspect selected durable head")
                .map(|barrier| barrier.scheduler_ordinal()),
            Some(first_scheduler_ordinal)
        );
        assert_eq!(
            gate.dormant_ingress_scheduler_ordinal()
                .expect("second owner remains dormant behind selected head"),
            Some(second_scheduler_ordinal)
        );
        let resumed_second = gate
            .reserve(second.request(), &via, true, 22)
            .expect("reattach next durable waiter")
            .expect("current-height retry retains Serve reservation");
        assert_eq!(resumed_second.id.0, second_scheduler_ordinal);
        assert_eq!(
            command_tx
                .serve_barrier()
                .expect("later carrier cannot replace selected durable head")
                .map(|barrier| barrier.scheduler_ordinal()),
            Some(first_scheduler_ordinal)
        );
        drop(resumed_second);
        drop(resumed_first);
    }

    #[test]
    fn fair_ingress_producer_episode_wins_or_yields_without_partial_exact_admission() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let request = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let via = service.context.roster[0].validator.clone();
        let (command_tx, _command_rx, _admission) = test_io_command_channel(2);
        let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
        let producer_episode = command_tx
            .try_begin_producer_episode()
            .expect("start finite producer episode")
            .expect("no exact ticket precedes the episode");
        let before = fair_ingress_accounting_snapshot(&ingress);

        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
            Err(FairV2IngressPushError::Full(_))
        ));
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            before,
            "a running producer episode cannot partially consume fair-ingress capacity"
        );
        {
            let state = command_tx.queue.lock();
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 0);
            assert_eq!(
                command_tx
                    .queue
                    .lifecycle_ordinals
                    .next_ordinal_for_test()
                    .expect("inspect shared ordinal source"),
                Some(1),
                "Busy admission must not mint an actor-global ordinal"
            );
            assert!(state.producer_episode_active);
        }

        drop(producer_episode);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(request.request(), via)),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        {
            let state = command_tx.queue.lock();
            assert!(state.serve_ingress_reservation.is_some());
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert!(!state.producer_episode_active);
        }

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire producer-race fixture ticket");
    }

    #[test]
    fn fair_ingress_full_prefix_materializes_exact_serve_before_later_churn() {
        let (service, keys) = fixture_with_block_payload();
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let request = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = service.context.roster[0].validator.clone();
        let mut route_fixture = NetworkReplyRouteTestFixture::new(via.clone());
        let route = route_fixture.mint_via(requester.clone(), via.clone());
        let (command_tx, command_rx, _admission) = test_io_command_channel(1);
        command_tx
            .try_send_as(
                V2IoAdmissionClass::Auxiliary,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(700),
                    subject: proposal.subject,
                },
            )
            .expect("install the frozen physical predecessor");
        let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));

        let mut saw_backpressure = false;
        assert!(
            ingress
                .try_recv_if(|_| {
                    match command_tx.prepare_reserved_serve(
                        CertifiedServeOwnerKey::Roster(requester.clone()),
                        request.clone(),
                    ) {
                        Err(CertifiedServePrepareError::Backpressure) => {
                            saw_backpressure = true;
                            false
                        }
                        result => panic!("full prefix must retain target ownership: {result:?}"),
                    }
                })
                .is_none()
        );
        assert!(saw_backpressure);
        let retained = fair_ingress_accounting_snapshot(&ingress);
        assert_eq!(retained.len, 1);
        assert!(
            retained
                .lanes
                .iter()
                .flat_map(|lane| &lane.entries)
                .all(|entry| entry.owns_certified_serve_ticket)
        );
        let lifecycle_id = {
            let state = command_tx.queue.lock();
            let lifecycle_id = state
                .serve_barrier
                .expect("backpressured target owns one off-queue barrier");
            assert_eq!(
                state
                    .serve_ingress_reservation
                    .as_ref()
                    .map(|reservation| reservation.state),
                Some(CertifiedServeIngressReservationState::Prepared(
                    lifecycle_id
                ))
            );
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|serve| serve.state),
                Some(V2IoServeState::PendingCapacity)
            );
            lifecycle_id
        };
        for ordinal in 701..705 {
            assert!(matches!(
                command_tx.try_send_as(
                    V2IoAdmissionClass::Control,
                    V2IoCommand::LoadCandidate {
                        acquisition_id: LockedCandidateAcquisitionId(ordinal),
                        subject: proposal.subject,
                    },
                ),
                Err(V2IoTrySendError::Full(_))
            ));
        }

        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(700),
                ..
            })
        ));
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state
                    .commands
                    .front()
                    .and_then(V2IoCommand::serve_lifecycle_id),
                Some(lifecycle_id),
                "predecessor release materializes the exact barrier under the queue lock"
            );
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|serve| serve.state),
                Some(V2IoServeState::Reserved)
            );
        }
        let (admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(admission.lifecycle_id, lifecycle_id);
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve {
                lifecycle_id: drained,
                ..
            }) if drained == lifecycle_id
        ));

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire full-prefix fixture gate");
    }

    #[test]
    fn dormant_exact_head_reattaches_after_saturated_fair_prefix_and_drains_frozen_predecessor() {
        let (service, keys) = fixture_with_block_payload();
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let requests = [1_usize, 2, 3].map(|index| {
            authenticated_serve_request(
                &service.context,
                &keys[index],
                proposal.round,
                proposal.subject,
                wire::GlobalPhase::Prepare,
            )
        });
        let higher = authenticated_serve_request(
            &service.context,
            &keys[1],
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let via = service.context.roster[0].validator.clone();
        let (command_tx, command_rx, _admission) = test_io_command_channel(4);
        for ordinal in 710..714 {
            command_tx
                .try_send_as(
                    V2IoAdmissionClass::Auxiliary,
                    V2IoCommand::LoadCandidate {
                        acquisition_id: LockedCandidateAcquisitionId(ordinal),
                        subject: proposal.subject,
                    },
                )
                .expect("install the frozen I/O predecessor prefix");
        }

        let fair_capacity = fair_v2_ingress_required_capacity(service.context.roster.len(), None)
            .expect("fixture roster has representable protected-slot geometry");
        let ingress = FairV2Ingress::new(
            fair_capacity,
            5 * 64 * 1024 * 1024,
            64 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        ingress
            .configure_roster(
                service
                    .context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone()),
            )
            .expect("minimum protected-slot geometry fits the fixture roster");
        ingress.require_certified_serve_gate();
        let gate = CertifiedServeIngressGate {
            queue: Arc::clone(&command_tx.queue),
        };
        ingress
            .bind_certified_serve_gate(gate.clone())
            .expect("bind saturated-prefix Serve gate");
        ingress.open().expect("open saturated-prefix ingress");

        let fair_predecessor_height = service.context.height.saturating_add(1);
        let fair_predecessor = InboundBlockMessage::from_transport(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                    wire::CommitCertificateRequest {
                        protocol_version: wire::PROTOCOL_VERSION,
                        chain_id: service.context.chain_id.clone(),
                        context_id: service.context.id(),
                        height: fair_predecessor_height,
                        requester: via.clone(),
                        signature: vec![0xA5],
                    },
                ),
            )),
            via.clone(),
            via.clone(),
        );
        assert!(matches!(
            ingress.try_push(fair_predecessor),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        for request in &requests {
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        }
        assert_eq!(ingress.len(), requests.len() + 1);
        assert!(
            ingress
                .try_recv_if(fair_v2_ingress_is_certified_body_request)
                .is_none(),
            "predicate-selective service cannot bypass the saturated Fair predecessor"
        );
        let before_full = fair_ingress_accounting_snapshot(&ingress);
        let serve_high_watermark_before_full = command_tx
            .queue
            .lock()
            .next_serve_ingress_reservation_ordinal;
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(higher.request(), via.clone())),
            Err(FairV2IngressPushError::Full(_))
        ));
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            before_full,
            "Fair capacity rejection cannot mint carrierless Serve ownership"
        );
        let first_barrier = command_tx
            .serve_barrier()
            .expect("inspect saturated-prefix selected target")
            .expect("the first exact owner remains selected");
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.serve_ingress_waiters.len(), requests.len() - 1);
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal, serve_high_watermark_before_full,
                "Fair capacity rejection cannot advance the durable Serve high-watermark"
            );
        }

        // The only production transition which removes an undrained carrier
        // from a closed height clears every Fair lane under the same lock.
        // Therefore a dormant head cannot coexist with the saturated volatile
        // prefix after unbinding.
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("atomically retire the saturated volatile prefix");
        assert_eq!(ingress.len(), 0);
        assert_eq!(
            gate.dormant_ingress_scheduler_ordinal()
                .expect("inspect carrierless head after prefix retirement"),
            Some(first_barrier.scheduler_ordinal())
        );
        assert!(command_tx.queue.lock().serve_ingress_reservation.is_none());

        ingress
            .bind_certified_serve_gate(gate.clone())
            .expect("rebind dormant exact owners to empty ingress");
        ingress.open().expect("reopen empty ingress");
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(higher.request(), via.clone())),
            Err(FairV2IngressPushError::Full(_))
        ));
        assert_eq!(ingress.len(), 0, "dormant debt excludes fresh churn");
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(requests[0].request(), via)),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            command_tx
                .serve_barrier()
                .expect("inspect reattached exact head")
                .map(|barrier| barrier.scheduler_ordinal()),
            Some(first_barrier.scheduler_ordinal())
        );
        assert!(
            gate.dormant_ingress_scheduler_ordinal()
                .expect("later exact owners remain dormant")
                .is_some(),
            "a later dormant owner must not suppress the selected head's turn"
        );
        assert!(
            command_tx
                .try_begin_producer_episode()
                .expect("inspect producer exclusion behind mixed exact ownership")
                .is_none()
        );

        let mut saw_backpressure = false;
        assert!(
            ingress
                .try_recv_if(|_| {
                    match command_tx.prepare_reserved_serve(
                        CertifiedServeOwnerKey::Roster(requests[0].request().requester.clone()),
                        requests[0].clone(),
                    ) {
                        Err(CertifiedServePrepareError::Backpressure) => {
                            saw_backpressure = true;
                            false
                        }
                        result => {
                            panic!("the frozen predecessor must retain exact ownership: {result:?}")
                        }
                    }
                })
                .is_none()
        );
        assert!(saw_backpressure);
        assert_eq!(ingress.len(), 1);
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(710),
                ..
            })
        ));
        assert_eq!(
            command_tx
                .queue
                .lock()
                .commands
                .back()
                .and_then(V2IoCommand::serve_lifecycle_id),
            Some(first_barrier.lifecycle_id()),
            "one released predecessor materializes the exact target behind the finite prefix"
        );

        let (admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requests[0].request().requester.clone()),
            &requests[0],
        );
        assert_eq!(admission.lifecycle_id, first_barrier.lifecycle_id());
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        for ordinal in 711..714 {
            assert!(matches!(
                command_rx.try_recv(),
                Ok(V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(candidate),
                    ..
                }) if candidate == ordinal
            ));
        }
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve {
                lifecycle_id: drained,
                ..
            }) if drained == admission.lifecycle_id
        ));

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire reopened saturated-prefix fixture");
    }

    #[test]
    fn fair_ingress_serve_only_prefix_materializes_after_frozen_completion_ack() {
        let (service, keys) = fixture_with_block_payload();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let predecessor = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let target = authenticated_serve_request(
            &service.context,
            &keys[2],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let predecessor_response = certified_serve_response(
            &predecessor,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let predecessor_requester = predecessor.request().requester.clone();
        let target_requester = target.request().requester.clone();
        let via = service.context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let predecessor_route = routes.mint_via(predecessor_requester.clone(), via.clone());
        let target_route = routes.mint_via(target_requester.clone(), via.clone());
        let (command_tx, command_rx, admission) = test_io_command_channel(1);

        let predecessor_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(predecessor_requester.clone()),
                predecessor.clone(),
            )
            .expect("reserve the sole Serve unit for the frozen predecessor");
        let predecessor_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(
                predecessor_admission.request.clone(),
            ),
        ));
        let (predecessor_routes, predecessor_ownership) = fair_ingress_route_owner(
            predecessor_message,
            predecessor_requester,
            via.clone(),
            predecessor_route,
        );
        assert!(matches!(
            command_tx
                .commit_serve(
                    &predecessor_admission,
                    predecessor_routes,
                    predecessor_ownership,
                )
                .expect("commit the frozen Serve predecessor"),
            CertifiedServeCommit::Queued
        ));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == predecessor_admission.lifecycle_id
        ));
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);

        let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                target.request(),
                via,
                target_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(
            ingress
                .try_recv_if(|_| {
                    matches!(
                        command_tx.prepare_reserved_serve(
                            CertifiedServeOwnerKey::Roster(target_requester.clone()),
                            target.clone(),
                        ),
                        Ok(_)
                    )
                })
                .is_none(),
            "the exact carrier remains Fair-owned while its Serve predecessor owns capacity"
        );
        let target_id = {
            let state = command_tx.queue.lock();
            let target_id = state
                .serve_barrier
                .expect("the target owns the off-queue Serve barrier");
            assert_eq!(
                state.serves.get(&target_id).map(|tracked| tracked.state),
                Some(V2IoServeState::PendingCapacity)
            );
            assert_eq!(
                state.serve_barrier_predecessors,
                BTreeSet::from([predecessor_admission.lifecycle_id])
            );
            target_id
        };

        command_rx
            .complete_serve_response(predecessor_admission.lifecycle_id, &predecessor_response)
            .expect("seal the frozen predecessor response");
        command_tx
            .acknowledge_serve_completion(
                predecessor_admission.lifecycle_id,
                V2IoServeTerminal::Response(predecessor_response),
            )
            .expect("acknowledge only the frozen Serve predecessor");
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state.serves.get(&target_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Reserved)
            );
            assert_eq!(
                state
                    .commands
                    .front()
                    .and_then(V2IoCommand::serve_lifecycle_id),
                Some(target_id)
            );
            assert!(state.serve_barrier_predecessors.is_empty());
        }

        let (target_admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(target_requester),
            &target,
        );
        assert_eq!(target_admission.lifecycle_id, target_id);
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire Serve-only prefix fixture gate");
    }

    #[test]
    fn fair_ingress_terminal_retry_replays_without_lifecycle_resurrection() {
        let (service, keys) = fixture_with_block_payload();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let request = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = service.context.roster[0].validator.clone();
        let mut route_fixture = NetworkReplyRouteTestFixture::new(via.clone());
        let initial_route = route_fixture.mint_via(requester.clone(), via.clone());
        let retry_route = route_fixture
            .redeliver(&initial_route)
            .expect("redeliver drained exact request");
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let (command_tx, command_rx, _admission) = test_io_command_channel(2);
        let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);

        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via.clone(),
                initial_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (original, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester.clone()),
            &request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == original.lifecycle_id
        ));
        command_rx
            .complete_serve_response(original.lifecycle_id, &response)
            .expect("seal original terminal response");
        command_tx
            .acknowledge_serve_completion(
                original.lifecycle_id,
                V2IoServeTerminal::Response(response.clone()),
            )
            .expect("terminalize original exact lifecycle");

        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (retry, replay) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(retry.lifecycle_id, original.lifecycle_id);
        assert!(matches!(
            replay,
            CertifiedServeCommit::Replay {
                response: replayed,
                ..
            } if replayed == response
        ));
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_admission_ordinal, 1);
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal, 2,
                "an exact terminal retry retains its tombstone but mints a fresh physical scheduler ordinal"
            );
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(state.serves.len(), 1);
            assert_eq!(
                state
                    .serves
                    .get(&original.lifecycle_id)
                    .map(|serve| serve.state),
                Some(V2IoServeState::Terminal)
            );
            assert!(state.commands.is_empty());
        }

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire terminal replay fixture gate");
    }

    #[test]
    fn fair_ingress_higher_view_waits_out_active_family_before_admission() {
        let (service, keys) = fixture_with_block_payload();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let original = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let higher = authenticated_serve_request(
            &service.context,
            &keys[1],
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = original.request().requester.clone();
        let via = service.context.roster[0].validator.clone();
        let mut route_fixture = NetworkReplyRouteTestFixture::new(via.clone());
        let original_route = route_fixture.mint_via(requester.clone(), via.clone());
        let blocked_higher_route = route_fixture.mint_via(requester.clone(), via.clone());
        let admitted_higher_route = route_fixture
            .redeliver(&blocked_higher_route)
            .expect("retry higher-view route after terminalization");
        let response = certified_serve_response(
            &original,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let (command_tx, command_rx, _admission) = test_io_command_channel(2);
        let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);

        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                original.request(),
                via.clone(),
                original_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (original_admission, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester.clone()),
            &original,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        let before_higher = fair_ingress_accounting_snapshot(&ingress);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                higher.request(),
                via.clone(),
                blocked_higher_route,
            )),
            Err(FairV2IngressPushError::Full(_))
        ));
        assert_eq!(
            fair_ingress_accounting_snapshot(&ingress),
            before_higher,
            "a replacement cannot become selector-visible while its family is active"
        );
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == original_admission.lifecycle_id
        ));
        command_rx
            .complete_serve_response(original_admission.lifecycle_id, &response)
            .expect("seal original family response");
        command_tx
            .acknowledge_serve_completion(
                original_admission.lifecycle_id,
                V2IoServeTerminal::Response(response),
            )
            .expect("terminalize original family");

        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                higher.request(),
                via.clone(),
                admitted_higher_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (replacement, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &higher,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert_eq!(
            replacement.lifecycle_id.admission_ordinal,
            original_admission.lifecycle_id.admission_ordinal + 1
        );
        let resurrected_route =
            route_fixture.mint_via(original.request().requester.clone(), via.clone());
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                original.request(),
                via,
                resurrected_route,
            )),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 2);
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state
                    .serve_by_family
                    .get(&CertifiedServeFamilyKey {
                        requester: higher.request().requester.clone(),
                        phase: higher.request().certificate.phase,
                    })
                    .copied(),
                Some(replacement.lifecycle_id),
                "the logical higher-view owner remains without resurrecting a drained ticket"
            );
        }

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire active-family fixture gate");
    }

    #[test]
    fn durable_serve_restart_before_terminal_seal_uses_fresh_physical_ordinal() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let initial_route = routes.mint_via(requester.clone(), via.clone());
        let retry_route = routes
            .redeliver(&initial_route)
            .expect("redeliver exact request after unsealed restart");
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let durable_receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist exact body before serving");
        assert_durable_body_receipt_matches(&durable_receipt, &context, payload.manifest());

        let first_lifecycle = {
            let (command_tx, command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open first durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    initial_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let (admission, committed) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &request,
            );
            assert!(matches!(committed, CertifiedServeCommit::Queued));
            assert!(matches!(
                command_rx.try_recv(),
                Ok(V2IoCommand::Serve { lifecycle_id, .. })
                    if lifecycle_id == admission.lifecycle_id
            ));
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire unsealed crash fixture gate");
            admission.lifecycle_id
        };

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restore queue after crash before terminal seal");
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert_eq!(state.next_serve_admission_ordinal, 1);
            assert_eq!(
                state.serve_ingress_waiters.len(),
                0,
                "a successfully drained physical occurrence cannot be restored"
            );
            assert_eq!(
                state.serves.get(&first_lifecycle).map(|serve| serve.state),
                Some(V2IoServeState::AwaitingRetry),
                "unsealed active work restores as a non-runnable exact retry owner"
            );
            assert!(state.commands.is_empty());
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let restored_barrier = command_tx
            .serve_barrier()
            .expect("inspect retransmitted exact request")
            .expect("retransmission reserves a fresh physical carrier");
        assert_eq!(
            restored_barrier.scheduler_ordinal(),
            2,
            "restart retains the logical lifecycle but not its drained physical ordinal"
        );
        assert!(
            command_tx
                .claim_serve_runtime_episode(restored_barrier)
                .expect("restart reconstructs the volatile episode claim"),
            "the claim may reset across a crash; liveness assumes a suffix without infinitely recurring restarts, while the restored high-watermark keeps all new runtime owners younger"
        );
        let (retried, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert_eq!(
            retried.lifecycle_id, first_lifecycle,
            "a lost unsealed episode resumes its original monotone lifecycle"
        );
        assert_eq!(command_tx.queue.lock().next_serve_admission_ordinal, 1);
        assert_eq!(
            command_tx
                .queue
                .lock()
                .next_serve_ingress_reservation_ordinal,
            2,
            "restart retry advances to a fresh physical scheduler ordinal"
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire restored unsealed fixture gate");
    }

    #[test]
    fn durable_new_physical_drain_before_commit_restarts_at_fresh_ordinal() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let initial_route = routes.mint_via(requester.clone(), via.clone());
        let retry_route = routes
            .redeliver(&initial_route)
            .expect("redeliver exact request after pre-commit crash");
        let body_root = TempDir::new().expect("pre-commit crash body root");
        let serve_root = TempDir::new().expect("pre-commit crash Serve root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");

        let lifecycle_id = {
            let (command_tx, _command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open pre-commit crash queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    initial_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let drained_ordinal = command_tx
                .serve_barrier()
                .expect("inspect pre-commit crash barrier")
                .expect("new request owns a physical barrier")
                .scheduler_ordinal();
            let mut admission = None;
            let inbound = ingress
                .try_recv_if_checked(|_| {
                    admission = Some(
                        command_tx
                            .prepare_reserved_serve(
                                CertifiedServeOwnerKey::Roster(requester.clone()),
                                request.clone(),
                            )
                            .expect("prepare new pre-commit crash handoff"),
                    );
                    true
                })
                .expect("publish new physical drain")
                .expect("drain new physical carrier");
            drop(inbound);
            let admission = admission.expect("retain new pre-commit admission");
            assert!(matches!(
                command_tx.prepare_reserved_serve(
                    CertifiedServeOwnerKey::Roster(requester.clone()),
                    request.clone(),
                ),
                Err(CertifiedServePrepareError::Service(reason))
                    if reason.contains("attempted to prepare one physically drained occurrence twice")
            ));
            {
                let state = command_tx.queue.lock();
                assert_eq!(
                    state
                        .serve_ingress_reservation
                        .as_ref()
                        .map(|reservation| reservation.state),
                    Some(
                        CertifiedServeIngressReservationState::PhysicallyDrainedPrepared(
                            admission.lifecycle_id
                        )
                    )
                );
                command_tx
                    .queue
                    .persist_serve_state(
                        &state,
                        state.next_serve_ingress_reservation_ordinal,
                        state.next_serve_admission_ordinal,
                        None,
                        None,
                        None,
                        None,
                    )
                    .expect("unrelated later snapshot keeps drained occurrence omitted");
            }
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("fixture retains its Serve state store")
                .load(&context)
                .expect("reload pre-commit crash snapshot");
            assert!(
                persisted
                    .ingress_waiters
                    .iter()
                    .all(|waiter| waiter.ingress_ordinal != drained_ordinal),
                "later snapshots cannot republish a physically drained occurrence"
            );
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire pre-commit crash gate");
            admission.lifecycle_id
        };

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restart after new physical drain before Commit");
        {
            let state = command_tx.queue.lock();
            assert!(state.commands.is_empty());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|serve| serve.state),
                Some(V2IoServeState::AwaitingRetry)
            );
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            command_tx
                .serve_barrier()
                .expect("inspect post-crash retransmission")
                .expect("post-crash retransmission owns a barrier")
                .scheduler_ordinal(),
            2,
            "post-crash retransmission receives a fresh physical ordinal"
        );
        let (resumed, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(resumed.lifecycle_id, lifecycle_id);
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire restored pre-commit crash gate");
    }

    #[test]
    fn drained_prepared_teardown_never_restores_a_waiter() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();

        for close_receiver in [false, true] {
            let body_root = TempDir::new().expect("drained teardown body root");
            let serve_root = TempDir::new().expect("drained teardown Serve root");
            let body_store =
                V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
            let (command_tx, command_rx, admission_owner) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open drained teardown queue");
            let mut command_rx = Some(command_rx);
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
            let route = routes.mint_via(requester.clone(), via.clone());
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let drained_ordinal = command_tx
                .serve_barrier()
                .expect("inspect drained teardown barrier")
                .expect("teardown request owns a physical barrier")
                .scheduler_ordinal();
            let mut prepared = None;
            let inbound = ingress
                .try_recv_if_checked(|_| {
                    prepared = Some(
                        command_tx
                            .prepare_reserved_serve(
                                CertifiedServeOwnerKey::Roster(requester.clone()),
                                request.clone(),
                            )
                            .expect("prepare drained teardown handoff"),
                    );
                    true
                })
                .expect("publish drained teardown occurrence")
                .expect("drain teardown carrier");
            drop(inbound);
            let lifecycle_id = prepared
                .as_ref()
                .expect("retain teardown admission")
                .lifecycle_id;

            if close_receiver {
                drop(command_rx.take());
            } else {
                command_tx
                    .rollback_serve_barrier_for_shutdown()
                    .expect("explicit shutdown retires drained prepared handoff");
            }
            {
                let state = command_tx.queue.lock();
                assert!(state.serve_barrier.is_none());
                assert!(state.serve_ingress_reservation.is_none());
                assert!(state.serve_ingress_waiters.is_empty());
                assert!(state.commands.is_empty());
                assert_eq!(
                    state.serves.get(&lifecycle_id).map(|serve| serve.state),
                    Some(V2IoServeState::AwaitingRetry)
                );
            }
            assert_eq!(
                admission_owner.queued.load(AtomicOrdering::Acquire),
                0,
                "teardown releases the uncommitted physical queue unit"
            );
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("fixture retains its Serve state store")
                .load(&context)
                .expect("reload drained teardown snapshot");
            assert!(
                persisted
                    .ingress_waiters
                    .iter()
                    .all(|waiter| waiter.ingress_ordinal != drained_ordinal),
                "neither shutdown nor receiver close can resurrect a drained waiter"
            );
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire drained teardown fixture gate");
        }
    }

    #[test]
    fn durable_coalesced_retransmission_restart_uses_fresh_physical_ordinal() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let initial_route = routes.mint_via(requester.clone(), via.clone());
        let coalesced_route = routes
            .redeliver(&initial_route)
            .expect("redeliver exact request while its command is queued");
        let post_restart_route = routes
            .redeliver(&coalesced_route)
            .expect("redeliver exact request after coalesced-retry restart");
        let body_root = TempDir::new().expect("coalesced-retry body root");
        let serve_root = TempDir::new().expect("coalesced-retry Serve root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");

        let (lifecycle_id, coalesced_ingress_ordinal) = {
            let (command_tx, _command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open first coalesced-retry queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    initial_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let first_barrier = command_tx
                .serve_barrier()
                .expect("inspect first coalesced-retry barrier")
                .expect("first request owns a physical barrier");
            let (first_admission, first_commit) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &request,
            );
            assert!(matches!(first_commit, CertifiedServeCommit::Queued));

            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    coalesced_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let coalesced_barrier = command_tx
                .serve_barrier()
                .expect("inspect coalesced retransmission barrier")
                .expect("coalesced retransmission owns a fresh physical barrier");
            assert!(coalesced_barrier.scheduler_ordinal() > first_barrier.scheduler_ordinal());
            let mut coalesced_admission = None;
            let inbound = ingress
                .try_recv_if_checked(|_| {
                    coalesced_admission = Some(
                        command_tx
                            .prepare_reserved_serve(
                                CertifiedServeOwnerKey::Roster(requester.clone()),
                                request.clone(),
                            )
                            .expect("prepare coalesced retransmission handoff"),
                    );
                    true
                })
                .expect("publish coalesced physical drain")
                .expect("drain coalesced physical carrier");
            drop(inbound);
            let coalesced_admission =
                coalesced_admission.expect("retain coalesced pre-commit admission");
            assert_eq!(
                coalesced_admission.lifecycle_id, first_admission.lifecycle_id,
                "coalescing retains the immutable logical Serve lifecycle"
            );
            {
                let state = command_tx.queue.lock();
                assert_eq!(
                    state
                        .serve_ingress_reservation
                        .as_ref()
                        .map(|reservation| reservation.state),
                    Some(
                        CertifiedServeIngressReservationState::PhysicallyDrainedPrepared(
                            first_admission.lifecycle_id
                        )
                    ),
                    "crash window retains only the volatile Commit handoff"
                );
                assert!(state.serve_ingress_waiters.is_empty());
                assert_eq!(
                    state
                        .commands
                        .iter()
                        .filter(|command| {
                            command.serve_lifecycle_id() == Some(first_admission.lifecycle_id)
                        })
                        .count(),
                    1,
                    "coalesced retransmission cannot duplicate its queued command"
                );
            }
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("fixture retains its Serve state store")
                .load(&context)
                .expect("reload snapshot after coalesced drain");
            assert!(
                persisted.ingress_waiters.iter().all(|waiter| {
                    waiter.ingress_ordinal != coalesced_barrier.scheduler_ordinal()
                }),
                "coalesced physical occurrence is durable before Commit and restart"
            );
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire first coalesced-retry gate");
            (
                first_admission.lifecycle_id,
                coalesced_barrier.scheduler_ordinal(),
            )
        };

        let (command_tx, command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restart after coalesced exact retransmission");
        {
            let state = command_tx.queue.lock();
            assert!(state.commands.is_empty());
            assert!(state.serve_ingress_reservation.is_none());
            assert!(state.serve_ingress_waiters.is_empty());
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|serve| serve.state),
                Some(V2IoServeState::AwaitingRetry)
            );
            assert_eq!(
                state.next_serve_ingress_reservation_ordinal,
                coalesced_ingress_ordinal
            );
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                post_restart_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let post_restart_barrier = command_tx
            .serve_barrier()
            .expect("inspect post-coalesced-restart barrier")
            .expect("post-restart retransmission owns a physical barrier");
        assert!(
            post_restart_barrier.scheduler_ordinal() > coalesced_ingress_ordinal,
            "restart retransmission cannot restore the consumed coalesced ordinal"
        );
        let (resumed, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(
            resumed.lifecycle_id, lifecycle_id,
            "restart resumes the same immutable logical lifecycle"
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve {
                lifecycle_id: queued,
                ..
            }) if queued == lifecycle_id
        ));
        assert!(matches!(
            command_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));

        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire post-coalesced-restart gate");
    }

    #[test]
    fn restored_serve_waiter_advances_shared_runtime_source() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let body_root = TempDir::new().expect("restored waiter body root");
        let serve_root = TempDir::new().expect("restored waiter state root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
        let family_capacity = certified_serve_family_capacity(context.roster.len(), 4, 4)
            .expect("fixture family capacity");
        let (store, _) =
            CertifiedServeStateStore::open(serve_root.path(), &context, family_capacity)
                .expect("open durable Serve state");
        let lifecycle_id = CertifiedServeLifecycleId {
            admission_ordinal: 1,
            request_hash: request.request_hash(),
        };
        let mut persisted = PersistedCertifiedServeState::empty(&context);
        persisted.next_ingress_reservation_ordinal = 41;
        persisted.next_lifecycle_admission_ordinal = 1;
        persisted
            .ingress_waiters
            .push(PersistedCertifiedServeIngressWaiter {
                ingress_ordinal: 41,
                lifecycle_id,
                owner: CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                request: request.request().clone(),
            });
        persisted
            .unsealed_lifecycles
            .push(PersistedCertifiedServeLifecycle {
                lifecycle_id,
                owner: CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                request: request.request().clone(),
            });
        store
            .persist(&persisted)
            .expect("persist exact undrained waiter high-watermark");

        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let admission = V2IoAdmission::unbounded_for_tests();
        let (command_tx, _command_rx) = persistent_v2_io_command_channel(
            4,
            context.roster.len(),
            4,
            4,
            Arc::clone(&admission),
            serve_root.path(),
            &context,
            Some(0),
            None,
            &body_store,
            lifecycle_ordinals.clone(),
            CertifiedServeRestartDischarge::PreserveFixtureState,
        )
        .expect("restore waiter with shared actor-global source");
        assert!(
            command_tx
                .queue
                .lock()
                .serve_ingress_waiters
                .contains_key(&CertifiedServeIngressReservationId(41))
        );
        assert_eq!(
            lifecycle_ordinals
                .reserve_one()
                .expect("new runtime owner follows restored waiter"),
            42
        );
    }

    #[test]
    fn durable_serve_abort_before_commit_restarts_retry_only_without_runnable_work() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let rejected_route = routes.mint_via(requester.clone(), via.clone());
        let retry_route = routes
            .redeliver(&rejected_route)
            .expect("redeliver exact request after abort");
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");

        let lifecycle_id = {
            let (command_tx, _command_rx, admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open first durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    rejected_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let mut prepared = None;
            let inbound = ingress
                .try_recv_if(|_| {
                    prepared = Some(
                        command_tx
                            .prepare_reserved_serve(
                                CertifiedServeOwnerKey::Roster(requester.clone()),
                                request.clone(),
                            )
                            .expect("prepare exact Serve before route rejection"),
                    );
                    true
                })
                .expect("remove the rejected exact carrier from Fair ingress");
            drop(inbound);
            let prepared = prepared.expect("prepared abort fixture lifecycle");
            let lifecycle_id = prepared.lifecycle_id;
            command_tx
                .abort_serve(prepared)
                .expect("abort prepared physical Serve handoff");
            {
                let state = command_tx.queue.lock();
                assert!(state.serve_barrier.is_none());
                assert!(state.serve_barrier_predecessors.is_empty());
                assert!(state.serve_ingress_reservation.is_none());
                assert!(state.commands.is_empty());
                assert_eq!(
                    state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
                    Some(V2IoServeState::AwaitingRetry)
                );
                assert_eq!(state.next_serve_admission_ordinal, 1);
            }
            assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire abort-before-commit fixture gate");
            lifecycle_id
        };

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restore retry-only owner after abort-before-commit");
        {
            let state = command_tx.queue.lock();
            assert!(state.commands.is_empty());
            assert!(
                state.serve_ingress_waiters.is_empty(),
                "a completed abort cannot restore its drained physical occurrence"
            );
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
                Some(V2IoServeState::AwaitingRetry)
            );
            assert_eq!(state.next_serve_admission_ordinal, 1);
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            command_tx
                .serve_barrier()
                .expect("inspect post-abort retransmission")
                .expect("post-abort retransmission owns a physical carrier")
                .scheduler_ordinal(),
            2,
            "post-abort retransmission must not reuse the drained scheduler ordinal"
        );
        let (resumed, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(resumed.lifecycle_id, lifecycle_id);
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert_eq!(command_tx.queue.lock().next_serve_admission_ordinal, 1);
        assert_eq!(
            command_tx
                .queue
                .lock()
                .next_serve_ingress_reservation_ordinal,
            2
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire restored abort fixture gate");
    }

    #[test]
    fn durable_serve_seal_before_completion_post_restores_terminal_replay() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let initial_route = routes.mint_via(requester.clone(), via.clone());
        let retry_route = routes
            .redeliver(&initial_route)
            .expect("redeliver exact request after sealed restart");
        let post_replay_restart_route = routes
            .redeliver(&retry_route)
            .expect("redeliver exact request after terminal replay restart");
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let durable_receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist exact body before serving");
        assert_durable_body_receipt_matches(&durable_receipt, &context, payload.manifest());

        let lifecycle_id = {
            let (command_tx, command_rx, admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open first durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    initial_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let (prepared, committed) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &request,
            );
            assert!(matches!(committed, CertifiedServeCommit::Queued));
            assert!(matches!(
                command_rx.try_recv(),
                Ok(V2IoCommand::Serve { lifecycle_id, .. })
                    if lifecycle_id == prepared.lifecycle_id
            ));
            command_rx
                .complete_serve_response(prepared.lifecycle_id, &response)
                .expect("persist terminal seal before completion exposure");
            assert_eq!(
                admission.queued.load(AtomicOrdering::Acquire),
                1,
                "sealing retains the physical admission owner until ack"
            );
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire sealed-before-post fixture gate");
            prepared.lifecycle_id
        };

        let (command_tx, _command_rx, admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restore sealed completion without its channel item");
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|serve| serve.state),
                Some(V2IoServeState::Terminal)
            );
            assert_eq!(state.next_serve_admission_ordinal, 1);
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via.clone(),
                retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (retried, replay) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester.clone()),
            &request,
        );
        assert_eq!(retried.lifecycle_id, lifecycle_id);
        assert!(matches!(
            replay,
            CertifiedServeCommit::Replay {
                response: replayed,
                ..
            } if replayed == response
        ));
        assert_eq!(command_tx.queue.lock().next_serve_admission_ordinal, 1);
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire restored sealed fixture gate");
        drop(ingress);
        drop(gate);
        drop(command_tx);
        drop(_command_rx);

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restart after the terminal replay drained its physical ingress");
        {
            let state = command_tx.queue.lock();
            assert!(state.serve_ingress_reservation.is_none());
            assert!(
                state.serve_ingress_waiters.is_empty(),
                "a drained terminal replay cannot restore its old physical scheduler owner"
            );
            assert_eq!(state.next_serve_ingress_reservation_ordinal, 2);
            assert_eq!(
                state.serves.get(&lifecycle_id).map(|serve| serve.state),
                Some(V2IoServeState::Terminal)
            );
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                post_replay_restart_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (retried_again, replayed_again) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(retried_again.lifecycle_id, lifecycle_id);
        assert!(matches!(
            replayed_again,
            CertifiedServeCommit::Replay {
                response: replayed,
                ..
            } if replayed == response
        ));
        assert_eq!(
            command_tx
                .queue
                .lock()
                .next_serve_ingress_reservation_ordinal,
            3,
            "post-restart retransmission reserves a fresh physical scheduler ordinal"
        );
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire post-replay restart fixture gate");
    }

    #[test]
    fn durable_serve_seal_survives_post_before_physical_ack() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let requester = request.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let initial_route = routes.mint_via(requester.clone(), via.clone());
        let retry_route = routes
            .redeliver(&initial_route)
            .expect("redeliver exact request after post-before-ack crash");
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let durable_receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist exact body before serving");
        assert_durable_body_receipt_matches(&durable_receipt, &context, payload.manifest());

        let lifecycle_id = {
            let (command_tx, command_rx, admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open first durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    request.request(),
                    via.clone(),
                    initial_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let (prepared, committed) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &request,
            );
            assert!(matches!(committed, CertifiedServeCommit::Queued));
            assert!(matches!(
                command_rx.try_recv(),
                Ok(V2IoCommand::Serve { lifecycle_id, .. })
                    if lifecycle_id == prepared.lifecycle_id
            ));
            command_rx
                .complete_serve_response(prepared.lifecycle_id, &response)
                .expect("seal terminal response before external post");
            let (recipient, reply_routes, ingress_ownership) = command_tx
                .serve_completion_ownership(prepared.lifecycle_id, response.request_hash)
                .expect("sealed completion retains exact post ownership");
            service
                .post_to_peer_on_reply_routes(
                    recipient,
                    reply_routes,
                    ingress_ownership,
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response.clone()),
                    ),
                )
                .expect("admit sealed response to the external actor corridor");
            assert_eq!(
                admission.queued.load(AtomicOrdering::Acquire),
                1,
                "external post does not acknowledge the physical completion owner"
            );
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire post-before-ack fixture gate");
            prepared.lifecycle_id
        };
        drop(service);

        let (command_tx, _command_rx, admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restore after external post before physical ack");
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                request.request(),
                via,
                retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (retried, replay) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &request,
        );
        assert_eq!(retried.lifecycle_id, lifecycle_id);
        assert!(matches!(
            replay,
            CertifiedServeCommit::Replay {
                response: replayed,
                ..
            } if replayed == response
        ));
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire restored post-before-ack fixture gate");
    }

    #[test]
    fn durable_serve_state_v5_rejects_v4_header_and_payload_layouts() {
        let (service, _) = fixture();
        let state = PersistedCertifiedServeState::empty(&service.context);
        let frame = encode_certified_serve_state_frame(&state, u64::MAX)
            .expect("encode current durable Serve state");
        let version_offset = CERTIFIED_SERVE_STATE_MAGIC.len();
        assert_eq!(
            u16::from_le_bytes(
                frame[version_offset..version_offset + 2]
                    .try_into()
                    .expect("fixed frame version width")
            ),
            5,
            "negative terminal outcomes own a new fixed codec version"
        );

        let mut v4_header = frame;
        v4_header[version_offset..version_offset + 2].copy_from_slice(&4_u16.to_le_bytes());
        let header_error = decode_certified_serve_state_frame(&v4_header, u64::MAX)
            .expect_err("the former frame version cannot decode the new waiter layout");
        assert!(
            header_error.contains("unsupported version 4"),
            "unexpected former-header rejection: {header_error}"
        );

        let mut v4_payload = state;
        v4_payload.format_version = 4;
        let v4_payload_frame = encode_certified_serve_state_frame(&v4_payload, u64::MAX)
            .expect("encode checksummed former payload marker");
        let payload_error = decode_certified_serve_state_frame(&v4_payload_frame, u64::MAX)
            .expect_err("the former payload marker cannot claim the new waiter layout");
        assert!(
            payload_error.contains("payload uses unsupported version 4"),
            "unexpected former-payload rejection: {payload_error}"
        );
    }

    #[test]
    fn durable_serve_corruption_fails_closed_without_highwater_reset() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let via = context.roster[0].validator.clone();
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        {
            let (command_tx, _command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(request.request(), via)),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire corruption fixture ticket");
        }
        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        let mut corrupted = fs::read(&state_path).expect("read durable Serve state");
        let last = corrupted
            .last_mut()
            .expect("durable Serve frame has a checksum-protected payload");
        *last ^= 0xA5;
        fs::write(&state_path, &corrupted).expect("publish corrupt durable Serve state fixture");

        let error =
            match persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store) {
                Ok(_) => panic!("corrupt durable Serve state must fail closed"),
                Err(error) => error,
            };
        assert!(
            error.contains("checksum")
                || error.contains("decode")
                || error.contains("canonically encoded"),
            "unexpected corrupt restore error: {error}"
        );
        assert_eq!(
            fs::read(&state_path).expect("failed restore leaves corrupt evidence intact"),
            corrupted,
            "startup failure cannot silently replace the lost ordinal high-watermark"
        );
    }

    #[test]
    fn durable_serve_frame_bound_covers_max_layout_manifest_hashes() {
        let (service, keys) = fixture();
        let mut context = service.context.clone();
        context.da_layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 256 * 1024,
            data_shards: 4,
            parity_shards: 2,
            max_payload_size_bytes: 16 * 1024 * 1024,
            max_chunk_count: 1_024,
        };
        context
            .validate()
            .expect("recommended maximum layout context");
        let (_, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let lifecycle_id = CertifiedServeLifecycleId {
            admission_ordinal: 1,
            request_hash: request.request_hash(),
        };
        let mut manifest = payload.manifest().clone();
        manifest.chunk_hashes = (0..context.da_layout.max_chunk_count)
            .map(|index| Hash::new(index.to_le_bytes()))
            .collect();
        manifest.chunk_root = Hash::new(b"maximum durable Serve manifest hash vector");
        let expected = PersistedCertifiedServeState {
            format_version: CERTIFIED_SERVE_STATE_VERSION,
            context_id: context.id(),
            height: context.height,
            next_ingress_reservation_ordinal: 1,
            next_lifecycle_admission_ordinal: 1,
            ingress_waiters: vec![PersistedCertifiedServeIngressWaiter {
                ingress_ordinal: 1,
                lifecycle_id,
                owner: CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                request: request.request().clone(),
            }],
            unsealed_lifecycles: vec![PersistedCertifiedServeLifecycle {
                lifecycle_id,
                owner: CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                request: request.request().clone(),
            }],
            negative_tombstones: Vec::new(),
            terminal_tombstones: vec![PersistedCertifiedServeTombstone {
                lifecycle_id,
                owner: CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
                request: request.request().clone(),
                response_manifest: manifest,
                response_responder: 0,
                response_signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }],
        };
        let root = TempDir::new().expect("maximum durable Serve frame root");
        let (store, empty) =
            CertifiedServeStateStore::open(root.path(), &context, 1).expect("open bounded store");
        assert!(empty.terminal_tombstones.is_empty());
        store
            .persist(&expected)
            .expect("maximum-layout terminal record fits its derived frame bound");
        assert!(
            fs::metadata(&store.path)
                .expect("inspect maximum-layout durable frame")
                .len()
                <= store.max_frame_bytes
        );
        assert_eq!(
            store
                .load(&context)
                .expect("roundtrip maximum-layout durable frame"),
            expected
        );
    }

    #[test]
    fn durable_raw_higher_view_drop_retains_admitted_owner_and_displaced_terminal() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let lower = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let higher = authenticated_serve_request(
            &context,
            &keys[1],
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &lower,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let requester = lower.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let body_root = TempDir::new().expect("raw replacement-drop body root");
        let serve_root = TempDir::new().expect("raw replacement-drop Serve root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let _ = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist lower exact body");

        let (lower_id, higher_id, higher_scheduler_ordinal) = {
            let (command_tx, command_rx, _admission) =
                persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                    .expect("open raw replacement-drop queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(lower.request(), via.clone())),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let (lower_admission, committed) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &lower,
            );
            assert!(matches!(committed, CertifiedServeCommit::Queued));
            assert!(matches!(
                command_rx.try_recv(),
                Ok(V2IoCommand::Serve { lifecycle_id, .. })
                    if lifecycle_id == lower_admission.lifecycle_id
            ));
            command_rx
                .complete_serve_response(lower_admission.lifecycle_id, &response)
                .expect("seal lower terminal response");
            command_tx
                .acknowledge_serve_completion(
                    lower_admission.lifecycle_id,
                    V2IoServeTerminal::Response(response.clone()),
                )
                .expect("terminalize lower family");

            assert!(matches!(
                ingress.try_push(certified_serve_inbound(higher.request(), via.clone())),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let barrier = command_tx
                .serve_barrier()
                .expect("inspect raw higher ticket")
                .expect("raw higher replacement owns its physical ticket");
            let higher_admission = command_tx
                .prepare_reserved_serve(
                    CertifiedServeOwnerKey::Roster(requester.clone()),
                    higher.clone(),
                )
                .expect("prepare raw higher replacement before physical drain");
            assert_eq!(higher_admission.lifecycle_id, barrier.lifecycle_id());
            assert_eq!(
                higher_admission.ingress_reservation_id,
                Some(CertifiedServeIngressReservationId(
                    barrier.scheduler_ordinal()
                ))
            );
            assert!(
                command_tx
                    .queue
                    .lock()
                    .serve_replacements
                    .contains_key(&higher_admission.lifecycle_id)
            );

            // Closing fair ingress drops the still-undrained raw carrier. This
            // is a rollback to durable retry ownership, not a pre-gate abort.
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("detach prepared raw replacement carrier");
            {
                let state = command_tx.queue.lock();
                assert!(state.serve_barrier.is_none());
                assert!(state.serve_ingress_reservation.is_none());
                assert_eq!(
                    state
                        .serve_ingress_waiters
                        .get(&CertifiedServeIngressReservationId(
                            barrier.scheduler_ordinal()
                        ))
                        .map(|waiter| waiter.lifecycle_id),
                    Some(higher_admission.lifecycle_id)
                );
                assert_eq!(
                    state
                        .serves
                        .get(&higher_admission.lifecycle_id)
                        .map(|serve| serve.state),
                    Some(V2IoServeState::AwaitingRetry)
                );
                assert_eq!(
                    state
                        .serve_replacements
                        .get(&higher_admission.lifecycle_id)
                        .map(|(previous, serve)| (*previous, serve.state)),
                    Some((lower_admission.lifecycle_id, V2IoServeState::Terminal))
                );
            }
            let persisted = command_tx
                .queue
                .serve_state_store
                .as_ref()
                .expect("fixture retains durable Serve state")
                .load(&context)
                .expect("reload raw replacement rollback snapshot");
            assert_eq!(
                persisted
                    .ingress_waiters
                    .iter()
                    .map(|waiter| (waiter.ingress_ordinal, waiter.lifecycle_id))
                    .collect::<Vec<_>>(),
                vec![(barrier.scheduler_ordinal(), higher_admission.lifecycle_id)]
            );
            assert_eq!(
                persisted
                    .unsealed_lifecycles
                    .iter()
                    .map(|lifecycle| lifecycle.lifecycle_id)
                    .collect::<Vec<_>>(),
                vec![higher_admission.lifecycle_id]
            );
            assert_eq!(
                persisted
                    .terminal_tombstones
                    .iter()
                    .map(|tombstone| tombstone.lifecycle_id)
                    .collect::<Vec<_>>(),
                vec![lower_admission.lifecycle_id]
            );
            (
                lower_admission.lifecycle_id,
                higher_admission.lifecycle_id,
                barrier.scheduler_ordinal(),
            )
        };

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("restart after raw replacement carrier drop");
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state.serves.get(&higher_id).map(|serve| serve.state),
                Some(V2IoServeState::AwaitingRetry)
            );
            assert_eq!(
                state
                    .serve_replacements
                    .get(&higher_id)
                    .map(|(previous, serve)| (*previous, serve.state)),
                Some((lower_id, V2IoServeState::Terminal))
            );
            assert_eq!(
                state
                    .serve_ingress_waiters
                    .get(&CertifiedServeIngressReservationId(
                        higher_scheduler_ordinal
                    ))
                    .map(|waiter| waiter.lifecycle_id),
                Some(higher_id)
            );
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(lower.request(), via.clone())),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(higher.request(), via)),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let resumed = command_tx
            .serve_barrier()
            .expect("inspect resumed raw replacement")
            .expect("exact higher retry reattaches its dormant ticket");
        assert_eq!(resumed.lifecycle_id(), higher_id);
        assert_eq!(resumed.scheduler_ordinal(), higher_scheduler_ordinal);
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire resumed raw replacement fixture");
    }

    #[test]
    fn durable_higher_view_abort_republishes_displaced_terminal_before_restart() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let lower = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let higher = authenticated_serve_request(
            &context,
            &keys[1],
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &lower,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let requester = lower.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let lower_route = routes.mint_via(requester.clone(), via.clone());
        let replay_route = routes
            .redeliver(&lower_route)
            .expect("redeliver lower request after durable replacement abort");
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let durable_receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist lower exact body");
        assert_durable_body_receipt_matches(&durable_receipt, &context, payload.manifest());

        let lower_id = {
            let (command_tx, command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open durable replacement-abort queue");
            let lower_admission = command_tx
                .prepare_serve(
                    CertifiedServeOwnerKey::Roster(requester.clone()),
                    lower.clone(),
                )
                .expect("admit lower durable lifecycle");
            let lower_id = commit_and_terminalize_serve(
                &command_tx,
                &command_rx,
                &lower_admission,
                via,
                lower_route,
                response,
            );
            let higher_admission = command_tx
                .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), higher)
                .expect("publish higher durable replacement");
            assert_eq!(higher_admission.lifecycle_id.admission_ordinal, 2);
            assert!(
                command_tx
                    .queue
                    .lock()
                    .serve_replacements
                    .contains_key(&higher_admission.lifecycle_id)
            );
            command_tx
                .abort_serve(higher_admission)
                .expect("abort durable higher-view replacement");
            {
                let state = command_tx.queue.lock();
                assert_eq!(
                    state.serves.get(&lower_id).map(|tracked| tracked.state),
                    Some(V2IoServeState::Terminal)
                );
                assert!(state.serve_replacements.is_empty());
                assert_eq!(state.next_serve_admission_ordinal, 2);
            }
            lower_id
        };

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("restore displaced terminal after higher abort");
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state.serves.get(&lower_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Terminal)
            );
            assert_eq!(state.next_serve_admission_ordinal, 2);
            assert!(state.serve_replacements.is_empty());
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                lower.request(),
                context.roster[0].validator.clone(),
                replay_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (retried, replay) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &lower,
        );
        assert_eq!(retried.lifecycle_id, lower_id);
        assert!(matches!(replay, CertifiedServeCommit::Replay { .. }));
        assert_eq!(command_tx.queue.lock().next_serve_admission_ordinal, 2);
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire durable replacement-abort replay gate");
    }

    #[test]
    fn durable_higher_view_admission_crash_retains_lower_terminal_family() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let lower = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let higher = authenticated_serve_request(
            &context,
            &keys[1],
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let other_family = authenticated_serve_request(
            &context,
            &keys[2],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Commit,
        );
        let response = certified_serve_response(
            &lower,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let requester = lower.request().requester.clone();
        let via = context.roster[0].validator.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
        let lower_route = routes.mint_via(requester.clone(), via.clone());
        let higher_route = routes.mint_via(requester.clone(), via.clone());
        let higher_retry_route = routes
            .redeliver(&higher_route)
            .expect("redeliver higher request after replacement crash");
        let other_route = routes.mint_via(other_family.request().requester.clone(), via.clone());
        let replay_route = routes
            .redeliver(&lower_route)
            .expect("redeliver lower request after replacement crash");
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let durable_receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist lower exact body");
        assert_durable_body_receipt_matches(&durable_receipt, &context, payload.manifest());

        let (lower_id, higher_id) = {
            let (command_tx, command_rx, _admission) =
                persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                    .expect("open first durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    lower.request(),
                    via.clone(),
                    lower_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let (lower_admission, committed) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &lower,
            );
            assert!(matches!(committed, CertifiedServeCommit::Queued));
            assert!(matches!(
                command_rx.try_recv(),
                Ok(V2IoCommand::Serve { lifecycle_id, .. })
                    if lifecycle_id == lower_admission.lifecycle_id
            ));
            command_rx
                .complete_serve_response(lower_admission.lifecycle_id, &response)
                .expect("seal lower terminal response");
            command_tx
                .acknowledge_serve_completion(
                    lower_admission.lifecycle_id,
                    V2IoServeTerminal::Response(response.clone()),
                )
                .expect("terminalize lower family");

            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    higher.request(),
                    via.clone(),
                    higher_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            let (higher_admission, committed) = drain_and_commit_gated_serve(
                &ingress,
                &command_tx,
                CertifiedServeOwnerKey::Roster(requester.clone()),
                &higher,
            );
            assert!(matches!(committed, CertifiedServeCommit::Queued));
            assert_eq!(higher_admission.lifecycle_id.admission_ordinal, 2);
            assert!(
                command_tx
                    .queue
                    .lock()
                    .serve_replacements
                    .contains_key(&higher_admission.lifecycle_id),
                "committed replacement retains the old tombstone until its own seal"
            );

            // Force one more durable high-watermark rewrite after the lower
            // tombstone left `serves`. The displaced record must still be
            // projected from `serve_replacements`.
            assert!(matches!(
                ingress.try_push(certified_serve_inbound_with_route(
                    other_family.request(),
                    via.clone(),
                    other_route,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire post-replacement provisional ticket");
            (lower_admission.lifecycle_id, higher_admission.lifecycle_id)
        };

        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store)
                .expect("restore durable lower tombstone after replacement crash");
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_admission_ordinal, 2);
            assert_eq!(
                state.serves.get(&higher_id).map(|serve| serve.state),
                Some(V2IoServeState::AwaitingRetry)
            );
            assert_eq!(state.serves.len(), 1);
            assert_eq!(
                state
                    .serve_replacements
                    .get(&higher_id)
                    .map(|(previous_id, previous)| (*previous_id, previous.state)),
                Some((lower_id, V2IoServeState::Terminal)),
                "the lower response remains durable but cannot reclaim the active family"
            );
        }
        let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                lower.request(),
                via,
                replay_route,
            )),
            Err(FairV2IngressPushError::Rejected(_))
        ));
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_admission_ordinal, 2);
            assert_eq!(
                state.serves.get(&higher_id).map(|serve| serve.state),
                Some(V2IoServeState::AwaitingRetry),
                "an older family request cannot replace the restored high-watermark owner"
            );
            assert_eq!(
                state
                    .serve_replacements
                    .get(&higher_id)
                    .map(|(previous_id, previous)| (*previous_id, previous.state)),
                Some((lower_id, V2IoServeState::Terminal)),
                "rejected resurrection preserves the displaced terminal tombstone"
            );
        }

        assert!(matches!(
            ingress.try_push(certified_serve_inbound_with_route(
                higher.request(),
                context.roster[0].validator.clone(),
                higher_retry_route,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let (resumed, committed) = drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &higher,
        );
        assert_eq!(resumed.lifecycle_id, higher_id);
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert_eq!(command_tx.queue.lock().next_serve_admission_ordinal, 2);
        ingress.close();
        ingress
            .unbind_certified_serve_gate(&gate)
            .expect("retire restored lower replay fixture");
    }

    #[test]
    fn durable_serve_restore_rejects_capacity_owner_swap_across_replacement() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let observer = KeyPair::random();
        let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
        let lower = authenticated_serve_request(
            &context,
            &observer,
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let higher = authenticated_serve_request(
            &context,
            &observer,
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let response = certified_serve_response(
            &lower,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let source_a = PeerId::new(KeyPair::random().public_key().clone());
        let source_b = PeerId::new(KeyPair::random().public_key().clone());
        let lower_id = CertifiedServeLifecycleId {
            admission_ordinal: 1,
            request_hash: lower.request_hash(),
        };
        let higher_id = CertifiedServeLifecycleId {
            admission_ordinal: 2,
            request_hash: higher.request_hash(),
        };
        let body_root = TempDir::new().expect("owner-swap durable body root");
        let serve_root = TempDir::new().expect("owner-swap durable Serve root");
        let mut body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        let durable_receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("persist owner-swap terminal body");
        assert_durable_body_receipt_matches(&durable_receipt, &context, payload.manifest());
        let family_capacity = certified_serve_family_capacity(context.roster.len(), 4, 4)
            .expect("owner-swap fixture family capacity");
        let (store, _) =
            CertifiedServeStateStore::open(serve_root.path(), &context, family_capacity)
                .expect("open owner-swap durable state");
        store
            .persist(&PersistedCertifiedServeState {
                format_version: CERTIFIED_SERVE_STATE_VERSION,
                context_id: context.id(),
                height: context.height,
                next_ingress_reservation_ordinal: 2,
                next_lifecycle_admission_ordinal: 2,
                ingress_waiters: Vec::new(),
                unsealed_lifecycles: vec![PersistedCertifiedServeLifecycle {
                    lifecycle_id: higher_id,
                    owner: CertifiedServeOwnerKey::AuthenticatedSource(source_b),
                    request: higher.request().clone(),
                }],
                negative_tombstones: Vec::new(),
                terminal_tombstones: vec![PersistedCertifiedServeTombstone {
                    lifecycle_id: lower_id,
                    owner: CertifiedServeOwnerKey::AuthenticatedSource(source_a),
                    request: lower.request().clone(),
                    response_manifest: response.manifest,
                    response_responder: response.responder,
                    response_signature: response.signature,
                }],
            })
            .expect("publish canonically checksummed owner-swap mutation");

        let error =
            match persistent_test_io_command_channel(4, serve_root.path(), &context, &body_store) {
                Ok(_) => panic!("replacement cannot switch its retained capacity owner"),
                Err(error) => error,
            };
        assert!(
            error.contains("same-owner strict predecessor"),
            "unexpected owner-swap rejection: {error}"
        );
    }

    #[test]
    fn durable_serve_state_is_pruned_only_with_successor_rollover_root() {
        let (service, keys) = fixture_with_block_payload();
        let context = service.context.clone();
        let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
        let request = authenticated_serve_request(
            &context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let via = context.roster[0].validator.clone();
        let body_root = TempDir::new().expect("durable Serve body root");
        let serve_root = TempDir::new().expect("durable Serve state root");
        let body_store =
            V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
        {
            let (command_tx, _command_rx, _admission) =
                persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                    .expect("open durable Serve queue");
            let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
            assert!(matches!(
                ingress.try_push(certified_serve_inbound(request.request(), via)),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            ingress.close();
            ingress
                .unbind_certified_serve_gate(&gate)
                .expect("retire rollover fixture ticket");
        }
        let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
        assert!(state_path.is_file());

        // `ProductionV2Services::finish_height` removes this exact per-context
        // root only after typed successor rollover authority is established.
        fs::remove_dir_all(serve_root.path()).expect("simulate established successor cleanup");
        let (command_tx, _command_rx, _admission) =
            persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
                .expect("a pruned successor root starts a fresh height-local store");
        let state = command_tx.queue.lock();
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 0);
        assert_eq!(state.next_serve_admission_ordinal, 0);
        assert!(state.serves.is_empty());
    }
