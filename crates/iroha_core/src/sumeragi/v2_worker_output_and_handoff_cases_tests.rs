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

#[test]
fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let fanout = || {
        PendingExactFanout::new(
            vec![lane_commit_qc_message(peer.clone())],
            vec![peer.clone()],
        )
        .expect("one exact topology fanout")
    };

    let mut tight = PendingExactOutput::new(1, 1, 1, &[])
        .expect("one individual fanout fits the exact corridor");
    assert!(
        tight
            .prepare_atomic_fanout_batch(vec![fanout()])
            .expect("one fanout is structurally exact")
            .is_some(),
        "each child fits independently"
    );
    tight.next_fanout_fifo_id = ExactFanoutFifoId::MAX;
    assert!(
        tight
            .prepare_atomic_fanout_batch(vec![fanout(), fanout()])
            .expect("the pair is structurally exact")
            .is_none(),
        "aggregate demand must be checked before admitting either child"
    );
    assert!(tight.fanouts.is_empty());
    assert!(tight.source_fifo_owners.is_empty());
    assert!(tight.reservation_owner_counts.is_empty());
    assert_eq!(tight.next_fanout_fifo_id, ExactFanoutFifoId::MAX);

    let mut roomy = PendingExactOutput::new(2, 1, 1, &[])
        .expect("the exact pair fits the aggregate corridor");
    roomy.next_fanout_fifo_id = ExactFanoutFifoId::MAX;
    let plan = roomy
        .prepare_atomic_fanout_batch(vec![fanout(), fanout()])
        .expect("prepare the exact pair")
        .expect("aggregate capacity retains both children");
    assert!(
        roomy.fanouts.is_empty(),
        "preflight cannot publish the pair"
    );
    assert_eq!(roomy.next_fanout_fifo_id, ExactFanoutFifoId::MAX);
    roomy.commit_atomic_fanout_batch(plan);
    assert_eq!(roomy.fanouts.len(), 2);
    assert_eq!(roomy.fanouts[0].fifo_id, Some(0));
    assert_eq!(roomy.fanouts[1].fifo_id, Some(1));
    assert_eq!(roomy.next_fanout_fifo_id, 2);
    assert_eq!(roomy.ownership_units, 2);
    assert_eq!(roomy.shared_ownership_units, 2);
}

#[test]
fn armed_recovered_proposal_output_reservation_fails_stop_on_drop() {
    use super::super::v2_lifecycle_coordinator::{
        RecoveredLifecycleSignClassV1, RecoveredLifecycleSignDispatchIdentityV1,
    };

    let (mut service, keys) = fixture_with_block_payload();
    let (_, payload, mut proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = service.active_tag;
    let request = super::super::v2::SignRequest::Proposal(proposal.clone());
    let dispatch_key = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        92,
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
    let directory = TempDir::new().expect("temporary armed Proposal output store");
    let identity = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open armed Proposal output store")
        .instance_identity();
    let wal_append = RecoveredLifecycleProposalPrepareWalAppendSealV1 {
        dispatch_key,
        body_store_identity: identity.clone(),
        output_guard: Arc::clone(&service.output_guard),
        attempted: false,
    };
    let authority =
        super::super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1::for_test(
            &service.context,
            dispatch_key,
            tag,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal)),
            payload,
            identity,
            Arc::clone(&service.output_guard),
        )
        .expect("mint exact armed Proposal authority");
    let operation = service
        .output_guard
        .begin_fail_stop_operation()
        .expect("arm the exact Proposal output cut");
    let pending = service
        .lock_pending_exact_output()
        .expect("retain the exact Proposal corridor mutex");
    drop(RecoveredLifecycleProposalExactOutputReservationV1 {
        operation: Some(operation),
        pending: Some(pending),
        batch: None,
        authority: Some(authority),
        wal_append,
    });
    assert!(
        service.output_guard.restart_required(),
        "dropping an armed Proposal reservation must close process output"
    );
}

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
