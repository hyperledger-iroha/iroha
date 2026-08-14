#[test]
fn timeout_vote_episode_reaches_its_predicate_across_a_selected_serve_barrier() {
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let serve_via = service.context.roster[0].validator.clone();
    let timeout_messages = [2_u32, 3_u32].map(|signer| {
        let source = service.context.roster
            [usize::try_from(signer).expect("small timeout signer index")]
        .validator
        .clone();
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                round: proposal.round,
                highest_prepare_qc: None,
                signer,
                signature: vec![
                    0x7A_u8
                        .checked_add(u8::try_from(signer).expect("small signer marker"))
                        .expect("small signer marker does not overflow");
                    48
                ],
            }),
        ));
        (message, source)
    });
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
        .expect("configure the production-shaped timeout/Serve ingress");
    ingress.require_certified_serve_gate();
    ingress.require_leader_wire_lifecycle_gate();
    let directory = TempDir::new().expect("temporary timeout/Serve ingress gate");
    let owner = [0xAC; 32];
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            service.context.da_layout.max_chunk_count,
        )
        .expect("derive finite timeout/Serve lifecycle capacity");
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
            &directory.path().join("timeout-vote-serve-bypass.wal"),
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
        .expect("open the timeout/Serve leader lifecycle gate");
    let serve_gate = CertifiedServeIngressGate {
        queue: Arc::clone(&command_tx.queue),
    };
    ingress
        .bind_certified_serve_gate(serve_gate.clone())
        .expect("bind the exact Serve ingress gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&leader_gate),
            restore,
            command_tx.queue.lifecycle_ordinals.clone(),
            service.context.id(),
            service.context.height,
        )
        .expect("bind the timeout-vote lifecycle to the shared ordinal source");
    ingress.open().expect("open the timeout/Serve ingress");
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), serve_via,)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    for (message, source) in timeout_messages {
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(message, Some(source))),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
    }
    let serve_barrier = serve_gate
        .selected_barrier()
        .expect("inspect the selected Serve barrier")
        .expect("the exact request owns one selected Serve turn");
    assert_eq!(serve_barrier.carrier_ordinal(), 1);
    assert_eq!(serve_barrier.scheduler_ordinal(), 1);
    assert_eq!(
        leader_gate
            .earliest_ingress_scheduler_ordinal()
            .expect("inspect the queued TimeoutVote owner"),
        Some(2)
    );
    assert!(
        ingress
            .try_recv_if_checked_retiring_obsolete(|inbound| {
                matches!(
                    inbound.message(),
                    BlockMessage::V2(wire::ConsensusMessageV2 {
                        payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                        ..
                    })
                )
            })
            .expect("ordinary selection preserves the selected Serve barrier")
            .is_none(),
        "ordinary ingress cannot move a later TimeoutVote ahead of Serve"
    );
    assert!(
        ingress
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
                FairV2IngressBarrierBypass::TimeoutVoteEpisode,
                |_| false,
            )
            .expect("the bypass pass still executes its downstream predicate")
            .is_none(),
        "the internal bypass never admits a TimeoutVote by itself"
    );
    let mut selected_slots = BTreeSet::new();
    for expected_scheduler_ordinal in [2_u128, 3_u128] {
        let (mut timeout_vote, disposition) = ingress
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
                FairV2IngressBarrierBypass::TimeoutVoteEpisode,
                |inbound| {
                    matches!(
                        inbound.message(),
                        BlockMessage::V2(wire::ConsensusMessageV2 {
                            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                            ..
                        })
                    )
                },
            )
            .expect("each timeout episode turn preserves both durable gates")
            .expect("each exact TimeoutVote reaches the authoritative predicate");
        assert_eq!(
            disposition,
            super::super::FairV2IngressDequeueDisposition::Admit
        );
        let ownership = timeout_vote
            .take_ingress_ownership()
            .expect("the selected TimeoutVote retains exact ownership");
        assert!(ownership.validate_exact());
        let token = ownership
            .leader_wire_token()
            .expect("the selected TimeoutVote retains its productive token");
        assert_eq!(token.scheduler_ordinal(), expected_scheduler_ordinal);
        assert!(
            selected_slots.insert(token.slot.clone()),
            "each roster signer owns a distinct timeout episode slot"
        );
        assert!(ownership.leader_wire_runtime_receipt().is_some());
        assert_eq!(
            serve_gate
                .selected_barrier()
                .expect("inspect the retained Serve barrier")
                .map(|barrier| barrier.carrier_ordinal()),
            Some(1),
            "each timeout turn leaves the older Serve carrier selected"
        );
    }
    assert_eq!(selected_slots.len(), 2);
    assert_eq!(
        ingress.len(),
        1,
        "both timeout slots drain while the selected Serve carrier remains queued"
    );
    assert_eq!(
        serve_gate
            .selected_barrier()
            .expect("inspect the retained Serve barrier")
            .map(|barrier| barrier.carrier_ordinal()),
        Some(1)
    );
}
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
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
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
    let same_height_recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
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
    let decision_recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
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
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
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
    let distinct_route = route_fixture.mint_via(distinct_requester.clone(), distinct_via.clone());
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
            .claim_serve_runtime_episode(stale_carrier)
            .expect_err("a different physical carrier cannot claim this exact ticket")
            .contains("changed barrier identity")
    );
    assert!(
        command_tx
            .claim_serve_runtime_episode(first_barrier)
            .expect("claim the bounded predecessor episode")
    );
    assert!(
        !command_tx
            .claim_serve_runtime_episode(first_barrier)
            .expect("same ticket cannot reopen its episode")
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
        !command_tx
            .claim_serve_runtime_episode(first_barrier)
            .expect("carrier retry retains the currently claimed episode turn")
    );
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
    let distinct_admission = distinct_prepared.expect("predicate retained the second admission");
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
    assert!(
        command_tx
            .claim_serve_runtime_episode(first_barrier)
            .expect("claim first physical occurrence")
    );
    command_tx
        .finish_serve_runtime_episode_turn(first_barrier, false)
        .expect("seal the drained occurrence after its full predecessor recheck");
    assert!(
        !command_tx
            .claim_serve_runtime_episode(first_barrier)
            .expect("one physical occurrence cannot resurrect its sealed episode")
    );
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
    assert!(
        command_tx
            .claim_serve_runtime_episode(retry_barrier)
            .expect("fresh physical occurrence owns its own bounded episode")
    );
    command_tx
        .finish_serve_runtime_episode_turn(retry_barrier, false)
        .expect("seal the retransmission occurrence independently");
    assert!(
        !command_tx
            .claim_serve_runtime_episode(retry_barrier)
            .expect("the retransmission cannot reopen its completed episode")
    );
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
#[test]
fn checked_serve_dequeue_persistence_failure_retains_exact_entry() {
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
    let predecessor_source = context.roster[0].validator.clone();
    let serve_source = context.roster[1].validator.clone();
    let later_source = context.roster[3].validator.clone();
    let mut routes = NetworkReplyRouteTestFixture::new(serve_source.clone());
    let route = routes.mint_via(requester.clone(), serve_source.clone());
    let ordinary = |height, source: &PeerId| {
        InboundBlockMessage::from_transport(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                    wire::CommitCertificateRequest {
                        protocol_version: wire::PROTOCOL_VERSION,
                        network_id: context.network_id,
                        context_id: context.id(),
                        height,
                        requester: source.clone(),
                        signature: vec![u8::try_from(height).unwrap_or(u8::MAX)],
                    },
                ),
            )),
            source.clone(),
            source.clone(),
        )
    };
    let body_root = TempDir::new().expect("checked-dequeue body root");
    let serve_root = TempDir::new().expect("checked-dequeue Serve root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
            .expect("open checked-dequeue persistent queue");
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert!(matches!(
        ingress.try_push(ordinary(
            context.height.saturating_add(1),
            &predecessor_source,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(certified_serve_inbound_with_route(
            request.request(),
            serve_source,
            route,
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(ordinary(context.height.saturating_add(2), &later_source)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let predecessor = ingress
        .try_recv_if(|inbound| {
            matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload:
                        wire::ConsensusMessageV2Payload::CommitCertificateRequest(request),
                    ..
                }) if request.height == context.height.saturating_add(1)
            )
        })
        .expect("drain the frozen physical predecessor before testing target publication");
    assert_eq!(predecessor.sender(), Some(&predecessor_source));
    let admitted = fair_ingress_accounting_snapshot(&ingress);
    assert_eq!(
        admitted.ready.len(),
        2,
        "fixture must cover the failing target and later ready source"
    );
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect checked-dequeue barrier")
        .expect("exact carrier owns the checked-dequeue barrier");
    let temporary_state = serve_root
        .path()
        .join(CERTIFIED_SERVE_STATE_FILE)
        .with_extension("norito.tmp");
    let mut first_lifecycle = None;
    let error = ingress
        .try_recv_if_checked(|inbound| {
            let is_selected_serve = matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                    ..
                }) if HashOf::new(candidate) == request.request_hash()
            );
            if !is_selected_serve {
                return false;
            }
            let admission = command_tx
                .prepare_reserved_serve(
                    CertifiedServeOwnerKey::Roster(requester.clone()),
                    request.clone(),
                )
                .expect("prepare exact lifecycle before forcing drain persistence failure");
            first_lifecycle = Some(admission.lifecycle_id);
            fs::create_dir(&temporary_state).expect("block the atomic Serve-state temporary file");
            true
        })
        .expect_err("failed retirement publication retains the ingress entry");
    assert!(
        error.contains("failed to create Sumeragi v2 Serve temporary state"),
        "unexpected checked-dequeue error: {error}"
    );
    let lifecycle_id = first_lifecycle.expect("failed dequeue prepared one logical lifecycle");
    let retained = fair_ingress_accounting_snapshot(&ingress);
    assert_eq!(retained.ready, admitted.ready);
    assert_eq!(retained.pending_wire_owners, admitted.pending_wire_owners);
    assert_eq!(retained.lanes, admitted.lanes);
    assert_eq!(retained.len, admitted.len);
    assert_eq!(retained.bytes, admitted.bytes);
    {
        let state = command_tx.queue.lock();
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| (reservation.id.0, reservation.state)),
            Some((
                barrier.scheduler_ordinal(),
                CertifiedServeIngressReservationState::Prepared(lifecycle_id),
            )),
            "failed publication leaves the selected physical occurrence live"
        );
        assert_eq!(state.commands.len(), 1);
    }
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("fixture retains its Serve state store")
        .load(&context)
        .expect("reload pre-drain durable snapshot");
    assert!(
        persisted
            .ingress_waiters
            .iter()
            .any(|waiter| waiter.ingress_ordinal == barrier.scheduler_ordinal()),
        "failed publication must not consume the durable ingress occurrence"
    );
    {
        let mut state = command_tx.queue.lock();
        let before = (
            state.serve_barrier,
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| (reservation.id, reservation.state)),
            state.serves.get(&lifecycle_id).map(|serve| serve.state),
            state.commands.len(),
        );
        assert!(
            command_tx
                .queue
                .rollback_serve_barrier(&mut state)
                .expect_err("failed rollback persistence must fail stop")
                .contains("failed to create Sumeragi v2 Serve temporary state")
        );
        let after = (
            state.serve_barrier,
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| (reservation.id, reservation.state)),
            state.serves.get(&lifecycle_id).map(|serve| serve.state),
            state.commands.len(),
        );
        assert_eq!(
            after, before,
            "rollback persistence failure cannot partially mutate the logical handoff"
        );
    }
    fs::remove_dir(&temporary_state).expect("unblock Serve-state retirement publication");
    let mut retried_admission = None;
    let mut inbound = ingress
        .try_recv_if_checked(|inbound| {
            let is_selected_serve = matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                    ..
                }) if HashOf::new(candidate) == request.request_hash()
            );
            if !is_selected_serve {
                return false;
            }
            retried_admission = Some(
                command_tx
                    .prepare_reserved_serve(
                        CertifiedServeOwnerKey::Roster(requester.clone()),
                        request.clone(),
                    )
                    .expect("retry retained exact lifecycle"),
            );
            true
        })
        .expect("retry publishes physical retirement")
        .expect("retry drains the retained exact entry");
    let retried_admission =
        retried_admission.expect("successful checked dequeue retained its admission");
    assert_eq!(retried_admission.lifecycle_id, lifecycle_id);
    {
        let state = command_tx.queue.lock();
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| reservation.state),
            Some(CertifiedServeIngressReservationState::PhysicallyDrainedPrepared(lifecycle_id))
        );
        assert_eq!(
            state
                .commands
                .iter()
                .filter(|command| command.serve_lifecycle_id() == Some(lifecycle_id))
                .count(),
            1,
            "retrying the checked cut cannot duplicate the prepared command"
        );
    }
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("fixture retains its Serve state store")
        .load(&context)
        .expect("reload post-drain durable snapshot");
    assert!(
        persisted
            .ingress_waiters
            .iter()
            .all(|waiter| waiter.ingress_ordinal != barrier.scheduler_ordinal()),
        "successful checked dequeue publishes retirement before returning"
    );
    let ingress_ownership = inbound
        .take_ingress_ownership()
        .expect("retained entry carries exact fair ownership");
    let (_, _, reply_routes) = inbound.into_message_sender_and_reply_routes();
    assert!(matches!(
        command_tx
            .commit_serve(
                &retried_admission,
                reply_routes.expect("retained entry carries its reply route"),
                ingress_ownership,
            )
            .expect("commit the crash-safe checked dequeue"),
        CertifiedServeCommit::Queued
    ));
    assert!(command_tx.queue.lock().serve_ingress_reservation.is_none());
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire checked-dequeue fixture gate");
}
#[test]
fn negative_checked_dequeue_persistence_failure_rolls_back_without_losing_carrier() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let invalid = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = context.roster[3].validator.clone();
    let body_root = TempDir::new().expect("negative rollback body root");
    let serve_root = TempDir::new().expect("negative rollback Serve root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
            .expect("open negative rollback queue");
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(invalid.request(), via.clone())),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect negative rollback barrier")
        .expect("invalid raw admission owns a selected occurrence");
    let fair_before = fair_ingress_accounting_snapshot(&ingress);
    let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
    let durable_before = fs::read(&state_path).expect("read pre-rejection Serve snapshot");
    let temporary_state = state_path.with_extension("norito.tmp");
    let error = ingress
        .try_recv_if_checked(|inbound| {
            let selected = matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                    ..
                }) if HashOf::new(candidate) == invalid.request_hash()
            );
            if !selected {
                return false;
            }
            command_tx
                .stage_selected_serve_rejection(
                    invalid.request_hash(),
                    CertifiedServeNegativeOutcome::InvalidCertificate,
                )
                .expect("stage deterministic invalid-certificate outcome");
            fs::create_dir(&temporary_state)
                .expect("block atomic negative Serve-state publication");
            true
        })
        .expect_err("failed negative publication retains the exact fair carrier");
    assert!(
        error.contains("failed to create Sumeragi v2 Serve temporary state"),
        "unexpected negative publication error: {error}"
    );
    assert_eq!(
        fair_ingress_accounting_snapshot(&ingress),
        fair_before,
        "failed negative publication cannot consume or reorder the fair carrier"
    );
    {
        let state = command_tx.queue.lock();
        assert_eq!(
            state
                .serve_ingress_reservation
                .as_ref()
                .map(|reservation| (reservation.id.0, reservation.state)),
            Some((
                barrier.scheduler_ordinal(),
                CertifiedServeIngressReservationState::DeterministicallyRejected(
                    CertifiedServeNegativeOutcome::InvalidCertificate,
                ),
            )),
            "the selected negative outcome remains staged for an exact retry"
        );
        let tracked = state
            .serves
            .get(&barrier.lifecycle_id())
            .expect("failed publication retains the logical lifecycle");
        assert_eq!(tracked.state, V2IoServeState::AwaitingRetry);
        assert!(tracked.terminal.is_none());
        assert!(tracked.reply_routes.is_none());
        assert!(tracked.ingress_ownership.is_none());
    }
    assert_eq!(
        fs::read(&state_path).expect("reload failed negative publication"),
        durable_before,
        "failed negative publication cannot alter the last durable snapshot"
    );
    fs::remove_dir(&temporary_state).expect("unblock negative Serve-state publication");
    let drained = ingress
        .try_recv_if_checked(|inbound| {
            let selected = matches!(
                inbound.message(),
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyRequest(candidate),
                    ..
                }) if HashOf::new(candidate) == invalid.request_hash()
            );
            if selected {
                command_tx
                    .stage_selected_serve_rejection(
                        invalid.request_hash(),
                        CertifiedServeNegativeOutcome::InvalidCertificate,
                    )
                    .expect("retain the exact staged negative outcome");
            }
            selected
        })
        .expect("retry publishes negative retirement")
        .expect("retry drains the retained exact carrier");
    drop(drained);
    {
        let state = command_tx.queue.lock();
        assert!(state.serve_ingress_reservation.is_none());
        let tracked = state
            .serves
            .get(&barrier.lifecycle_id())
            .expect("successful publication retains one negative tombstone");
        assert_eq!(
            tracked.state,
            V2IoServeState::Rejected(CertifiedServeNegativeOutcome::InvalidCertificate)
        );
        assert!(tracked.terminal.is_none());
        assert!(tracked.reply_routes.is_none());
        assert!(tracked.ingress_ownership.is_none());
    }
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("negative rollback fixture retains a durable store")
        .load(&context)
        .expect("reload successful negative publication");
    assert!(persisted.ingress_waiters.is_empty());
    assert!(persisted.unsealed_lifecycles.is_empty());
    assert_eq!(
        persisted
            .negative_tombstones
            .iter()
            .map(|tombstone| (tombstone.lifecycle_id, tombstone.outcome))
            .collect::<Vec<_>>(),
        vec![(
            barrier.lifecycle_id(),
            CertifiedServeNegativeOutcome::InvalidCertificate,
        )]
    );
    let actor_ordinal_after_negative = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(invalid.request(), via)),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    assert_eq!(
        command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        actor_ordinal_after_negative,
        "an exact negative retry cannot consume another actor-global ordinal"
    );
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire negative rollback gate");
}
#[test]
fn provisional_current_height_drain_is_rejected_and_unbacked_lifecycle_is_detected() {
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
    let body_root = TempDir::new().expect("provisional guard body root");
    let serve_root = TempDir::new().expect("provisional guard Serve root");
    let body_store = V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let (command_tx, _command_rx, _admission) =
        persistent_test_io_command_channel(2, serve_root.path(), &context, &body_store)
            .expect("open provisional guard queue");
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via.clone())),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect provisional guard barrier")
        .expect("current-height request owns one raw reservation");
    let before = fair_ingress_accounting_snapshot(&ingress);
    let error = ingress
        .try_recv_if_checked(|_| true)
        .expect_err("untyped current-height Serve drain must fail closed");
    assert!(
        error.contains(
            "requires a prepared response or deterministic negative outcome before physical drain"
        ),
        "unexpected provisional-drain rejection: {error}"
    );
    assert_eq!(
        fair_ingress_accounting_snapshot(&ingress),
        before,
        "the mechanical guard retains the exact physical carrier"
    );
    assert_eq!(
        gate.dormant_ingress_scheduler_ordinal()
            .expect("inspect live provisional owner"),
        None,
        "a live raw admission is backed by exactly one physical carrier"
    );
    {
        let mut state = command_tx.queue.lock();
        let removed = state
            .serve_ingress_reservation
            .take()
            .expect("mutation removes the raw reservation");
        assert_eq!(removed.lifecycle_id, barrier.lifecycle_id());
        assert_eq!(
            state
                .serves
                .get(&barrier.lifecycle_id())
                .map(|tracked| tracked.state),
            Some(V2IoServeState::AwaitingRetry)
        );
    }
    assert_eq!(
        gate.dormant_ingress_scheduler_ordinal()
            .expect("detect legacy unbacked AwaitingRetry shape"),
        Some(barrier.lifecycle_id().admission_ordinal),
        "the loop-facing invariant reports logical debt even when no waiter survives"
    );
    let actor_ordinal_before_retry = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    let lifecycle_ordinal_before_retry = command_tx.queue.lock().next_serve_admission_ordinal;
    assert!(matches!(
        gate.reserve(request.request(), &via, true, 99),
        Err(CertifiedServeIngressReserveError::Closed)
    ));
    assert_eq!(
        command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        actor_ordinal_before_retry,
        "an exact retry cannot mint an actor-global owner around unbacked debt"
    );
    assert_eq!(
        command_tx.queue.lock().next_serve_admission_ordinal,
        lifecycle_ordinal_before_retry,
        "an exact retry cannot mint or mask a logical lifecycle around unbacked debt"
    );
    let producer_error = match command_tx.try_begin_producer_episode() {
        Err(error) => error,
        Ok(_) => panic!("direct producers cannot cross unbacked AwaitingRetry debt"),
    };
    assert!(
        producer_error.contains("unbacked AwaitingRetry"),
        "the queue lock enforces the same invariant between runner polls"
    );
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire provisional guard gate");
}
#[test]
fn live_waiter_rejects_higher_view_without_restart_or_ordinal_mutation() {
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
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
            view: proposal.round.view.saturating_add(1),
            ..proposal.round
        },
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let via = service.context.roster[0].validator.clone();
    let (command_tx, _command_rx, _admission) = test_io_command_channel(2);
    let gate = CertifiedServeIngressGate {
        queue: Arc::clone(&command_tx.queue),
    };
    let retained = gate
        .reserve(request.request(), &via, true, 10)
        .expect("reserve the original exact owner")
        .expect("current-height request owns one physical reservation");
    let barrier_before = command_tx
        .serve_barrier()
        .expect("inspect live exact owner")
        .expect("the original exact owner is selected");
    let actor_ordinal_before = command_tx.queue.lifecycle_ordinals.next_ordinal_for_test();
    let lifecycle_ordinal_before = command_tx.queue.lock().next_serve_admission_ordinal;
    assert!(matches!(
        gate.reserve(higher.request(), &via, true, 11),
        Err(CertifiedServeIngressReserveError::Busy)
    ));
    assert_eq!(
        command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
        actor_ordinal_before,
        "live higher-view contention cannot mint an actor-global owner"
    );
    assert_eq!(
        command_tx.queue.lock().next_serve_admission_ordinal,
        lifecycle_ordinal_before,
        "live higher-view contention cannot replace the admitted lifecycle"
    );
    assert_eq!(
        command_tx
            .serve_barrier()
            .expect("reinspect live exact owner"),
        Some(barrier_before),
        "higher-view traffic cannot dislodge the selected physical owner"
    );
    assert!(command_tx.queue.lock().sender_open);
    assert!(command_tx.queue.lock().receiver_open);
    drop(retained);
}
#[test]
fn checked_serve_dequeue_rejects_mutated_fair_lifecycle_ordinal() {
    let (service, keys) = fixture_with_block_payload();
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
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(request.request(), via)),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let barrier = command_tx
        .serve_barrier()
        .expect("inspect mutation fixture barrier")
        .expect("exact request owns one selected barrier");
    let mutated_ordinal = gate
        .reserve_ordinary_lifecycle_ordinal()
        .expect("mint a distinct but source-valid mutation ordinal");
    assert_ne!(mutated_ordinal, barrier.scheduler_ordinal());
    {
        let mut state = ingress.state.lock();
        let entry = state
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .next()
            .expect("mutation fixture retains one fair carrier");
        let inbound = Arc::make_mut(&mut entry.inbound);
        let ownership = inbound
            .ingress_ownership
            .as_mut()
            .expect("fair carrier retains exact ownership");
        ownership.first.lifecycle_ordinal = Some(mutated_ordinal);
        ownership.latest.lifecycle_ordinal = Some(mutated_ordinal);
        assert!(
            ownership.validate_exact(),
            "the mutation weakens only the reservation/evidence binding"
        );
    }
    let error = ingress
        .try_recv_if_checked(|_| true)
        .expect_err("checked dequeue must reject the mismatched lifecycle owner");
    assert!(
        error.contains("Serve carrier ownership disagreed with its reserved lifecycle ordinal"),
        "unexpected lifecycle-binding rejection: {error}"
    );
    assert_eq!(ingress.len(), 1);
    assert_eq!(
        command_tx
            .serve_barrier()
            .expect("failed mutation dequeue retains the durable barrier"),
        Some(barrier)
    );
    {
        let mut state = ingress.state.lock();
        let entry = state
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .next()
            .expect("failed mutation dequeue retains the fair carrier");
        let inbound = Arc::make_mut(&mut entry.inbound);
        let ownership = inbound
            .ingress_ownership
            .as_mut()
            .expect("retained carrier keeps ownership");
        ownership.first.lifecycle_ordinal = Some(barrier.scheduler_ordinal());
        ownership.latest.lifecycle_ordinal = Some(barrier.scheduler_ordinal());
        assert!(ownership.validate_exact());
    }
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire restored mutation fixture");
}
#[test]
fn restored_serviceable_lifecycle_seals_response_before_exposing_producers() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
    let (request, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let body_root = TempDir::new().expect("startup-discharge body root");
    let serve_root = TempDir::new().expect("startup-discharge Serve root");
    let mut body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open durable body store");
    let _ = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("retain canonical startup-discharge body");
    let lifecycle_id = persist_unsealed_serve_fixture(
        serve_root.path(),
        &context,
        &request,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        1,
        Some(7),
    );
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let (command_tx, command_rx, _admission) = production_persistent_test_io_command_channel(
        4,
        serve_root.path(),
        &context,
        &body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        lifecycle_ordinals.clone(),
    )
    .expect("startup locally discharges valid retained Serve owner");
    {
        let state = command_tx.queue.lock();
        assert!(state.serve_ingress_waiters.is_empty());
        assert!(state.serve_ingress_reservation.is_none());
        assert_eq!(
            state.serves.get(&lifecycle_id).map(|tracked| tracked.state),
            Some(V2IoServeState::Terminal)
        );
        assert_eq!(state.next_serve_admission_ordinal, 1);
        assert_eq!(state.next_serve_ingress_reservation_ordinal, 7);
    }
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("production fixture has durable Serve store")
        .load(&context)
        .expect("reload startup-discharge response");
    assert!(persisted.ingress_waiters.is_empty());
    assert!(persisted.unsealed_lifecycles.is_empty());
    assert!(persisted.negative_tombstones.is_empty());
    assert_eq!(persisted.terminal_tombstones.len(), 1);
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            request.request(),
            context.roster[3].validator.clone(),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let replay_barrier = command_tx
        .serve_barrier()
        .expect("inspect startup response replay barrier")
        .expect("response replay owns one fresh physical occurrence");
    assert_eq!(replay_barrier.lifecycle_id(), lifecycle_id);
    assert!(replay_barrier.scheduler_ordinal() > 7);
    let (admission, replay) = drain_and_commit_gated_serve(
        &ingress,
        &command_tx,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        &request,
    );
    assert_eq!(admission.lifecycle_id, lifecycle_id);
    assert!(matches!(replay, CertifiedServeCommit::Replay { .. }));
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire startup-discharge replay gate");
}
#[test]
fn restored_serviceable_lifecycle_missing_or_corrupt_body_preserves_serve_snapshot() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
    let (request, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let missing_body_root = TempDir::new().expect("missing-body root");
    let missing_serve_root = TempDir::new().expect("missing-body Serve root");
    let missing_body_store = V2BodyStore::open(missing_body_root.path(), context.clone())
        .expect("open empty missing-body store");
    persist_unsealed_serve_fixture(
        missing_serve_root.path(),
        &context,
        &request,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        1,
        Some(3),
    );
    let missing_state_path = missing_serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
    let missing_before =
        fs::read(&missing_state_path).expect("read pre-discharge missing-body snapshot");
    let missing_result = production_persistent_test_io_command_channel(
        4,
        missing_serve_root.path(),
        &context,
        &missing_body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    );
    let Err(missing_error) = missing_result else {
        panic!("missing canonical body must fail startup discharge")
    };
    assert!(
        missing_error.contains("failed to discharge restored serviceable Serve lifecycle"),
        "unexpected missing-body startup error: {missing_error}"
    );
    assert_eq!(
        fs::read(&missing_state_path).expect("reload missing-body Serve snapshot"),
        missing_before,
        "a missing canonical body cannot publish a terminal Serve transition"
    );
    let corrupt_body_root = TempDir::new().expect("corrupt-body root");
    let corrupt_serve_root = TempDir::new().expect("corrupt-body Serve root");
    let mut corrupt_body_store = V2BodyStore::open(corrupt_body_root.path(), context.clone())
        .expect("open corrupt-body store");
    let _ = corrupt_body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("store canonical body before corrupting its final frame");
    let context_directory = corrupt_body_root
        .path()
        .join(hex::encode(context.id().0.as_ref()));
    let final_body_path = fs::read_dir(&context_directory)
        .expect("list corrupt-body context directory")
        .map(|entry| entry.expect("read corrupt-body directory entry").path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("norito"))
        .expect("find durable body frame");
    let mut corrupt_bytes =
        fs::read(&final_body_path).expect("read durable body frame before corruption");
    *corrupt_bytes
        .last_mut()
        .expect("durable body frame is non-empty") ^= 0x80;
    fs::write(&final_body_path, corrupt_bytes).expect("corrupt durable body frame");
    persist_unsealed_serve_fixture(
        corrupt_serve_root.path(),
        &context,
        &request,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        1,
        Some(4),
    );
    let corrupt_state_path = corrupt_serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
    let corrupt_before =
        fs::read(&corrupt_state_path).expect("read pre-discharge corrupt-body snapshot");
    let corrupt_result = production_persistent_test_io_command_channel(
        4,
        corrupt_serve_root.path(),
        &context,
        &corrupt_body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    );
    let Err(corrupt_error) = corrupt_result else {
        panic!("corrupt canonical body must fail startup discharge")
    };
    assert!(
        corrupt_error.contains("failed to discharge restored serviceable Serve lifecycle"),
        "unexpected corrupt-body startup error: {corrupt_error}"
    );
    assert_eq!(
        fs::read(&corrupt_state_path).expect("reload corrupt-body Serve snapshot"),
        corrupt_before,
        "a corrupt canonical body cannot publish a terminal Serve transition"
    );
}
#[test]
fn restored_serviceable_lifecycle_rejects_wrong_local_signing_key_before_mutation() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&context, &keys);
    let (request, validator_pops) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let body_root = TempDir::new().expect("wrong-key body root");
    let serve_root = TempDir::new().expect("wrong-key Serve root");
    let mut body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open body store");
    let _ = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("retain canonical wrong-key fixture body");
    persist_unsealed_serve_fixture(
        serve_root.path(),
        &context,
        &request,
        CertifiedServeOwnerKey::Roster(request.request().requester.clone()),
        1,
        Some(6),
    );
    let state_path = serve_root.path().join(CERTIFIED_SERVE_STATE_FILE);
    let before = fs::read(&state_path).expect("read wrong-key Serve snapshot");
    let result = production_persistent_test_io_command_channel(
        4,
        serve_root.path(),
        &context,
        &body_store,
        &keys[1],
        &validator_pops,
        Some(0),
        None,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
    );
    let Err(error) = result else {
        panic!("mismatched local validator key must fail startup")
    };
    assert!(
        error.contains("signing key does not match its frozen validator index"),
        "unexpected wrong-key startup error: {error}"
    );
    assert_eq!(
        fs::read(&state_path).expect("reload wrong-key Serve snapshot"),
        before,
        "the key/index binding is checked before any response is signed or persisted"
    );
}
#[test]
fn restored_invalid_qc_is_negative_and_exact_retry_consumes_no_new_ordinal() {
    let (service, keys) = fixture_with_block_payload();
    let context = service.context.clone();
    let (_, _, proposal) = proposal_body_and_payload(&context, &keys);
    let invalid = authenticated_serve_request(
        &context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let validator_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator proof of possession")
        })
        .collect::<Vec<_>>();
    let body_root = TempDir::new().expect("invalid startup body root");
    let serve_root = TempDir::new().expect("invalid startup Serve root");
    let body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open empty body store");
    let lifecycle_id = persist_unsealed_serve_fixture(
        serve_root.path(),
        &context,
        &invalid,
        CertifiedServeOwnerKey::Roster(invalid.request().requester.clone()),
        1,
        Some(5),
    );
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let (command_tx, _command_rx, _admission) = production_persistent_test_io_command_channel(
        4,
        serve_root.path(),
        &context,
        &body_store,
        &keys[0],
        &validator_pops,
        Some(0),
        None,
        lifecycle_ordinals,
    )
    .expect("startup negatively terminalizes invalid QC");
    assert_eq!(
        command_tx
            .queue
            .lock()
            .serves
            .get(&lifecycle_id)
            .map(|tracked| tracked.state),
        Some(V2IoServeState::Rejected(
            CertifiedServeNegativeOutcome::InvalidCertificate
        ))
    );
    let (ingress, gate) = gated_fair_ingress(&context, &command_tx);
    let ordinal_before = command_tx
        .queue
        .lock()
        .next_serve_ingress_reservation_ordinal;
    assert!(matches!(
        ingress.try_push(certified_serve_inbound(
            invalid.request(),
            context.roster[3].validator.clone(),
        )),
        Err(FairV2IngressPushError::Rejected(_))
    ));
    let state = command_tx.queue.lock();
    assert_eq!(
        state.next_serve_ingress_reservation_ordinal, ordinal_before,
        "negative exact retry is rejected before reserving a scheduler ordinal"
    );
    assert_eq!(state.next_serve_admission_ordinal, 1);
    drop(state);
    let persisted = command_tx
        .queue
        .serve_state_store
        .as_ref()
        .expect("production fixture has durable Serve store")
        .load(&context)
        .expect("reload invalid-QC negative");
    assert_eq!(
        persisted
            .negative_tombstones
            .iter()
            .map(|tombstone| tombstone.outcome)
            .collect::<Vec<_>>(),
        vec![CertifiedServeNegativeOutcome::InvalidCertificate]
    );
    ingress.close();
    ingress
        .unbind_certified_serve_gate(&gate)
        .expect("retire invalid-QC negative gate");
}
