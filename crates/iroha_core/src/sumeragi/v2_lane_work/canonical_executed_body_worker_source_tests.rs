// The worker is source-ready while production canonical ingress/output remains closed.
use crate::sumeragi::message::BlockMessageWire;

fn canonical_executed_body_worker_request(
    adapter: &V2LaneWorkAdapter,
    need: CanonicalExecutedBlockNeedV1,
) -> LaneHistoricalRecoveryRequestV1 {
    LaneHistoricalRecoveryRequestV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
        requester: adapter.local_peer.clone(),
        certificate: None,
        signer_pops: BTreeMap::new(),
        kind: LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock {
            need: Box::new(need),
            chunk_index: 0,
        },
    }
}

fn canonical_worker_inbound(
    adapter: &V2LaneWorkAdapter,
    request: LaneHistoricalRecoveryRequestV1,
    sender: PeerId,
) -> InboundBlockMessage {
    use iroha_p2p::network::NetworkReplyRouteTestFixture;

    let mut routes =
        NetworkReplyRouteTestFixture::with_source_capacity(adapter.local_peer.clone(), 1);
    let inbound = InboundBlockMessage::try_from_transport_with_reply_route(
        BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone())),
        sender.clone(),
        sender.clone(),
        routes.mint_via(sender.clone(), sender.clone()),
    )
    .expect("construct authenticated canonical return route");
    fair_v2_ingress_admit_for_test(inbound)
}

fn bound_canonical_worker_task(
    adapter: &V2LaneWorkAdapter,
    request: LaneHistoricalRecoveryRequestV1,
    sender: PeerId,
    limits: V2LaneWorkLimits,
) -> Result<
    crate::sumeragi::v2_block_sync::CanonicalExecutedBodyServeTask,
    CanonicalRecoveryReadError,
> {
    crate::sumeragi::v2_block_sync::CanonicalExecutedBodyServeTask::from_authenticated_inbound(
        canonical_worker_inbound(adapter, request, sender),
        adapter.context.clone(),
        Arc::clone(&adapter.state),
        limits,
    )
    .map_err(|(_, error)| error)
}

fn recv_canonical_worker_completion(
    server: &mut crate::sumeragi::v2_block_sync::V2BlockSyncServer,
) -> crate::sumeragi::v2_block_sync::CanonicalExecutedBodyServeCompletion {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(completion) = server
            .try_recv_canonical_executed_body_completion()
            .expect("worker keeps its completion channel")
        {
            return completion;
        }
        assert!(
            Instant::now() < deadline,
            "canonical worker made no bounded progress"
        );
        thread::yield_now();
    }
}

#[test]
fn canonical_executed_body_worker_prepares_source_without_local_repair_need() {
    use crate::sumeragi::v2_block_sync::{
        CanonicalExecutedBodyServeAdmission, CanonicalExecutedBodyServeCompletion,
        HistoricalBodyServeLimits, V2BlockSyncServer,
    };

    let (adapter, keys, block, finality) = canonical_executed_block_recovery_fixture();
    let need = canonical_executed_block_need(&block, &finality);
    let request = canonical_executed_body_worker_request(&adapter, need);
    let responder_key = keys[0].clone();
    let responder = PeerId::new(responder_key.public_key().clone());
    let mut server = V2BlockSyncServer::new_with_historical_body_service(
        adapter.context.network_id,
        1,
        Arc::clone(&adapter.kura),
        responder_key,
        HistoricalBodyServeLimits::first_release(1, 1).expect("bounded worker limits"),
    )
    .expect("install chain-scoped worker without a local recovery owner");
    let task = bound_canonical_worker_task(
        &adapter,
        request.clone(),
        adapter.local_peer.clone(),
        adapter.limits,
    )
    .expect("bind authenticated canonical request");
    assert!(matches!(
        server.try_enqueue_canonical_executed_body(task),
        CanonicalExecutedBodyServeAdmission::Queued
    ));
    let CanonicalExecutedBodyServeCompletion::Prepared(prepared) =
        recv_canonical_worker_completion(&mut server)
    else {
        panic!("committed State and Kura must produce one exact chunk")
    };
    assert!(prepared.task.ingress_ownership().validate_exact());
    assert!(prepared.proof.covers_message(
        &adapter.context.network_id,
        &responder,
        &prepared.message
    ));
    assert_eq!(prepared.proof.source_height(), need.height);
    let crate::NetworkMessage::SumeragiBlock(envelope) = &prepared.message else {
        panic!("canonical output must be block traffic")
    };
    let BlockMessage::LaneHistoricalRecoveryResponse(response) = envelope.as_message() else {
        panic!("worker prepares only canonical response traffic")
    };
    assert_eq!(response.request_hash, HashOf::new(&request));
    let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
        finality_artifact,
        wire_len,
        chunk_index,
        bytes,
        ..
    } = &response.payload
    else {
        panic!("worker prepares the requested canonical chunk")
    };
    assert_eq!(finality_artifact, &finality);
    assert_eq!(*wire_len, need.executed_block_wire_len);
    assert_eq!(*chunk_index, 0);
    assert!(!bytes.is_empty());
    assert!(!server.has_pending_historical_body_serve());

    let mut substituted = response.as_ref().clone();
    substituted.request_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"substituted canonical request"));
    let substituted = crate::NetworkMessage::SumeragiBlock(Arc::new(
        BlockMessageWire::try_preencoded(Arc::new(BlockMessage::LaneHistoricalRecoveryResponse(
            Box::new(substituted),
        )))
        .expect("encode substituted response"),
    ));
    let _ = substituted.exact_output_hash();
    assert!(
        !prepared
            .proof
            .covers_message(&adapter.context.network_id, &responder, &substituted)
    );
}

#[test]
fn canonical_executed_body_worker_rejects_wrong_request_source_and_frame_limit() {
    use crate::sumeragi::v2_block_sync::{
        CanonicalExecutedBodyServeAdmission, CanonicalExecutedBodyServeCompletion,
        HistoricalBodyServeAdmission, HistoricalBodyServeLimits, V2BlockSyncServer,
    };

    let (adapter, keys, block, finality) = canonical_executed_block_recovery_fixture();
    let need = canonical_executed_block_need(&block, &finality);
    let request = canonical_executed_body_worker_request(&adapter, need);
    let outsider = PeerId::new(
        KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::BlsNormal)
            .expect("deterministic outsider")
            .public_key()
            .clone(),
    );
    assert!(matches!(
        bound_canonical_worker_task(&adapter, request.clone(), outsider.clone(), adapter.limits),
        Err(CanonicalRecoveryReadError::Rejected(_))
    ));

    let mut uninstalled = V2BlockSyncServer::new(adapter.context.network_id, 1)
        .expect("construct server without a worker");
    let wrong_sender = canonical_worker_inbound(&adapter, request.clone(), outsider);
    let Err((returned, CanonicalRecoveryReadError::Rejected(_))) = uninstalled
        .try_enqueue_canonical_executed_body_ingress(
            wrong_sender,
            adapter.context.clone(),
            Arc::clone(&adapter.state),
            adapter.limits,
        )
    else {
        panic!("wrong semantic sender must return the unchanged fair-ingress carrier")
    };
    assert_eq!(returned.sender(), returned.via());
    assert!(
        returned
            .ingress_ownership()
            .is_some_and(|ownership| ownership.validate_exact())
    );
    assert!(
        returned
            .ingress_ownership()
            .is_some_and(|ownership| ownership.matches_message(returned.message()))
    );
    let mut wrong_kind = request.clone();
    wrong_kind.kind = LaneHistoricalRecoveryKindV1::CanonicalBlock {
        finality_artifact_hash: HashOf::new(&finality),
    };
    let old_request =
        canonical_worker_inbound(&adapter, wrong_kind.clone(), adapter.local_peer.clone());
    let Err((returned, CanonicalRecoveryReadError::Rejected(_))) = uninstalled
        .try_enqueue_canonical_executed_body_ingress(
            old_request,
            adapter.context.clone(),
            Arc::clone(&adapter.state),
            adapter.limits,
        )
    else {
        panic!("the broader old lane-recovery kind must not enter the canonical worker")
    };
    assert!(
        returned
            .ingress_ownership()
            .is_some_and(|ownership| ownership.matches_message(returned.message()))
    );
    assert!(matches!(
        returned.message(),
        BlockMessage::LaneHistoricalRecoveryRequest(old) if old.as_ref() == &wrong_kind
    ));
    let detached_inbound =
        canonical_worker_inbound(&adapter, request.clone(), adapter.local_peer.clone());
    let CanonicalExecutedBodyServeAdmission::Failed {
        task: returned,
        error: _,
    } = uninstalled
        .try_enqueue_canonical_executed_body_ingress(
            detached_inbound,
            adapter.context.clone(),
            Arc::clone(&adapter.state),
            adapter.limits,
        )
        .unwrap_or_else(|(_, error)| panic!("valid ingress lost binding: {error}"))
    else {
        panic!("missing worker must return its original request owner")
    };
    assert_eq!(HashOf::new(&returned.request), HashOf::new(&request));
    assert!(returned.ingress_ownership().validate_exact());

    let mut server = V2BlockSyncServer::new_with_historical_body_service(
        adapter.context.network_id,
        1,
        Arc::clone(&adapter.kura),
        keys[0].clone(),
        HistoricalBodyServeLimits::first_release(1, 1).expect("bounded worker limits"),
    )
    .expect("install shared historical worker");
    let mut wrong_need = need;
    wrong_need.finality_artifact_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign finality"));
    let wrong_request = canonical_executed_body_worker_request(&adapter, wrong_need);
    let wrong_task = bound_canonical_worker_task(
        &adapter,
        wrong_request,
        adapter.local_peer.clone(),
        adapter.limits,
    )
    .expect("wire shape remains valid before source validation");
    assert!(matches!(
        server.try_enqueue_canonical_executed_body(wrong_task),
        CanonicalExecutedBodyServeAdmission::Queued
    ));
    assert!(matches!(
        recv_canonical_worker_completion(&mut server),
        CanonicalExecutedBodyServeCompletion::Failed(_, CanonicalRecoveryReadError::Rejected(_))
    ));

    let mut limited_server = V2BlockSyncServer::new_with_historical_body_service(
        adapter.context.network_id,
        1,
        Arc::clone(&adapter.kura),
        keys[0].clone(),
        HistoricalBodyServeLimits::first_release(1, 1).expect("bounded frame test limits"),
    )
    .expect("install separate frame-limit worker");
    let mut limited = adapter.limits;
    limited.historical_recovery_response_frame_capacity =
        NonZeroUsize::new(1).expect("one-byte negative frame bound");
    let limited_task =
        bound_canonical_worker_task(&adapter, request, adapter.local_peer.clone(), limited)
            .expect("request shape still fits its separate request bound");
    assert!(matches!(
        limited_server.try_enqueue_canonical_executed_body(limited_task),
        CanonicalExecutedBodyServeAdmission::Queued
    ));
    assert!(matches!(
        recv_canonical_worker_completion(&mut limited_server),
        CanonicalExecutedBodyServeCompletion::Failed(_, CanonicalRecoveryReadError::Rejected(_))
    ));

    let pressure_request = canonical_executed_body_worker_request(&adapter, need);
    let first = canonical_worker_inbound(
        &adapter,
        pressure_request.clone(),
        adapter.local_peer.clone(),
    );
    let request_hash = HashOf::new(&pressure_request);
    let second = canonical_worker_inbound(&adapter, pressure_request, adapter.local_peer.clone());
    let mut pressure_server = V2BlockSyncServer::new_with_historical_body_service(
        adapter.context.network_id,
        1,
        Arc::clone(&adapter.kura),
        keys[0].clone(),
        HistoricalBodyServeLimits::first_release(1, 1).expect("bounded pressure limits"),
    )
    .expect("install pressure worker");
    assert!(matches!(
        pressure_server
            .try_enqueue_canonical_executed_body_ingress(
                first,
                adapter.context.clone(),
                Arc::clone(&adapter.state),
                adapter.limits,
            )
            .unwrap_or_else(|(_, error)| panic!("first ingress lost binding: {error}")),
        CanonicalExecutedBodyServeAdmission::Queued
    ));
    // The second original owner is not admitted while the first response
    // consumes the fixed frame budget; its requester remains free to retry.
    let mut retained = match pressure_server
        .try_enqueue_canonical_executed_body_ingress(
            second,
            adapter.context.clone(),
            Arc::clone(&adapter.state),
            adapter.limits,
        )
        .unwrap_or_else(|(_, error)| panic!("second ingress lost binding: {error}"))
    {
        CanonicalExecutedBodyServeAdmission::Refused { task, reason }
            if matches!(
                reason,
                HistoricalBodyServeAdmission::RateLimited | HistoricalBodyServeAdmission::Busy
            ) =>
        {
            task
        }
        _ => panic!("pressure must return the unchanged canonical request owner"),
    };
    assert_eq!(HashOf::new(&retained.request), request_hash);
    assert!(retained.ingress_ownership().validate_exact());
    assert!(retained.ingress_ownership().matches_message(
        &BlockMessage::LaneHistoricalRecoveryRequest(Box::new(retained.request.clone()))
    ));
    assert!(
        retained
            .ingress_ownership()
            .matches_reply_routes(Some(&retained.reply_routes))
    );
    assert!(matches!(
        recv_canonical_worker_completion(&mut pressure_server),
        CanonicalExecutedBodyServeCompletion::Prepared(_)
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match pressure_server.try_enqueue_canonical_executed_body(retained) {
            CanonicalExecutedBodyServeAdmission::Queued => break,
            CanonicalExecutedBodyServeAdmission::Refused { task, reason }
                if matches!(
                    reason,
                    HistoricalBodyServeAdmission::RateLimited | HistoricalBodyServeAdmission::Busy
                ) =>
            {
                retained = task;
                assert_eq!(HashOf::new(&retained.request), request_hash);
                assert!(retained.ingress_ownership().validate_exact());
                assert!(Instant::now() < deadline, "unchanged owner could not retry");
                thread::sleep(Duration::from_millis(10));
            }
            CanonicalExecutedBodyServeAdmission::Failed { error, .. } => {
                panic!("canonical retry lost worker service: {error}")
            }
            CanonicalExecutedBodyServeAdmission::Refused { .. } => {
                panic!("canonical retry returned an invalid refusal reason")
            }
        }
    }
    assert!(matches!(
        recv_canonical_worker_completion(&mut pressure_server),
        CanonicalExecutedBodyServeCompletion::Prepared(_)
    ));
}
