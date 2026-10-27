// Fee quotes exercise the real signature, memory admission, publication, and fee paths.
fn fee_quote_admission_router(app: SharedAppState) -> axum::Router {
    axum::Router::new()
        .route("/v1/fees/quote", axum::routing::post(handler_fee_quote))
        .layer(DefaultBodyLimit::max(app.transaction_max_content_len))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            enforce_fee_quote_body_admission,
        ))
        .with_state(app)
}

fn fee_quote_http_request(headers: &HeaderMap, body: &Bytes) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/fees/quote")
        .body(Body::from(body.clone()))
        .unwrap();
    *request.headers_mut() = headers.clone();
    request.headers_mut().insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from(body.len()),
    );
    request
}

fn fee_quote_admission_request(
    app: &SharedAppState,
    key: &KeyPair,
    self_register: bool,
) -> (
    HeaderMap,
    Bytes,
    iroha_data_model::transaction::TransactionPayload,
) {
    let account = AccountId::new(key.public_key().clone());
    let builder = TransactionBuilder::new(
        *app.state.network_id_ref(),
        account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    let payload = if self_register {
        builder.with_instructions([iroha_data_model::isi::Register::account(Account::new(
            account.clone(),
        ))])
    } else {
        builder.with_instructions([iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "queued fee quote".to_owned(),
        )])
    }
    .into_payload()
    .expect("unsigned fee quote payload");
    let body = norito::json::to_vec(&FeeQuoteRequest {
        payload: payload.clone(),
    })
    .expect("fee quote JSON");
    let headers = crate::tests_runtime_handlers::signed_network_app_headers(
        app.state.network_id_ref(),
        &account,
        key,
        &Method::POST,
        &"/v1/fees/quote".parse().unwrap(),
        &body,
    );
    (headers, Bytes::from(body), payload)
}

fn assert_fee_quote_permission_refusal(error: Error, expected: &str) {
    assert!(
        matches!(&error, Error::Query(ValidationFail::NotPermitted(message))
        if message == expected),
        "unexpected permission refusal: {error:?}"
    );
    assert_eq!(error.into_response().status(), StatusCode::FORBIDDEN);
}

fn fee_quote_hold_publication(
    state: Arc<iroha_core::state::State>,
) -> (std::sync::mpsc::SyncSender<()>, std::thread::JoinHandle<()>) {
    let (held_tx, held_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let holder = std::thread::spawn(move || {
        state.with_held_header_for_reader_test(|_| {
            held_tx.send(()).expect("publication acquired");
            let _ = release_rx.recv();
        })
    });
    held_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("original publication held");
    (release_tx, holder)
}

#[tokio::test(start_paused = true)]
async fn fee_quote_admission_does_not_poll_unowned_bodies_and_refunds_on_cancellation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt as _;
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app).unwrap().query_queue_timeout = Duration::from_millis(100);
    let router = fee_quote_admission_router(app.clone());
    let before = app.app_routed_read_body_inflight.available_bytes();
    let waiters_before = app.app_routed_read_waiters.available_permits();
    let occupied = app
        .app_routed_read_body_inflight
        .try_acquire_parts([before])
        .unwrap();
    let polls = Arc::new(AtomicUsize::new(0));
    let counted = polls.clone();
    let body = Body::from_stream(futures::stream::poll_fn(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        std::task::Poll::<Option<Result<Bytes, std::io::Error>>>::Pending
    }));
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/fees/quote")
        .header(axum::http::header::CONTENT_LENGTH, "100")
        .body(body)
        .unwrap();
    let mut waiting = Box::pin(router.clone().oneshot(request));
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    assert_eq!(
        polls.load(Ordering::SeqCst),
        0,
        "body stays unread until its own memory is admitted"
    );
    drop(occupied);
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    assert!(polls.load(Ordering::SeqCst) > 0);
    assert_eq!(
        app.app_routed_read_body_inflight.available_bytes(),
        before - 100
    );
    drop(waiting);
    assert_eq!(app.app_routed_read_body_inflight.available_bytes(), before);
    assert_eq!(
        app.app_routed_read_waiters.available_permits(),
        waiters_before
    );

    // Oversized declared input is rejected before the stream is polled, rather than
    // bypassing the real byte budget while waiting behind other fee quotes.
    let polls = Arc::new(AtomicUsize::new(0));
    let counted = polls.clone();
    let body = Body::from_stream(futures::stream::poll_fn(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        std::task::Poll::<Option<Result<Bytes, std::io::Error>>>::Pending
    }));
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/fees/quote")
        .header(axum::http::header::CONTENT_LENGTH, before + 1)
        .body(body)
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert_eq!(app.app_routed_read_body_inflight.available_bytes(), before);
}

#[tokio::test]
async fn fee_quote_admission_serves_100_signed_callers_after_pool_and_publication_release() {
    use tower::ServiceExt as _;
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let keys: Vec<_> = (1_u8..=100)
        .map(|seed| checked_torii_test_ed25519_keypair(seed, "fee quote cohort signer"))
        .collect();
    let accounts: Vec<_> = keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect();
    let world = World::with(
        [],
        accounts
            .iter()
            .map(|account| Account::new(account.clone()).build(account)),
        [],
    );
    let mut app = mk_app_state_for_tests_with_world(world);
    let state = Arc::get_mut(&mut app).expect("unshared cohort app");
    // Actual deployed query pool geometry: 3/4 of 2,048 MB, with a 192 MB ceiling.
    state.query_fanout_inflight = ByteWeightedMemoryPool::new(1_536_000_000).unwrap();
    state.query_fanout_working_set_bytes = 192_000_000;
    state.query_queue_timeout = Duration::from_secs(30);
    let before = app.query_fanout_inflight.available_bytes();
    let body_before = app.app_routed_read_body_inflight.available_bytes();
    let waiters_before = app.app_routed_read_waiters.available_permits();
    let occupied = app
        .query_fanout_inflight
        .try_acquire_parts([before])
        .unwrap();
    let requests: Vec<_> = keys
        .iter()
        .map(|key| fee_quote_admission_request(&app, key, false))
        .collect();
    let (release, holder) = fee_quote_hold_publication(app.state.clone());
    let router = fee_quote_admission_router(app.clone());
    let started = tokio::time::Instant::now();
    let mut waiting: Vec<_> = requests
        .iter()
        .map(|(headers, body, _)| {
            Box::pin(
                router
                    .clone()
                    .oneshot(fee_quote_http_request(headers, body)),
            )
        })
        .collect();
    for request in &mut waiting {
        // Manual polling shares one Tokio task; renew its cooperative budget as real HTTP tasks do.
        tokio::task::yield_now().await;
        assert!(
            futures::poll!(request.as_mut()).is_pending(),
            "all 100 must queue, not return 429"
        );
    }
    assert_eq!(
        app.app_routed_read_waiters.available_permits(),
        waiters_before - 100
    );
    assert_eq!(
        body_before - app.app_routed_read_body_inflight.available_bytes(),
        requests
            .iter()
            .map(|(_, body, _)| body.len() as u64)
            .sum::<u64>(),
        "every exact queued body retains its own byte credit before execution admission"
    );
    drop(occupied);
    for request in &mut waiting {
        // Manual polling shares one Tokio task; renew its cooperative budget as real HTTP tasks do.
        tokio::task::yield_now().await;
        assert!(
            futures::poll!(request.as_mut()).is_pending(),
            "original publication remains held"
        );
    }
    assert_eq!(
        app.app_routed_read_waiters.available_permits(),
        waiters_before
    );
    let smallest =
        authentication_owner_working_set(&app, Some(CANONICAL_AUTHENTICATION_MIN_UNIT_BYTES));
    let weight = routed_read_owner_weight(&app, smallest).unwrap().0 as u64;
    assert_eq!(
        before - app.query_fanout_inflight.available_bytes(),
        100 * weight,
        "100 independent authentication owners fit; no full 192 MB owner or global workflow permit"
    );
    drop(release);
    for (request, (_, _, payload)) in waiting.into_iter().zip(&requests) {
        let response = request.await.expect("signed fee quote after release");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let quote: FeeQuoteResponse = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(quote.intent, payload.fee_payment);
        assert_eq!(quote.observation.route_dataspace_id, DataSpaceId::UNIVERSAL);
        assert!(matches!(quote.decision, FeeQuoteDecision::Accepted {
            debit_source: iroha_data_model::nexus::FeeDebitSource::Account(ref account),
            program_revision: None,
        } if account == &payload.authority));
    }
    holder.join().expect("publication released");
    eprintln!(
        "fee quote cohort: 100 distinct signed HTTP requests, {} authentication bytes, elapsed {:?}",
        100 * weight,
        started.elapsed()
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cohort must finish inside the shipped client deadline"
    );
    assert_eq!(app.query_fanout_inflight.available_bytes(), before);
    assert_eq!(
        app.app_routed_read_body_inflight.available_bytes(),
        body_before
    );
    assert_eq!(
        app.app_routed_read_waiters.available_permits(),
        waiters_before
    );
    let (headers, body, _) = &requests[0];
    let replay = handler_fee_quote(
        State(app),
        Method::POST,
        "/v1/fees/quote".parse().unwrap(),
        headers.clone(),
        body.clone(),
    )
    .await
    .expect_err("successful quote consumes its request nonce");
    assert_fee_quote_permission_refusal(replay, "request nonce already used");
}

#[tokio::test(start_paused = true)]
async fn fee_quote_admission_keeps_one_deadline_and_preserves_nonce_after_timeout() {
    use tower::ServiceExt as _;
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key = checked_torii_test_ed25519_keypair(0xc1, "fee quote deadline signer");
    let account = AccountId::new(key.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&account));
    Arc::get_mut(&mut app).unwrap().query_queue_timeout = Duration::from_millis(100);
    let (headers, body, _) = fee_quote_admission_request(&app, &key, false);
    let before = app.query_fanout_inflight.available_bytes();
    let body_before = app.app_routed_read_body_inflight.available_bytes();
    let waiters_before = app.app_routed_read_waiters.available_permits();
    let body_occupied = app
        .app_routed_read_body_inflight
        .try_acquire_parts([body_before])
        .unwrap();
    let occupied = app
        .query_fanout_inflight
        .try_acquire_parts([before])
        .unwrap();
    let (release, holder) = fee_quote_hold_publication(app.state.clone());
    let router = fee_quote_admission_router(app.clone());
    let mut waiting = Box::pin(router.oneshot(fee_quote_http_request(&headers, &body)));
    let started = tokio::time::Instant::now();
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    tokio::time::advance(Duration::from_millis(30)).await;
    drop(body_occupied);
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    tokio::time::advance(Duration::from_millis(30)).await;
    drop(occupied);
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    tokio::time::advance(Duration::from_millis(39)).await;
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    tokio::time::advance(Duration::from_millis(1)).await;
    let error = waiting
        .await
        .expect("same absolute deadline expires during publication");
    assert_eq!(started.elapsed(), Duration::from_millis(100));
    assert_eq!(
        error.into_response().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(app.query_fanout_inflight.available_bytes(), before);
    assert_eq!(
        app.app_routed_read_body_inflight.available_bytes(),
        body_before
    );
    assert_eq!(
        app.app_routed_read_waiters.available_permits(),
        waiters_before
    );
    drop(release);
    holder.join().expect("publication released");
    let accepted = handler_fee_quote(
        State(app.clone()),
        Method::POST,
        "/v1/fees/quote".parse().unwrap(),
        headers.clone(),
        body.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        accepted.status(),
        StatusCode::OK,
        "timeout did not consume the nonce"
    );
    let replay = handler_fee_quote(
        State(app),
        Method::POST,
        "/v1/fees/quote".parse().unwrap(),
        headers,
        body,
    )
    .await
    .expect_err("successful quote remains replay protected");
    assert_fee_quote_permission_refusal(replay, "request nonce already used");
}

#[tokio::test]
async fn fee_quote_admission_waits_for_self_registration_without_weakening_signature_or_replay() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key = checked_torii_test_ed25519_keypair(0xc2, "fee quote self-register wait signer");
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app).unwrap().query_queue_timeout = Duration::from_secs(5);
    let (headers, body, _) = fee_quote_admission_request(&app, &key, true);
    let (release, holder) = fee_quote_hold_publication(app.state.clone());
    let mut waiting = Box::pin(handler_fee_quote(
        State(app.clone()),
        Method::POST,
        "/v1/fees/quote".parse().unwrap(),
        headers.clone(),
        body.clone(),
    ));
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    drop(release);
    assert_eq!(waiting.await.unwrap().status(), StatusCode::OK);
    holder.join().expect("publication released");
    let replay = handler_fee_quote(
        State(app.clone()),
        Method::POST,
        "/v1/fees/quote".parse().unwrap(),
        headers,
        body,
    )
    .await
    .unwrap_err();
    assert_fee_quote_permission_refusal(replay, "request nonce already used");
    let (headers, body, _) = fee_quote_admission_request(&app, &key, true);
    let mut altered = body.to_vec();
    altered.push(b' ');
    let invalid = handler_fee_quote(
        State(app.clone()),
        Method::POST,
        "/v1/fees/quote".parse().unwrap(),
        headers.clone(),
        Bytes::from(altered),
    )
    .await
    .unwrap_err();
    assert_fee_quote_permission_refusal(invalid, "query signature failed verification");
    assert_eq!(
        handler_fee_quote(
            State(app),
            Method::POST,
            "/v1/fees/quote".parse().unwrap(),
            headers,
            body
        )
        .await
        .unwrap()
        .status(),
        StatusCode::OK,
        "invalid signature did not consume a valid exact request"
    );
}

#[tokio::test]
async fn fee_quote_admission_grows_an_insufficient_owner_before_consuming_multisig_nonce() {
    use iroha_data_model::{
        account::{MultisigMember, MultisigPolicy},
        soracloud::{
            CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
            CanonicalRequestWitnessV1,
        },
    };
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let signers: Vec<_> = (1_u8..=48)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::MlDsa).expect("ML-DSA witness signer")
        })
        .collect();
    let policy = MultisigPolicy::new(
        48,
        signers
            .iter()
            .map(|key| MultisigMember::new(key.public_key().clone(), 1).unwrap())
            .collect(),
    )
    .unwrap();
    let account = AccountId::new_multisig(policy);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let payload = TransactionBuilder::new(
        *app.state.network_id_ref(),
        account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([iroha_data_model::isi::Log::new(
        iroha_data_model::Level::INFO,
        "multisig quote".to_owned(),
    )])
    .into_payload()
    .unwrap();
    let body = norito::json::to_vec(&FeeQuoteRequest { payload }).unwrap();
    let method = Method::POST;
    let uri: axum::http::Uri = "/v1/fees/quote".parse().unwrap();
    let mut witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: account.clone(),
        timestamp_ms: crate::utils::unix_now_ms(),
        nonce: "fee-quote-large-witness-owner-retry".to_owned(),
        canonical_request_hash: crate::app_auth::canonical_network_request_hash(
            app.state.network_id_ref(),
            &method,
            &uri,
            &body,
        )
        .unwrap(),
        signatures: Vec::new(),
    };
    let message = crate::app_auth::canonical_request_witness_message(&witness).unwrap();
    witness.signatures = signers
        .iter()
        .map(|key| CanonicalRequestSignatureWitnessV1 {
            signer: key.public_key().clone(),
            signature: IrohaSignature::try_new(key.private_key(), &message).unwrap(),
        })
        .collect();
    let mut headers = HeaderMap::new();
    headers.insert(
        crate::app_auth::HEADER_WITNESS,
        HeaderValue::from_str(&crate::app_auth::witness_header_value(&witness).unwrap()).unwrap(),
    );
    let before = app.query_fanout_inflight.available_bytes();
    let initial = crate::history_producer::HistoryProducerOwner::authentication_read_waiting(
        &app,
        Some(CANONICAL_AUTHENTICATION_MIN_UNIT_BYTES),
    )
    .await
    .unwrap();
    assert!(initial.0.working_set_bytes() < app.query_fanout_working_set_bytes);
    let (verified, owner) = authenticate_fee_quote_request(
        &app,
        &headers,
        crate::app_auth::CanonicalRequestParts {
            method: &method,
            uri: &uri,
            body: &body,
        },
        initial,
        tokio::time::Instant::now() + Duration::from_secs(10),
    )
    .await
    .expect("valid large witness grows its insufficient original owner");
    assert_eq!(verified.unwrap().account, account);
    assert_eq!(
        owner.working_set_bytes(),
        app.query_fanout_working_set_bytes
    );
    drop(owner);
    assert_eq!(app.query_fanout_inflight.available_bytes(), before);
    let replay = handler_fee_quote(State(app), method, uri, headers, Bytes::from(body))
        .await
        .expect_err("growth must not bypass replay protection");
    assert_fee_quote_permission_refusal(replay, "request nonce already used");
}
