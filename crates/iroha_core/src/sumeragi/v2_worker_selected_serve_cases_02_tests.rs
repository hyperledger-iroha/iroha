#[test]
fn closed_height_atomically_retires_serve_and_leader_ingress() {
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let round = proposal.round;
    let proposer = service.context.roster
        [usize::try_from(proposal.proposer).expect("fixture proposer index fits usize")]
    .validator
    .clone();
    let timeout_signer = service.context.roster[1].validator.clone();
    let serve_via = service.context.roster[0].validator.clone();
    let serve_request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let proposal_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(proposal),
    ));
    let timeout_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 1,
            signature: vec![0x5A],
        }),
    ));
    let (command_tx, _command_rx, _admission) = test_io_command_channel(4);
    let ingress = FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
        128,
        512 * 1024 * 1024,
        64 * 1024 * 1024,
        super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        None,
    );
    let roster = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<BTreeSet<_>>();
    ingress
        .configure_roster_for_context(
            roster.iter().cloned(),
            &service.context.network_id,
            service.context.da_layout,
        )
        .expect("configure production-shaped combined ingress");
    ingress.require_certified_serve_gate();
    ingress.require_leader_wire_lifecycle_gate();

    let directory = TempDir::new().expect("temporary combined ingress gate");
    let owner = [0xAB; 32];
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            service.context.da_layout.max_chunk_count,
        )
        .expect("derive finite leader lifecycle capacity");
    let recovery_authority = super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
        service.context.id(),
        service.context.height,
        owner,
        0,
        false,
    );
    let wal_path = directory.path().join("atomic-height-retirement.wal");
    let recovery_roster = roster.clone();
    let (leader_gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &wal_path,
            service.context.id(),
            service.context.height,
            owner,
            roster,
            capacity,
            service.context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open production-shaped leader lifecycle gate");
    let serve_gate = CertifiedServeIngressGate {
        queue: Arc::clone(&command_tx.queue),
    };
    ingress
        .bind_certified_serve_gate(serve_gate.clone())
        .expect("bind exact Serve ingress gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&leader_gate),
            restore,
            command_tx.queue.lifecycle_ordinals.clone(),
            service.context.id(),
            service.context.height,
        )
        .expect("bind leader gate to the same actor-global ordinal source");
    ingress.open().expect("open combined production ingress");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(proposal_message, Some(proposer),)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            timeout_message,
            Some(timeout_signer),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(serve_request.request(), serve_via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(ingress.len(), 3);
    assert!(
        serve_gate
            .selected_barrier()
            .expect("inspect live Serve reservation")
            .is_some(),
        "the closed-height lanes include a live Serve RAII carrier"
    );
    let durable_ingress_ordinals = leader_gate
        .ingress_scheduler_ordinals()
        .expect("inspect retained productive carriers");
    assert_eq!(
        durable_ingress_ordinals.len(),
        2,
        "Proposal and TimeoutVote own independent durable lifecycles"
    );
    let scheduler_high_watermark = *durable_ingress_ordinals
        .last()
        .expect("two durable ingress owners have a high-watermark");

    ingress.close();
    ingress
        .unbind_height_ingress_gates(&serve_gate, &leader_gate)
        .expect("joint retirement cannot expose a carrierless Ingress record");

    let state = ingress.state.lock();
    assert_eq!(state.len, 0);
    assert!(state.certified_serve_gate.is_none());
    assert!(state.leader_wire_lifecycle_gate.is_none());
    assert!(state.leader_wire_lifecycles.is_empty());
    ingress.debug_assert_consistent(&state);
    drop(state);
    assert_eq!(
        serve_gate
            .selected_barrier()
            .expect("inspect retired Serve reservation"),
        None,
        "joint lane retirement rolls back the live Serve RAII carrier"
    );

    assert_eq!(
        leader_gate
            .ingress_scheduler_ordinals()
            .expect("detached finalized-height gate remains readable"),
        durable_ingress_ordinals,
        "detachment must not forge a backward durable lifecycle transition"
    );
    drop(leader_gate);
    let same_height_recovery_authority = super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
        service.context.id(),
        service.context.height,
        owner,
        round.view,
        false,
    );
    let (dormant_gate, dormant_restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &wal_path,
            service.context.id(),
            service.context.height,
            owner,
            recovery_roster.clone(),
            capacity,
            service.context.da_layout.max_chunk_count,
            same_height_recovery_authority,
            &[],
            &[],
        )
        .expect("same-height restart normalizes detached active records");
    assert_eq!(dormant_restore.records().len(), 2);
    assert!(dormant_restore.records().iter().all(|record| {
        record.status()
            == super::super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    }));
    assert_eq!(
        dormant_restore.scheduler_ordinal_high_watermark(),
        scheduler_high_watermark
    );
    assert_eq!(
        dormant_gate
            .earliest_ingress_scheduler_ordinal()
            .expect("inspect same-height dormant selector"),
        None,
        "restart-dormant records own no physical selector turn"
    );
    drop(dormant_gate);

    let decision_recovery_authority = super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
        service.context.id(),
        service.context.height,
        owner,
        round.view,
        true,
    );
    let (reconciled_gate, reconciled_restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &wal_path,
            service.context.id(),
            service.context.height,
            owner,
            recovery_roster,
            capacity,
            service.context.da_layout.max_chunk_count,
            decision_recovery_authority,
            &[],
            &[],
        )
        .expect("durable Decision retires finalized-height ingress records on replay");
    assert!(reconciled_restore.records().is_empty());
    assert_eq!(
        reconciled_restore.scheduler_ordinal_high_watermark(),
        scheduler_high_watermark,
        "obsolete records leave the anti-ABA scheduler high-watermark intact"
    );
    assert_eq!(
        reconciled_gate
            .earliest_ingress_scheduler_ordinal()
            .expect("inspect reconciled finalized-height gate"),
        None
    );
}

#[test]
fn selected_serve_physical_carrier_precedes_reactivated_older_leader_lifecycle() {
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
    let response = certified_serve_response(
        &request,
        payload.manifest().clone(),
        canonical_wire,
        &keys[0],
    );
    let requester = request.request().requester.clone();
    let via = service.context.roster[0].validator.clone();
    let proposer = service.context.roster
        [usize::try_from(proposal.proposer).expect("fixture proposer index fits usize")]
    .validator
    .clone();
    let leader_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
    ));
    let BlockMessage::V2(leader_envelope) = &leader_message else {
        unreachable!("leader fixture is a v2 envelope");
    };

    let (command_tx, command_rx, _admission) = test_io_command_channel(4);
    let leader_scheduler_ordinal = (1_u128..=41)
        .map(|expected| {
            let ordinal = command_tx
                .queue
                .lifecycle_ordinals
                .reserve_one()
                .expect("reserve the pre-restart shared lifecycle prefix");
            assert_eq!(ordinal, expected);
            ordinal
        })
        .last()
        .expect("the shared prefix is non-empty");
    let ingress = FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
        128,
        512 * 1024 * 1024,
        64 * 1024 * 1024,
        super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        None,
    );
    let roster = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<BTreeSet<_>>();
    ingress
        .configure_roster_for_context(
            roster.iter().cloned(),
            &service.context.network_id,
            service.context.da_layout,
        )
        .expect("configure the combined Serve/leader ingress");
    ingress.require_certified_serve_gate();
    ingress.require_leader_wire_lifecycle_gate();

    let (identity, slot) = {
        let state = ingress.state.lock();
        match super::super::fair_v2_ingress_leader_wire_identity(
            &state,
            &leader_message,
            &proposer,
            Hash::new(leader_envelope.encode()),
        ) {
            super::super::FairV2IngressLeaderWireDerivation::Exact { identity, slot } => {
                (identity, slot)
            }
            _ => panic!("proposal fixture must derive one exact leader lifecycle"),
        }
    };
    let source_class = identity.phase.source_class();
    let leader_token = super::super::FairV2IngressLeaderWireToken {
        identity,
        slot,
        admission_ordinal: 7,
        scheduler_ordinal: leader_scheduler_ordinal,
        source_class,
    };
    assert_eq!(leader_token.scheduler_ordinal(), 41);

    let directory = TempDir::new().expect("temporary combined ingress gate");
    let wal_path = directory.path().join("serve-leader-order.wal");
    let owner = [0xA9; 32];
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            service.context.da_layout.max_chunk_count,
        )
        .expect("derive finite leader lifecycle capacity");
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            service.context.id(),
            service.context.height,
            owner,
            0,
            false,
        );
    let (leader_gate, _) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &wal_path,
            service.context.id(),
            service.context.height,
            owner,
            roster.clone(),
            capacity,
            service.context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open pre-restart leader lifecycle gate");
    leader_gate
        .reserve(leader_token.clone())
        .expect("reserve the pre-restart leader lifecycle");
    drop(leader_gate);

    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            service.context.id(),
            service.context.height,
            owner,
            0,
            false,
        );
    let (leader_gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &wal_path,
            service.context.id(),
            service.context.height,
            owner,
            roster,
            capacity,
            service.context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("restore the dormant leader lifecycle");
    assert_eq!(restore.records().len(), 1);
    assert_eq!(
        restore.records()[0].status(),
        super::super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    );
    let serve_gate = CertifiedServeIngressGate {
        queue: Arc::clone(&command_tx.queue),
    };
    ingress
        .bind_certified_serve_gate(serve_gate.clone())
        .expect("bind the real Serve ingress gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&leader_gate),
            restore,
            command_tx.queue.lifecycle_ordinals.clone(),
            service.context.id(),
            service.context.height,
        )
        .expect("bind the restored leader lifecycle to the shared source");
    ingress
        .open()
        .expect("open the combined production ingress");

    let mut route_fixture = NetworkReplyRouteTestFixture::new(via.clone());
    let route = route_fixture.mint_via(requester.clone(), via.clone());
    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            via,
            route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let serve_barrier = serve_gate
        .selected_barrier()
        .expect("inspect the bound Serve gate")
        .expect("the exact Serve request owns the selected barrier");
    assert_eq!(serve_barrier.scheduler_ordinal(), 42);
    assert_eq!(serve_barrier.carrier_ordinal(), 8);

    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(leader_message, Some(proposer),)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let leader_carrier_ordinal = {
        let state = ingress.state.lock();
        let record = state
            .leader_wire_lifecycles
            .get(&leader_token.slot)
            .expect("reactivated leader lifecycle remains indexed");
        assert_eq!(
            record.status,
            super::super::FairV2IngressLeaderWireStatus::Ingress
        );
        assert_eq!(record.token, leader_token);
        state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.leader_wire_token.as_ref() == Some(&leader_token))
            .expect("reactivated leader owns one exact physical carrier")
            .admission_ordinal
    };
    assert_eq!(leader_carrier_ordinal, 9);
    assert!(
        serve_barrier.scheduler_ordinal() > leader_token.scheduler_ordinal(),
        "the retained leader scheduler identity is older than the selected Serve identity"
    );
    assert!(
        serve_barrier.carrier_ordinal() < leader_carrier_ordinal,
        "the selected Serve occurrence owns the earlier physical position"
    );
    assert_eq!(
        leader_gate
            .earliest_ingress_scheduler_ordinal()
            .expect("inspect durable leader selector"),
        Some(leader_token.scheduler_ordinal())
    );

    assert!(
        ingress
            .try_recv_if_checked(|inbound| {
                matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload: wire::ConsensusMessageV2Payload::Proposal(candidate),
                        ..
                    }) if candidate.round == proposal.round
                        && candidate.subject == proposal.subject
                )
            })
            .expect("checked selector preserves both durable gates")
            .is_none(),
        "an older retained scheduler identity cannot cross the earlier Serve carrier"
    );

    let (admission, committed) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(requester),
        &request,
    );
    assert!(matches!(committed, CertifiedServeCommit::Queued));
    assert!(
        serve_gate
            .selected_barrier()
            .expect("inspect Serve gate after physical retirement")
            .is_none()
    );
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve {
            lifecycle_id,
            ..
        }) if lifecycle_id == admission.lifecycle_id
    ));
    command_rx
        .complete_serve_response(admission.lifecycle_id, &response)
        .expect("seal exact Serve response before later leader I/O");
    command_tx
        .acknowledge_serve_completion(
            admission.lifecycle_id,
            V2IoServeTerminal::Response(response.clone()),
        )
        .expect("retain the drained logical request as a terminal tombstone");
    {
        let state = command_tx.queue.lock();
        assert_eq!(
            state.serves.len(),
            1,
            "acknowledged Serve retains only its exact replay tombstone"
        );
        let tracked = state
            .serves
            .get(&admission.lifecycle_id)
            .expect("acknowledged Serve retains its exact replay tombstone");
        assert_eq!(tracked.state, V2IoServeState::Terminal);
        assert_eq!(
            tracked.terminal.as_ref(),
            Some(&V2IoServeTerminal::Response(response))
        );
        assert!(state.commands.is_empty());
    }

    let mut leader = ingress
        .try_recv_if_checked(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Proposal(candidate),
                    ..
                }) if candidate.round == proposal.round
                    && candidate.subject == proposal.subject
            )
        })
        .expect("checked leader dequeue validates the exact durable carrier")
        .expect("leader follows after the selected Serve retires");
    let mut ownership = leader
        .take_ingress_ownership()
        .expect("leader retains its exact fair-ingress ownership");
    assert_eq!(ownership.leader_wire_token(), Some(&leader_token));
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind the exact leader carrier to runtime");
    let runtime = ownership
        .leader_wire_runtime_receipt()
        .expect("leader runtime receipt is installed");
    assert_eq!(runtime.token(), &leader_token);
    ingress
        .mark_leader_wire_volatile_terminal(runtime)
        .expect("retire the validated leader runtime owner");
    command_tx
        .try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(720),
                subject: proposal.subject,
            },
        )
        .expect("leader-caused I/O becomes admissible only after Serve terminalization");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id: LockedCandidateAcquisitionId(720),
            ..
        })
    ));
}

#[test]
fn fair_ingress_exact_ticket_coalesces_and_commits_before_later_io_producers() {
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let distinct = authenticated_serve_request(
        &service.context,
        &keys[2],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let requester = request.request().requester.clone();
    let distinct_requester = distinct.request().requester.clone();
    let via = service.context.roster[0].validator.clone();
    let distinct_via = service.context.roster[3].validator.clone();
    let mut route_fixture = NetworkReplyRouteTestFixture::new(via.clone());
    let route = route_fixture.mint_via(requester.clone(), via.clone());
    let retry_route = route_fixture
        .redeliver(&route)
        .expect("mint exact retry route");
    let distinct_route =
        route_fixture.mint_via(distinct_requester.clone(), distinct_via.clone());
    let (command_tx, command_rx, _admission) = test_io_command_channel(4);
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);

    let before = fair_ingress_accounting_snapshot(&ingress);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            via.clone(),
            route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let admitted = fair_ingress_accounting_snapshot(&ingress);
    assert_eq!(admitted.last_admission_ordinal, 1);
    assert_eq!(admitted.len, 1);
    assert!(
        admitted
            .lanes
            .iter()
            .flat_map(|lane| &lane.entries)
            .all(|entry| { entry.admission_ordinal == 1 && entry.owns_certified_serve_ticket })
    );
    {
        let state = command_tx.queue.lock();
        let reservation = state
            .serve_ingress_reservation
            .as_ref()
            .expect("selector-visible exact request owns its future-slot ticket");
        assert_eq!(
            reservation.state,
            CertifiedServeIngressReservationState::Provisional
        );
        assert_eq!(reservation.projection.request_hash, request.request_hash());
        assert_eq!(reservation.lifecycle_id.admission_ordinal, 1);
        assert_eq!(
            reservation.lifecycle_id.request_hash,
            request.request_hash()
        );
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
        assert_eq!(state.next_serve_admission_ordinal, 1);
    }
    assert_eq!(
        command_tx
            .serve_barrier_request_hash()
            .expect("provisional target is runner-visible"),
        Some(request.request_hash())
    );
    let first_barrier = command_tx
        .serve_barrier()
        .expect("inspect exact actor-global barrier")
        .expect("provisional target retains its barrier");
    assert_eq!(first_barrier.scheduler_ordinal(), 1);
    assert_eq!(first_barrier.carrier_ordinal(), 1);
    let stale_carrier = CertifiedServeBarrier {
        carrier_ordinal: first_barrier
            .carrier_ordinal()
            .checked_add(1)
            .expect("fixture carrier has a successor"),
        ..first_barrier
    };
    assert!(
        command_tx
            .open_serve_predecessor_admission(stale_carrier)
            .expect_err("a different physical carrier cannot open this exact ticket")
            .contains("changed barrier identity")
    );
    command_tx
        .open_serve_predecessor_admission(first_barrier)
        .expect("open the bounded predecessor admission");
    assert!(
        command_tx
            .open_serve_predecessor_admission(first_barrier)
            .is_err(),
        "same ticket cannot overlap predecessor admissions"
    );

    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            via.clone(),
            retry_route,
        )),
        Ok(FairV2IngressPushDisposition::Coalesced)
    ));
    assert_eq!(
        fair_ingress_accounting_snapshot(&ingress),
        admitted,
        "coalescing must retain the ticket, ordinal, and every capacity owner"
    );
    assert_eq!(
        command_tx
            .serve_barrier()
            .expect("exact retry retains the selected barrier"),
        Some(first_barrier)
    );
    assert!(
        command_tx
            .open_serve_predecessor_admission(first_barrier)
            .is_err(),
        "carrier retry retains the currently open predecessor admission"
    );
    command_tx
        .close_serve_predecessor_admission(first_barrier)
        .expect("close the bounded predecessor admission before target drain");
    {
        let state = command_tx.queue.lock();
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 1);
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.projection.request_hash),
            Some(request.request_hash())
        );
    }

    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            distinct.request(),
            distinct_via,
            distinct_route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    {
        let state = command_tx.queue.lock();
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 2);
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.projection.request_hash),
            Some(request.request_hash()),
            "the first live fair carrier remains the selected exact target"
        );
        assert_eq!(state.serve_ingress_waiters.len(), 1);
        assert_eq!(state.next_serve_admission_ordinal, 2);
    }
    assert!(
        command_tx
            .try_begin_producer_episode()
            .expect("inspect target priority")
            .is_none(),
        "an admitted exact target precedes every later local producer episode"
    );
    assert!(matches!(
        command_tx.try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(90),
                subject: proposal.subject,
            },
        ),
        Err(V2IoTrySendError::Full(_))
    ));

    let mut prepared = None;
    let mut popped = ingress
        .try_recv_if(|_| {
            prepared = Some(
                command_tx
                    .prepare_reserved_serve(
                        CertifiedServeOwnerKey::Roster(requester.clone()),
                        request.clone(),
                    )
                    .expect("predicate-time preparation promotes the exact ticket"),
            );
            true
        })
        .expect("prepared exact target drains first");
    let admission = prepared.expect("predicate retained the promoted admission");
    let ingress_ownership = popped
        .take_ingress_ownership()
        .expect("dequeued target retains exact fair ownership");
    let (_, _, reply_routes) = popped.into_message_sender_and_reply_routes();
    assert!(matches!(
        command_tx
            .commit_serve(
                &admission,
                reply_routes.expect("target retains its authenticated reply route"),
                ingress_ownership,
            )
            .expect("commit promoted exact ticket"),
        CertifiedServeCommit::Queued
    ));
    {
        let state = command_tx.queue.lock();
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.projection.request_hash),
            Some(distinct.request_hash()),
            "committing one target atomically promotes the next live waiter"
        );
        assert!(state.serve_barrier.is_none());
        assert_eq!(
            state.next_serve_ingress_reservation_ordinal, 2,
            "promotion moves a frozen waiter without minting a replacement"
        );
        assert_eq!(state.next_serve_admission_ordinal, 2);
    }
    assert!(
        command_tx
            .try_begin_producer_episode()
            .expect("inspect second target priority")
            .is_none(),
        "the promoted waiter precedes every later local producer episode"
    );

    let mut distinct_prepared = None;
    let mut distinct_popped = ingress
        .try_recv_if(|_| {
            distinct_prepared = Some(
                command_tx
                    .prepare_reserved_serve(
                        CertifiedServeOwnerKey::Roster(distinct_requester.clone()),
                        distinct.clone(),
                    )
                    .expect("prepare the promoted exact waiter"),
            );
            true
        })
        .expect("promoted exact waiter drains second");
    let distinct_admission =
        distinct_prepared.expect("predicate retained the second admission");
    let distinct_ownership = distinct_popped
        .take_ingress_ownership()
        .expect("second target retains exact fair ownership");
    let (_, _, distinct_routes) = distinct_popped.into_message_sender_and_reply_routes();
    assert!(matches!(
        command_tx
            .commit_serve(
                &distinct_admission,
                distinct_routes.expect("second target retains its authenticated route"),
                distinct_ownership,
            )
            .expect("commit promoted exact waiter"),
        CertifiedServeCommit::Queued
    ));
    {
        let state = command_tx.queue.lock();
        assert!(state.serve_ingress_reservation.is_none());
        assert_eq!(
            state.serve_ingress_waiters.len(),
            0,
            "drained physical tickets retire while logical Serve owners remain indexed"
        );
        assert_eq!(state.next_serve_admission_ordinal, 2);
    }

    let producer_episode = command_tx
        .try_begin_producer_episode()
        .expect("open post-target producer episode")
        .expect("committed target releases local producers");
    command_tx
        .try_send_as(
            V2IoAdmissionClass::Control,
            V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(91),
                subject: proposal.subject,
            },
        )
        .expect("later local work queues behind the committed Serve target");
    {
        let state = command_tx.queue.lock();
        assert!(matches!(
            state.commands.front(),
            Some(V2IoCommand::Serve { lifecycle_id, .. })
                if *lifecycle_id == admission.lifecycle_id
        ));
        assert!(matches!(
            state.commands.get(1),
            Some(V2IoCommand::Serve { lifecycle_id, .. })
                if *lifecycle_id == distinct_admission.lifecycle_id
        ));
        assert!(matches!(
            state.commands.get(2),
            Some(V2IoCommand::LoadCandidate {
                acquisition_id: LockedCandidateAcquisitionId(91),
                ..
            })
        ));
    }
    drop(producer_episode);
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. })
            if lifecycle_id == admission.lifecycle_id
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. })
            if lifecycle_id == distinct_admission.lifecycle_id
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id: LockedCandidateAcquisitionId(91),
            ..
        })
    ));

    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire test gate before queue teardown");
    assert_eq!(before.last_admission_ordinal, 0);
}

#[test]
fn final_serve_retirement_yields_one_producer_episode_before_replenishment() {
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
    let replenishment = authenticated_serve_request(
        &service.context,
        &keys[3],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let first_requester = first.request().requester.clone();
    let second_requester = second.request().requester.clone();
    let replenishment_requester = replenishment.request().requester.clone();
    let via = service.context.roster[0].validator.clone();
    let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
    let first_route = routes.mint_via(first_requester.clone(), via.clone());
    let second_route = routes.mint_via(second_requester.clone(), via.clone());
    let replenishment_route = routes.mint_via(replenishment_requester.clone(), via.clone());
    let (command_tx, _command_rx, _admission) = test_io_command_channel(6);
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);

    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            first.request(),
            via.clone(),
            first_route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            second.request(),
            via.clone(),
            second_route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(first_requester),
            &first,
        )
        .1,
        CertifiedServeCommit::Queued
    ));
    assert!(matches!(
        drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(second_requester),
            &second,
        )
        .1,
        CertifiedServeCommit::Queued
    ));

    let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    let lifecycle_ordinal_before = command_tx.queue.lock().next_serve_admission_ordinal;
    {
        let state = command_tx.queue.lock();
        assert!(state.producer_episode_due);
        assert!(!state.producer_episode_active);
        assert!(state.serve_ingress_reservation.is_none());
        assert!(state.serve_ingress_waiters.is_empty());
    }
    assert!(matches!(
        gate.reserve(replenishment.request(), &via, true, 3),
        Err(CertifiedServeIngressReserveError::Busy)
    ));
    assert_eq!(
        command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        actor_ordinal_before,
        "post-Serve replenishment cannot mint an actor-global ordinal before the producer turn"
    );
    assert_eq!(
        command_tx.queue.lock().next_serve_admission_ordinal,
        lifecycle_ordinal_before,
        "post-Serve replenishment cannot mint a lifecycle before the producer turn"
    );

    let producer_episode = command_tx
        .try_begin_producer_episode()
        .expect("consume the atomic post-Serve handoff")
        .expect("the final frozen Serve batch owes one producer episode");
    {
        let state = command_tx.queue.lock();
        assert!(!state.producer_episode_due);
        assert!(state.producer_episode_active);
    }
    assert!(matches!(
        gate.reserve(replenishment.request(), &via, true, 3),
        Err(CertifiedServeIngressReserveError::Busy)
    ));
    drop(producer_episode);
    {
        let state = command_tx.queue.lock();
        assert!(!state.producer_episode_due);
        assert!(!state.producer_episode_active);
    }

    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            replenishment.request(),
            via,
            replenishment_route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        drain_and_commit_gated_serve(
            &ingress,
            &command_tx,
            CertifiedServeOwnerKey::Roster(replenishment_requester),
            &replenishment,
        )
        .1,
        CertifiedServeCommit::Queued
    ));

    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire post-Serve producer handoff fixture gate");
}

#[test]
fn drained_exact_retransmission_gets_fresh_scheduler_ordinal() {
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
    let mut routes = NetworkReplyRouteTestFixture::new(via.clone());
    let first_route = routes.mint_via(requester.clone(), via.clone());
    let retry_route = routes
        .redeliver(&first_route)
        .expect("mint post-drain exact retransmission route");
    let (command_tx, _command_rx, _admission) = test_io_command_channel(4);
    let (ingress, gate) = gated_fair_ingress(&service.context, &command_tx);

    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            via.clone(),
            first_route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let first_barrier = command_tx
        .serve_barrier()
        .expect("inspect first exact barrier")
        .expect("first carrier owns a barrier");
    assert_eq!(first_barrier.carrier_ordinal(), 1);
    command_tx
        .open_serve_predecessor_admission(first_barrier)
        .expect("open first physical occurrence");
    command_tx
        .close_serve_predecessor_admission(first_barrier)
        .expect("close the occurrence after its full predecessor recheck");
    let (first_admission, first_commit) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(requester.clone()),
        &request,
    );
    assert!(matches!(first_commit, CertifiedServeCommit::Queued));
    assert_eq!(first_barrier.lifecycle_id(), first_admission.lifecycle_id);
    assert!(command_tx.queue.lock().serve_ingress_waiters.is_empty());

    let scheduler_before_retry = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    assert!(matches!(
        gate.reserve(request.request(), &via, true, 2),
        Err(CertifiedServeIngressReserveError::Busy)
    ));
    assert_eq!(
        command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        scheduler_before_retry,
        "post-drain retry cannot mint a scheduler owner before the owed producer turn"
    );
    let post_drain_producer_episode = command_tx
        .try_begin_producer_episode()
        .expect("consume the post-drain producer handoff")
        .expect("final Serve retirement owes one producer episode");
    drop(post_drain_producer_episode);

    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            via,
            retry_route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let retry_barrier = command_tx
        .serve_barrier()
        .expect("inspect post-drain retransmission barrier")
        .expect("post-drain retransmission owns a fresh barrier");
    assert_eq!(retry_barrier.request_hash(), first_barrier.request_hash());
    assert!(retry_barrier.scheduler_ordinal() > first_barrier.scheduler_ordinal());
    assert_eq!(
        retry_barrier.lifecycle_id(),
        first_barrier.lifecycle_id(),
        "fresh physical scheduler ownership retains immutable logical lineage"
    );
    assert!(
        retry_barrier.carrier_ordinal() > first_barrier.carrier_ordinal(),
        "a drained exact retransmission receives a fresh physical carrier position"
    );
    command_tx
        .open_serve_predecessor_admission(retry_barrier)
        .expect("fresh physical occurrence owns its own bounded admission");
    command_tx
        .close_serve_predecessor_admission(retry_barrier)
        .expect("close the retransmission occurrence independently");
    let (retry_admission, retry_commit) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(requester),
        &request,
    );
    assert_eq!(
        retry_admission.lifecycle_id, first_admission.lifecycle_id,
        "wire retransmission retains the logical Serve lifecycle/tombstone"
    );
    assert!(matches!(retry_commit, CertifiedServeCommit::Coalesced));

    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire post-drain retransmission gate");
}
