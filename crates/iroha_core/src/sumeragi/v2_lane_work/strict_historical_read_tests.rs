// Regressions exercise the production ownership boundary and actual durable files.
fn corrupt_durable_file_for_test(path: &std::path::Path) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .expect("open the existing durable fixture");
    file.sync_all().expect("make injected corruption durable");
}
#[test]
fn owned_lane_ingress_without_opaque_ownership_requires_restart() {
    let (mut adapter, _, request) = self_contained_historical_recovery_request_fixture();
    let sender = request.requester.clone();
    let inbound = InboundBlockMessage::from_authenticated_peer(
        BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request)),
        sender,
    );
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(inbound, 0),
        Err(V2LaneWorkError::RestartRequired)
    );
    assert!(adapter.output_guard.restart_required());
    assert!(adapter.effects.is_empty());
}
#[test]
fn historical_request_detects_durable_body_corruption_with_warm_cache() {
    let (mut adapter, _, request) = self_contained_historical_recovery_request_fixture();
    let sender = request.requester.clone();
    adapter
        .validate_historical_recovery_request(&request, &sender)
        .expect("source initially authenticates the exact request");
    let height = NonZeroUsize::new(1).expect("historical fixture height");
    assert!(
        adapter.kura.get_block(height).is_some(),
        "warm the body cache"
    );
    corrupt_durable_file_for_test(&adapter.kura.store_root().join("blocks.data"));
    let inbound = fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request)),
        sender,
    ));
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(inbound, 0),
        Err(V2LaneWorkError::RestartRequired)
    );
    assert!(adapter.output_guard.restart_required());
    assert!(
        adapter.effects.is_empty(),
        "corrupt storage emits no response"
    );
}
#[test]
fn historical_response_corrupt_local_finality_retains_exact_retry_owner() {
    let (mut adapter, _, request) = self_contained_historical_recovery_request_fixture();
    let certificate = request
        .certificate
        .as_ref()
        .expect("lane request certificate");
    let session = CommittedLaneBlockSession {
        proposal: certificate.proposal.clone(),
        prepare_qc: certificate.prepare_qc.clone(),
        commit_qc: certificate.commit_qc.clone(),
    };
    let identity = HistoricalRecoveryIdentity::from_proposal(&session.proposal)
        .expect("exact historical identity");
    let observation = adapter.historical_recovery_diagnostics.observe(
        identity,
        HistoricalRecoveryWaitReason::CanonicalBlockPending,
    );
    adapter
        .schedule_historical_recovery_request(identity, &session, observation, Instant::now(), &[])
        .expect("schedule an authenticated canonical recovery owner");
    let outstanding = adapter
        .historical_recovery_requests
        .get(&identity)
        .expect("production scheduler retains the request");
    let request_hash = outstanding.request_hash;
    let sender = outstanding
        .canonical_body_destinations
        .iter()
        .next()
        .expect("production scheduler authorizes a responder")
        .clone();
    let block = adapter
        .kura
        .read_block_body(NonZeroUsize::new(1).expect("height"))
        .expect("read durable carrier")
        .expect("carrier exists");
    let finality_artifact = adapter
        .kura
        .v2_finality_artifact(1)
        .expect("read durable finality")
        .expect("finality exists");
    let response = LaneHistoricalRecoveryResponseV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
        request_hash,
        payload: LaneHistoricalRecoveryPayloadV1::CanonicalBlock {
            block: block.as_ref().clone(),
            finality_artifact,
        },
    };
    corrupt_durable_file_for_test(&adapter.kura.v2_finality_artifact_path_for_testing(1));
    let inbound = fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::LaneHistoricalRecoveryResponse(Box::new(response)),
        sender,
    ));
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(inbound, 0),
        Err(V2LaneWorkError::RestartRequired)
    );
    assert!(adapter.output_guard.restart_required());
    assert_eq!(
        adapter
            .historical_recovery_request_owners
            .get(&request_hash),
        Some(&identity)
    );
    assert_eq!(
        adapter
            .historical_recovery_requests
            .get(&identity)
            .expect("retain exact retry cadence and request")
            .request_hash,
        request_hash
    );
    assert!(
        adapter
            .retired_historical_recovery_request_hashes
            .is_empty(),
        "a storage failure cannot acknowledge or refund a consumed recovery owner"
    );
}
#[test]
fn canonical_chunk_recovery_corrupt_body_requires_restart_and_retains_need() {
    let (adapter, _, block, finality) = canonical_executed_block_recovery_fixture();
    let need = canonical_executed_block_need(&block, &finality);
    let request = LaneHistoricalRecoveryRequestV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
        requester: adapter.local_peer.clone(),
        certificate: None,
        signer_pops: BTreeMap::new(),
        kind: LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock {
            need: Box::new(need),
            chunk_index: 0,
        },
    };
    build_canonical_executed_block_response(
        &adapter.context,
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        adapter.limits,
        &request,
        &request.requester,
    )
    .expect("source initially serves an exact canonical chunk");
    let mut recovery = CanonicalExecutedBlockRecovery::new(
        adapter.context.clone(),
        adapter.local_peer.clone(),
        Arc::clone(&adapter.state),
        Arc::clone(&adapter.kura),
        Arc::clone(&adapter.output_guard),
        adapter.limits,
        vec![need],
    )
    .expect("install one retained canonical repair need");
    corrupt_durable_file_for_test(&adapter.kura.store_root().join("blocks.data"));
    let sender = request.requester.clone();
    let inbound = fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request)),
        sender,
    ));
    assert!(matches!(
        recovery.accept_with_ingress_ownership(inbound),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    assert_eq!(recovery.needs.front(), Some(&need));
    assert!(recovery.effects.is_empty());
}
