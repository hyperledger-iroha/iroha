    #[test]
    fn certified_serve_future_slot_blocks_control_and_consensus_replenishment() {
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
        let request_hash = request.request_hash();
        let requester = request.request().requester.clone();
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        command_tx
            .try_send_as(
                V2IoAdmissionClass::Auxiliary,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(1),
                    subject: proposal.subject,
                },
            )
            .expect("install the frozen auxiliary predecessor");

        assert!(matches!(
            command_tx.prepare_serve(
                CertifiedServeOwnerKey::Roster(requester.clone()),
                request.clone(),
            ),
            Err(CertifiedServePrepareError::Backpressure)
        ));
        let lifecycle_id = command_tx
            .queue
            .lock()
            .serve_barrier
            .expect("full auxiliary admission installs a future-slot owner");
        assert_eq!(lifecycle_id.admission_ordinal, 1);
        assert_eq!(lifecycle_id.request_hash, request_hash);
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&lifecycle_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::PendingCapacity)
        );

        for occurrence in 2..34 {
            for class in [V2IoAdmissionClass::Consensus, V2IoAdmissionClass::Control] {
                assert!(matches!(
                    command_tx.try_send_as(
                        class,
                        V2IoCommand::LoadCandidate {
                            acquisition_id: LockedCandidateAcquisitionId(occurrence),
                            subject: proposal.subject,
                        },
                    ),
                    Err(V2IoTrySendError::Full(_))
                ));
            }
        }
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(1),
                ..
            })
        ));
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&lifecycle_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::Reserved)
        );
        assert!(matches!(
            command_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));

        let prepared = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), request)
            .expect("materialized future slot is claimed by the exact target");
        assert_eq!(prepared.lifecycle_id, lifecycle_id);
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester, source.clone());
        commit_and_terminalize_serve(&command_tx, &command_rx, &prepared, source, route, response);
        let state = command_tx.queue.lock();
        assert!(state.serve_barrier.is_none());
        assert!(state.commands.is_empty());
        assert_eq!(
            state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
    }

    #[test]
    fn certified_serve_cross_relay_retry_replays_one_terminal_tombstone() {
        let (service, keys) = fixture_with_block_payload();
        let observer = KeyPair::random();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let request = authenticated_serve_request(
            &service.context,
            &observer,
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = request.request().requester.clone();
        let source_a = PeerId::new(KeyPair::random().public_key().clone());
        let source_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(source_a.clone(), 2);
        let route_a = route_fixture.mint_via(requester.clone(), source_a.clone());
        let route_b = route_fixture.mint_via(requester.clone(), source_b.clone());
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let expected_response = response.clone();
        let (command_tx, command_rx, _) = test_io_command_channel(4);
        let prepared = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_a.clone()),
                request.clone(),
            )
            .expect("admit observer Serve lifecycle");
        let lifecycle_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &prepared,
            source_a.clone(),
            route_a,
            response,
        );

        let retry = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_b.clone()),
                request,
            )
            .expect("cross-relay exact retry coalesces");
        assert_eq!(retry.lifecycle_id, lifecycle_id);
        assert_eq!(retry.kind, CertifiedServeAdmissionKind::Existing);
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(retry.request.clone()),
        ));
        let (routes, ownership) = fair_ingress_route_owner(message, requester, source_b, route_b);
        assert!(matches!(
            command_tx
                .commit_serve(&retry, routes, ownership)
                .expect("terminal retry replays cached response"),
            CertifiedServeCommit::Replay { response, .. } if response == expected_response
        ));
        let state = command_tx.queue.lock();
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state
                .serves
                .get(&lifecycle_id)
                .map(|tracked| &tracked.owner),
            Some(&CertifiedServeOwnerKey::AuthenticatedSource(source_a))
        );
        assert!(state.commands.is_empty());
    }

    #[test]
    fn certified_serve_terminal_replay_waits_for_barrier_then_bypasses_full_serve_fifo() {
        let (service, keys) = fixture_with_block_payload();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let terminal_request = authenticated_serve_request(
            &service.context,
            &keys[1],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let newer_request = authenticated_serve_request(
            &service.context,
            &keys[2],
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let terminal_response = certified_serve_response(
            &terminal_request,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let expected_terminal_response = terminal_response.clone();
        let newer_response = certified_serve_response(
            &newer_request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let terminal_requester = terminal_request.request().requester.clone();
        let newer_requester = newer_request.request().requester.clone();
        let terminal_source = PeerId::new(KeyPair::random().public_key().clone());
        let newer_source = PeerId::new(KeyPair::random().public_key().clone());
        let mut terminal_routes = NetworkReplyRouteTestFixture::new(terminal_source.clone());
        let mut newer_routes = NetworkReplyRouteTestFixture::new(newer_source.clone());
        let terminal_route =
            terminal_routes.mint_via(terminal_requester.clone(), terminal_source.clone());
        let newer_route = newer_routes.mint_via(newer_requester.clone(), newer_source.clone());
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("one Serve admission slot"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );

        let terminal_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(terminal_requester.clone()),
                terminal_request.clone(),
            )
            .expect("admit the terminal replay lifecycle");
        let terminal_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &terminal_admission,
            terminal_source.clone(),
            terminal_route.clone(),
            terminal_response,
        );
        assert_eq!(terminal_id.admission_ordinal, 1);

        command_tx
            .try_send_as(
                V2IoAdmissionClass::Auxiliary,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(70),
                    subject: proposal.subject,
                },
            )
            .expect("fill the sole auxiliary Serve admission slot");
        assert!(matches!(
            command_tx.prepare_serve(
                CertifiedServeOwnerKey::Roster(newer_requester.clone()),
                newer_request.clone(),
            ),
            Err(CertifiedServePrepareError::Backpressure)
        ));
        let newer_id = command_tx
            .queue
            .lock()
            .serve_barrier
            .expect("newer request owns the single future-slot barrier");
        assert_eq!(newer_id.admission_ordinal, 2);
        assert_eq!(
            command_tx
                .serve_barrier_request_hash()
                .expect("inspect the runner-visible Serve barrier"),
            Some(newer_request.request_hash())
        );
        assert_ne!(terminal_request.request_hash(), newer_id.request_hash);
        {
            let state = command_tx.queue.lock();
            assert_eq!(
                state.serves.get(&terminal_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Terminal)
            );
            assert_eq!(
                state.serves.get(&newer_id).map(|tracked| tracked.state),
                Some(V2IoServeState::PendingCapacity)
            );
            assert_eq!(state.next_serve_admission_ordinal, 2);
        }

        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(70),
                ..
            })
        ));
        let newer_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(newer_requester.clone()),
                newer_request.clone(),
            )
            .expect("materialized barrier is claimed by the newer request");
        assert_eq!(newer_admission.lifecycle_id, newer_id);
        assert_eq!(newer_admission.kind, CertifiedServeAdmissionKind::New);
        let newer_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(newer_admission.request.clone()),
        ));
        let (newer_reply_routes, newer_ownership) =
            fair_ingress_route_owner(newer_message, newer_requester, newer_source, newer_route);
        assert!(matches!(
            command_tx
                .commit_serve(&newer_admission, newer_reply_routes, newer_ownership)
                .expect("commit the materialized newer Serve barrier"),
            CertifiedServeCommit::Queued
        ));

        let queued_retry = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(terminal_requester.clone()),
                terminal_request.clone(),
            )
            .expect("terminal retry bypasses a queued newer Serve job");
        assert_eq!(queued_retry.lifecycle_id, terminal_id);
        assert_eq!(queued_retry.kind, CertifiedServeAdmissionKind::Existing);
        let queued_retry_route = terminal_routes
            .redeliver(&terminal_route)
            .expect("redeliver the terminal request while newer work is queued");
        let queued_retry_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(queued_retry.request.clone()),
        ));
        let (queued_retry_routes, queued_retry_ownership) = fair_ingress_route_owner(
            queued_retry_message,
            terminal_requester.clone(),
            terminal_source.clone(),
            queued_retry_route.clone(),
        );
        assert!(matches!(
            command_tx
                .commit_serve(&queued_retry, queued_retry_routes, queued_retry_ownership)
                .expect("queued newer Serve work cannot delay cached replay"),
            CertifiedServeCommit::Replay { response, .. }
                if response == expected_terminal_response
        ));
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_admission_ordinal, 2);
            assert_eq!(state.serves.len(), 2);
            assert_eq!(
                state.serves.get(&terminal_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Terminal)
            );
            assert_eq!(
                state.serves.get(&newer_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Queued)
            );
            assert_eq!(
                state
                    .commands
                    .iter()
                    .filter_map(V2IoCommand::serve_lifecycle_id)
                    .collect::<Vec<_>>(),
                vec![newer_id]
            );
        }

        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. }) if lifecycle_id == newer_id
        ));
        let active_retry = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(terminal_requester.clone()),
                terminal_request,
            )
            .expect("terminal retry bypasses an active newer Serve job");
        assert_eq!(active_retry.lifecycle_id, terminal_id);
        assert_eq!(active_retry.kind, CertifiedServeAdmissionKind::Existing);
        let active_retry_route = terminal_routes
            .redeliver(&queued_retry_route)
            .expect("redeliver the terminal request while newer work is active");
        let active_retry_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(active_retry.request.clone()),
        ));
        let (active_retry_routes, active_retry_ownership) = fair_ingress_route_owner(
            active_retry_message,
            terminal_requester,
            terminal_source,
            active_retry_route,
        );
        assert!(matches!(
            command_tx
                .commit_serve(&active_retry, active_retry_routes, active_retry_ownership)
                .expect("active newer Serve work cannot delay cached replay"),
            CertifiedServeCommit::Replay { response, .. }
                if response == expected_terminal_response
        ));
        {
            let state = command_tx.queue.lock();
            assert_eq!(state.next_serve_admission_ordinal, 2);
            assert_eq!(state.serves.len(), 2);
            assert_eq!(
                state.serves.get(&terminal_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Terminal)
            );
            assert_eq!(
                state.serves.get(&newer_id).map(|tracked| tracked.state),
                Some(V2IoServeState::Active)
            );
            assert!(state.commands.is_empty());
        }
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);

        command_rx
            .complete_serve_response(newer_id, &newer_response)
            .expect("seal newer terminal Serve response");
        command_tx
            .acknowledge_serve_completion(newer_id, V2IoServeTerminal::Response(newer_response))
            .expect("finish the newer Serve fixture without changing the replay tombstone");
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    }

    #[test]
    fn certified_serve_terminal_replay_source_retains_retired_route_and_reconnects() {
        let (mut service, keys) = fixture_with_block_payload();
        let replay_admissions = Arc::new(AtomicUsize::new(0));
        let replay_admissions_for_hook = Arc::clone(&replay_admissions);
        service.set_exact_output_admission_hook(move |_post, ticket| {
            assert!(
                ticket.is_none(),
                "the first live replay owns no retry ticket"
            );
            replay_admissions_for_hook.fetch_add(1, AtomicOrdering::AcqRel);
            Ok(())
        });
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
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let initial_route = route_fixture.mint_via(requester.clone(), source.clone());
        let response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        let original = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(requester.clone()),
                request.clone(),
            )
            .expect("admit original exact Serve request");
        let lifecycle_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &original,
            source.clone(),
            initial_route.clone(),
            response,
        );

        let retired_retry = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(requester.clone()),
                request.clone(),
            )
            .expect("prepare terminal exact retry");
        assert_eq!(retired_retry.lifecycle_id, lifecycle_id);
        assert_eq!(retired_retry.kind, CertifiedServeAdmissionKind::Existing);
        let retired_route = route_fixture
            .redeliver(&initial_route)
            .expect("redeliver exact request on its retained tenure");
        let request_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(retired_retry.request.clone()),
        ));
        let (routes, ownership) = fair_ingress_route_owner(
            request_message,
            requester.clone(),
            source.clone(),
            retired_route.clone(),
        );
        assert!(
            route_fixture.retire(&retired_route),
            "sole retry route retires before terminal replay commit"
        );
        let (recipient, routes, ownership, response) = match command_tx
            .commit_serve(&retired_retry, routes, ownership)
            .expect("retired-route exact retry coalesces with its tombstone")
        {
            CertifiedServeCommit::Replay {
                recipient,
                reply_routes,
                ingress_ownership,
                response,
            } => (recipient, reply_routes, ingress_ownership, response),
            _ => panic!("terminal exact retry must replay its retained response"),
        };
        assert!(routes.is_empty());
        service
            .post_to_peer_on_reply_routes(
                recipient,
                routes,
                ownership,
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                ),
            )
            .expect("empty validated retry routes remain source-retained");
        assert!(!service.output_guard.restart_required());
        assert!(
            !service
                .has_pending_exact_output()
                .expect("inspect empty replay fanout"),
            "a retired sole route cannot create an empty exact-output fanout"
        );

        let reconnected = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), request)
            .expect("prepare retry after requester reconnect");
        assert_eq!(reconnected.lifecycle_id, lifecycle_id);
        let reconnected_route = route_fixture.mint_via(requester.clone(), source.clone());
        let request_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(reconnected.request.clone()),
        ));
        let (routes, ownership) =
            fair_ingress_route_owner(request_message, requester, source, reconnected_route);
        let (recipient, routes, ownership, response) = match command_tx
            .commit_serve(&reconnected, routes, ownership)
            .expect("reconnected exact retry merges into its tombstone")
        {
            CertifiedServeCommit::Replay {
                recipient,
                reply_routes,
                ingress_ownership,
                response,
            } => (recipient, reply_routes, ingress_ownership, response),
            _ => panic!("reconnected terminal exact retry must replay"),
        };
        assert!(!routes.is_empty());
        service
            .post_to_peer_on_reply_routes(
                recipient,
                routes,
                ownership,
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                ),
            )
            .expect("reconnected exact retry acquires an active replay route");
        assert_eq!(replay_admissions.load(AtomicOrdering::Acquire), 1);
        assert!(!service.output_guard.restart_required());
    }

    #[test]
    fn certified_serve_terminal_rejects_mismatched_response_hash_without_releasing_owner() {
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
        let mut response = certified_serve_response(
            &request,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let correct_response = response.clone();
        response.request_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"wrong Serve response request hash"));
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        let prepared = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), request)
            .expect("prepare exact Serve lifecycle");
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester.clone(), source.clone());
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(prepared.request.clone()),
        ));
        let (routes, ownership) = fair_ingress_route_owner(message, requester, source, route);
        assert!(matches!(
            command_tx
                .commit_serve(&prepared, routes, ownership)
                .expect("commit exact Serve lifecycle"),
            CertifiedServeCommit::Queued
        ));
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Serve { lifecycle_id, .. })
                if lifecycle_id == prepared.lifecycle_id
        ));
        command_rx
            .complete_serve_response(prepared.lifecycle_id, &correct_response)
            .expect("seal exact terminal response");

        let error = command_tx
            .serve_completion_ownership(prepared.lifecycle_id, response.request_hash)
            .expect_err("wrong request hash cannot acquire pre-send ownership");
        assert!(error.contains("changed its exact Serve request hash before delivery"));
        command_tx
            .serve_completion_ownership(prepared.lifecycle_id, correct_response.request_hash)
            .expect("correct request hash retains pre-send ownership");

        let error = command_tx
            .acknowledge_serve_completion(
                prepared.lifecycle_id,
                V2IoServeTerminal::Response(response),
            )
            .expect_err("wrong request hash cannot become a replay tombstone");
        assert!(error.contains("changed its exact Serve request hash"));
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&prepared.lifecycle_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::CompletionPending)
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);

        command_tx
            .acknowledge_serve_completion(
                prepared.lifecycle_id,
                V2IoServeTerminal::Response(correct_response),
            )
            .expect("correct exact response becomes the terminal tombstone");
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    }

    #[test]
    fn certified_serve_observer_owner_contains_prepare_and_commit_subfamilies() {
        let (service, keys) = fixture_with_block_payload();
        let observer = KeyPair::random();
        let other_observer = KeyPair::random();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let prepare = authenticated_serve_request(
            &service.context,
            &observer,
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let commit = authenticated_serve_request(
            &service.context,
            &observer,
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Commit,
        );
        let source_a = PeerId::new(KeyPair::random().public_key().clone());
        let source_b = PeerId::new(KeyPair::random().public_key().clone());
        let requester = prepare.request().requester.clone();
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source_a.clone());
        let route = route_fixture.mint_via(requester, source_a.clone());
        let response = certified_serve_response(
            &prepare,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("one auxiliary owner"));
        let (command_tx, command_rx) =
            v2_io_command_channel(admission.capacity(), 0, 1, 1, Arc::clone(&admission));
        let prepared = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_a.clone()),
                prepare,
            )
            .expect("observer Prepare family");
        commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &prepared,
            source_a.clone(),
            route,
            response,
        );

        let commit_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_b),
                commit,
            )
            .expect("same observer owns an independent Commit subfamily");
        assert_eq!(commit_admission.lifecycle_id.admission_ordinal, 2);
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&commit_admission.lifecycle_id)
                .map(|tracked| tracked.owner.clone()),
            Some(CertifiedServeOwnerKey::AuthenticatedSource(
                source_a.clone()
            ))
        );
        command_tx
            .abort_serve(commit_admission)
            .expect("abort observer ownership replacement");

        let other = authenticated_serve_request(
            &service.context,
            &other_observer,
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        assert!(matches!(
            command_tx.prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_a),
                other,
            ),
            Err(CertifiedServePrepareError::Rejected(reason))
                if reason.contains("bounded Serve quota")
        ));
    }

    #[test]
    fn certified_serve_higher_view_abort_restores_terminal_high_watermark() {
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
        let requester = original.request().requester.clone();
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester.clone(), source.clone());
        let response = certified_serve_response(
            &original,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let (command_tx, command_rx, _) = test_io_command_channel(4);
        let original_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::Roster(requester.clone()),
                original.clone(),
            )
            .expect("admit original view");
        let original_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &original_admission,
            source,
            route,
            response,
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
        let higher_hash = higher.request_hash();
        let higher_admission = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester), higher)
            .expect("transactionally replace terminal high-watermark");
        assert_ne!(higher_admission.lifecycle_id, original_id);
        command_tx
            .abort_serve(higher_admission)
            .expect("abort materialized terminal replacement");

        let state = command_tx.queue.lock();
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state.serve_by_request.get(&original.request_hash()),
            Some(&original_id)
        );
        assert!(!state.serve_by_request.contains_key(&higher_hash));
        assert_eq!(
            state.serves.get(&original_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
    }

    #[test]
    fn certified_serve_receiver_close_aborts_reserved_replacement_without_orphan() {
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
        let requester = original.request().requester.clone();
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester.clone(), source.clone());
        let response = certified_serve_response(
            &original,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        let original_admission = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), original)
            .expect("admit original view");
        let original_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &original_admission,
            source,
            route,
            response,
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
        let replacement = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester), higher)
            .expect("reserve replacement before receiver closes");
        drop(command_rx);
        let error = command_tx
            .abort_serve(replacement)
            .expect_err("receiver teardown already settled the replacement");
        assert!(
            error.contains("lost its logical lifecycle"),
            "unexpected redundant-abort error: {error}"
        );

        let state = command_tx.queue.lock();
        assert!(!state.receiver_open);
        assert!(state.serve_barrier.is_none());
        assert!(state.commands.is_empty());
        assert!(state.serve_replacements.is_empty());
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state.serves.get(&original_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    }

    #[test]
    fn certified_serve_receiver_close_rolls_back_pending_capacity_replacement() {
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
        let requester = original.request().requester.clone();
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester.clone(), source.clone());
        let response = certified_serve_response(
            &original,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        let original_admission = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), original)
            .expect("admit original view");
        let original_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &original_admission,
            source,
            route,
            response,
        );
        command_tx
            .try_send_as(
                V2IoAdmissionClass::Auxiliary,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(90),
                    subject: proposal.subject,
                },
            )
            .expect("fill the auxiliary prefix before replacement");
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
        assert!(matches!(
            command_tx.prepare_serve(CertifiedServeOwnerKey::Roster(requester), higher),
            Err(CertifiedServePrepareError::Backpressure)
        ));
        let replacement_id = command_tx
            .queue
            .lock()
            .serve_barrier
            .expect("replacement owns an off-queue future slot");
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&replacement_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::PendingCapacity)
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);

        // The pending request never reserved a physical admission unit. Closing
        // the receiver releases only the predecessor while rolling back the
        // higher-view transaction and restoring its displaced tombstone.
        drop(command_rx);
        let state = command_tx.queue.lock();
        assert!(!state.receiver_open);
        assert!(state.serve_barrier.is_none());
        assert!(state.serve_replacements.is_empty());
        assert!(state.pending_serve_requests.is_empty());
        assert!(state.commands.is_empty());
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state.serves.get(&original_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    }

    #[test]
    fn certified_serve_receiver_close_rolls_back_materialized_unclaimed_replacement() {
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
        let requester = original.request().requester.clone();
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester.clone(), source.clone());
        let response = certified_serve_response(
            &original,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        let original_admission = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), original)
            .expect("admit original view");
        let original_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &original_admission,
            source,
            route,
            response,
        );
        command_tx
            .try_send_as(
                V2IoAdmissionClass::Auxiliary,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(91),
                    subject: proposal.subject,
                },
            )
            .expect("fill the auxiliary prefix before replacement");
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
        assert!(matches!(
            command_tx.prepare_serve(CertifiedServeOwnerKey::Roster(requester), higher),
            Err(CertifiedServePrepareError::Backpressure)
        ));
        let replacement_id = command_tx
            .queue
            .lock()
            .serve_barrier
            .expect("replacement owns an off-queue future slot");
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&replacement_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::PendingCapacity)
        );
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(91),
                ..
            })
        ));
        assert_eq!(
            command_tx
                .queue
                .lock()
                .serves
                .get(&replacement_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::Reserved)
        );

        // No second prepare occurred, so no admission token exists outside
        // the queue. Receiver teardown itself must roll back the transaction.
        drop(command_rx);
        let state = command_tx.queue.lock();
        assert!(!state.receiver_open);
        assert!(state.serve_barrier.is_none());
        assert!(state.serve_replacements.is_empty());
        assert!(state.pending_serve_requests.is_empty());
        assert!(state.commands.is_empty());
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state.serves.get(&original_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    }

    #[test]
    fn certified_serve_shutdown_rolls_back_materialized_unclaimed_replacement() {
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
        let requester = original.request().requester.clone();
        let source = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(source.clone());
        let route = route_fixture.mint_via(requester.clone(), source.clone());
        let response = certified_serve_response(
            &original,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            service.context.roster.len(),
            1,
            1,
            Arc::clone(&admission),
        );
        let original_admission = command_tx
            .prepare_serve(CertifiedServeOwnerKey::Roster(requester.clone()), original)
            .expect("admit original view");
        let original_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &original_admission,
            source,
            route,
            response,
        );
        command_tx
            .try_send_as(
                V2IoAdmissionClass::Auxiliary,
                V2IoCommand::LoadCandidate {
                    acquisition_id: LockedCandidateAcquisitionId(92),
                    subject: proposal.subject,
                },
            )
            .expect("fill the auxiliary prefix before replacement");
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
        assert!(matches!(
            command_tx.prepare_serve(CertifiedServeOwnerKey::Roster(requester), higher),
            Err(CertifiedServePrepareError::Backpressure)
        ));
        let replacement_id = command_tx
            .queue
            .lock()
            .serve_barrier
            .expect("replacement owns an off-queue future slot");

        let channel_capacity = admission.capacity();
        let (completion_tx, completion_rx) = mpsc::sync_channel(channel_capacity);
        let worker_admission = Arc::clone(&admission);
        let join = thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                match command {
                    V2IoCommand::LoadCandidate {
                        acquisition_id: LockedCandidateAcquisitionId(92),
                        ..
                    } => {
                        send_tracked_completion(
                            &completion_tx,
                            &worker_admission,
                            V2IoCompletion::AuxiliaryNoop,
                        )
                        .expect("publish predecessor completion");
                    }
                    V2IoCommand::Shutdown => break,
                    V2IoCommand::Serve { .. } => {
                        panic!("shutdown must retire an unclaimed reserved Serve")
                    }
                    _ => panic!("shutdown fixture received an unexpected I/O command"),
                }
            }
        });
        let queue = Arc::clone(&command_tx.queue);
        let io = V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission: Arc::clone(&admission),
        };
        assert!(matches!(
            io.recv_completion_timeout(Duration::from_secs(5)),
            Ok(V2IoCompletion::AuxiliaryNoop)
        ));
        assert_eq!(
            queue
                .lock()
                .serves
                .get(&replacement_id)
                .map(|tracked| tracked.state),
            Some(V2IoServeState::Reserved)
        );

        io.shutdown()
            .expect("shutdown retires the materialized unclaimed replacement");
        let state = queue.lock();
        assert!(!state.receiver_open);
        assert!(state.serve_barrier.is_none());
        assert!(state.serve_replacements.is_empty());
        assert!(state.pending_serve_requests.is_empty());
        assert!(state.commands.is_empty());
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state.serves.get(&original_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
    }

    #[test]
    fn certified_serve_delayed_lower_view_cross_relay_cannot_resurrect() {
        let (service, keys) = fixture_with_block_payload();
        let observer = KeyPair::random();
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let lower = authenticated_serve_request(
            &service.context,
            &observer,
            proposal.round,
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let requester = lower.request().requester.clone();
        let source_a = PeerId::new(KeyPair::random().public_key().clone());
        let source_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(source_a.clone(), 2);
        let route_lower = route_fixture.mint_via(requester.clone(), source_a.clone());
        let route_higher = route_fixture.mint_via(requester.clone(), source_a.clone());
        let route_delayed = route_fixture.mint_via(requester.clone(), source_b.clone());
        let lower_response = certified_serve_response(
            &lower,
            payload.manifest().clone(),
            canonical_wire.clone(),
            &keys[0],
        );
        let (command_tx, command_rx, _) = test_io_command_channel(4);
        let lower_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_a.clone()),
                lower.clone(),
            )
            .expect("admit lower view");
        commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &lower_admission,
            source_a.clone(),
            route_lower,
            lower_response,
        );

        let higher = authenticated_serve_request(
            &service.context,
            &observer,
            wire::ConsensusRound {
                view: proposal.round.view + 1,
                ..proposal.round
            },
            proposal.subject,
            wire::GlobalPhase::Prepare,
        );
        let higher_response = certified_serve_response(
            &higher,
            payload.manifest().clone(),
            canonical_wire,
            &keys[0],
        );
        let higher_admission = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_a.clone()),
                higher,
            )
            .expect("replace terminal at higher view");
        let higher_id = commit_and_terminalize_serve(
            &command_tx,
            &command_rx,
            &higher_admission,
            source_a,
            route_higher,
            higher_response,
        );

        let stale = command_tx
            .prepare_serve(
                CertifiedServeOwnerKey::AuthenticatedSource(source_b.clone()),
                lower,
            )
            .expect("delayed lower view resolves to the retained high-watermark");
        assert_eq!(stale.kind, CertifiedServeAdmissionKind::Stale);
        assert_eq!(stale.lifecycle_id, higher_id);
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(stale.request.clone()),
        ));
        let (routes, ownership) =
            fair_ingress_route_owner(message, requester, source_b, route_delayed);
        assert!(matches!(
            command_tx
                .commit_serve(&stale, routes, ownership)
                .expect("stale carrier is consumed without resurrection"),
            CertifiedServeCommit::Ignored
        ));
        let state = command_tx.queue.lock();
        assert_eq!(state.serves.len(), 1);
        assert_eq!(
            state.serves.get(&higher_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
    }

    #[test]
    fn remote_auxiliary_flood_cannot_consume_consensus_or_control_reservations() {
        let admission = Arc::new(V2IoAdmission::new(1, 2).expect("bounded I/O admission"));
        assert_eq!(admission.capacity(), 4);
        let (command_tx, command_rx) = v2_io_command_channel(
            admission.capacity(),
            admission.capacity(),
            admission.capacity(),
            admission.capacity(),
            Arc::clone(&admission),
        );
        let (_completion_tx, completion_rx) = mpsc::sync_channel(admission.capacity());
        let io = V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"reserved I/O block",
            )),
            payload_hash: Hash::new(b"reserved I/O payload"),
        };
        let command = |view| V2IoCommand::LoadCandidate {
            acquisition_id: LockedCandidateAcquisitionId(view),
            subject,
        };
        assert_eq!(command(97).admission_class(), V2IoAdmissionClass::Control);
        assert!(V2IoAdmission::new(usize::MAX, 1).is_err());

        io.try_enqueue_as(V2IoAdmissionClass::Auxiliary, command(0))
            .expect("first authenticated service request occupies its prefix");
        assert!(!io.can_enqueue_as(V2IoAdmissionClass::Auxiliary));
        assert!(io.can_enqueue_as(V2IoAdmissionClass::Consensus));
        assert!(io.can_enqueue_as(V2IoAdmissionClass::Control));
        assert!(matches!(
            io.try_enqueue_as(V2IoAdmissionClass::Auxiliary, command(99)),
            Err(V2IoTrySendError::Full(_))
        ));
        io.try_enqueue_as(V2IoAdmissionClass::Consensus, command(1))
            .expect("first reserved consensus command");
        io.try_enqueue_as(V2IoAdmissionClass::Consensus, command(2))
            .expect("second reserved consensus command");
        assert!(matches!(
            io.try_enqueue_as(V2IoAdmissionClass::Consensus, command(98)),
            Err(V2IoTrySendError::Full(_))
        ));
        io.try_enqueue_as(V2IoAdmissionClass::Control, command(3))
            .expect("trusted local control reserve");

        let subjects = command_rx
            .try_iter()
            .map(|command| match command {
                V2IoCommand::LoadCandidate { subject, .. } => subject,
                _ => panic!("unexpected command in admission test"),
            })
            .collect::<Vec<_>>();
        assert_eq!(subjects, vec![subject; 4]);
        assert_eq!(io.admission.queued.load(AtomicOrdering::Acquire), 0);
        assert!(io.can_enqueue_as(V2IoAdmissionClass::Auxiliary));
        io.try_enqueue_as(V2IoAdmissionClass::Auxiliary, command(4))
            .expect("worker receive releases auxiliary admission");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate { subject: queued, .. }) if queued == subject
        ));
    }
