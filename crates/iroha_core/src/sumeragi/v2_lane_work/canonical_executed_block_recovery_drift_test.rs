fn commit_result_bearing_lane_parent(
    valid: ValidBlock,
    state: &State,
    leader_private: &PrivateKey,
) -> CommittedBlock {
    let mut signed: SignedBlock = valid.into();
    let axt_policy_snapshot = state.block(signed.header()).axt_policy_snapshot();
    signed
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &[],
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            axt_policy_snapshot,
        )
        .expect("attach the required lane-work parent AXT policy snapshot");
    let signature = SignatureOf::try_from_hash(leader_private, signed.header().hash())
        .expect("sign the result-bearing lane-work parent");
    signed
        .replace_signatures([BlockSignature::new(0, signature)].into_iter().collect())
        .expect("replace the lane-work parent signature after attaching results");
    ValidBlock::new_unverified_for_tests(signed)
        .commit_unchecked()
        .unpack(|_| {})
}

#[test]
#[allow(clippy::too_many_lines)]
fn canonical_executed_block_recovery_rejects_drift_rotates_signers_and_caches_exact_body() {
    let (adapter, keys, canonical_block, finality) = canonical_executed_block_recovery_fixture();
    let height = NonZeroUsize::new(
        usize::try_from(canonical_block.header().height().get())
            .expect("canonical fixture height fits usize"),
    )
    .expect("non-zero canonical fixture height");
    let need = canonical_executed_block_need(&canonical_block, &finality);
    let requester = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .find(|peer| peer != &adapter.local_peer)
        .expect("fixture has a remote finality signer");
    let responder = finality
        .commit_qc
        .signers
        .iter()
        .filter_map(|index| {
            usize::try_from(*index)
                .ok()
                .and_then(|index| finality.height_context.roster.get(index))
                .map(|entry| entry.validator.clone())
        })
        .find(|peer| peer != &requester)
        .expect("fixture has a first deterministic remote responder");
    let request = canonical_executed_block_request(requester.clone(), need, 0);
    let response = build_canonical_executed_block_response(
        &adapter.context,
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        adapter.limits,
        &request,
        &requester,
    )
    .expect("exact CommitQC signer serves canonical body before pruning");
    evict_canonical_executed_block_fixture(&adapter, &keys, &canonical_block);
    let context = adapter.context.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    assert!(kura.get_block_without_merge_sidecar(height).is_none());
    let output_guard = ConsensusOutputGuard::isolated();
    let mut recovery = CanonicalExecutedBlockRecovery::new(
        context,
        requester,
        Arc::clone(&state),
        Arc::clone(&kura),
        Arc::clone(&output_guard),
        limits,
        vec![need],
    )
    .expect("install exact canonical executed-block recovery need");
    let attempt_limit = limits
        .historical_recovery_stuck_attempts
        .get()
        .saturating_mul(limits.historical_recovery_max_retry_tier.get());
    for _ in 0..attempt_limit {
        recovery
            .record_front_attempt()
            .expect("attempt remains inside the configured recovery bound");
    }
    assert!(
        recovery.record_front_attempt().is_err(),
        "an unresponsive signer set must not permit unbounded startup recovery attempts"
    );
    assert!(recovery.has_pending());
    assert!(!output_guard.restart_required());
    recovery.front_attempts = 0;
    assert!(
        recovery.service_next().expect("emit first signer request"),
        "a missing body queues its first exact request"
    );
    let retained_request_hash = recovery
        .outstanding
        .as_ref()
        .expect("the first queued request is outstanding")
        .request_hash;
    let retained_responder = recovery
        .assembly_responder
        .as_ref()
        .expect("the first queued request pins one responder")
        .clone();
    for _ in 0..attempt_limit.saturating_add(1) {
        let retained = recovery
            .drain_effects(1)
            .pop()
            .expect("source-retained request remains queued");
        assert!(recovery.requeue_effect(retained));
        assert!(
            !recovery
                .service_next()
                .expect("local backpressure does not consume a retry"),
            "an undispatched request must not mint a retry"
        );
        assert_eq!(recovery.effect_count(), 1);
        assert_eq!(recovery.front_attempts, 1);
        assert_eq!(recovery.whole_wire_restarts, 0);
    }
    let outstanding = recovery
        .outstanding
        .as_ref()
        .expect("source retention preserves the outstanding request");
    assert_eq!(outstanding.request_hash, retained_request_hash);
    let pinned_responder = recovery
        .assembly_responder
        .as_ref()
        .expect("source retention preserves the pinned responder");
    assert_eq!(pinned_responder.peer, retained_responder.peer);
    assert_eq!(pinned_responder.index, retained_responder.index);
    assert_eq!(pinned_responder.count, retained_responder.count);
    let first = recovery
        .drain_effects(1)
        .pop()
        .expect("first CommitQC signer request");
    recovery
        .service_next()
        .expect("retry the pinned signer request");
    let second = recovery
        .drain_effects(1)
        .pop()
        .expect("second CommitQC signer request");
    let (
        V2LaneWorkEffect::PostLaneBlock {
            peer: first_peer,
            message: first_message,
        },
        V2LaneWorkEffect::PostLaneBlock {
            peer: second_peer,
            message: second_message,
        },
    ) = (first, second)
    else {
        panic!("recovery retries must use lane transport");
    };
    assert_eq!(
        first_peer, second_peer,
        "the first retry remains pinned to one exact remote QC signer"
    );
    assert_eq!(
        first_message.encode(),
        second_message.encode(),
        "a pinned-signer retry preserves exact request bytes"
    );
    assert_eq!(
        first_message.encode(),
        BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone())).encode()
    );
    let admit = |response: LaneHistoricalRecoveryResponseV1, sender: PeerId| {
        fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::LaneHistoricalRecoveryResponse(Box::new(response)),
            sender,
        ))
    };
    let outsider = PeerId::new(
        KeyPair::try_from_seed(vec![0xE2; 32], Algorithm::BlsNormal)
            .expect("derive recovery outsider")
            .public_key()
            .clone(),
    );
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(response.clone(), outsider))
            .expect("reject non-QC response without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut wrong_request_hash = response.clone();
    wrong_request_hash.request_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong canonical executed-block request"));
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(wrong_request_hash, responder.clone()))
            .expect("reject wrong request hash without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut wrong_finality = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
        finality_artifact, ..
    } = &mut wrong_finality.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    finality_artifact.block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong canonical finality block"));
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(wrong_finality, responder.clone()))
            .expect("reject finality drift without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut reordered = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk { chunk_index, .. } =
        &mut reordered.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    *chunk_index = 1;
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(reordered, responder.clone()))
            .expect("reject reordered chunk without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut wrong_len = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk { wire_len, .. } =
        &mut wrong_len.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    *wire_len = wire_len.saturating_add(1);
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(wrong_len, responder.clone()))
            .expect("reject wire-length drift without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut wrong_count = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk { chunk_count, .. } =
        &mut wrong_count.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    *chunk_count = chunk_count.saturating_add(1);
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(wrong_count, responder.clone()))
            .expect("reject chunk-count drift without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut oversized = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk { wire_len, .. } =
        &mut oversized.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    *wire_len = STRICT_INIT_MAX_BLOCK_BYTES.saturating_add(1);
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(oversized, responder.clone()))
            .expect("reject oversized wire without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut oversized_chunk = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk { bytes, .. } =
        &mut oversized_chunk.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    bytes.push(0);
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(oversized_chunk, responder.clone()))
            .expect("reject oversized chunk without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let mut wrong_body = response.clone();
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk { bytes, .. } =
        &mut wrong_body.payload
    else {
        panic!("fixture response is a canonical chunk");
    };
    bytes[0] ^= 1;
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(wrong_body, responder.clone()))
            .expect("reject body-hash drift without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(recovery.has_pending());
    assert!(!output_guard.restart_required());
    assert!(kura.get_block_without_merge_sidecar(height).is_none());
    recovery
        .service_next()
        .expect("restart at chunk zero after a poisoned complete assembly");
    let retry = recovery
        .drain_effects(1)
        .pop()
        .expect("emit a fresh exact request after body-hash drift");
    let V2LaneWorkEffect::PostLaneBlock {
        peer: retry_peer,
        message,
    } = retry
    else {
        panic!("restarted recovery must use lane transport");
    };
    assert_ne!(
        retry_peer, responder,
        "a malformed pinned-signer response advances to the next exact signer"
    );
    assert_eq!(
        message.encode(),
        BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone())).encode(),
        "a poisoned assembly restarts from the exact first chunk"
    );
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(response.clone(), retry_peer.clone()))
            .expect("accept exact canonical executed block"),
        V2LaneIngressOutcome::Inserted
    );
    assert!(!recovery.has_pending());
    assert!(
        !recovery
            .service_next()
            .expect("completed recovery has no successor request"),
        "completed recovery cannot consume a retry deadline"
    );
    assert!(!output_guard.restart_required());
    let cached = kura
        .get_block_without_merge_sidecar(height)
        .expect("exact canonical body is restored to the Kura cache");
    assert_eq!(cached.hash(), canonical_block.hash());
    assert_eq!(
        cached
            .encode_wire()
            .expect("encode restored canonical body"),
        canonical_block
            .encode_wire()
            .expect("encode original canonical body")
    );
    assert_eq!(
        kura.v2_finality_artifact(canonical_block.header().height().get())
            .expect("read finality after body cache"),
        Some(finality),
        "body recovery must not rewrite finality"
    );
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(admit(response, retry_peer))
            .expect("reject duplicate after recovery without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    let effect_capacity = recovery.limits.effect_capacity.get();
    for _ in 0..effect_capacity {
        recovery.effects.push_back(V2LaneWorkEffect::PostLaneBlock {
            peer: request.requester.clone(),
            message: BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone())),
        });
    }
    let mut invalid_saturated_request = request.clone();
    invalid_saturated_request.version = 0;
    let invalid_sender = invalid_saturated_request.requester.clone();
    let invalid_saturated_request =
        fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::LaneHistoricalRecoveryRequest(Box::new(invalid_saturated_request)),
            invalid_sender,
        ));
    assert_eq!(
        recovery
            .accept_with_ingress_ownership(invalid_saturated_request)
            .expect("a saturated response queue rejects work before rebuilding a body"),
        V2LaneIngressOutcome::Duplicate
    );
    assert_eq!(recovery.effect_count(), effect_capacity);
    recovery.effects.clear();
    let guarded_request_sender = request.requester.clone();
    let guarded_request =
        fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request)),
            guarded_request_sender,
        ));
    output_guard.close_admission_for_restart();
    let effects_before = recovery.effect_count();
    assert!(matches!(
        recovery.accept_with_ingress_ownership(guarded_request),
        Err(V2LaneWorkError::RestartRequired)
    ));
    assert_eq!(
        recovery.effect_count(),
        effects_before,
        "a closed fail-stop guard cannot enqueue canonical recovery output"
    );
}
