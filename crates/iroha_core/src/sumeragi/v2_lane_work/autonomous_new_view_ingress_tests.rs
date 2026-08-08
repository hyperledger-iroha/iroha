#[test]
fn autonomous_payload_and_new_view_ingress_are_exact_and_contiguous() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
        std::borrow::Cow::Owned(entrypoint.clone()),
    );
    let routing_plan = RoutingPlan::single(RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let mut reservation = crate::queue::LaneQueueReservationKeyV2 {
        version: crate::queue::LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: accepted.hash(),
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"autonomous-ingress-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"autonomous-ingress-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let producer = adapter
        .expected_autonomous_lane_author(&proposal)
        .expect("deterministic autonomous producer")
        .clone();
    bind_canonical_autonomous_reservation_identity(
        &adapter,
        &proposal,
        &producer,
        &mut reservation,
    );
    let producer_key = keys
        .iter()
        .find(|key| key.public_key() == producer.public_key())
        .expect("producer key");
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
        proposal,
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer.clone(),
        producer_key.private_key(),
    )
    .expect("signed autonomous payload");

    let wrong_sender = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .find(|peer| peer != &producer)
        .expect("non-producer sender");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(wrong_sender),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected
    );

    let mut zero_owner = payload.clone();
    zero_owner.reservation_keys[0].reservation_owner_hash = Hash::prehashed([0; Hash::LENGTH]);
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(zero_owner),
                Some(producer.clone()),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(producer.clone()),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(producer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Duplicate
    );
    let premature_body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &payload.origin_proposal,
        &payload,
        1,
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
    )
    .expect("well-formed pre-lock NewView body");
    let premature_signer = payload.origin_proposal.descriptor.validator_set[0].clone();
    let premature_key = keys
        .iter()
        .find(|key| key.public_key() == premature_signer.public_key())
        .expect("pre-lock NewView signer key");
    let premature_vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        premature_body,
        premature_signer.clone(),
        premature_key.private_key(),
    )
    .expect("signed pre-lock NewView vote");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockNewViewVote(premature_vote),
                Some(premature_signer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected,
        "hinted bytes cannot advance before their exact protected carrier is durable"
    );
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "NewView is legal only after the exact carrier and payload are durable"
    );
    let payload_key = AutonomousLanePayloadKey::from(&payload.origin_proposal);
    assert_eq!(
        adapter
            .autonomous_new_view_started_at
            .get(&payload_key)
            .map(|(view, _)| *view),
        Some(0)
    );

    let synthetic_view_one =
        crate::lane_consensus::retarget_lane_block_proposal_exact_view(&payload.origin_proposal, 1)
            .expect("synthetic view-one cursor");
    let gap_body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &synthetic_view_one,
        &payload,
        2,
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
    )
    .expect("well-formed but premature view-two transition");
    let gap_signer = payload.origin_proposal.descriptor.validator_set[0].clone();
    let gap_key = keys
        .iter()
        .find(|key| key.public_key() == gap_signer.public_key())
        .expect("gap signer key");
    let gap_vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        gap_body,
        gap_signer.clone(),
        gap_key.private_key(),
    )
    .expect("signed premature NewView vote");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockNewViewVote(gap_vote),
                Some(gap_signer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected,
        "a valid future transition cannot skip the retained view cursor"
    );

    let body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &payload.origin_proposal,
        &payload,
        1,
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
    )
    .expect("view-one transition");
    let quorum = usize::try_from(body.min_quorum).expect("quorum fits usize");
    let started_at = Instant::now();
    adapter
        .autonomous_new_view_started_at
        .insert(payload_key, (0, started_at));
    let _ = adapter.drain_effects(usize::MAX);
    let timeout = Duration::from_secs(1);
    adapter
        .schedule_autonomous_new_view_timeouts(
            started_at + timeout - Duration::from_nanos(1),
            0,
            timeout,
        )
        .expect("pre-deadline NewView tick");
    assert!(
        adapter
            .autonomous_new_view_votes
            .votes_for_signer(&adapter.local_peer)
            .is_empty(),
        "a lane must not emit NewView before its independent deadline"
    );
    assert!(adapter.drain_effects(usize::MAX).is_empty());

    adapter
        .schedule_autonomous_new_view_timeouts(started_at + timeout, 0, timeout)
        .expect("deadline NewView tick");
    let first_fanout = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .filter_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockNewViewVote(vote),
                ..
            } => Some(vote),
            _ => None,
        })
        .collect::<Vec<_>>();
    let local_vote = first_fanout
        .first()
        .cloned()
        .expect("deadline emits the local NewView vote");
    assert!(
        first_fanout.iter().all(|vote| vote == &local_vote),
        "one timeout fanout must retain byte-identical vote evidence"
    );
    adapter
        .schedule_autonomous_new_view_timeouts(
            started_at + timeout + Duration::from_millis(1),
            0,
            timeout,
        )
        .expect("due NewView retransmission tick");
    let repeated_fanout = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .filter_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockNewViewVote(vote),
                ..
            } => Some(vote),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !repeated_fanout.is_empty() && repeated_fanout.iter().all(|vote| vote == &local_vote),
        "a still-due clock retransmits the exact cached vote without changing identity"
    );

    let mut accepted = 1_usize;
    let mut last_vote = local_vote;
    let local_peer = adapter.local_peer.clone();
    for signer in payload
        .origin_proposal
        .descriptor
        .validator_set
        .iter()
        .filter(|signer| *signer != &local_peer)
    {
        if accepted >= quorum {
            break;
        }
        let signer_key = keys
            .iter()
            .find(|key| key.public_key() == signer.public_key())
            .expect("NewView signer key");
        let vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
            body.clone(),
            signer.clone(),
            signer_key.private_key(),
        )
        .expect("signed contiguous NewView vote");
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockNewViewVote(vote.clone()),
                    Some(signer.clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        last_vote = vote;
        accepted = accepted.saturating_add(1);
    }
    assert_eq!(adapter.autonomous_payload_views.get(&payload_key), Some(&1));
    let (durable_payload, durable_cursor) = adapter
        .kura
        .current_autonomous_lane_payload(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            adapter.native_chain_id_hash(),
            adapter.context.epoch,
        )
        .expect("quorum persists the exact autonomous NewView cursor");
    assert_eq!(durable_payload, payload);
    assert_eq!(durable_cursor.descriptor.lane_block_view, 1);
    let incomplete_lane_sessions = adapter.lane_sessions.proposals_without_commit_qc();
    assert!(
        incomplete_lane_sessions.contains(&payload.origin_proposal)
            && incomplete_lane_sessions
                .iter()
                .all(|proposal| proposal.descriptor.lane_block_view == 0),
        "NewView must not create a second READY/Prepare/Commit subject"
    );
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockNewViewVote(last_vote.clone()),
                Some(last_vote.signer.clone()),
            ),
            0,
        ),
        V2LaneIngressOutcome::Duplicate,
        "a post-seal retransmission remains idempotent after the cursor advances"
    );

    let durable_certificate = adapter
        .kura
        .read_autonomous_lane_block_artifact(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            adapter.native_chain_id_hash(),
            adapter.context.epoch,
        )
        .and_then(|artifact| {
            V2LaneWorkAdapter::latest_durable_autonomous_new_view_certificate(&artifact, 1).cloned()
        })
        .expect("durable view-one certificate");
    let context = adapter.context.clone();
    let local_peer = adapter.local_peer.clone();
    let key_pair = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);

    let mut recovered = V2LaneWorkAdapter::new_with_output_guard(
        context,
        local_peer,
        key_pair,
        true,
        state,
        kura,
        limits,
        None,
        None,
        ConsensusOutputGuard::isolated(),
    )
    .expect("reopen adapter after durable NewView publication");
    assert!(
        recovered.autonomous_payloads.is_empty(),
        "constructor cannot trust a hinted payload before reducer lock replay"
    );
    let (recovered_round, recovered_subject) = global_lock_for_block(&recovered, &block);
    assert_eq!(
        recovered.mark_global_body_locked(recovered_round, recovered_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        recovered.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        recovered.autonomous_payload_views.get(&payload_key),
        Some(&1),
        "lock replay must restore Kura's cursor rather than regress to origin view"
    );
    assert!(
        recovered
            .autonomous_new_view_certificates
            .contains(&durable_certificate.certificate)
    );
    let _ = recovered.drain_effects(usize::MAX);
    recovered
        .schedule_retransmission()
        .expect("retransmit restored durable NewView certificate");
    assert!(
        recovered
            .drain_effects(usize::MAX)
            .into_iter()
            .any(|effect| {
                matches!(
                    effect,
                    V2LaneWorkEffect::PostLaneBlock {
                        message: BlockMessage::LaneBlockNewViewCertificate(ref certificate),
                        ..
                    } if certificate == &durable_certificate.certificate
                )
            })
    );
}
