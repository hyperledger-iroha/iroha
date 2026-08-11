#[tokio::test]
async fn handler_post_transaction_entrypoint_uses_authenticated_api_token_rate_limit_key() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.high_load_tx_threshold = usize::MAX;
        app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        app_mut.fee_policy = FeePolicy::Disabled;
        app_mut.require_api_token = true;
        app_mut.api_tokens_set = Arc::new(HashSet::from(["entrypoint-token".to_owned()]));
    }

    let first_keypair = checked_torii_test_keypair_from_seed_byte(
        0xc7,
        Algorithm::Ed25519,
        "derive first entrypoint API-token fixture key",
    );
    let second_keypair = checked_torii_test_keypair_from_seed_byte(
        0xc8,
        Algorithm::Ed25519,
        "derive second entrypoint API-token fixture key",
    );
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        AccountId::new(first_keypair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "entrypoint-token-rate-limit-1".to_string(),
    )])
    .sign(first_keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        AccountId::new(second_keypair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "entrypoint-token-rate-limit-2".to_string(),
    )])
    .sign(second_keypair.private_key());
    let mut headers = HeaderMap::new();
    headers.insert("x-api-token", HeaderValue::from_static("entrypoint-token"));

    let first = super::handler_post_transaction_entrypoint(
        State(app.clone()),
        headers.clone(),
        None,
        versioned_entrypoint_for_test(TransactionEntrypoint::External(tx1)),
    )
    .await
    .expect("first token-keyed entrypoint accepted")
    .into_response();
    assert_eq!(first.status(), StatusCode::ACCEPTED);

    let err = match super::handler_post_transaction_entrypoint(
        State(app),
        headers,
        None,
        versioned_entrypoint_for_test(TransactionEntrypoint::External(tx2)),
    )
    .await
    {
        Ok(_) => panic!("expected shared token rate limit"),
        Err(err) => err,
    };
    assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn handler_post_transaction_entrypoint_reports_full_queue_before_rate_limit() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        app_mut.fee_policy = FeePolicy::Disabled;
    }
    install_single_slot_transaction_queue(&mut app);

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xcf,
        Algorithm::Ed25519,
        "derive entrypoint queue-before-rate-limit fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "entrypoint-queue-before-rate-1".to_string(),
    )])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "entrypoint-queue-before-rate-2".to_string(),
    )])
    .sign(keypair.private_key());
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-token",
        HeaderValue::from_static("entrypoint-queue-before-rate"),
    );

    let first = super::handler_post_transaction_entrypoint(
        State(app.clone()),
        headers.clone(),
        None,
        versioned_entrypoint_for_test(TransactionEntrypoint::External(tx1)),
    )
    .await
    .expect("first entrypoint should fill the queue")
    .into_response();
    assert_eq!(first.status(), StatusCode::ACCEPTED);

    let err = match super::handler_post_transaction_entrypoint(
        State(app.clone()),
        headers,
        None,
        versioned_entrypoint_for_test(TransactionEntrypoint::External(tx2)),
    )
    .await
    {
        Ok(_) => panic!("expected queue full before token rate limit"),
        Err(err) => err,
    };
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("PRTRY:QUEUE_FULL")
    );
}

#[tokio::test]
async fn handler_post_transaction_honors_prefer_return_minimal() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xc9,
        Algorithm::Ed25519,
        "derive minimal post-transaction response fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let transaction = TransactionBuilder::new(
        (*app.chain_id).clone(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "minimal-submit-response".to_string())])
    .sign(keypair.private_key());
    let submitted_hash = transaction.hash().to_string();
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("prefer"),
        HeaderValue::from_static("respond-async, return=minimal"),
    );

    let response = super::handler_post_transaction(
        State(app),
        headers,
        None,
        versioned_signed_for_test(&transaction),
    )
    .await
    .expect("accepted")
    .into_response();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("preference-applied")
            .and_then(|value| value.to_str().ok()),
        Some(PREFER_RETURN_MINIMAL)
    );
    assert_eq!(
        response
            .headers()
            .get("x-iroha-transaction-hash")
            .and_then(|value| value.to_str().ok()),
        Some(submitted_hash.as_str())
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    assert!(
        body.is_empty(),
        "minimal response should not sign a receipt body"
    );
}

fn transaction_batch_body_for_test(
    payloads: Vec<Vec<u8>>,
) -> crate::utils::extractors::NoritoBytes {
    crate::utils::extractors::NoritoBytes(Bytes::from(
        norito::to_bytes(&payloads).expect("encode transaction batch envelope"),
    ))
}

#[test]
fn transaction_ingress_compute_corridor_enforces_configured_parallelism() {
    let limiter = Arc::new(tokio::sync::Semaphore::new(2));
    let first = try_acquire_transaction_ingress_compute(&limiter).expect("first permit");
    let second = try_acquire_transaction_ingress_compute(&limiter).expect("second permit");
    let error = match try_acquire_transaction_ingress_compute(&limiter) {
        Ok(_) => panic!("configured parallelism must reject excess physical work"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::AppServiceUnavailable {
            code: "transaction_ingress_compute_saturated",
            ..
        }
    ));

    drop(first);
    let replacement = try_acquire_transaction_ingress_compute(&limiter).expect("released capacity");
    drop((second, replacement));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_transaction_ingress_worker_retains_physical_capacity() {
    let limiter = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = try_acquire_transaction_ingress_compute(&limiter).expect("compute permit");
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let request = tokio::spawn(run_transaction_ingress_compute_job(
        permit,
        "fixture_worker_failed",
        move || {
            started_tx.send(()).expect("announce physical start");
            release_rx.recv().expect("release physical worker");
            Ok(())
        },
    ));
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("physical worker started");

    request.abort();
    let _ = request.await;
    let retained_after_cancellation = try_acquire_transaction_ingress_compute(&limiter).is_err();

    release_tx.send(()).expect("release physical worker");
    let replacement = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(permit) = try_acquire_transaction_ingress_compute(&limiter) {
                break permit;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("physical completion releases capacity");
    drop(replacement);
    assert!(
        retained_after_cancellation,
        "HTTP cancellation must not free capacity while physical work continues"
    );
}

#[test]
fn transaction_batch_body_limit_rejects_before_decode() {
    let body = transaction_batch_body_for_test(vec![vec![0_u8; 16]]).0;
    let error = validate_transaction_batch_body_size(&body, body.len() - 1)
        .expect_err("wire bytes over the configured limit must be rejected");
    assert!(matches!(
        error,
        Error::AppQueryValidation {
            code: "transaction_batch_payload_too_large",
            ..
        }
    ));
}

#[test]
fn transaction_batch_declared_decoded_limit_rejects_compressed_expansion() {
    let payloads = vec![vec![0_u8; 64 * 1024]];
    let mut encoded = Vec::new();
    norito::serialize_into(&mut encoded, &payloads, norito::Compression::Zstd)
        .expect("encode compressed transaction batch envelope");
    let body = Bytes::from(encoded);
    let header = norito::core::Header::read(std::io::Cursor::new(body.as_ref()))
        .expect("read compressed envelope header");
    assert!(
        header.length > u64::try_from(body.len()).unwrap_or(u64::MAX),
        "fixture must expand beyond its encoded length"
    );

    let error = validate_transaction_batch_body_size(&body, body.len())
        .expect_err("declared decoded bytes over the limit must be rejected");
    assert!(matches!(
        error,
        Error::AppQueryValidation {
            code: "transaction_batch_payload_too_large",
            ..
        }
    ));
}

#[tokio::test]
async fn transaction_batch_count_limit_rejects_before_transaction_decode() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .transaction_batch_max_transactions = 1;
    let body = transaction_batch_body_for_test(vec![vec![0xff], vec![0xff]]);

    let error =
        match super::handler_post_transactions_batch(State(app.clone()), HeaderMap::new(), body)
            .await
        {
            Ok(_) => panic!("oversized batch must be rejected"),
            Err(error) => error,
        };
    assert!(matches!(
        error,
        Error::AppQueryValidation {
            code: "transaction_batch_too_large",
            ..
        }
    ));
    assert_eq!(
        app.queue.active_len(),
        0,
        "count rejection must not push a valid prefix"
    );
}

#[tokio::test]
async fn transaction_batch_queue_capacity_rejects_before_transaction_decode() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;
    install_single_slot_transaction_queue(&mut app);
    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xaf,
        Algorithm::Ed25519,
        "derive batch queue-capacity fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let transaction = TransactionBuilder::new(
        (*app.chain_id).clone(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "batch-queue-before-decode".to_owned(),
    )])
    .sign(keypair.private_key());
    let accepted = super::handler_post_transactions_batch(
        State(app.clone()),
        HeaderMap::new(),
        transaction_batch_body_for_test(vec![
            <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(
                &transaction,
            ),
        ]),
    )
    .await
    .expect("fixture transaction should fill the queue");
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 1);

    let error = match super::handler_post_transactions_batch(
        State(app.clone()),
        HeaderMap::new(),
        transaction_batch_body_for_test(vec![vec![0xff]]),
    )
    .await
    {
        Ok(_) => panic!("full queue must reject before invalid transaction payload decode"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::PushIntoQueue { .. }));
    assert_eq!(
        app.queue.active_len(),
        1,
        "queue rejection must leave the existing transaction untouched"
    );
}

#[tokio::test]
async fn handler_post_transactions_batch_accepts_multiple_payloads() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xca,
        Algorithm::Ed25519,
        "derive post-transaction batch submit fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "batch-submit-1".to_string())])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "batch-submit-2".to_string())])
    .sign(keypair.private_key());
    let payloads = vec![
        iroha_version::codec::EncodeVersioned::encode_versioned(&tx1),
        iroha_version::codec::EncodeVersioned::encode_versioned(&tx2),
    ];

    let response = super::handler_post_transactions_batch(
        State(app.clone()),
        HeaderMap::new(),
        transaction_batch_body_for_test(payloads),
    )
    .await
    .expect("accepted");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-transactions-accepted")
            .and_then(|value| value.to_str().ok()),
        Some("2")
    );
    assert_eq!(app.queue.active_len(), 2);
}

#[tokio::test]
async fn handler_post_transactions_batch_rate_limits_api_token_as_single_key_batch() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.high_load_tx_threshold = usize::MAX;
        app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(2));
        app_mut.require_api_token = true;
        app_mut.api_tokens_set = Arc::new(HashSet::from(["batch-token".to_owned()]));
    }

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xcb,
        Algorithm::Ed25519,
        "derive post-transaction batch token fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let payloads = (0..3)
        .map(|index| {
            let tx = TransactionBuilder::new(
                chain.clone(),
                authority.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("batch-token-rate-limit-{index}"),
            )])
            .sign(keypair.private_key());
            iroha_version::codec::EncodeVersioned::encode_versioned(&tx)
        })
        .collect();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::HeaderName::from_static("x-api-token"),
        HeaderValue::from_static("batch-token"),
    );

    let err = match super::handler_post_transactions_batch(
        State(app.clone()),
        headers,
        transaction_batch_body_for_test(payloads),
    )
    .await
    {
        Ok(_) => panic!("expected token rate limit"),
        Err(err) => err,
    };

    assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(app.queue.active_len(), 0);
    assert!(
        !app.tx_rate_limiter.allow("batch-token").await,
        "failed same-key batch should consume the token prefix that would have passed"
    );
}

#[tokio::test]
async fn handler_post_transactions_batch_uses_authenticated_token_for_distinct_authorities() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.high_load_tx_threshold = usize::MAX;
        app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(2));
        app_mut.require_api_token = true;
        app_mut.api_tokens_set = Arc::new(HashSet::from(["batch-distinct-token".to_owned()]));
    }

    let chain = (*app.chain_id).clone();
    let payloads = (0..3)
        .map(|index| {
            let keypair = checked_torii_test_keypair_from_seed_byte(
                0xcc_u8.wrapping_add(index as u8),
                Algorithm::Ed25519,
                "derive distinct-authority batch token fixture key",
            );
            let authority = AccountId::new(keypair.public_key().clone());
            let tx = TransactionBuilder::new(
                chain.clone(),
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("batch-token-distinct-authority-{index}"),
            )])
            .sign(keypair.private_key());
            iroha_version::codec::EncodeVersioned::encode_versioned(&tx)
        })
        .collect();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::HeaderName::from_static("x-api-token"),
        HeaderValue::from_static("batch-distinct-token"),
    );

    let err = match super::handler_post_transactions_batch(
        State(app.clone()),
        headers,
        transaction_batch_body_for_test(payloads),
    )
    .await
    {
        Ok(_) => panic!("expected token rate limit"),
        Err(err) => err,
    };

    assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(app.queue.active_len(), 0);
    assert!(
        !app.tx_rate_limiter.allow("batch-distinct-token").await,
        "distinct authorities should still consume the shared API-token key"
    );
}

#[tokio::test]
async fn handler_post_transactions_batch_rejects_invalid_ed25519_precheck_without_partial_push() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xd0,
        Algorithm::Ed25519,
        "derive invalid precheck batch fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "batch-valid-before-invalid".to_string(),
    )])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "batch-invalid-signature".to_string())])
    .sign(keypair.private_key());
    let tx2 = transaction_with_invalid_signature_for_test(tx2);
    let payloads = vec![
        iroha_version::codec::EncodeVersioned::encode_versioned(&tx1),
        iroha_version::codec::EncodeVersioned::encode_versioned(&tx2),
    ];

    let err = match super::handler_post_transactions_batch(
        State(app.clone()),
        HeaderMap::new(),
        transaction_batch_body_for_test(payloads),
    )
    .await
    {
        Ok(_) => panic!("expected invalid signature rejection"),
        Err(err) => err,
    };
    let response = err.into_response();
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some(SignatureRejectionCode::InvalidSignature.as_str())
    );
    assert_eq!(app.queue.active_len(), 0);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn handler_post_transaction_rejects_unfunded_nexus_fee_tx_before_history() {
    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xd1,
        Algorithm::Ed25519,
        "derive unfunded fee fixture authority key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let fee_sink_keypair = checked_torii_test_keypair_from_seed_byte(
        0xd2,
        Algorithm::Ed25519,
        "derive unfunded fee fixture sink key",
    );
    let fee_sink = AccountId::new(fee_sink_keypair.public_key().clone());
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let fee_asset_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("asset name"),
    );
    let domain = Domain::new(domain_id).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let fee_sink_account = Account::new(fee_sink.clone()).build(&fee_sink);
    let fee_asset_definition =
        iroha_data_model::asset::AssetDefinition::numeric(fee_asset_id.clone(), "xor".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
            .build(&authority);
    let world = World::with(
        [domain],
        [account, fee_sink_account],
        [fee_asset_definition],
    );
    let mut app = mk_app_state_for_tests_with_world(world);
    configure_nexus_fee_admission_for_test(&mut app, &fee_asset_id, &fee_sink);

    let tx = TransactionBuilder::new(
        (*app.chain_id).clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "fee-insolvent".to_owned())])
    .sign(keypair.private_key());
    let tx_hash = tx.hash();
    let tx_hash_hex = tx_hash.to_string();

    let response = match super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx),
    )
    .await
    {
        Ok(_) => panic!("expected Nexus fee admission rejection"),
        Err(err) => err.into_response(),
    };

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("PRTRY:NEXUS_FEE_ADMISSION_REJECTED")
    );
    assert_eq!(app.queue.active_len(), 0);
    assert!(
        !app.state.has_committed_transaction(tx_hash),
        "ingress rejection should not create committed history"
    );

    let explorer = super::handler_explorer_transaction_detail(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(tx_hash_hex),
    )
    .await
    .expect("explorer detail response");
    assert_eq!(explorer.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn handler_policy_reports_tx_rate_limit_as_always_enforced() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.fee_policy = FeePolicy::Disabled;
        app_mut.high_load_tx_threshold = usize::MAX;
    }

    let response =
        super::handler_policy(State(app), HeaderMap::new(), crate::loopback_connect_info())
            .await
            .expect("policy response")
            .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("policy body");
    let json: norito::json::Value = norito::json::from_slice(&body).expect("valid policy json");
    assert_eq!(
        json.get("rate_limit_enforced"),
        Some(&norito::json::Value::Bool(true))
    );
}

#[tokio::test]
async fn handler_policy_reports_required_token_even_when_configuration_is_unavailable() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.require_api_token = true;
        app_mut.api_tokens_set = Arc::new(HashSet::new());
        app_mut.api_rate_limit_bypass_nets = Arc::new(vec![
            limits::parse_cidr("127.0.0.0/8").expect("loopback CIDR"),
        ]);
    }

    let response =
        super::handler_policy(State(app), HeaderMap::new(), crate::loopback_connect_info())
            .await
            .expect("allowlisted policy response")
            .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("policy body");
    let json: norito::json::Value = norito::json::from_slice(&body).expect("valid policy JSON");
    assert_eq!(
        json.get("require_api_token"),
        Some(&norito::json::Value::Bool(true))
    );
    assert_eq!(
        json.get("token_required"),
        Some(&norito::json::Value::Bool(true))
    );
}

#[tokio::test]
async fn handler_post_transaction_high_load_threshold_does_not_reject_before_enqueue() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = 1;

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xd3,
        Algorithm::Ed25519,
        "derive high-load threshold fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "early-shed-1".to_string())])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "early-shed-2".to_string())])
    .sign(keypair.private_key());

    let first = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx1),
    )
    .await
    .expect("first transaction should be accepted")
    .into_response();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 1);

    let second = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx2),
    )
    .await
    .expect("second transaction should not be rejected")
    .into_response();
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 2);
}

#[tokio::test]
async fn handler_post_transaction_allows_enqueue_when_queue_age_saturates() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xd4,
        Algorithm::Ed25519,
        "derive queue-age saturation fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "age-shed-1".to_string())])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "age-shed-2".to_string())])
    .sign(keypair.private_key());

    let first = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx1),
    )
    .await
    .expect("first transaction should be accepted")
    .into_response();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 1);

    let snapshot = app
        .queue
        .backdate_queued_transactions_for_tests(Duration::from_secs(3));
    assert!(
        snapshot.saturated_by_age,
        "test setup should make queue age saturation observable"
    );

    let pressure = app.queue.pressure_snapshot();
    assert!(
        pressure.saturated_by_age,
        "age saturation should still be observable in queue pressure telemetry"
    );
    assert!(
        !app.queue.current_backpressure().is_saturated(),
        "age-only queue pressure must not reject ingress before capacity"
    );

    let second = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx2),
    )
    .await
    .expect("second transaction should not be age-shed")
    .into_response();
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 2);
}

#[tokio::test]
async fn handler_post_transaction_returns_queue_full_only_for_real_capacity_overflow() {
    let mut app = mk_app_state_for_tests();
    install_single_slot_transaction_queue(&mut app);

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xd5,
        Algorithm::Ed25519,
        "derive queue capacity fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "queue-full-1".to_string())])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "queue-full-2".to_string())])
    .sign(keypair.private_key());

    let first = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx1),
    )
    .await
    .expect("first transaction should be accepted")
    .into_response();
    assert_eq!(first.status(), StatusCode::ACCEPTED);

    let err = match super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx2),
    )
    .await
    {
        Ok(_) => panic!("expected real queue overflow"),
        Err(err) => err,
    };
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("PRTRY:QUEUE_FULL")
    );
}

#[tokio::test]
async fn handler_post_transaction_does_not_early_shed_when_only_inflight_tx_is_old() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;

    let keypair = checked_torii_test_keypair_from_seed_byte(
        0xd6,
        Algorithm::Ed25519,
        "derive in-flight queue age fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let chain = (*app.chain_id).clone();
    let tx1 = TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "age-inflight-1".to_string())])
    .sign(keypair.private_key());
    let tx2 = TransactionBuilder::new(
        chain,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "age-inflight-2".to_string())])
    .sign(keypair.private_key());

    let _ = app
        .queue
        .refresh_pressure_budget_from_block_time(Duration::ZERO);

    let first = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx1),
    )
    .await
    .expect("first transaction should be accepted")
    .into_response();
    assert_eq!(first.status(), StatusCode::ACCEPTED);

    let mut guards = Vec::new();
    app.queue.get_transactions_for_block(
        &app.state.view(),
        NonZeroUsize::new(1).expect("nonzero tx count"),
        &mut guards,
    );
    assert_eq!(guards.len(), 1, "queue should expose one in-flight guard");
    assert_eq!(app.queue.queued_len(), 0, "no queued transactions remain");

    std::thread::sleep(Duration::from_millis(2_100));

    let second = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&tx2),
    )
    .await
    .expect("second transaction should not be age-shed")
    .into_response();
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    assert_eq!(
        app.queue.queued_len(),
        1,
        "second transaction should enqueue"
    );

    drop(guards);
}

#[test]
fn signed_query_scope_classifies_trigger_inventory_queries_as_local_replicated() {
    let key_pair =
        checked_torii_test_ed25519_keypair(0xd7, "derive trigger inventory query fixture key");
    let authority = AccountId::new(key_pair.public_key().clone());

    let find_triggers = signed_find_triggers_query_for_test(authority.clone(), &key_pair);
    assert_eq!(
        super::signed_query_scope(&find_triggers.payload),
        super::SignedQueryScope::LocalReplicated
    );

    let find_active_ids = signed_find_active_trigger_ids_query_for_test(authority, &key_pair);
    assert_eq!(
        super::signed_query_scope(&find_active_ids.payload),
        super::SignedQueryScope::LocalReplicated
    );
}

#[test]
fn signed_query_scope_classifies_exact_peer_inventory_as_public_control_plane() {
    let authority =
        checked_torii_test_account_id(0xf6, "derive peer inventory authority fixture key");
    let request = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Start(build_find_peers_query_for_test()),
    );

    assert_eq!(
        super::signed_query_scope(&request),
        super::SignedQueryScope::PublicControlPlane
    );
}

#[test]
fn signed_query_scope_rejects_cross_kind_target_payload_collisions() {
    let authority = checked_torii_test_account_id(
        0xf7,
        "derive cross-kind query collision authority fixture key",
    );
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id,
        "asset-definition".parse().expect("asset definition name"),
    );
    let mut history_with_target_payload =
        build_find_accounts_with_asset_query_for_test(asset_definition_id);
    history_with_target_payload.item = iroha_data_model::query::QueryItemKind::CommittedTransaction;
    let mut global_roles_with_account_payload =
        build_find_roles_by_account_query_for_test(authority.clone());
    global_roles_with_account_payload.item = iroha_data_model::query::QueryItemKind::Role;

    for query in [
        history_with_target_payload,
        global_roles_with_account_payload,
    ] {
        assert_eq!(
            super::signed_query_scope(&request_for_test(
                &authority,
                iroha_data_model::query::QueryRequest::Start(query),
            )),
            super::SignedQueryScope::CrossDataspaceFanout,
            "a payload from a narrower query must not downscope a different global item kind",
        );
    }
}

#[test]
fn peer_inventory_scope_requires_exact_canonical_payload() {
    let authority = checked_torii_test_account_id(
        0xf8,
        "derive malformed peer inventory authority fixture key",
    );
    let mut trailing = build_find_peers_query_for_test();
    trailing.query_payload.push(0xa5);

    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(trailing),
        )),
        super::SignedQueryScope::CrossDataspaceFanout
    );
}

#[test]
fn signed_query_scope_uses_escrow_party_discriminants() {
    use iroha_data_model::{
        escrow::AssetEscrowStatus,
        query::{
            QueryItemKind, QueryWithParams,
            escrow::prelude::{
                FindAssetEscrowsByBuyer, FindAssetEscrowsBySeller, FindAssetEscrowsByStatus,
            },
            parameters::QueryParams,
        },
    };

    let authority =
        checked_torii_test_account_id(0xfa, "derive escrow query routing authority fixture key");
    let target = checked_torii_test_account_id(0xfb, "derive escrow query target fixture key");

    let seller_query = QueryWithParams {
        query: (),
        query_payload: norito::codec::Encode::encode(&FindAssetEscrowsBySeller {
            seller: target.clone(),
        }),
        item: QueryItemKind::AssetEscrowsBySeller,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(seller_query),
        )),
        super::SignedQueryScope::TargetAccount(target.clone())
    );

    let buyer_query = QueryWithParams {
        query: (),
        query_payload: norito::codec::Encode::encode(&FindAssetEscrowsByBuyer {
            buyer: target.clone(),
        }),
        item: QueryItemKind::AssetEscrowsByBuyer,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(buyer_query),
        )),
        super::SignedQueryScope::TargetAccount(target.clone())
    );

    let legacy_item_tag = QueryWithParams {
        query: (),
        query_payload: norito::codec::Encode::encode(&FindAssetEscrowsBySeller {
            seller: target.clone(),
        }),
        item: QueryItemKind::AssetEscrowRecord,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(legacy_item_tag),
        )),
        super::SignedQueryScope::CrossDataspaceFanout
    );

    let status_query = QueryWithParams {
        query: (),
        query_payload: norito::codec::Encode::encode(&FindAssetEscrowsByStatus {
            status: AssetEscrowStatus::PaymentSent,
        }),
        item: QueryItemKind::AssetEscrowsByStatus,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(status_query),
        )),
        super::SignedQueryScope::CrossDataspaceFanout
    );
}

#[tokio::test]
async fn public_peer_inventory_does_not_require_foreign_dataspace_read_grants() {
    let key_pair = checked_torii_test_ed25519_keypair(
        0xf9,
        "derive public peer inventory handler fixture key",
    );
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_private_ingress_with_offline_foreign_route_for_test(&mut app);
    assert!(
        super::torii_all_dataspace_routes(app.as_ref()).len() > 1,
        "test requires multiple restricted dataspace routes",
    );
    let signed = iroha_data_model::query::QueryRequest::Start(build_find_peers_query_for_test())
        .with_authority(authority)
        .sign(&key_pair);

    let response = super::handler_signed_query(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::NoritoQuery(QueryOptions::default()),
        versioned_query_for_test(signed),
    )
    .await
    .expect("canonical public peer inventory should execute locally")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-iroha-routed-by").is_none());
}

#[test]
fn account_id_inventory_does_not_collide_with_trigger_inventory_scope() {
    let authority =
        checked_torii_test_account_id(0xf0, "derive account inventory authority fixture key");
    let request = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Start(build_find_account_ids_query_for_test()),
    );

    assert_eq!(
        super::signed_query_scope(&request),
        super::SignedQueryScope::CrossDataspaceFanout
    );
}

#[test]
fn query_scope_payload_matching_rejects_malformed_and_trailing_bytes() {
    use iroha_data_model::query::transaction::prelude::FindTransactions;

    let mut payload = norito::to_bytes(&FindTransactions::new())
        .expect("encode canonical transactions query payload");
    assert!(super::payload_matches_query::<FindTransactions>(&payload));

    payload.push(0xa5);
    assert!(
        !super::payload_matches_query::<FindTransactions>(&payload),
        "a valid prefix with trailing bytes must not inherit history-query routing"
    );
    assert!(
        !super::payload_matches_query::<FindTransactions>(&[0xff, 0x00]),
        "malformed bytes must not inherit a privileged routing scope"
    );
}

#[test]
fn iterable_target_account_query_builders_capture_target_payload() {
    let account_id =
        checked_torii_test_account_id(0xd8, "derive iterable account target fixture key");

    let domains_query = build_find_domains_by_account_query_for_test(account_id.clone());
    assert_domains_query_targets_account(&domains_query, &account_id);

    let assets_query = build_find_assets_by_account_query_for_test(account_id.clone());
    assert_assets_query_targets_account(&assets_query, &account_id);

    let nfts_query = build_find_nfts_by_account_query_for_test(account_id.clone());
    assert_nfts_query_targets_account(&nfts_query, &account_id);

    let permissions_query = build_find_permissions_by_account_query_for_test(account_id.clone());
    assert_permissions_query_targets_account(&permissions_query, &account_id);

    let roles_query = build_find_roles_by_account_query_for_test(account_id.clone());
    assert_roles_query_targets_account(&roles_query, &account_id);
}

#[test]
fn iterable_target_domain_query_builders_capture_target_payload() {
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id,
        "asset-definition".parse().expect("asset definition name"),
    );

    let accounts_query = build_find_accounts_with_asset_query_for_test(asset_definition_id.clone());
    assert_accounts_with_asset_query_targets_domain(&accounts_query, &asset_definition_id);
}

#[test]
fn signed_query_scope_classifies_find_asset_by_id_as_target_account() {
    let account_id = checked_torii_test_account_id(0xd9, "derive asset-by-id account fixture key");
    let authority = checked_torii_test_account_id(0xda, "derive asset-by-id authority fixture key");
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );

    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAssetById(
                    iroha_data_model::query::asset::prelude::FindAssetById::new(
                        iroha_data_model::asset::AssetId::new(
                            asset_definition_id,
                            account_id.clone(),
                        ),
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id)
    );
}

#[test]
fn signed_query_scope_classifies_find_asset_definition_by_id_as_target_domain() {
    let authority =
        checked_torii_test_account_id(0xdb, "derive asset-definition scope authority key");
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );

    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAssetDefinitionById(
                    iroha_data_model::query::asset::prelude::FindAssetDefinitionById::new(
                        asset_definition_id,
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetDomain(domain_id)
    );
}

#[tokio::test]
async fn signed_query_scope_for_app_keeps_opaque_find_asset_by_id_targeted_to_account() {
    let account_id =
        checked_torii_test_account_id(0xdc, "derive opaque asset-by-id account fixture key");
    let authority =
        checked_torii_test_account_id(0xdd, "derive opaque asset-by-id authority fixture key");
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );
    let app = mk_app_state_for_tests();
    seed_asset_definition_for_test(&app, &asset_definition_id, Some(&domain_id));
    let request = roundtrip_request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Singular(
            iroha_data_model::query::SingularQueryBox::FindAssetById(
                iroha_data_model::query::asset::prelude::FindAssetById::new(
                    iroha_data_model::asset::AssetId::new(asset_definition_id, account_id.clone()),
                ),
            ),
        ),
    );

    assert_eq!(
        super::signed_query_scope(&request),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope_for_app(app.as_ref(), &request),
        super::SignedQueryScope::TargetAccount(account_id)
    );
}

#[tokio::test]
async fn signed_query_scope_for_app_classifies_opaque_find_asset_definition_by_id_as_target_domain()
{
    let authority =
        checked_torii_test_account_id(0xde, "derive opaque asset-definition authority fixture key");
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );
    let app = mk_app_state_for_tests();
    seed_asset_definition_for_test(&app, &asset_definition_id, Some(&domain_id));
    let request = roundtrip_request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Singular(
            iroha_data_model::query::SingularQueryBox::FindAssetDefinitionById(
                iroha_data_model::query::asset::prelude::FindAssetDefinitionById::new(
                    asset_definition_id,
                ),
            ),
        ),
    );

    assert_eq!(
        super::signed_query_scope(&request),
        super::SignedQueryScope::CrossDataspaceFanout
    );
    assert_eq!(
        super::signed_query_scope_for_app(app.as_ref(), &request),
        super::SignedQueryScope::TargetDomain(domain_id)
    );
}

#[tokio::test]
async fn signed_query_scope_for_app_classifies_opaque_find_accounts_with_asset_as_target_domain() {
    let authority =
        checked_torii_test_account_id(0xdf, "derive accounts-with-asset authority fixture key");
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );
    let app = mk_app_state_for_tests();
    seed_asset_definition_for_test(&app, &asset_definition_id, Some(&domain_id));
    let request = roundtrip_request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Start(
            build_find_accounts_with_asset_query_for_test(asset_definition_id),
        ),
    );

    assert_eq!(
        super::signed_query_scope(&request),
        super::SignedQueryScope::CrossDataspaceFanout
    );
    assert_eq!(
        super::signed_query_scope_for_app(app.as_ref(), &request),
        super::SignedQueryScope::TargetDomain(domain_id)
    );
}

#[tokio::test]
async fn resolve_signed_query_routing_for_app_uses_target_domain_route() {
    let authority_key_pair = checked_torii_test_ed25519_keypair(
        0xe0,
        "derive target-domain routing authority fixture key",
    );
    let authority = AccountId::new(authority_key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests();
    let (restricted_lane, restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    let query = iroha_data_model::query::QueryRequest::Singular(
        iroha_data_model::query::SingularQueryBox::FindDomainById(
            iroha_data_model::query::domain::prelude::FindDomainById::new(
                iroha_data_model::domain::DomainId::try_new("hbl", "restricted")
                    .expect("domain id"),
            ),
        ),
    )
    .with_authority(authority)
    .sign(&authority_key_pair);

    assert_eq!(
        super::resolve_signed_query_routing_for_app(app.as_ref(), &query)
            .expect("target-domain signed query should resolve a routed dataspace"),
        RoutingDecision::new(restricted_lane, restricted_dataspace)
    );
}

#[tokio::test]
async fn resolve_signed_query_routing_for_app_uses_target_domain_route_for_opaque_asset_definition_query()
 {
    let authority_key_pair = checked_torii_test_ed25519_keypair(
        0xe1,
        "derive opaque asset-definition routing authority fixture key",
    );
    let authority = AccountId::new(authority_key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests();
    let (restricted_lane, restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );
    seed_asset_definition_for_test(&app, &asset_definition_id, Some(&domain_id));
    let query = iroha_data_model::query::QueryRequest::Singular(
        iroha_data_model::query::SingularQueryBox::FindAssetDefinitionById(
            iroha_data_model::query::asset::prelude::FindAssetDefinitionById::new(
                asset_definition_id,
            ),
        ),
    )
    .with_authority(authority)
    .sign(&authority_key_pair);

    assert_eq!(
        super::resolve_signed_query_routing_for_app(app.as_ref(), &query)
            .expect("opaque asset-definition signed query should resolve a routed dataspace"),
        RoutingDecision::new(restricted_lane, restricted_dataspace)
    );
}

#[tokio::test]
async fn resolve_signed_query_routing_for_app_uses_target_alias_route() {
    let authority_key_pair = checked_torii_test_ed25519_keypair(
        0xe2,
        "derive target-alias routing authority fixture key",
    );
    let authority = AccountId::new(authority_key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests();
    let (restricted_lane, restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    let alias = iroha_data_model::account::AccountAlias::domainless(
        "banking".parse().expect("alias label"),
        restricted_dataspace,
    );
    let query = iroha_data_model::query::QueryRequest::Singular(
        iroha_data_model::query::SingularQueryBox::FindAccountByAlias(
            iroha_data_model::query::account::prelude::FindAccountByAlias::new(alias),
        ),
    )
    .with_authority(authority)
    .sign(&authority_key_pair);

    assert_eq!(
        super::resolve_signed_query_routing_for_app(app.as_ref(), &query)
            .expect("target-alias signed query should resolve a routed dataspace"),
        RoutingDecision::new(restricted_lane, restricted_dataspace)
    );
}

#[test]
fn signed_query_scope_classifies_target_account_queries() {
    let account_id = checked_torii_test_account_id(0xe3, "derive target-account scope fixture key");
    let authority =
        checked_torii_test_account_id(0xe4, "derive target-account authority fixture key");
    let alias = iroha_data_model::account::AccountAlias::domainless(
        "banking".parse().expect("alias label"),
        DataSpaceId::new(10),
    );
    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "asset-definition".parse().expect("asset definition name"),
    );

    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAccountById(
                    iroha_data_model::query::account::prelude::FindAccountById::new(
                        account_id.clone(),
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAliasesByAccountId(
                    iroha_data_model::query::account::prelude::FindAliasesByAccountId::new(
                        account_id.clone(),
                        None,
                        None,
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAssetById(
                    iroha_data_model::query::asset::prelude::FindAssetById::new(
                        iroha_data_model::asset::AssetId::new(
                            asset_definition_id.clone(),
                            account_id.clone(),
                        ),
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAssetDefinitionById(
                    iroha_data_model::query::asset::prelude::FindAssetDefinitionById::new(
                        asset_definition_id.clone(),
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetDomain(domain_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(
                build_find_domains_by_account_query_for_test(account_id.clone()),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(
                build_find_assets_by_account_query_for_test(account_id.clone()),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(
                build_find_nfts_by_account_query_for_test(account_id.clone()),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id.clone())
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(
                build_find_roles_by_account_query_for_test(account_id.clone()),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id)
    );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindAccountByAlias(
                    iroha_data_model::query::account::prelude::FindAccountByAlias::new(
                        alias.clone(),
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetAlias(alias.clone())
    );
    assert_eq!(
            super::signed_query_scope(&request_for_test(
                &authority,
                iroha_data_model::query::QueryRequest::Singular(
                    iroha_data_model::query::SingularQueryBox::FindAccountRecoveryPolicyByAlias(
                        iroha_data_model::query::account::prelude::FindAccountRecoveryPolicyByAlias::new(
                            alias.clone(),
                        ),
                    ),
                ),
            )),
            super::SignedQueryScope::TargetAlias(alias.clone())
        );
    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Singular(
                iroha_data_model::query::SingularQueryBox::FindDomainById(
                    iroha_data_model::query::domain::prelude::FindDomainById::new(
                        domain_id.clone(),
                    ),
                ),
            ),
        )),
        super::SignedQueryScope::TargetDomain(domain_id)
    );
}

#[test]
fn signed_query_scope_classifies_account_permissions_queries_as_target_account() {
    let account_id =
        checked_torii_test_account_id(0xe5, "derive account permissions target fixture key");
    let authority =
        checked_torii_test_account_id(0xe6, "derive account permissions authority fixture key");

    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(
                build_find_permissions_by_account_query_for_test(account_id.clone()),
            ),
        )),
        super::SignedQueryScope::TargetAccount(account_id)
    );
}

#[tokio::test]
async fn signed_query_scope_for_app_classifies_account_permissions_queries_as_target_account() {
    let account_id =
        checked_torii_test_account_id(0xe7, "derive app account permissions target fixture key");
    let authority =
        checked_torii_test_account_id(0xe8, "derive app account permissions authority fixture key");
    let app = mk_app_state_for_tests();

    assert_eq!(
        super::signed_query_scope_for_app(
            app.as_ref(),
            &request_for_test(
                &authority,
                iroha_data_model::query::QueryRequest::Start(
                    build_find_permissions_by_account_query_for_test(account_id.clone()),
                ),
            ),
        ),
        super::SignedQueryScope::TargetAccount(account_id)
    );
}

#[test]
fn signed_query_scope_classifies_find_transactions_as_authority_routed() {
    let authority =
        checked_torii_test_account_id(0xe9, "derive transactions authority fixture key");

    assert_eq!(
        super::signed_query_scope(&request_for_test(
            &authority,
            iroha_data_model::query::QueryRequest::Start(build_find_transactions_query_for_test(),),
        )),
        super::SignedQueryScope::AuthorityRouted
    );
}

#[tokio::test]
async fn signed_query_authorization_allows_exact_self_target_without_broad_read_grant() {
    let authority = checked_torii_test_account_id(0xf1, "derive self-read authority fixture key");
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::signed-query-self-read"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    configure_private_ingress_routes_for_test(&mut app);
    let request = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Singular(
            iroha_data_model::query::account::prelude::FindAccountById::new(authority.clone())
                .into(),
        ),
    );
    let scope = super::signed_query_scope_for_app(app.as_ref(), &request);

    let routes = super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
        .expect("an account may read its exact identity routes");

    assert!(
        routes
            .iter()
            .any(|route| route.dataspace_id == restricted_dataspace)
    );
}

#[tokio::test]
async fn signed_query_authorization_denies_foreign_restricted_account_without_exact_grant() {
    let target = checked_torii_test_account_id(0xf2, "derive foreign-read target fixture key");
    let authority =
        checked_torii_test_account_id(0xf3, "derive foreign-read authority fixture key");
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::foreign-read-target"));
    let mut app =
        mk_app_state_for_tests_with_world(world_with_target_and_caller_bound_to_dataspace(
            &target,
            &authority,
            uaid,
            restricted_dataspace,
        ));
    configure_private_ingress_routes_for_test(&mut app);
    let request = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Singular(
            iroha_data_model::query::account::prelude::FindAccountById::new(target.clone()).into(),
        ),
    );
    let scope = super::signed_query_scope_for_app(app.as_ref(), &request);

    let response = super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
        .expect_err("foreign restricted account reads require an exact grant");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("permission_denied")
    );

    grant_account_permission_for_test(
        &app,
        &authority,
        CanReadRestrictedDataspace {
            dataspace: restricted_dataspace,
        }
        .into(),
    );
    let routes = super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
        .expect("the exact restricted dataspace grant should allow the target read");
    assert!(
        routes
            .iter()
            .any(|route| route.dataspace_id == restricted_dataspace)
    );
}

#[tokio::test]
async fn signed_alias_query_requires_exact_alias_permission_not_broad_read_access() {
    let authority = checked_torii_test_account_id(0xfa, "derive alias query authority fixture key");
    let restricted_dataspace = DataSpaceId::new(10);
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_private_ingress_routes_for_test(&mut app);
    let alias = iroha_data_model::account::AccountAlias::domainless(
        "recipient".parse().expect("alias label"),
        restricted_dataspace,
    );
    let request = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Singular(
            iroha_data_model::query::account::prelude::FindAccountByAlias::new(alias.clone())
                .into(),
        ),
    );
    let scope = super::signed_query_scope_for_app(app.as_ref(), &request);

    let response = super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
        .expect_err("an alias query without its exact permission must fail closed");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    grant_account_permission_for_test(
        &app,
        &authority,
        CanReadRestrictedDataspace {
            dataspace: restricted_dataspace,
        }
        .into(),
    );
    super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
        .expect_err("broad restricted-data read access must not substitute for alias access");

    grant_account_permission_for_test(
        &app,
        &authority,
        CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(restricted_dataspace),
        }
        .into(),
    );
    let routes = super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
        .expect("the exact alias permission should authorize its route");
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].dataspace_id, restricted_dataspace);
}

#[tokio::test]
async fn signed_query_authorization_gates_global_history_and_replicated_inventories() {
    let authority = checked_torii_test_account_id(0xf4, "derive global-read authority fixture key");
    let restricted_dataspace = DataSpaceId::new(10);
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_private_ingress_routes_for_test(&mut app);
    let history = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Start(build_find_transactions_query_for_test()),
    );
    let triggers = request_for_test(
        &authority,
        iroha_data_model::query::QueryRequest::Start(build_find_triggers_query_for_test()),
    );

    for request in [&history, &triggers] {
        let scope = super::signed_query_scope_for_app(app.as_ref(), request);
        let response = super::torii_authorized_signed_query_routes(app.as_ref(), request, &scope)
            .expect_err("global restricted reads must fail without exact grants");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    grant_account_permission_for_test(
        &app,
        &authority,
        CanReadRestrictedDataspace {
            dataspace: restricted_dataspace,
        }
        .into(),
    );
    for request in [&history, &triggers] {
        let scope = super::signed_query_scope_for_app(app.as_ref(), request);
        super::torii_authorized_signed_query_routes(app.as_ref(), request, &scope)
            .expect("an exact grant for every restricted route should allow a global read");
    }
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_target_scope_routes_resolve_alias_and_domain_dataspaces() {
    let mut app = mk_app_state_for_tests();
    let (_restricted_lane, restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);

    let alias = iroha_data_model::account::AccountAlias::domainless(
        "banking".parse().expect("alias label"),
        restricted_dataspace,
    );
    let alias_routes =
        super::torii_target_alias_routes(app.as_ref(), &alias).expect("alias routes");
    assert_eq!(alias_routes.len(), 1);
    assert_eq!(alias_routes[0].dataspace_id, restricted_dataspace);

    let domain_id =
        iroha_data_model::domain::DomainId::try_new("hbl", "restricted").expect("domain id");
    let domain_routes =
        super::torii_target_domain_routes(app.as_ref(), &domain_id).expect("domain routes");
    assert_eq!(domain_routes.len(), 1);
    assert_eq!(domain_routes[0].dataspace_id, restricted_dataspace);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_account_read_routes_use_target_account_scope_for_signed_and_internal_reads() {
    let authority = checked_torii_test_account_id(
        0xea,
        "derive target-account read routing authority fixture key",
    );
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::target-account-routes"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes = super::torii_account_read_routes(app.as_ref(), &authority, None, true)
        .expect("target-account routes should resolve");
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([
            DataSpaceId::UNIVERSAL,
            governance_dataspace,
            restricted_dataspace,
        ]),
        "signed/internal account reads should fan out across the target account scope plus public dataspaces",
    );
    assert!(
        dataspaces.contains(&governance_dataspace),
        "public dataspaces must remain visible in target-account routing",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_account_read_routes_keep_unsigned_public_reads_on_visible_routes() {
    let authority = checked_torii_test_account_id(
        0xeb,
        "derive public account read routing authority fixture key",
    );
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::public-account-routes"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes = super::torii_account_read_routes(app.as_ref(), &authority, None, false)
        .expect("public visibility routes should resolve");
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([DataSpaceId::UNIVERSAL, governance_dataspace]),
        "unsigned public reads should stay on caller/public visibility routes",
    );
    assert!(
        !dataspaces.contains(&restricted_dataspace),
        "unsigned public reads must not automatically gain private dataspace visibility",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_account_assets_read_routes_fan_out_across_all_dataspaces() {
    let authority =
        checked_torii_test_account_id(0xec, "derive account asset fanout authority fixture key");
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes = super::torii_account_assets_read_routes(app.as_ref());
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([
            DataSpaceId::UNIVERSAL,
            governance_dataspace,
            restricted_dataspace,
        ]),
        "account asset reads must fan out like asset-holder reads so dataspace-scoped balances are visible",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn handler_account_assets_fanout_reports_merged_route_headers() {
    let authority = checked_torii_test_account_id(
        0xed,
        "derive account asset handler fanout authority fixture key",
    );
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::assets-known-scope"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let uri: axum::http::Uri = format!("/v1/accounts/{authority}/assets")
        .parse()
        .expect("valid account assets uri");
    let response = super::handler_account_assets(
        State(app),
        axum::http::Method::GET,
        uri,
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(authority.to_string()),
        AxQuery(crate::routing::AccountAssetsGetParams::default()),
    )
    .await
    .expect("account assets should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers().get("x-iroha-route-lane-id").is_none(),
        "account asset fanout should not expose a singular route lane",
    );
    assert!(
        response
            .headers()
            .get("x-iroha-route-dataspace-id")
            .is_none(),
        "account asset fanout should not expose a singular dataspace",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_account_permissions_read_routes_fan_out_across_all_dataspaces_for_signed_and_internal_reads()
 {
    let authority = checked_torii_test_account_id(
        0xee,
        "derive signed account permissions fanout authority fixture key",
    );
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::permissions-account-routes"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes = super::torii_account_permissions_read_routes(
        app.as_ref(),
        &authority,
        Some(&authority),
        true,
    )
    .expect("target-account permissions routes should resolve");
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([
            DataSpaceId::UNIVERSAL,
            governance_dataspace,
            restricted_dataspace,
        ]),
        "signed/internal permissions reads must fan out across all configured dataspaces to include dataspace-scoped grants",
    );
    assert_eq!(
        super::torii_account_permissions_route_scope(&authority, Some(&authority), true),
        ToriiFanoutRouteScopeV1::AllDataspaces
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn fanout_routed_by_uses_attempted_routes_even_when_only_local_payloads_survive() {
    let mut app = mk_app_state_for_tests();
    let (local_route, foreign_route) =
        configure_private_ingress_with_offline_foreign_route_for_test(&mut app);

    assert_eq!(super::routed_by_for_routes(&app, &[local_route]), "local");
    assert_eq!(
        super::routed_by_for_routes(&app, &[local_route, foreign_route]),
        "proxy",
        "fanout responses must report proxy routing when any attempted route is non-local"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_account_permissions_read_routes_keep_unsigned_public_reads_visible() {
    let authority = checked_torii_test_account_id(
        0xef,
        "derive public account permissions fanout authority fixture key",
    );
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid =
        UniversalAccountId::from_hash(Hash::new(b"torii::permissions-public-account-routes"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes =
        super::torii_account_permissions_read_routes(app.as_ref(), &authority, None, false)
            .expect("public visibility routes should resolve");
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([DataSpaceId::UNIVERSAL, governance_dataspace]),
        "unsigned permissions reads should stay on caller/public visibility routes",
    );
    assert!(
        !dataspaces.contains(&restricted_dataspace),
        "unsigned permissions reads must not automatically gain private dataspace visibility",
    );
    assert_eq!(
        super::torii_account_permissions_route_scope(&authority, None, false),
        ToriiFanoutRouteScopeV1::VisibleAccount {
            caller_account_id: None
        }
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn torii_target_account_routes_fan_out_when_local_scope_is_unknown() {
    let authority = checked_torii_test_account_id(
        0xf0,
        "derive unknown target-account routes authority fixture key",
    );
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let mut app = mk_app_state_for_tests();
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes = super::torii_target_account_routes(app.as_ref(), &authority)
        .expect("unknown target-account scope should fall back to all configured routes");
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([
            DataSpaceId::UNIVERSAL,
            governance_dataspace,
            restricted_dataspace,
        ]),
        "target-account queries with unknown local scope should fan out across all configured dataspace routes",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn nexus_fanout_recomputes_unknown_target_account_routes_from_catalog() {
    let authority = checked_torii_test_account_id(
        0xf1,
        "derive Nexus target-account fanout authority fixture key",
    );
    let governance_dataspace = DataSpaceId::new(1);
    let restricted_dataspace = DataSpaceId::new(10);
    let mut app = mk_app_state_for_tests();
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let routes = super::torii_fanout_scope_routes(
        app.as_ref(),
        &ToriiFanoutRouteScopeV1::TargetAccount {
            account_id: authority.to_string(),
        },
    )
    .expect("Nexus fanout target-account routes should resolve");
    let dataspaces = routes
        .into_iter()
        .map(|route| route.dataspace_id)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        dataspaces,
        std::collections::BTreeSet::from([
            DataSpaceId::UNIVERSAL,
            governance_dataspace,
            restricted_dataspace,
        ]),
        "Nexus must recompute unknown target-account fanout from its own dataspace catalog",
    );
}

#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[test]
fn nexus_fanout_proxy_variants_use_route_budget() {
    let query_request = ToriiProxyRequestKindV4::SignedQueryFanout {
        query_bytes: Vec::new(),
        response_format: ToriiProxyResponseFormatV1::Norito,
    };
    assert_eq!(
        super::torii_proxy_attempt_timeout(&query_request),
        DEFAULT_ROUTE_TIMEOUT + Duration::from_secs(5)
    );
    assert_eq!(
        super::torii_proxy_request_kind_name(&query_request),
        "signed_query_fanout"
    );

    let read_request = ToriiProxyRequestKindV4::ReadFanout(super::torii_read_fanout_request(
        ToriiReadEndpointV1::AccountGet,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::Account,
        vec![
            checked_torii_test_account_id(0xf2, "derive read fanout proxy account fixture key")
                .to_string(),
        ],
        None,
        Vec::new(),
        ToriiProxyResponseFormatV1::Json,
    ));
    assert_eq!(
        super::torii_proxy_attempt_timeout(&read_request),
        DEFAULT_ROUTE_TIMEOUT + Duration::from_secs(5)
    );
    assert_eq!(
        super::torii_proxy_request_kind_name(&read_request),
        "read_fanout"
    );
}

#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[test]
fn http_route_timeout_covers_read_fanout_proxy_budget() {
    let fanout_budget = super::torii_proxy_attempt_timeout(&ToriiProxyRequestKindV4::ReadFanout(
        super::torii_read_fanout_request(
            ToriiReadEndpointV1::AccountPermissionsGet,
            ToriiFanoutRouteScopeV1::AllDataspaces,
            ToriiReadFanoutMergeV1::List,
            vec![
                checked_torii_test_account_id(
                    0xf3,
                    "derive read fanout HTTP budget account fixture key",
                )
                .to_string(),
            ],
            None,
            Vec::new(),
            ToriiProxyResponseFormatV1::Json,
        ),
    ));
    assert!(
        super::route_timeout_for_path("/v1/accounts/example/permissions") >= fanout_budget,
        "outer HTTP timeout must not expire before the read-fanout proxy budget"
    );
    assert_eq!(
        super::route_timeout_for_path("/v1/zk/ivm/derive"),
        ZK_IVM_ROUTE_TIMEOUT,
        "ZK IVM derive can legitimately exceed the default route timeout"
    );
    assert_eq!(
        super::route_timeout_for_path("/v1/zk/ivm/prove"),
        ZK_IVM_ROUTE_TIMEOUT,
        "ZK IVM prove can legitimately exceed the default route timeout"
    );
}

#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[test]
fn client_default_timeout_covers_nexus_fanout_http_budget() {
    let fanout_http_budget = super::route_timeout_for_path("/v1/accounts/example/permissions");
    assert!(
        iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT >= fanout_http_budget,
        "client default timeout {:?} must cover routed fanout HTTP budget {:?}",
        iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        fanout_http_budget,
    );
}

#[tokio::test]
async fn handler_signed_query_executes_find_triggers_locally_with_multiple_dataspaces() {
    let key_pair =
        checked_torii_test_ed25519_keypair(0xf4, "derive find triggers authority fixture key");
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_multiple_dataspace_routes_for_test(&mut app);
    assert!(
        super::torii_all_dataspace_routes(app.as_ref()).len() > 1,
        "test requires multiple dataspace routes"
    );

    let response = super::handler_signed_query(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::NoritoQuery(QueryOptions::default()),
        versioned_query_for_test(signed_find_triggers_query_for_test(authority, &key_pair)),
    )
    .await
    .expect("find triggers query should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers().get("x-iroha-routed-by").is_none(),
        "local trigger queries should not fan out"
    );
    assert!(
        response.headers().get("x-iroha-route-lane-id").is_none(),
        "local trigger queries should not publish a routed lane"
    );
}

#[tokio::test]
async fn handler_signed_query_executes_find_active_trigger_ids_locally_with_multiple_dataspaces() {
    let key_pair = checked_torii_test_ed25519_keypair(
        0xf5,
        "derive find active trigger ids authority fixture key",
    );
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_multiple_dataspace_routes_for_test(&mut app);
    assert!(
        super::torii_all_dataspace_routes(app.as_ref()).len() > 1,
        "test requires multiple dataspace routes"
    );

    let response = super::handler_signed_query(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::NoritoQuery(QueryOptions::default()),
        versioned_query_for_test(signed_find_active_trigger_ids_query_for_test(
            authority, &key_pair,
        )),
    )
    .await
    .expect("find active trigger ids query should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers().get("x-iroha-routed-by").is_none(),
        "local trigger queries should not fan out"
    );
    assert!(
        response
            .headers()
            .get("x-iroha-route-dataspace-id")
            .is_none(),
        "local trigger queries should not publish a routed dataspace"
    );
}

#[tokio::test]
async fn handler_accounts_list_prefers_local_restricted_routes_on_private_ingress() {
    let mut app = mk_app_state_for_tests();
    configure_private_ingress_routes_for_test(&mut app);

    let response = super::handler_accounts_list(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxQuery(crate::routing::ListFilterParams::default()),
    )
    .await
    .expect("accounts list should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("local"),
        "private ingress account listings should stay on the local restricted lane",
    );
}

#[tokio::test]
async fn handler_account_assets_fan_outs_across_visible_dataspaces() {
    let authority = checked_torii_test_account_id(
        0xf6,
        "derive visible account assets fanout authority fixture key",
    );
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_multiple_dataspace_routes_for_test(&mut app);

    let uri: axum::http::Uri = format!("/v1/accounts/{authority}/assets")
        .parse()
        .expect("valid account assets uri");
    let response = super::handler_account_assets(
        State(app),
        axum::http::Method::GET,
        uri,
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(authority.to_string()),
        AxQuery(crate::routing::AccountAssetsGetParams::default()),
    )
    .await
    .expect("account assets should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("local"),
        "account asset reads should still execute locally in unit tests",
    );
    assert!(
        response.headers().get("x-iroha-route-lane-id").is_none(),
        "visible dataspace fanout should not expose a singular route lane",
    );
    assert!(
        response
            .headers()
            .get("x-iroha-route-dataspace-id")
            .is_none(),
        "visible dataspace fanout should not expose a singular dataspace",
    );
}

#[tokio::test]
async fn handler_transactions_query_fan_outs_across_dataspaces() {
    let mut app = mk_app_state_for_tests();
    configure_multiple_dataspace_routes_for_test(&mut app);
    assert!(
        super::torii_all_dataspace_routes(app.as_ref()).len() > 1,
        "test requires multiple dataspace routes"
    );

    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: None,
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: Some("exact".to_owned()),
    };
    let response = super::handler_transactions_query(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(env),
    )
    .await
    .expect("transactions query should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-fanout-routes-attempted")
            .and_then(|value| value.to_str().ok()),
        Some("2"),
        "global transaction queries must enter the read-fanout path",
    );
    assert!(
        response.headers().get("x-iroha-route-lane-id").is_none(),
        "transaction query fanout should not expose a singular route lane",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn public_dataspace_upstream_serves_routed_account_assets() {
    let captured = Arc::new(std::sync::Mutex::new(Vec::<(String, String)>::new()));
    let captured_for_route = Arc::clone(&captured);
    let upstream = Router::new().route(
            "/v1/accounts/{account_id}/assets",
            get(
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      uri: Uri| {
                    let captured = Arc::clone(&captured_for_route);
                    async move {
                        captured
                            .lock()
                            .expect("capture lock")
                            .push((account_id, uri.query().unwrap_or_default().to_owned()));
                        Response::builder()
                            .status(StatusCode::OK)
                            .header(axum::http::header::CONTENT_TYPE, "application/json")
                            .body(Body::from(
                                br#"{"items":[{"asset":"xor#universal","quantity":"74.7664","scope":"global"}],"total":1}"#
                                    .to_vec(),
                            ))
                            .expect("upstream response")
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream addr");
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, upstream.into_make_service())
            .await
            .expect("serve upstream");
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .public_dataspace_upstreams = Arc::new(BTreeMap::from([(
        DataSpaceId::UNIVERSAL,
        format!("http://{addr}"),
    )]));

    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let account_id =
        checked_torii_test_account_id(0xf7, "derive routed public upstream account fixture key")
            .to_string();
    let request = torii_read_request(
        ToriiReadEndpointV1::AccountAssetsGet,
        route,
        vec![account_id.clone()],
        Some("limit=500".to_owned()),
        Vec::new(),
    );
    let response = execute_torii_read_for_route(&app, route, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("external"),
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json: Value = norito::json::from_slice(&body).expect("json response");
    assert_eq!(json["items"][0]["quantity"].as_str(), Some("74.7664"));
    assert_eq!(
        captured.lock().expect("capture lock").as_slice(),
        &[(account_id, "limit=500".to_owned())],
    );
    upstream_task.abort();
}

#[tokio::test]
async fn handler_account_get_fan_outs_across_global_dataspaces() {
    let authority = checked_torii_test_account_id(
        0xf8,
        "derive global account get fanout authority fixture key",
    );
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    configure_multiple_dataspace_routes_for_test(&mut app);

    let uri: axum::http::Uri = format!("/v1/accounts/{authority}")
        .parse()
        .expect("valid account uri");
    let response = super::handler_account_get(
        State(app),
        axum::http::Method::GET,
        uri,
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        AxPath(authority.to_string()),
    )
    .await
    .expect("account get should execute");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("local"),
        "global account reads should still execute locally in unit tests",
    );
    assert!(
        response.headers().get("x-iroha-route-lane-id").is_none(),
        "global account fanout should not expose a singular route lane",
    );
    assert!(
        response
            .headers()
            .get("x-iroha-route-dataspace-id")
            .is_none(),
        "global account fanout should not expose a singular dataspace",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn routing_space_directory_manifests_reports_inactive_pending_and_uncataloged_expired_rows() {
    let authority = checked_torii_test_account_id(
        0xf9,
        "derive space directory manifest authority fixture key",
    );
    let uaid =
        UniversalAccountId::from_hash(Hash::new(b"torii::space-directory-manifest-inactive"));
    let pending_dataspace = DataSpaceId::new(10);
    let expired_dataspace = DataSpaceId::new(11);
    let mut world = world_with_account(&authority);

    let mut bindings = iroha_core::nexus::space_directory::UaidDataspaceBindings::default();
    bindings.bind_account(pending_dataspace, authority.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let pending_manifest = iroha_data_model::nexus::AssetPermissionManifest {
        version: iroha_data_model::nexus::ManifestVersion::V1,
        uaid,
        dataspace: pending_dataspace,
        issued_ms: 1_710_000_000_000,
        activation_epoch: 10,
        expiry_epoch: None,
        entries: Vec::new(),
    };
    let pending_record =
        iroha_core::nexus::space_directory::SpaceDirectoryManifestRecord::new(pending_manifest);

    let expired_manifest = iroha_data_model::nexus::AssetPermissionManifest {
        version: iroha_data_model::nexus::ManifestVersion::V1,
        uaid,
        dataspace: expired_dataspace,
        issued_ms: 1_710_000_000_100,
        activation_epoch: 20,
        expiry_epoch: Some(30),
        entries: Vec::new(),
    };
    let mut expired_record =
        iroha_core::nexus::space_directory::SpaceDirectoryManifestRecord::new(expired_manifest);
    expired_record.lifecycle.mark_activated(21);
    expired_record.lifecycle.mark_expired(31);

    let mut set = iroha_core::nexus::space_directory::SpaceDirectoryManifestSet::default();
    set.upsert(pending_record);
    set.upsert(expired_record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut app = mk_app_state_for_tests_with_world(world);
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        state.nexus.get_mut().dataspace_catalog =
            iroha_data_model::nexus::DataSpaceCatalog::new(vec![
                iroha_data_model::nexus::DataSpaceMetadata::default(),
                iroha_data_model::nexus::DataSpaceMetadata {
                    id: pending_dataspace,
                    alias: "restricted".to_owned(),
                    description: None,
                    fault_tolerance: 1,
                },
            ])
            .expect("dataspace catalog");
    }

    let response = routing::handle_v1_space_directory_manifests(
        app.state.clone(),
        AxPath(uaid.to_string()),
        crate::NoritoQuery(routing::SpaceDirectoryManifestQuery {
            dataspace: None,
            status: Some("Inactive".to_owned()),
            limit: None,
            offset: None,
            count_mode: None,
        }),
        app.telemetry.clone(),
    )
    .await
    .expect("inactive manifest read should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("inactive manifest body");
    let json: norito::json::Value =
        norito::json::from_slice(&body).expect("inactive manifest json");
    assert_eq!(json["total"].as_u64(), Some(2));
    let manifests = json["manifests"].as_array().expect("manifests array");
    assert_eq!(manifests.len(), 2);

    let pending = manifests
        .iter()
        .find(|row| row["dataspace_id"].as_u64() == Some(pending_dataspace.as_u64()))
        .expect("pending manifest row");
    assert_eq!(pending["status"].as_str(), Some("Pending"));
    assert_eq!(pending["dataspace_alias"].as_str(), Some("restricted"));
    assert_eq!(
        pending["accounts"][0].as_str(),
        Some(authority.to_string().as_str())
    );

    let expired = manifests
        .iter()
        .find(|row| row["dataspace_id"].as_u64() == Some(expired_dataspace.as_u64()))
        .expect("expired manifest row");
    assert_eq!(expired["status"].as_str(), Some("Expired"));
    assert!(expired["dataspace_alias"].is_null());
    assert_eq!(
        expired["accounts"]
            .as_array()
            .expect("expired accounts array")
            .len(),
        0,
        "uncataloged/unbound dataspaces should report empty account bindings",
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn handler_space_directory_manifests_executes_configured_dataspace_route_locally() {
    let authority = checked_torii_test_account_id(
        0xfa,
        "derive routed space-directory manifest authority fixture key",
    );
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::space-directory-manifest-route"));
    let mut world = world_with_account_bound_to_dataspace(&authority, uaid, restricted_dataspace);
    let mut bindings = iroha_core::nexus::space_directory::UaidDataspaceBindings::default();
    bindings.bind_account(restricted_dataspace, authority.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let mut app = mk_app_state_for_tests_with_world(world);
    let (restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let response = super::handler_space_directory_manifests(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(uaid.to_string()),
        AxQuery(routing::SpaceDirectoryManifestQuery {
            dataspace: Some(restricted_dataspace.as_u64()),
            status: Some("Active".to_owned()),
            limit: Some(1),
            offset: Some(0),
            count_mode: None,
        }),
    )
    .await
    .expect("manifest handler should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("local"),
        "configured dataspace route should execute locally in unit tests",
    );
    assert_eq!(
        response
            .headers()
            .get("x-iroha-route-lane-id")
            .and_then(|value| value.to_str().ok()),
        Some(restricted_lane.as_u32().to_string().as_str())
    );
    assert_eq!(
        response
            .headers()
            .get("x-iroha-route-dataspace-id")
            .and_then(|value| value.to_str().ok()),
        Some(restricted_dataspace.as_u64().to_string().as_str())
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("manifest handler body");
    let json: norito::json::Value = norito::json::from_slice(&body).expect("manifest handler json");
    assert_eq!(json["total"].as_u64(), Some(1));
    let manifests = json["manifests"].as_array().expect("manifests array");
    assert_eq!(manifests.len(), 1);
    assert_eq!(
        manifests[0]["dataspace_id"].as_u64(),
        Some(restricted_dataspace.as_u64())
    );
    assert_eq!(manifests[0]["status"].as_str(), Some("Active"));
    assert_eq!(
        manifests[0]["accounts"][0].as_str(),
        Some(authority.to_string().as_str())
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn handler_explorer_account_detail_uses_target_account_routes_for_internal_reads() {
    let authority =
        checked_torii_test_account_id(0xfb, "derive explorer account detail authority fixture key");
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::explorer-account-detail-routes"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        restricted_dataspace,
    ));
    let (_restricted_lane, configured_restricted_dataspace) =
        configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(configured_restricted_dataspace, restricted_dataspace);

    let response = super::handler_explorer_account_detail(
        State(app),
        axum::http::Method::GET,
        format!("/v1/explorer/accounts/{authority}")
            .parse()
            .expect("valid explorer account uri"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(authority.to_string()),
    )
    .await
    .expect("explorer account detail should execute")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("local"),
        "internal explorer account reads should use the routed target-account path",
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("explorer account detail body");
    let json: norito::json::Value =
        norito::json::from_slice(&body).expect("explorer account detail json");
    let authority_literal = authority.to_string();
    assert_eq!(json["id"].as_str(), Some(authority_literal.as_str()));
}
