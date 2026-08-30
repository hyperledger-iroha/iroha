#[test]
fn final_exact_output_seal_is_one_shot_and_blocks_late_enqueue() {
    let (mut service, keys) = fixture();
    let (receipt, artifact) = durable_finality_fixture(&service, &keys);
    let target = service.context.roster[1].validator.clone();
    let (request, _) = certified_sidecar_outputs(&service.local_peer, &target);
    let attempts = install_counting_exact_output_backpressure(&mut service);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"empty exact-output final seal lane witness"),
    );
    for expected_attempts in 1..=2 {
        assert_eq!(
            service
                .post_certified_merge_sidecar_with_reply_routes(
                    target.clone(),
                    None,
                    Arc::new(request.clone()),
                )
                .expect("pre-seal exact output remains admissible"),
            ExactFanoutOwnership::Owned
        );
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            expected_attempts,
            "each pre-seal pass reaches actor admission exactly once"
        );
        assert_eq!(
            service
                .handoff_applied_height_output_to_durable_reconstruction(
                    &receipt,
                    &artifact,
                    &lane_authority,
                )
                .expect("repeatable pass clears the current exact suffix"),
            1
        );
        assert!(
            !service
                .has_pending_exact_output()
                .expect("inspect the repeatable handoff")
        );
    }
    let handoff = service
        .seal_applied_height_output_handoff(&receipt, &artifact, &lane_authority)
        .expect("the final empty pass mints one receipt");
    assert!(handoff.matches_predecessor_context(&service.context));
    assert!(handoff.matches_finality_artifact(&artifact));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("sealed retry is a terminal no-op")
    );
    assert!(
        service
            .drain_certified_merge_sidecar_chunk_admissions(1)
            .expect("sealed receipt drain is a terminal no-op")
            .is_empty()
    );
    let sealed_output_guard = Arc::clone(&service.output_guard);
    assert!(
        service
            .post_certified_merge_sidecar_with_reply_routes(target, None, Arc::new(request),)
            .expect_err("late output cannot enter a sealed corridor")
            .contains("sealed after durable finality handoff")
    );
    assert_eq!(
        attempts.load(Ordering::Relaxed),
        2,
        "post-seal output never reaches actor admission"
    );
    assert!(
        sealed_output_guard.restart_required(),
        "a rejected late exact output fails closed after observing the seal"
    );
    let (reseal_service, reseal_keys) = fixture();
    let (reseal_receipt, reseal_artifact) = durable_finality_fixture(&reseal_service, &reseal_keys);
    let reseal_lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &reseal_artifact,
        Hash::new(b"one-shot exact-output seal lane witness"),
    );
    let _first_handoff = reseal_service
        .seal_applied_height_output_handoff(
            &reseal_receipt,
            &reseal_artifact,
            &reseal_lane_authority,
        )
        .expect("a separate empty exact-output corridor seals once");
    let reseal_guard = Arc::clone(&reseal_service.output_guard);
    let Err(reseal_error) = reseal_service.seal_applied_height_output_handoff(
        &reseal_receipt,
        &reseal_artifact,
        &reseal_lane_authority,
    ) else {
        panic!("the exact service can seal only once");
    };
    assert!(reseal_error.contains("already sealed"));
    assert!(
        reseal_guard.restart_required(),
        "a duplicate terminal seal is a fail-stop protocol misuse"
    );
}
#[test]
fn applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor() {
    let (service, keys) = fixture();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let requester = service.context.roster[1].validator.clone();
    let (_, chunk_message) = certified_sidecar_outputs(&service.local_peer, &requester);
    let CertifiedMergeSidecarMessage::Chunk(chunk) = chunk_message else {
        unreachable!("sidecar fixture returns one response chunk")
    };
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let hub_c = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 3);
    let route_a = routes.mint_via(requester.clone(), hub_a);
    let route_b = routes.mint_via(requester.clone(), hub_b);
    let route_c = routes.mint_via(requester.clone(), hub_c);
    let mut reply_routes =
        NetworkReplyRoutes::try_from_route(route_a.clone()).expect("source A route set");
    for route in [&route_b, &route_c] {
        reply_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route.clone())
                    .expect("independent sidecar source route set"),
            )
            .expect("retain every authenticated sidecar source");
    }
    let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
        CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
    ));
    let rollover_claim = ExactOutputRolloverClaim::CertifiedSidecarChunk {
        scope: service.exact_output_scope(),
        target: requester.clone(),
        transfer: CertifiedSidecarTransferIdentity::from_chunk(&chunk),
        chunk_index: chunk.chunk_index,
        chunk_count: chunk.chunk_count,
        response_hash: HashOf::new(&chunk),
    };
    let fanout = PendingExactFanout::claimed_with_reply_routes(
        vec![message],
        requester,
        reply_routes,
        rollover_claim,
    )
    .expect("valid sidecar reconstruction claim")
    .expect("three-source sidecar fanout");
    let mut pending =
        PendingExactOutput::new(4, 1, 3, &[]).expect("four bounded sidecar completion states");
    assert_eq!(pending.enqueue(fanout), Ok(ExactFanoutOwnership::Owned));
    let (_pending_control, pending_ack, pending_admission) =
        certified_sidecar_flush_fixture(&chunk, &route_a);
    let (mut flushed_control, flushed_ack, flushed_admission) =
        certified_sidecar_flush_fixture(&chunk, &route_b);
    assert!(flushed_control.flush());
    let (mut closed_control, closed_ack, closed_admission) =
        certified_sidecar_flush_fixture(&chunk, &route_c);
    assert!(closed_control.close());
    for (route, sidecar_admission, flush_ack) in [
        (&route_a, pending_admission, pending_ack),
        (&route_b, flushed_admission, flushed_ack),
        (&route_c, closed_admission, closed_ack),
    ] {
        let target = pending.fanouts[0]
            .targets
            .iter_mut()
            .find(|target| {
                matches!(&target.route, ExactTargetRoute::Reply(candidate) if candidate.same_source(route))
            })
            .expect("sidecar source retains its exact target");
        target.pending_flush = Some(PendingExactReplyFlush {
            flush_ack,
            reply_writer_timeout_attempt: 0,
            sidecar_admission: Some(sidecar_admission),
        });
    }
    let (_admitted_control, _admitted_ack, admitted) =
        certified_sidecar_flush_fixture(&chunk, &route_a);
    pending.admitted_sidecar_chunks.push_back(admitted);
    assert_eq!(pending.sidecar_control_units(), 4);
    assert_eq!(
        pending
            .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
            .expect("typed height handoff supersedes every volatile completion state"),
        4
    );
    assert!(!pending.is_pending());
    assert_eq!(pending.pending_sidecar_flushes(), 0);
    assert!(pending.admitted_sidecar_chunks.is_empty());
}
#[test]
fn applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically() {
    let (service, keys) = fixture();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let requester = service.context.roster[1].validator.clone();
    let message =
        ProductionV2Services::preencode_v2_network_message(global_commit_qc_message(&artifact))
            .expect("encode global CommitQC response");
    let class = exact_output_class(&message).expect("classify global CommitQC response");
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let route = routes.mint(requester.clone());
    let source = ExactTargetRoute::Reply(route.clone()).source(&requester, class);
    let mut pending =
        PendingExactOutput::new(1, 1, 1, &[]).expect("one parked applied-height response corridor");
    pending
        .enqueue(
            PendingExactFanout::claimed_with_routes(
                vec![message.clone()],
                vec![requester.clone()],
                vec![ExactTargetRoute::Reply(route.clone())],
                ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
            )
            .expect("valid routed global finality claim")
            .expect("one routed global response"),
        )
        .expect("retain the routed global response");
    let fifo_id = pending.fanouts[0]
        .fifo_id
        .expect("routed response owns stable FIFO age");
    assert!(routes.retire(&route));
    assert_eq!(
        pending.drive_with(|_post, _ticket, _route| {
            panic!("inactive response route must park before actor admission")
        }),
        Ok(None)
    );
    let parked = &pending.fanouts[0].targets[0];
    assert!(parked.parked);
    assert_eq!(parked.message_index, 0);
    assert!(parked.current.is_none());
    assert!(parked.ticket.is_none());
    assert!(!pending.is_pending());
    assert_eq!(pending.ownership_units, 1);
    assert_eq!(pending.shared_ownership_units, 1);
    assert_eq!(pending.reservation_owner_counts.values().sum::<usize>(), 1);
    assert_eq!(
        pending.source_fifo_owners.get(&source),
        Some(&BTreeSet::from([fifo_id]))
    );
    assert_eq!(
        pending
            .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
            .expect("durable finality counts and supersedes the parked cursor"),
        1
    );
    assert!(pending.fanouts.is_empty());
    assert!(pending.source_fifo_owners.is_empty());
    assert!(pending.reservation_owner_counts.is_empty());
    assert_eq!(pending.ownership_units, 0);
    assert_eq!(pending.shared_ownership_units, 0);
    let active_route = routes.mint(requester.clone());
    let mut rejected = PendingExactOutput::new(1, 1, 1, &[])
        .expect("one tampered applied-height response corridor");
    rejected
        .enqueue(
            PendingExactFanout::claimed_with_routes(
                vec![message],
                vec![requester],
                vec![ExactTargetRoute::Reply(active_route)],
                ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
            )
            .expect("valid active routed global finality claim")
            .expect("one active routed global response"),
        )
        .expect("retain the active routed global response");
    rejected.fanouts[0].targets[0].parked = true;
    let fifo_before = rejected.source_fifo_owners.clone();
    let reservations_before = rejected.reservation_owner_counts.clone();
    let error = rejected
        .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
        .expect_err("an active route cannot masquerade as a parked source");
    assert!(error.contains("parked reply source changed"));
    assert_eq!(rejected.fanouts.len(), 1);
    assert_eq!(rejected.source_fifo_owners, fifo_before);
    assert_eq!(rejected.reservation_owner_counts, reservations_before);
    assert_eq!(rejected.ownership_units, 1);
    assert_eq!(rejected.shared_ownership_units, 1);
}
#[test]
fn applied_height_handoff_rejects_unbound_lane_output_atomically() {
    let (service, keys) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let mut pending = PendingExactOutput::new(2, 1, 1, &[]).expect("two-fanout corridor");
    let global =
        ProductionV2Services::preencode_v2_network_message(global_commit_qc_message(&artifact))
            .expect("encode global CommitQC");
    let lane_output = lane_commit_qc_block_message(peer.clone());
    let BlockMessage::LaneBlockQc(lane_qc) = &lane_output else {
        unreachable!("lane output fixture must be a CommitQC")
    };
    let lane_message = NetworkMessage::SumeragiBlock(Arc::new(
        BlockMessageWire::try_preencoded(Arc::new(lane_output.clone()))
            .expect("encode lane CommitQC"),
    ));
    pending
        .enqueue(
            PendingExactFanout::claimed(
                vec![global],
                vec![peer.clone()],
                ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
            )
            .expect("valid global claim")
            .expect("global fanout"),
        )
        .expect("retain covered global fanout");
    pending
        .enqueue(
            PendingExactFanout::claimed(
                vec![lane_message],
                vec![peer],
                ExactOutputRolloverClaim::Lane(service.exact_output_scope()),
            )
            .expect("valid lane claim")
            .expect("unbound lane fanout"),
        )
        .expect("retain unbound lane fanout");
    let error = pending
        .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
        .expect_err("a global finality artifact cannot clear unbound lane output");
    assert!(error.contains("typed durable rollover authority"));
    assert_eq!(pending.fanouts.len(), 2, "handoff must be all-or-nothing");
    let missing = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        lane_qc.body.proposal_hash,
    );
    let error = pending
        .handoff_applied_height_to_durable_reconstruction(&artifact, Some(&missing), None)
        .expect_err("a winning lane output requires its durable session witness");
    assert!(
        error.contains("lacks its exact rollover session witness"),
        "unexpected missing-witness error: {error}"
    );
    assert_eq!(pending.fanouts.len(), 2, "handoff must be all-or-nothing");
    let mut wrong_qc = lane_qc.clone();
    wrong_qc.bls_aggregate_signature.push(2);
    let wrong =
        DurableLaneRolloverAuthority::for_test(&artifact, &BlockMessage::LaneBlockQc(wrong_qc));
    let error = pending
        .handoff_applied_height_to_durable_reconstruction(&artifact, Some(&wrong), None)
        .expect_err("a wrong exact lane witness cannot clear retained output");
    assert!(
        error.contains("does not match its exact rollover session witness"),
        "unexpected mismatched-witness error: {error}"
    );
    assert_eq!(pending.fanouts.len(), 2, "handoff must be all-or-nothing");
    let live_peer = service.context.roster[1].validator.clone();
    let live_message = non_retireable_lane_transport_messages(live_peer.clone())
        .into_iter()
        .next()
        .expect("non-retireable lane payload fixture");
    let live_wire = BlockMessageWire::try_preencoded(Arc::new(live_message.clone()))
        .expect("encode non-retireable lane payload");
    let mut live =
        PendingExactOutput::new(1, 1, 1, &[]).expect("one non-retireable lane transport corridor");
    live.enqueue(
        PendingExactFanout::claimed(
            vec![NetworkMessage::SumeragiBlock(Arc::new(live_wire))],
            vec![live_peer.clone()],
            ExactOutputRolloverClaim::NonRetireableLaneTransport {
                target: live_peer,
                message_hash: HashOf::new(&live_message),
            },
        )
        .expect("valid exact non-retireable lane transport claim")
        .expect("one non-retireable lane transport fanout"),
    )
    .expect("retain non-retireable lane transport");
    let fifo_before = live.source_fifo_owners.clone();
    let reservations_before = live.reservation_owner_counts.clone();
    let error = live
        .handoff_applied_height_to_durable_reconstruction(&artifact, Some(&wrong), None)
        .expect_err("non-retireable lane transport must drain before height handoff");
    assert!(error.contains("must drain before applied-height handoff"));
    assert_eq!(
        live.fanouts.len(),
        1,
        "handoff must retain the exact message"
    );
    assert_eq!(live.source_fifo_owners, fifo_before);
    assert_eq!(live.reservation_owner_counts, reservations_before);
    assert_eq!(live.ownership_units, 1);
    assert_eq!(live.shared_ownership_units, 1);
}

fn autonomous_retirement_handoff_fixture(
    attempt: &crate::kura::tests::AutonomousLaneAttemptFixture,
    base_context: &wire::HeightContext,
    validators: &[KeyPair],
) -> (
    ProductionV2Services,
    wire::finality::V2FinalityArtifact,
    DurableLaneRolloverAuthority,
) {
    let mut context = base_context.clone();
    context.network_id = attempt.payload.network_id;
    context.epoch = attempt.payload.epoch;
    context.height = attempt.payload.origin_proposal.descriptor.proposal_height;
    context.parent_commit_qc = None;
    context
        .validate()
        .expect("retired autonomous handoff context is valid");
    let service = service_for_history_context(
        crate::kura::Kura::blank_kura_for_testing(),
        context.clone(),
        validators,
    );
    let block = crate::block::ValidBlock::new_dummy_and_modify_header(
        validators[0].private_key(),
        |header| {
            header.set_height(
                NonZeroU64::new(context.height).expect("autonomous fixture height is non-zero"),
            );
            header.set_prev_block_hash(None);
        },
    )
    .commit_unchecked()
    .unpack(|_| {});
    let header = block.as_ref().header();
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.as_ref().hash(),
        payload_hash: Hash::new(b"control-only canonical autonomous carrier"),
    };
    attempt
        .kura
        .store_block(block)
        .expect("persist control-only canonical carrier");
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"autonomous handoff parent state"),
        Hash::new(b"autonomous handoff post state"),
        Hash::new(b"autonomous handoff ordinary writes"),
        1,
        Hash::new(b"autonomous handoff executed block wire"),
    );
    let artifact = signed_worker_finality_artifact(
        &context,
        validators,
        header.view_change_index(),
        subject,
        execution_commitment,
        [
            "aggregate autonomous handoff CommitQC",
            "autonomous handoff validator PoP",
            "valid autonomous handoff finality artifact",
        ],
    );
    artifact
        .validate_for_header(&header)
        .expect("autonomous handoff artifact matches its canonical carrier");
    artifact
        .verify()
        .expect("verify autonomous handoff finality proof");
    let authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"different canonical autonomous carrier proposal"),
    );
    (service, artifact, authority)
}

fn pending_autonomous_lane_output(
    service: &ProductionV2Services,
    messages: impl IntoIterator<Item = BlockMessage>,
) -> PendingExactOutput {
    let messages = messages.into_iter().collect::<Vec<_>>();
    let mut pending = PendingExactOutput::new(messages.len(), 1, 1, &[])
        .expect("bounded autonomous handoff corridor");
    let target = service.context.roster[1].validator.clone();
    for message in messages {
        let wire = BlockMessageWire::try_preencoded(Arc::new(message))
            .expect("encode autonomous handoff output");
        pending
            .enqueue(
                PendingExactFanout::claimed(
                    vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
                    vec![target.clone()],
                    ExactOutputRolloverClaim::AutonomousLane {
                        scope: service.exact_output_scope(),
                        local_peer: service.local_peer.clone(),
                        proposal_height: service.context.height,
                    },
                )
                .expect("valid autonomous handoff claim")
                .expect("one autonomous handoff fanout"),
            )
            .expect("retain autonomous handoff fanout");
    }
    pending
}

#[test]
fn autonomous_payload_carrier_comparison_promotes_only_a_missing_advisory_hint() {
    let (_, validators) = fixture();
    let attempt = crate::kura::tests::unretired_autonomous_lane_attempt_fixture(&validators[0]);
    let mut hint_free = attempt.payload;
    hint_free.origin_proposal.payload_block_hint = None;
    assert!(hint_free.origin_proposal.payload_block_hint.is_none());
    let canonical_hint = LaneBlockProposalPayloadHintV1 {
        proposal_height: hint_free.origin_proposal.descriptor.proposal_height,
        proposal_view: 0,
        proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"canonical autonomous carrier",
        )),
    };
    let canonical = hint_free
        .attach_global_hint_exact(canonical_hint, hint_free.network_id, hint_free.epoch)
        .expect("attach the post-finality canonical carrier hint");

    assert!(autonomous_payload_matches_canonical_carrier(
        &canonical,
        &canonical,
        hint_free.network_id,
        hint_free.epoch,
    ));
    assert!(autonomous_payload_matches_canonical_carrier(
        &hint_free,
        &canonical,
        hint_free.network_id,
        hint_free.epoch,
    ));

    let conflicting_hint = LaneBlockProposalPayloadHintV1 {
        proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"conflicting autonomous carrier",
        )),
        ..canonical_hint
    };
    let conflicting = hint_free
        .attach_global_hint_exact(conflicting_hint, hint_free.network_id, hint_free.epoch)
        .expect("attach a structurally valid but conflicting carrier hint");
    assert!(!autonomous_payload_matches_canonical_carrier(
        &conflicting,
        &canonical,
        hint_free.network_id,
        hint_free.epoch,
    ));

    let mut mutated = hint_free;
    mutated.producer_signature[0] ^= 1;
    assert!(!autonomous_payload_matches_canonical_carrier(
        &mutated,
        &canonical,
        mutated.network_id,
        mutated.epoch,
    ));
}

#[test]
fn applied_height_handoff_retires_only_exact_same_finality_nonwinning_autonomous_outputs_atomically()
 {
    let (base_service, validators) = fixture();
    let retired = crate::kura::tests::retired_autonomous_lane_attempt_fixture(&validators[0]);
    let (service, artifact, authority) =
        autonomous_retirement_handoff_fixture(&retired, &base_service.context, &validators);
    for output in [
        BlockMessage::LaneExecutablePayload(retired.payload.clone()),
        BlockMessage::LaneBlockNewViewVote(retired.new_view_vote.clone()),
        BlockMessage::LaneBlockNewViewCertificate(retired.new_view_certificate.clone()),
    ] {
        assert!(autonomous_lane_output_matches_payload_identity(
            &output,
            &retired.payload,
        ));
    }
    let mut pending = pending_autonomous_lane_output(
        &service,
        [
            BlockMessage::LaneExecutablePayload(retired.payload.clone()),
            BlockMessage::LaneBlockNewViewVote(retired.new_view_vote.clone()),
            BlockMessage::LaneBlockNewViewCertificate(retired.new_view_certificate.clone()),
        ],
    );
    assert_eq!(
        pending
            .handoff_applied_height_to_durable_reconstruction(
                &artifact,
                Some(&authority),
                Some(retired.kura.as_ref()),
            )
            .expect("exact retired attempt supersedes all autonomous output variants"),
        3
    );
    assert!(!pending.is_pending());

    let (_, unrelated_artifact) = durable_finality_fixture(&base_service, &validators);
    let unrelated_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &unrelated_artifact,
        Hash::new(b"unrelated autonomous handoff authority"),
    );
    let mut unbound = pending_autonomous_lane_output(
        &service,
        [BlockMessage::LaneExecutablePayload(retired.payload.clone())],
    );
    let error = unbound
        .handoff_applied_height_to_durable_reconstruction(
            &artifact,
            Some(&unrelated_authority),
            Some(retired.kura.as_ref()),
        )
        .expect_err("another finality artifact cannot lend rollover authority");
    assert!(error.contains("not bound to a nonwinning finalized carrier"));
    assert!(unbound.is_pending(), "failed handoff remains atomic");

    let mut mutated_payload = retired.payload.clone();
    mutated_payload.payload_hash = Hash::new(b"mutated retired autonomous output");
    let mut mutated = pending_autonomous_lane_output(
        &service,
        [BlockMessage::LaneExecutablePayload(mutated_payload)],
    );
    mutated
        .handoff_applied_height_to_durable_reconstruction(
            &artifact,
            Some(&authority),
            Some(retired.kura.as_ref()),
        )
        .expect_err("mutated output cannot borrow the exact retirement");
    assert!(mutated.is_pending(), "failed handoff remains atomic");

    let unretired = crate::kura::tests::unretired_autonomous_lane_attempt_fixture(&validators[0]);
    let (unretired_service, unretired_artifact, unretired_authority) =
        autonomous_retirement_handoff_fixture(&unretired, &base_service.context, &validators);
    let mut missing = pending_autonomous_lane_output(
        &unretired_service,
        [BlockMessage::LaneExecutablePayload(
            unretired.payload.clone(),
        )],
    );
    let error = missing
        .handoff_applied_height_to_durable_reconstruction(
            &unretired_artifact,
            Some(&unretired_authority),
            Some(unretired.kura.as_ref()),
        )
        .expect_err("a live attempt has no terminal handoff authority");
    assert!(error.contains("no exact durable slot retirement"));
    assert!(missing.is_pending(), "failed handoff remains atomic");
}

#[test]
fn applied_height_handoff_rejects_wrong_height_global_output() {
    let (service, keys) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let mut wrong_height = artifact.commit_qc.clone();
    wrong_height.round.height = wrong_height.round.height.saturating_add(1);
    let message =
        ProductionV2Services::preencode_v2_network_message(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(wrong_height),
        ))
        .expect("encode wrong-height global certificate");
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    pending
        .enqueue(
            PendingExactFanout::claimed(
                vec![message],
                vec![peer],
                ExactOutputRolloverClaim::GlobalV2(service.exact_output_scope()),
            )
            .expect("valid creation claim")
            .expect("wrong-height fanout"),
        )
        .expect("retain wrong-height fanout");
    let error = pending
        .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
        .expect_err("wrong-height output has no applied-height witness");
    assert!(error.contains("not bound to the applied height"));
    assert!(pending.is_pending());
}
#[test]
fn applied_height_handoff_accepts_historical_kura_global_responses_atomically() {
    let history = durable_history_fixture();
    let archive_successor = || {
        successor_service_for_history_as(
            Arc::clone(&history.kura),
            &history.artifact,
            &history.validators,
            3,
        )
    };
    let mut service = archive_successor();
    let (receipt, applied_artifact) = durable_finality_fixture(&service, &history.validators);
    let commit_message =
        ProductionV2Services::preencode_v2_network_message(history.commit_response.clone())
            .expect("encode historical CommitQC response");
    let mut manual = PendingExactOutput::new(1, 1, 1, &[]).expect("one manual response");
    manual
        .enqueue(
            PendingExactFanout::new(
                vec![commit_message.clone()],
                vec![history.requester.clone()],
            )
            .expect("manual historical response"),
        )
        .expect("retain manual historical response");
    let error = manual
        .handoff_applied_height_to_durable_reconstruction(
            &applied_artifact,
            None,
            Some(history.kura.as_ref()),
        )
        .expect_err("Kura presence cannot authorize an untyped manual response");
    assert!(error.contains("no typed applied-height rollover claim"));
    assert!(manual.is_pending());
    install_exact_output_backpressure(&mut service);
    for response in [
        history.commit_response.clone(),
        history.body_response.clone(),
    ] {
        let guard = Arc::clone(&service.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .expect("valid historical response operation");
        service
            .post_durable_history_response_with_permit(
                history.requester.clone(),
                response,
                operation.permit(),
            )
            .expect("live emitter accepts exact Kura response");
        operation.complete();
    }
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect live historical output");
    assert_eq!(
        pending.fanouts.len(),
        2,
        "both live responses remain retained behind target pressure"
    );
    assert!(matches!(
        pending.fanouts[0].rollover_claim,
        ExactOutputRolloverClaim::DurableCommitCertificateResponse { .. }
    ));
    assert!(matches!(
        pending.fanouts[1].rollover_claim,
        ExactOutputRolloverClaim::DurableCertifiedBodyResponse { .. }
    ));
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(archive_body_response) =
        &history.body_response.payload
    else {
        panic!("history fixture must contain a certified body response")
    };
    assert_eq!(
        archive_body_response.responder,
        PeerId::new(history.validators[3].public_key().clone()),
        "durable rollover must retain a frozen-roster archive that did not sign the old QC"
    );
    assert!(
        !history.artifact.commit_qc.signers.contains(&3),
        "the regression must exercise archive authority independently of QC signing"
    );
    drop(pending);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &applied_artifact,
        Hash::new(b"unused historical global-response lane witness"),
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &receipt,
                &applied_artifact,
                &lane_authority,
            )
            .expect("rollover independently rereads both Kura sources"),
        2
    );
    assert!(!service.has_pending_exact_output().expect("inspect handoff"));
    let wire::ConsensusMessageV2Payload::CommitCertificateResponse(mut substituted_commit) =
        history.commit_response.payload.clone()
    else {
        panic!("history fixture must contain a CommitQC response")
    };
    substituted_commit.certificate.aggregate_signature[0] ^= 0x01;
    substituted_commit.signature = Signature::new(
        history.validators[3].private_key(),
        &substituted_commit.signature_preimage(),
    )
    .payload()
    .to_vec();
    let substituted_message =
        ProductionV2Services::preencode_v2_network_message(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(substituted_commit.clone()),
        ))
        .expect("encode substituted historical CommitQC response");
    let mut mismatched = PendingExactOutput::new(1, 1, 1, &[]).expect("one mismatched response");
    mismatched
        .enqueue(
            PendingExactFanout::claimed(
                vec![substituted_message],
                vec![history.requester.clone()],
                ExactOutputRolloverClaim::DurableCommitCertificateResponse {
                    scope: service.exact_output_scope(),
                    target: history.requester.clone(),
                    responder: substituted_commit.responder.clone(),
                    source_height: substituted_commit.certificate.round.height,
                    source_context_id: substituted_commit.certificate.round.context_id,
                    response_hash: HashOf::new(&substituted_commit),
                },
            )
            .expect("self-consistent substituted CommitQC claim")
            .expect("substituted CommitQC fanout"),
        )
        .expect("retain substituted CommitQC response");
    let error = mismatched
        .handoff_applied_height_to_durable_reconstruction(
            &applied_artifact,
            None,
            Some(history.kura.as_ref()),
        )
        .expect_err("handoff must independently reject a non-Kura CommitQC");
    assert!(error.contains("differs from its Kura finality source"));
    assert!(mismatched.is_pending(), "failed handoff remains atomic");
    let mut rejected_commit_service = archive_successor();
    assert_rejected_before_actor_admission(
        &mut rejected_commit_service,
        |service| {
            post_history_response_for_rejection(
                service,
                history.requester.clone(),
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CommitCertificateResponse(substituted_commit),
                ),
                "invalid CommitQC response operation",
            )
        },
        "substituted CommitQC must fail before actor admission",
        "differs from its Kura finality source",
        "inspect rejected CommitQC response",
    );
    let mut rejected_service = archive_successor();
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(mut wrong_responder) =
        history.body_response.payload.clone()
    else {
        panic!("history fixture must contain a certified body response")
    };
    wrong_responder.responder = PeerId::new(history.validators[1].public_key().clone());
    wrong_responder.signature = Signature::new(
        history.validators[1].private_key(),
        &wrong_responder.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert_rejected_before_actor_admission(
        &mut rejected_service,
        |service| {
            post_history_response_for_rejection(
                service,
                history.requester.clone(),
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wrong_responder),
                ),
                "invalid historical response operation",
            )
        },
        "wrong historical responder must fail before actor admission",
        "serving network identity",
        "inspect rejected body response",
    );
    let mut rejected_body_service = archive_successor();
    let _ = archive_successor;
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(mut substituted_body) =
        history.body_response.payload
    else {
        panic!("history fixture must contain a certified body response")
    };
    substituted_body.body[0] ^= 0x01;
    let substituted_subject = wire::BlockSubject {
        payload_hash: Hash::new(&substituted_body.body),
        ..substituted_body.manifest.subject
    };
    let (substituted_manifest, _) = encode_payload(
        &history.artifact.height_context,
        substituted_body.manifest.round,
        substituted_subject,
        &substituted_body.body,
    )
    .expect("encode self-consistent substituted historical body")
    .into_parts();
    substituted_body.manifest = substituted_manifest;
    substituted_body.signature = Signature::new(
        history.validators[3].private_key(),
        &substituted_body.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert_rejected_before_actor_admission(
        &mut rejected_body_service,
        |service| {
            post_history_response_for_rejection(
                service,
                history.requester,
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(substituted_body),
                ),
                "invalid body response operation",
            )
        },
        "substituted canonical body must fail before actor admission",
        "differs from its Kura finality source",
        "inspect rejected canonical body response",
    );
}
#[test]
fn applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output() {
    let lane_history = durable_lane_history_fixture();
    let lane_kura = lane_history.kura;
    let certificate = lane_history.certificate;
    let lane_context = lane_history.context;
    let lane_validators = lane_history.validators;
    let parent_service =
        service_for_history_context(Arc::clone(&lane_kura), lane_context, &lane_validators);
    let (_, parent_artifact) = durable_finality_fixture(&parent_service, &lane_validators);
    let mut service =
        successor_service_for_history(Arc::clone(&lane_kura), &parent_artifact, &lane_validators);
    let (receipt, applied_artifact) = durable_finality_fixture(&service, &lane_validators);
    let target = service.context.roster[1].validator.clone();
    let historical_output = BlockMessage::LaneBlockQc(certificate.commit_qc.clone());
    install_exact_output_backpressure(&mut service);
    service
        .post_lane_block(target.clone(), historical_output.clone())
        .expect("retain exact ordinary historical lane output");
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect historical lane certification claim");
    assert_eq!(pending.fanouts.len(), 1);
    assert!(matches!(
        &pending.fanouts[0].rollover_claim,
        ExactOutputRolloverClaim::HistoricalLaneCertification {
            target: claimed_target,
            source_height,
            proposal_hash,
            message_hash,
            ..
        } if claimed_target == &target
            && *source_height == certificate.proposal.descriptor.proposal_height
            && *proposal_hash == certificate.proposal.proposal_hash
            && *message_hash == HashOf::new(&historical_output)
    ));
    drop(pending);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &applied_artifact,
        Hash::new(b"unused ordinary historical lane witness"),
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &receipt,
                &applied_artifact,
                &lane_authority,
            )
            .expect("ordinary historical output rereads its certificate and application receipt"),
        1
    );
    assert!(
        !service
            .has_pending_exact_output()
            .expect("inspect completed ordinary historical handoff")
    );
}

#[test]
fn applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate() {
    let lane_history = historical_autonomous_lane_certificate_fixture();
    let lane_kura = lane_history.kura;
    let certificate = lane_history.certificate;
    let lane_context = lane_history.context;
    let lane_validators = lane_history.validators;
    assert_eq!(
        lane_kura
            .historical_autonomous_lane_recovery_records_bounded(
                crate::kura::HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            )
            .expect("read exact historical autonomous recovery record")
            .len(),
        1,
    );
    assert!(
        lane_kura
            .read_lane_block_application_receipt(
                certificate.proposal.descriptor.lane_id,
                certificate.proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "record-backed autonomous history must not require an ordinary application receipt"
    );
    let parent_service =
        service_for_history_context(Arc::clone(&lane_kura), lane_context, &lane_validators);
    let (_, parent_artifact) = durable_finality_fixture(&parent_service, &lane_validators);
    let mut service =
        successor_service_for_history(Arc::clone(&lane_kura), &parent_artifact, &lane_validators);
    let (receipt, applied_artifact) = durable_finality_fixture(&service, &lane_validators);
    let target = service.context.roster[1].validator.clone();
    let historical_output = BlockMessage::LaneBlockCertificate(Box::new(certificate.clone()));
    install_exact_output_backpressure(&mut service);
    service
        .post_lane_block(target.clone(), historical_output.clone())
        .expect("retain exact autonomous historical lane certificate");
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect autonomous historical lane certification claim");
    assert_eq!(pending.fanouts.len(), 1);
    assert!(matches!(
        &pending.fanouts[0].rollover_claim,
        ExactOutputRolloverClaim::HistoricalLaneCertification {
            target: claimed_target,
            source_height,
            proposal_hash,
            message_hash,
            ..
        } if claimed_target == &target
            && *source_height == certificate.proposal.descriptor.proposal_height
            && *proposal_hash == certificate.proposal.proposal_hash
            && *message_hash == HashOf::new(&historical_output)
    ));
    drop(pending);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &applied_artifact,
        Hash::new(b"unused autonomous historical lane witness"),
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &receipt,
                &applied_artifact,
                &lane_authority,
            )
            .expect("autonomous historical output rereads its immutable recovery record"),
        1
    );
    assert!(
        !service
            .has_pending_exact_output()
            .expect("inspect completed autonomous historical handoff")
    );
}

#[test]
fn applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate() {
    let lane_history = durable_lane_history_fixture();
    let lane_kura = lane_history.kura;
    let certificate = lane_history.certificate;
    let lane_context = lane_history.context;
    let lane_validators = lane_history.validators;
    let parent_service =
        service_for_history_context(Arc::clone(&lane_kura), lane_context, &lane_validators);
    let (_, parent_artifact) = durable_finality_fixture(&parent_service, &lane_validators);
    let mut service =
        successor_service_for_history(Arc::clone(&lane_kura), &parent_artifact, &lane_validators);
    let (receipt, applied_artifact) = durable_finality_fixture(&service, &lane_validators);
    let target = service.context.roster[1].validator.clone();
    install_exact_output_backpressure(&mut service);
    service
        .post_durable_lane_certificate(target.clone(), certificate.clone())
        .expect("live emitter accepts exact certified Kura lane response");
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect live lane response");
    assert_eq!(pending.fanouts.len(), 1);
    assert!(matches!(
        pending.fanouts[0].rollover_claim,
        ExactOutputRolloverClaim::DurableLaneCertificateResponse { .. }
    ));
    drop(pending);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &applied_artifact,
        Hash::new(b"unused historical lane-response witness"),
    );
    assert_eq!(
        service
            .handoff_applied_height_output_to_durable_reconstruction(
                &receipt,
                &applied_artifact,
                &lane_authority,
            )
            .expect("rollover independently rereads the certified Kura lane artifact"),
        1
    );
    let mut substituted = certificate;
    substituted.commit_qc.bls_aggregate_signature[0] ^= 0x01;
    let mut rejected_service =
        successor_service_for_history(Arc::clone(&lane_kura), &parent_artifact, &lane_validators);
    assert_rejected_before_actor_admission(
        &mut rejected_service,
        |service| service.post_durable_lane_certificate(target, substituted),
        "a modified lane proof must fail before actor admission",
        "differs from its certified Kura source",
        "inspect rejected lane response",
    );
}
#[test]
fn applied_height_handoff_authenticates_exact_payload_chunk_fanout() {
    let (mut service, keys) = fixture_with_block_payload();
    let peer = service.context.roster[1].validator.clone();
    let (_, artifact) = durable_finality_fixture(&service, &keys);
    let (_, payload, _) = proposal_body_and_payload(&service.context, &keys);
    let manifest = payload.manifest().clone();
    let owner = service.active_tag;
    service
        .register_outbound_payload(owner, payload)
        .expect("sign and retain exact payload chunks");
    let retained_chunks = service
        .outbound_chunks
        .get(&HashOf::new(&manifest))
        .expect("registered payload owns its exact manifest")
        .messages
        .clone();
    let messages = retained_chunks.clone();
    let chunk_count = messages.len();
    let claim = ExactOutputRolloverClaim::PayloadChunks {
        scope: service.exact_output_scope(),
        manifest: manifest.clone(),
    };
    let mut pending = PendingExactOutput::new(1, chunk_count, 1, &[])
        .expect("one exact payload-chunk fanout corridor");
    pending
        .enqueue(
            PendingExactFanout::claimed(messages, vec![peer.clone()], claim)
                .expect("payload chunks match their exact manifest claim")
                .expect("non-empty payload-chunk fanout"),
        )
        .expect("retain exact payload-chunk fanout");
    assert_eq!(
        pending
            .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
            .expect("applied context authenticates every retained payload chunk"),
        chunk_count
    );
    assert!(!pending.is_pending());
    let mut tampered_chunks = retained_chunks;
    let NetworkMessage::SumeragiBlock(envelope) = &mut tampered_chunks[0] else {
        unreachable!("registered outbound payload contains only Sumeragi messages")
    };
    let BlockMessage::V2(message) = Arc::make_mut(envelope).make_mut() else {
        unreachable!("registered outbound payload contains only v2 messages")
    };
    let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &mut message.payload else {
        unreachable!("registered outbound payload contains only chunks")
    };
    chunk.signature[0] ^= 0x01;
    let mut tampered = PendingExactOutput::new(1, chunk_count, 1, &[])
        .expect("one tampered payload-chunk fanout corridor");
    tampered
        .enqueue(
            PendingExactFanout::claimed(
                tampered_chunks,
                vec![peer],
                ExactOutputRolloverClaim::PayloadChunks {
                    scope: service.exact_output_scope(),
                    manifest,
                },
            )
            .expect("tampered signature retains exact structural coordinates")
            .expect("non-empty tampered payload-chunk fanout"),
        )
        .expect("retain structurally exact tampered payload chunks");
    let error = tampered
        .handoff_applied_height_to_durable_reconstruction(&artifact, None, None)
        .expect_err("an altered chunk signature cannot cross finality handoff");
    assert!(error.contains("signature"));
    assert!(tampered.is_pending(), "rejection is atomic");
}
