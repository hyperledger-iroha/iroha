// Exact request custody across successor admission handoff.
#[cfg(feature = "connect")]
struct QueuePlanHandoffFixture {
    app: SharedAppState,
    request: ToriiProxyRequestV1,
    owner: iroha_core::sumeragi::SumeragiIngressTestHarness,
    signers: Vec<KeyPair>,
    _directory: tempfile::TempDir,
    journal: std::path::PathBuf,
    before: Vec<u8>,
}
#[cfg(feature = "connect")]
impl QueuePlanHandoffFixture {
    fn new() -> Self {
        let signers = (0_u8..4)
            .map(|offset| {
                checked_torii_test_keypair_from_seed_byte(
                    0xb7_u8.wrapping_add(offset),
                    Algorithm::BlsNormal,
                    "handoff admission authority",
                )
            })
            .collect::<Vec<_>>();
        let (mut app, request) = incoming_proxy_submit_fixture_with_validator_signers(
            0xb7,
            ToriiProxyTransactionAdmissionV1::QueuePlanSynced,
            &signers,
        );
        let owner = queue_plan_capacity_harness_for_test(
            *app.state.network_id_ref(),
            iroha_data_model::block::consensus_v2::recommended_data_availability_layout(),
            &signers,
        );
        owner.set_admission_ready(false);
        let app_mut = Arc::get_mut(&mut app).unwrap();
        app_mut.sumeragi = Some(owner.handle());
        app_mut.torii_proxy_memory_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        let directory = tempfile::tempdir().unwrap();
        let journal = directory.path().join("admission.norito");
        app.queue
            .install_plan_journal(&journal, 1024 * 1024, true)
            .unwrap();
        let before = std::fs::read(&journal).unwrap();
        Self {
            app,
            request,
            owner,
            signers,
            _directory: directory,
            journal,
            before,
        }
    }
    fn assert_unclaimed(&self) {
        assert_eq!(self.app.queue.active_len(), 0);
        assert_eq!(std::fs::read(&self.journal).unwrap(), self.before);
    }
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn incoming_queue_plan_handoff_waits_without_claim_then_attests_exact_request() {
    let fixture = QueuePlanHandoffFixture::new();
    let expected = super::queue_plan_synced_acceptance_expectation(&fixture.request)
        .unwrap()
        .unwrap();
    let mut request = Box::pin(
        super::execute_incoming_torii_proxy_request_with_proxy_memory(
            &fixture.app,
            fixture.request.clone(),
            None,
            Some(super::acquire_torii_proxy_memory(&fixture.app).unwrap()),
        ),
    );
    assert!(futures_util::poll!(&mut request).is_pending());
    fixture.assert_unclaimed();
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        0
    );
    fixture.owner.set_admission_ready(true);
    let response = tokio::time::timeout(Duration::from_secs(5), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let snapshot = super::response_to_torii_proxy_snapshot(response, 1024 * 1024).await;
    super::validate_queue_plan_synced_acceptance(&snapshot, &expected).unwrap();
    assert_eq!(fixture.app.queue.active_len(), 1);
    assert_ne!(std::fs::read(&fixture.journal).unwrap(), fixture.before);
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn ingress_queue_plan_handoff_cancellation_releases_memory_without_claim() {
    let fixture = QueuePlanHandoffFixture::new();
    let mut request = Box::pin(super::execute_torii_proxy_request_with_fallback_admitted(
        &fixture.app,
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        fixture.request.request.clone(),
        None,
    ));
    assert!(futures_util::poll!(&mut request).is_pending());
    fixture.assert_unclaimed();
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        0
    );
    drop(request);
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        1
    );
    fixture.owner.set_admission_ready(true);
    tokio::time::sleep(Duration::from_millis(50)).await;
    fixture.assert_unclaimed();
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn forwarded_queue_plan_handoff_preflight_preserves_request_and_deadline() {
    let fixture = QueuePlanHandoffFixture::new();
    let local_peer = fixture.app.local_peer_id.as_ref().unwrap();
    let forwarded = super::forwarded_torii_proxy_request_owned(fixture.request.clone(), local_peer);
    let before = norito::to_bytes(&forwarded).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(60);
    let response = super::queue_plan_request_service_capacity_error(
        &fixture.app,
        &forwarded.request,
        deadline,
        forwarded.deadline_unix_ms,
    )
    .await
    .unwrap();
    assert!(super::is_queue_plan_outcome_unknown_response(&response));
    assert_queue_plan_handoff_uncertainty(&response, &fixture.request);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let error: ErrorEnvelope = norito::decode_from_bytes(&body).unwrap();
    assert_eq!(
        error.code(),
        super::QUEUE_PLAN_OUTCOME_UNKNOWN_ENVELOPE_CODE
    );
    assert_eq!(norito::to_bytes(&forwarded).unwrap(), before);
    fixture.assert_unclaimed();
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn incoming_queue_plan_handoff_expiry_never_creates_journal_claim() {
    let mut fixture = QueuePlanHandoffFixture::new();
    fixture.request.deadline_unix_ms = super::torii_proxy_now_unix_ms().unwrap() + 1_100;
    let mut request = Box::pin(
        super::execute_incoming_torii_proxy_request_with_proxy_memory(
            &fixture.app,
            fixture.request.clone(),
            None,
            Some(super::acquire_torii_proxy_memory(&fixture.app).unwrap()),
        ),
    );
    assert!(futures_util::poll!(&mut request).is_pending());
    fixture.assert_unclaimed();
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        0
    );
    let response = request.await;
    assert!(super::is_queue_plan_outcome_unknown_response(&response));
    drop(response);
    fixture.assert_unclaimed();
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        1
    );
}

#[cfg(feature = "connect")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_plan_handoff_after_quorum_retains_certificate_and_times_out_indeterminate() {
    let fixture = QueuePlanHandoffFixture::new();
    let expected = super::queue_plan_synced_acceptance_expectation(&fixture.request)
        .unwrap()
        .unwrap();
    assert_eq!(
        expected.durability_threshold, 2,
        "four authorities require f+1 receipts"
    );
    let receipts = fixture
        .signers
        .iter()
        .take(expected.durability_threshold)
        .map(|signer| exact_queue_plan_synced_test_receipt(&fixture.request, signer, 73))
        .collect();
    let snapshot = queue_plan_synced_test_certificate_snapshot(&fixture.request, receipts);
    let input = queue_plan_synced_test_complete_input(&fixture.request, &snapshot.body);
    let input_hash = Hash::new(&input);
    let deadline = super::queue_plan_publication_wait::PersistenceDeadline::new(
        Instant::now(),
        super::torii_proxy_now_unix_ms().unwrap() + 100,
    );
    let memory = super::acquire_torii_proxy_memory(&fixture.app).unwrap();
    let mut future = Box::pin(super::proxy_response_finalization::complete(
        super::torii_proxy_snapshot_to_response(snapshot),
        memory,
        |response| async {
            super::persist_queue_plan_admission_certificate(
                &fixture.app,
                response,
                &expected.admission_binding,
                queue_plan_synced_test_entrypoint(&fixture.request),
                deadline,
            )
            .await
        },
    ));
    assert!(futures_util::poll!(&mut future).is_pending());
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        0
    );
    assert_eq!(
        fixture
            .app
            .kura
            .pending_queue_plan_admission_certificate(input_hash)
            .unwrap(),
        None
    );
    let response = future.await;
    assert!(super::is_queue_plan_outcome_unknown_response(&response));
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        0
    );
    assert_eq!(
        fixture
            .app
            .kura
            .pending_queue_plan_admission_certificate(input_hash)
            .unwrap(),
        None
    );
    drop(response);
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        1
    );
    fixture.assert_unclaimed();
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn incoming_queue_plan_handoff_partial_journal_retry_preserves_uncertainty() {
    let mut fixture = QueuePlanHandoffFixture::new();
    fixture.owner.set_admission_ready(true);
    let first =
        super::execute_incoming_torii_proxy_request(&fixture.app, fixture.request.clone(), None)
            .await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    drop(first);
    let claimed = std::fs::read(&fixture.journal).unwrap();
    fixture.owner.set_admission_ready(false);
    fixture.request.deadline_unix_ms = super::torii_proxy_now_unix_ms().unwrap() + 1_100;
    let mut retry = Box::pin(super::execute_incoming_torii_proxy_request(
        &fixture.app,
        fixture.request.clone(),
        None,
    ));
    assert!(futures_util::poll!(&mut retry).is_pending());
    let response = retry.await;
    assert!(super::is_queue_plan_outcome_unknown_response(&response));
    assert_eq!(fixture.app.queue.active_len(), 1);
    assert_eq!(std::fs::read(&fixture.journal).unwrap(), claimed);
}

#[cfg(feature = "connect")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_plan_handoff_after_quorum_resumes_exact_certificate_publication() {
    let fixture = QueuePlanHandoffFixture::new();
    let expected = super::queue_plan_synced_acceptance_expectation(&fixture.request)
        .unwrap()
        .unwrap();
    assert_eq!(
        expected.durability_threshold, 2,
        "four authorities require f+1 receipts"
    );
    let receipts = fixture
        .signers
        .iter()
        .take(expected.durability_threshold)
        .map(|signer| exact_queue_plan_synced_test_receipt(&fixture.request, signer, 73))
        .collect();
    let snapshot = queue_plan_synced_test_certificate_snapshot(&fixture.request, receipts);
    let input = queue_plan_synced_test_complete_input(&fixture.request, &snapshot.body);
    let input_hash = Hash::new(&input);
    let original_certificate = snapshot.body.clone();
    let deadline = super::queue_plan_publication_wait::PersistenceDeadline::new(
        Instant::now(),
        fixture.request.deadline_unix_ms,
    );
    let memory = super::acquire_torii_proxy_memory(&fixture.app).unwrap();
    let mut future = Box::pin(super::proxy_response_finalization::complete(
        super::torii_proxy_snapshot_to_response(snapshot),
        memory,
        |response| async {
            super::persist_queue_plan_admission_certificate(
                &fixture.app,
                response,
                &expected.admission_binding,
                queue_plan_synced_test_entrypoint(&fixture.request),
                deadline,
            )
            .await
        },
    ));
    assert!(futures_util::poll!(&mut future).is_pending());
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        0
    );
    assert_eq!(
        fixture
            .app
            .kura
            .pending_queue_plan_admission_certificate(input_hash)
            .unwrap(),
        None
    );
    fixture.owner.set_admission_ready(true);
    let response = tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        fixture
            .app
            .kura
            .pending_queue_plan_admission_certificate(input_hash)
            .unwrap(),
        Some(input)
    );
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), original_certificate.as_slice());
    assert_eq!(
        fixture.app.torii_proxy_memory_inflight.available_permits(),
        1
    );
}

#[cfg(feature = "connect")]
fn assert_queue_plan_handoff_uncertainty(response: &Response, request: &ToriiProxyRequestV1) {
    let transaction = queue_plan_synced_test_entrypoint(request);
    assert!(super::is_queue_plan_outcome_unknown_response(response));
    assert_eq!(
        response.headers()["x-iroha-entrypoint-hash"],
        transaction.hash().to_string()
    );
    assert_eq!(
        response.headers()["x-iroha-signed-transaction-hash"],
        super::signed_transaction_hash_for_entrypoint(transaction)
            .unwrap()
            .to_string()
    );
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn queue_plan_handoff_expiry_before_aggregation_preserves_partial_journal_uncertainty() {
    let fixture = QueuePlanHandoffFixture::new();
    fixture.owner.set_admission_ready(true);
    let first =
        super::execute_incoming_torii_proxy_request(&fixture.app, fixture.request.clone(), None)
            .await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    drop(first);
    let claimed = std::fs::read(&fixture.journal).unwrap();
    let dispatched = std::cell::Cell::new(0);
    let completed = std::cell::Cell::new(0);
    let expired_start = tokio::time::Instant::now()
        - super::TORII_PROXY_EXECUTION_BUDGET
        - Duration::from_millis(1);
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let response = super::execute_torii_proxy_request_across_candidates(
        expired_start,
        vec![ToriiProxyCandidate::Local(
            fixture.app.local_peer_id.as_ref().unwrap().clone(),
        )],
        route,
        fixture.request.clone(),
        TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1,
        Duration::from_millis(50),
        |_, _| {
            dispatched.set(dispatched.get() + 1);
            std::future::ready(Err(ToriiProxyAttemptError::DefinitelyNotDispatched(
                "expired request must not dispatch".to_owned(),
            )))
        },
        |_| {
            completed.set(completed.get() + 1);
            std::future::ready(())
        },
    )
    .await;
    assert_queue_plan_handoff_uncertainty(&response, &fixture.request);
    assert_eq!(dispatched.get(), 0);
    assert_eq!(completed.get(), 0);
    assert_eq!(fixture.app.queue.active_len(), 1);
    assert_eq!(std::fs::read(&fixture.journal).unwrap(), claimed);
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn incoming_queue_plan_expired_retry_preserves_partial_journal_uncertainty() {
    let fixture = QueuePlanHandoffFixture::new();
    fixture.owner.set_admission_ready(true);
    let first =
        super::execute_incoming_torii_proxy_request(&fixture.app, fixture.request.clone(), None)
            .await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    drop(first);
    let claimed = std::fs::read(&fixture.journal).unwrap();
    let now = super::torii_proxy_now_unix_ms().unwrap();
    for deadline in [now - 1, now + 500, u64::MAX] {
        let mut retry = fixture.request.clone();
        retry.deadline_unix_ms = deadline;
        let response = super::execute_incoming_torii_proxy_request_with_proxy_memory(
            &fixture.app,
            retry.clone(),
            None,
            Some(super::acquire_torii_proxy_memory(&fixture.app).unwrap()),
        )
        .await;
        assert_queue_plan_handoff_uncertainty(&response, &retry);
        assert_eq!(
            fixture.app.torii_proxy_memory_inflight.available_permits(),
            1
        );
        assert_eq!(fixture.app.queue.active_len(), 1);
        assert_eq!(std::fs::read(&fixture.journal).unwrap(), claimed);

        retry.request = ToriiProxyRequestKindV1::Read(super::torii_read_request(
            ToriiReadEndpointV1::AccountGet,
            ToriiFanoutRouteScopeV1::AllDataspaces,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![ALICE_ID.to_string()],
            None,
            Vec::new(),
        ));
        let generic = super::execute_incoming_torii_proxy_request(&fixture.app, retry, None).await;
        assert_eq!(generic.status(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(
            torii_response_header(&generic, "x-iroha-reject-code"),
            Some("proxy_deadline_exceeded")
        );
    }
}
