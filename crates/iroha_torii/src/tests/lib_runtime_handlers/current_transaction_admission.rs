#[cfg(feature = "connect")]
async fn current_admission_http(
    app: SharedAppState,
    transaction: &SignedTransaction,
    endpoint: &str,
) -> Response {
    use iroha_version::codec::EncodeVersioned as _;
    use tower::ServiceExt as _;
    let body = if endpoint == route_catalog::pipeline::TRANSACTIONS_BATCH.path() {
        norito::to_bytes(&vec![transaction.encode_versioned()]).unwrap()
    } else if endpoint == route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path() {
        TransactionEntrypoint::External(transaction.clone()).encode_versioned()
    } else {
        transaction.encode_versioned()
    };
    let router = axum::Router::new()
        .route(
            route_catalog::pipeline::TRANSACTION.path(),
            axum::routing::post(super::handler_post_transaction),
        )
        .route(
            route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path(),
            axum::routing::post(super::handler_post_transaction_entrypoint),
        )
        .route(
            route_catalog::pipeline::TRANSACTIONS_BATCH.path(),
            axum::routing::post(super::handler_post_transactions_batch),
        )
        .with_state(app);
    router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(endpoint)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    crate::utils::NORITO_MIME_TYPE,
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[cfg(feature = "connect")]
async fn assert_unsupported_current_admission(response: Response) {
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        torii_response_header(&response, "x-iroha-reject-code"),
        Some("unsupported_transaction_admission")
    );
    for header in [
        "x-iroha-transactions-accepted",
        "x-iroha-route",
        "preference-applied",
    ] {
        assert!(
            response.headers().get(header).is_none(),
            "rejection cannot contain {header}"
        );
    }
    let is_json = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let envelope: iroha_torii_shared::ErrorEnvelope = if is_json {
        norito::json::from_slice(&bytes).unwrap()
    } else {
        norito::decode_from_bytes(&bytes).unwrap()
    };
    assert_eq!(envelope.code, "unsupported_transaction_admission");
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn current_http_admission_rejects_unsupported_intent_before_any_durable_promise() {
    let (app, key, _, _, journal) = lifecycle_ordinary_fixture(true);
    let ordinary = lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![Log::new(Level::INFO, "current admission control".to_owned()).into()],
    );
    let unsupported = TransactionBuilder::from_payload(ordinary.payload().clone())
        .unwrap()
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
        .sign(key.private_key());
    let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
    for endpoint in [
        route_catalog::pipeline::TRANSACTION.path(),
        route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path(),
        route_catalog::pipeline::TRANSACTIONS_BATCH.path(),
    ] {
        assert_unsupported_current_admission(
            current_admission_http(app.clone(), &unsupported, endpoint).await,
        )
        .await;
        assert_eq!(app.queue.active_len(), 0);
        assert_eq!(
            std::fs::read(journal.path().join("queue.norito")).unwrap(),
            before
        );
    }
    let accepted = current_admission_http(
        app.clone(),
        &ordinary,
        route_catalog::pipeline::TRANSACTION.path(),
    )
    .await;
    assert_eq!(
        accepted.status(),
        StatusCode::ACCEPTED,
        "supported single-route work remains durable"
    );
    assert_eq!(app.queue.active_len(), 1);
    assert_ne!(
        std::fs::read(journal.path().join("queue.norito")).unwrap(),
        before
    );
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn current_http_admission_rejects_actual_multiroute_before_journal_write() {
    let domains = [
        DomainId::try_new("first", "universal").unwrap(),
        DomainId::try_new("second", "secondary").unwrap(),
    ];
    let world = World::with(
        domains
            .iter()
            .cloned()
            .map(|id| Domain::new(id).build(&ALICE_ID)),
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let app =
        mk_app_state_for_tests_with_world_and_nexus(world, multiple_dataspace_nexus_for_test());
    for domain in &domains {
        bind_domain_name_for_test(&app, &domain.to_string());
    }
    let transaction = TransactionBuilder::new(
        *app.state.network_id_ref(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(domains.into_iter().map(|domain| {
        iroha_data_model::isi::SetKeyValue::domain(
            domain,
            "current_admission".parse().unwrap(),
            iroha_primitives::json::Json::new(true),
        )
    }))
    .sign(ALICE_KEYPAIR.private_key());
    let plan = app
        .queue
        .route_payload_plan_with_state(transaction.payload(), &app.state)
        .unwrap();
    assert!(
        !matches!(plan, RoutingPlan::Single(_)),
        "exercise actual two-dataspace routing"
    );
    let journal = tempfile::tempdir().unwrap();
    let path = journal.path().join("queue.norito");
    app.queue
        .install_plan_journal(&path, 1024 * 1024, true)
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    for endpoint in [
        route_catalog::pipeline::TRANSACTION.path(),
        route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path(),
        route_catalog::pipeline::TRANSACTIONS_BATCH.path(),
    ] {
        assert_unsupported_current_admission(
            current_admission_http(app.clone(), &transaction, endpoint).await,
        )
        .await;
        assert_eq!(app.queue.active_len(), 0);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
